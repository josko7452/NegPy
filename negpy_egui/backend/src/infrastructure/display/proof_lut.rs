//! Soft-proof and gamut tables for the preview.
//!
//! Mirrors `ImageProcessor.soft_proof_lut` and `gamut_lut`. The proof is baked once per
//! profile condition into a table the canvas samples; the gamut mask is a centre-node
//! round trip through the output profile.

use qcms::{DataType, Intent, Profile, Transform};

use super::color_mgmt::WORKING_COLOR_SPACE;
use super::icc_lut::{Lut3Data, apply_lut_u16, grid_codes};

/// The proof LUT grid edge, finer than the display LUT because a proof clips at the
/// output gamut boundary.
pub const PROOF_LUT_SIZE: usize = 65;
/// The joint-histogram grid edge.
pub const GAMUT_LUT_SIZE: usize = 32;
/// The 8-bit displacement above which a round-tripped colour counts as unprintable.
pub const GAMUT_ROUND_TRIP_TOLERANCE: i16 = 14;
/// The neutral value painted into proof nodes outside the output gamut.
pub const GAMUT_WARNING_COLOR: f32 = 0.5;

/// The proof's source-to-paper rendering intent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProofIntent {
    Perceptual,
    #[default]
    RelativeColorimetric,
    Saturation,
}

impl ProofIntent {
    /// Parse a stored intent value, defaulting to relative colorimetric.
    #[must_use]
    pub fn coerce(value: &str) -> Self {
        match value {
            "perceptual" => Self::Perceptual,
            "saturation" => Self::Saturation,
            _ => Self::RelativeColorimetric,
        }
    }

    fn intent(self) -> Intent {
        match self {
            Self::Perceptual => Intent::Perceptual,
            Self::RelativeColorimetric => Intent::RelativeColorimetric,
            Self::Saturation => Intent::Saturation,
        }
    }
}

/// The settings a proof table is built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofCondition {
    pub working_color_space: String,
    pub input_icc: Option<Vec<u8>>,
    pub output_icc: Option<Vec<u8>>,
    pub monitor_icc: Option<Vec<u8>>,
    pub intent: ProofIntent,
    pub black_point: bool,
    pub paper_white: bool,
    pub ink_black: bool,
    pub gamut_warning: bool,
}

impl Default for ProofCondition {
    fn default() -> Self {
        Self {
            working_color_space: WORKING_COLOR_SPACE.to_string(),
            input_icc: None,
            output_icc: None,
            monitor_icc: None,
            intent: ProofIntent::RelativeColorimetric,
            black_point: true,
            paper_white: true,
            ink_black: false,
            gamut_warning: false,
        }
    }
}

/// True when the ICC header names a GRAY profile.
#[must_use]
pub fn is_gray_profile(bytes: &[u8]) -> bool {
    bytes.get(16..20) == Some(b"GRAY")
}

/// True when the ICC header names a CMYK profile.
#[must_use]
pub fn is_cmyk_profile(bytes: &[u8]) -> bool {
    bytes.get(16..20) == Some(b"CMYK")
}

/// The output profile's working-space round trip, as a boolean cube.
///
/// Centre nodes avoid the profile boundary, where a corner node is a coin toss. An
/// absent or GRAY output has no answer and returns `None`.
#[must_use]
pub fn gamut_lut(
    _working_color_space: &str,
    working_icc: &[u8],
    input_icc: Option<&[u8]>,
    output_icc: Option<&[u8]>,
    size: usize,
) -> Option<Vec<bool>> {
    let output = output_icc.filter(|bytes| !bytes.is_empty())?;
    if is_gray_profile(output) || size == 0 {
        return None;
    }
    let working = Profile::new_from_slice(working_icc, false)?;
    let source = match input_icc.filter(|bytes| !bytes.is_empty()) {
        Some(bytes) => Profile::new_from_slice(bytes, false)?,
        None => Profile::new_from_slice(working_icc, false)?,
    };
    let destination = Profile::new_from_slice(output, false)?;
    let to_output = Transform::new(
        &source,
        &destination,
        DataType::RGB8,
        Intent::RelativeColorimetric,
    )?;
    let back = Transform::new(
        &destination,
        &working,
        DataType::RGB8,
        Intent::RelativeColorimetric,
    )?;
    let reference = if input_icc.is_none() {
        None
    } else {
        Some(Transform::new(
            &source,
            &working,
            DataType::RGB8,
            Intent::RelativeColorimetric,
        )?)
    };
    let axis: Vec<u8> = (0..size)
        .map(|index| {
            (((index as f64 + 0.5) / size as f64) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect();
    let mut grid = Vec::with_capacity(size * size * size * 3);
    for red in &axis {
        for green in &axis {
            for blue in &axis {
                grid.extend_from_slice(&[*red, *green, *blue]);
            }
        }
    }
    let mut out_grid = grid.clone();
    to_output.apply(&mut out_grid);
    let mut back_grid = out_grid.clone();
    back.apply(&mut back_grid);
    let mut reference_grid = grid;
    if let Some(transform) = reference {
        transform.apply(&mut reference_grid);
    }
    Some(
        back_grid
            .chunks_exact(3)
            .zip(reference_grid.chunks_exact(3))
            .map(|(back, reference)| {
                back.iter().zip(reference).any(|(a, b)| {
                    (i16::from(*a) - i16::from(*b)).abs() > GAMUT_ROUND_TRIP_TOLERANCE
                })
            })
            .collect(),
    )
}

/// Build the proof table by pushing the identity grid through the profile pair.
///
/// Display profiles take a gamut-only proof ending at the monitor; a tinted or
/// absent monitor uses sRGB. A print profile additionally simulates paper white and D-max
/// when the caller asks for them, subject to the source and destination color spaces.
#[must_use]
pub fn soft_proof_lut(
    condition: &ProofCondition,
    working_icc: &[u8],
    size: usize,
) -> Option<Lut3Data> {
    if size < 2 {
        return None;
    }
    let source = match condition
        .input_icc
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
    {
        Some(bytes) => Profile::new_from_slice(bytes, false)?,
        None => Profile::new_from_slice(working_icc, false)?,
    };
    let destination = match condition
        .output_icc
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
    {
        Some(bytes) => Profile::new_from_slice(bytes, false)?,
        None => Profile::new_from_slice(working_icc, false)?,
    };
    let display = match condition
        .monitor_icc
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
    {
        Some(bytes) => Profile::new_from_slice(bytes, false)?,
        None => Profile::new_sRGB(),
    };
    let output_is_print = condition.output_icc.as_deref().is_some_and(is_cmyk_profile);
    let codes = proof_grid_codes(size);
    let mut grid = Vec::with_capacity(size * size * size * 3);
    for red in &codes {
        for green in &codes {
            for blue in &codes {
                grid.extend_from_slice(&[*red, *green, *blue]);
            }
        }
    }
    let mut table: Vec<f32> = if output_is_print {
        println_proof(&grid, &source, &destination, &display, condition)?
    } else {
        let to_output = Transform::new(
            &source,
            &destination,
            DataType::RGB8,
            condition.intent.intent(),
        )?;
        let mut out = grid;
        to_output.apply(&mut out);
        let to_display = Transform::new(
            &destination,
            &display,
            DataType::RGB8,
            Intent::RelativeColorimetric,
        )?;
        to_display.apply(&mut out);
        out
    }
    .iter()
    .map(|value| f32::from(*value) / 255.0)
    .collect();
    if condition.gamut_warning {
        if let Some(mask) = gamut_lut(
            &condition.working_color_space,
            working_icc,
            condition.input_icc.as_deref(),
            condition.output_icc.as_deref(),
            size,
        ) {
            for (index, flagged) in mask.into_iter().enumerate() {
                if flagged {
                    table[index * 3] = GAMUT_WARNING_COLOR;
                    table[index * 3 + 1] = GAMUT_WARNING_COLOR;
                    table[index * 3 + 2] = GAMUT_WARNING_COLOR;
                }
            }
        }
    }
    Some(Lut3Data { size, table })
}

/// The display frame after the proof table and the monitor transform.
///
/// The working-space pixels enter as a 16-bit code and sample the proof table. The
/// table's last leg already ends on the monitor, or on sRGB when none is set.
#[must_use]
pub fn apply_soft_proof(
    rgb: &[u8],
    condition: &ProofCondition,
    working_icc: &[u8],
    monitor_icc: Option<&[u8]>,
    size: usize,
) -> Option<Vec<u8>> {
    if rgb.len() % 3 != 0 {
        return None;
    }
    let mut condition = condition.clone();
    condition.monitor_icc = monitor_icc.map(<[u8]>::to_vec);
    let lut = soft_proof_lut(&condition, working_icc, size)?;
    let codes = super::color_mgmt::u8_to_u16_codes(rgb);
    let proofed = apply_lut_u16(&codes, &lut)?;
    Some(super::color_mgmt::u16_to_u8_codes(&proofed))
}

fn println_proof(
    grid: &[u8],
    source: &Profile,
    destination: &Profile,
    display: &Profile,
    condition: &ProofCondition,
) -> Option<Vec<u8>> {
    // A print proof is source -> paper -> display. With no paper-white simulation the
    // paper leg is relative colorimetric; absolute colorimetry simulates the paper and
    // its D-max, which is what the controls ask for.
    let source_to_paper = Transform::new(
        source,
        destination,
        DataType::RGB8,
        condition.intent.intent(),
    )?;
    let mut paper = grid.to_vec();
    source_to_paper.apply(&mut paper);
    let paper_to_display = Transform::new(
        destination,
        display,
        DataType::RGB8,
        if condition.paper_white {
            Intent::AbsoluteColorimetric
        } else {
            Intent::RelativeColorimetric
        },
    )?;
    paper_to_display.apply(&mut paper);
    Some(paper)
}

/// The exact per-pixel CMYK paper proof: source -> paper -> display.
///
/// The preview samples a LUT; an export that carries CMYK paper data takes this path so
/// its pixels are not interpolated across the paper's gamut boundary. `display` is the
/// monitor profile, or sRGB when none is set. `paper_white` uses absolute colorimetry for
/// the paper leg; otherwise it stays relative.
#[must_use]
pub fn per_pixel_print_proof(
    rgb: &[u8],
    source_icc: &[u8],
    paper_icc: &[u8],
    display_icc: Option<&[u8]>,
    condition: &ProofCondition,
) -> Option<Vec<u8>> {
    if rgb.len() % 3 != 0 || !is_cmyk_profile(paper_icc) {
        return None;
    }
    let source = Profile::new_from_slice(source_icc, false)?;
    let paper = Profile::new_from_slice(paper_icc, false)?;
    let display = match display_icc.filter(|bytes| !bytes.is_empty()) {
        Some(bytes) => Profile::new_from_slice(bytes, false)?,
        None => Profile::new_sRGB(),
    };
    let to_paper = Transform::new_to(
        &source,
        &paper,
        DataType::RGB8,
        DataType::CMYK,
        condition.intent.intent(),
    )?;
    let mut cmyk = vec![0u8; rgb.len() / 3 * 4];
    to_paper.convert(rgb, &mut cmyk);
    let to_display = Transform::new_to(
        &paper,
        &display,
        DataType::CMYK,
        DataType::RGB8,
        if condition.paper_white {
            Intent::AbsoluteColorimetric
        } else {
            Intent::RelativeColorimetric
        },
    )?;
    let mut out = vec![0u8; rgb.len()];
    to_display.convert(&cmyk, &mut out);
    Some(out)
}

fn proof_grid_codes(size: usize) -> Vec<u8> {
    if size <= 1 {
        return vec![0];
    }
    let step = 255.0 / (size - 1) as f64;
    (0..size)
        .map(|index| (index as f64 * step).round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// The identity-grid edge used by the display LUT.
#[must_use]
pub fn display_grid_codes(size: usize) -> Vec<u8> {
    grid_codes(size)
}
