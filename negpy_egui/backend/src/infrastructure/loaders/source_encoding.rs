//! Positive source decoding by Source Encoding.
//!
//! Mirrors `negpy/infrastructure/loaders/source_encoding.py`: an embedded matrix/TRC
//! profile supplies the tone curves and source-to-working primaries, and an untagged
//! 8-bit file reads as sRGB while an untagged 16-bit file reads as linear.

use negpy_core::color;
use negpy_core::loaders;
use negpy_core::mathf::pow_f64;

use crate::domain::models::SourceEncoding;
use crate::infrastructure::loaders::helpers::identify_color_space_from_icc;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// What ran when Auto read the file's profile or color tag.
pub const PROFILE: &str = "profile";

/// Adobe RGB (1998) colorants in the D50 PCS.
pub const WORKING_COLORANTS_D50: [[f64; 3]; 3] = [
    [0.60974, 0.20528, 0.14919],
    [0.31111, 0.62567, 0.06322],
    [0.01947, 0.06087, 0.74457],
];

/// sRGB colorants in the D50 PCS.
pub const SRGB_COLORANTS_D50: [[f64; 3]; 3] = [
    [0.43604, 0.38510, 0.14307],
    [0.22244, 0.71693, 0.06062],
    [0.01390, 0.09708, 0.71393],
];

const IDENTITY_TOLERANCE: f64 = 2e-3;
const INSIDE_TOLERANCE: f32 = -1e-3;
const WORKING_GAMMA: f64 = 563.0 / 256.0;

/// The sample width that selects the decode tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleDepth {
    Eight,
    Sixteen,
    Other,
}

impl SampleDepth {
    /// The curve length for this sample width.
    #[must_use]
    pub fn samples(self) -> usize {
        if self == Self::Eight { 256 } else { 65536 }
    }
}

/// A matrix/TRC profile: three tone curves sampled on 0..1, and source-to-working
/// primaries. `to_working` is `None` when the profile is the working space.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileDecode {
    pub curves: [Vec<f32>; 3],
    pub to_working: Option<[[f32; 3]; 3]>,
    /// Linear samples already in the working primaries pass through as uint16.
    pub identity: bool,
}

/// The decode Auto or an explicit encoding selected, and the name of what ran.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedDecode {
    pub ran: String,
    pub decode: ProfileDecode,
}

fn s15f16(raw: &[u8]) -> Option<f64> {
    let bytes = raw.get(..4)?;
    Some(i32::from_be_bytes(bytes.try_into().ok()?) as f64 / 65536.0)
}

fn tag_table(icc: &[u8]) -> Option<Vec<([u8; 4], usize, usize)>> {
    let count = u32::from_be_bytes(icc.get(128..132)?.try_into().ok()?) as usize;
    let mut tags = Vec::with_capacity(count);
    for index in 0..count {
        let start = 132 + index * 12;
        let entry = icc.get(start..start + 12)?;
        let kind = entry[0..4].try_into().ok()?;
        let offset = u32::from_be_bytes(entry[4..8].try_into().ok()?) as usize;
        let size = u32::from_be_bytes(entry[8..12].try_into().ok()?) as usize;
        tags.push((kind, offset, size));
    }
    Some(tags)
}

fn tag<'a>(tags: &[([u8; 4], usize, usize)], icc: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let (_, offset, size) = tags.iter().find(|(name, _, _)| name == kind)?;
    icc.get(*offset..offset.checked_add(*size)?)
}

fn curve(tag: &[u8], x: &[f64]) -> Option<Vec<f64>> {
    match tag.get(..4)? {
        b"curv" => {
            let n = u32::from_be_bytes(tag.get(8..12)?.try_into().ok()?) as usize;
            if n == 0 {
                return Some(x.to_vec());
            }
            if n == 1 {
                let gamma = u16::from_be_bytes(tag.get(12..14)?.try_into().ok()?) as f64 / 256.0;
                return Some(x.iter().map(|value| pow_f64(*value, gamma)).collect());
            }
            let raw = tag.get(12..12 + 2 * n)?;
            let table: Vec<f64> = raw
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]) as f64 / 65535.0)
                .collect();
            Some(interp(x, &table))
        }
        b"para" => {
            let function = u16::from_be_bytes(tag.get(8..10)?.try_into().ok()?) as usize;
            let count = match function {
                0 => 1,
                1 => 3,
                2 => 4,
                3 => 5,
                4 => 7,
                _ => return None,
            };
            let mut p = Vec::with_capacity(count);
            for index in 0..count {
                p.push(s15f16(tag.get(12 + 4 * index..16 + 4 * index)?)?);
            }
            let g = p[0];
            if function == 0 {
                return Some(x.iter().map(|value| pow_f64(*value, g)).collect());
            }
            let (a, b) = (p[1], p[2]);
            let cut = -b / a;
            Some(
                x.iter()
                    .map(|value| {
                        let lin = (a * value + b).max(0.0);
                        match function {
                            1 => {
                                if *value >= cut {
                                    pow_f64(lin, g)
                                } else {
                                    0.0
                                }
                            }
                            2 => {
                                if *value >= cut {
                                    pow_f64(lin, g) + p[3]
                                } else {
                                    p[3]
                                }
                            }
                            3 => {
                                if *value >= p[4] {
                                    pow_f64(lin, g)
                                } else {
                                    p[3] * value
                                }
                            }
                            _ => {
                                if *value >= p[4] {
                                    pow_f64(lin, g) + p[5]
                                } else {
                                    p[3] * value + p[6]
                                }
                            }
                        }
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

fn interp(x: &[f64], table: &[f64]) -> Vec<f64> {
    if table.is_empty() {
        return vec![0.0; x.len()];
    }
    if table.len() == 1 {
        return vec![table[0]; x.len()];
    }
    let scale = (table.len() - 1) as f64;
    x.iter()
        .map(|value| {
            let position = value.clamp(0.0, 1.0) * scale;
            let index = (position.floor() as usize).min(table.len() - 2);
            let fraction = position - index as f64;
            table[index] + (table[index + 1] - table[index]) * fraction
        })
        .collect()
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let mut m = [[0.0; 4]; 3];
    for row in 0..3 {
        m[row] = [a[row][0], a[row][1], a[row][2], b[row]];
    }
    for column in 0..3 {
        let pivot =
            (column..3).max_by(|&x, &y| m[x][column].abs().total_cmp(&m[y][column].abs()))?;
        if m[pivot][column] == 0.0 {
            return None;
        }
        m.swap(column, pivot);
        let divisor = m[column][column];
        for value in &mut m[column][column..] {
            *value /= divisor;
        }
        for row in 0..3 {
            if row == column {
                continue;
            }
            let factor = m[row][column];
            for index in column..4 {
                m[row][index] -= factor * m[column][index];
            }
        }
    }
    Some([m[0][3], m[1][3], m[2][3]])
}

/// Source-to-working primaries from D50 colorants; `None` for the working space.
#[must_use]
pub fn to_working(source_d50: [[f64; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let mut m = [[0.0; 3]; 3];
    for column in 0..3 {
        let solved = solve3(
            WORKING_COLORANTS_D50,
            [
                source_d50[0][column],
                source_d50[1][column],
                source_d50[2][column],
            ],
        )?;
        for row in 0..3 {
            m[row][column] = solved[row];
        }
    }
    let finite = m.iter().flatten().all(|value| value.is_finite());
    let identity = (0..3).all(|row| {
        (0..3).all(|column| {
            let target = if row == column { 1.0 } else { 0.0 };
            (m[row][column] - target).abs() <= IDENTITY_TOLERANCE
        })
    });
    if !finite || identity {
        return None;
    }
    Some(m.map(|row| row.map(|value| value as f32)))
}

/// Parse a matrix/TRC or gray TRC ICC profile into curves and primaries.
#[must_use]
pub fn parse_profile(icc: &[u8], samples: usize) -> Option<ProfileDecode> {
    if icc.len() < 132 {
        return None;
    }
    let tags = tag_table(icc)?;
    let x: Vec<f64> = (0..samples)
        .map(|index| index as f64 / (samples.saturating_sub(1)) as f64)
        .collect();
    let rgb = [b"rTRC", b"gTRC", b"bTRC"].map(|kind| tag(&tags, icc, kind));
    let curves = if let [Some(r), Some(g), Some(b)] = rgb {
        [curve(r, &x)?, curve(g, &x)?, curve(b, &x)?]
    } else if let Some(gray) = tag(&tags, icc, b"kTRC") {
        let curve = curve(gray, &x)?;
        [curve.clone(), curve.clone(), curve]
    } else {
        return None;
    };
    let to_working = if let [Some(r), Some(g), Some(b)] =
        [b"rXYZ", b"gXYZ", b"bXYZ"].map(|kind| tag(&tags, icc, kind))
    {
        let mut source = [[0.0; 3]; 3];
        for (column, xyz) in [r, g, b].into_iter().enumerate() {
            for row in 0..3 {
                source[row][column] = s15f16(xyz.get(8 + 4 * row..12 + 4 * row)?)?;
            }
        }
        to_working(source)
    } else {
        None
    };
    let curves = curves.map(|values| {
        values
            .into_iter()
            .map(|value| value.clamp(0.0, 1.0) as f32)
            .collect()
    });
    Some(ProfileDecode {
        curves,
        to_working,
        identity: false,
    })
}

fn power(gamma: f64, samples: usize, to_working: Option<[[f32; 3]; 3]>) -> ProfileDecode {
    let curve: Vec<f32> = (0..samples)
        .map(|index| {
            let value = index as f64 / (samples.saturating_sub(1)) as f64;
            pow_f64(value, gamma) as f32
        })
        .collect();
    ProfileDecode {
        curves: [curve.clone(), curve.clone(), curve],
        to_working,
        identity: gamma == 1.0 && to_working.is_none(),
    }
}

fn srgb_curve(samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|index| {
            let value = index as f64 / (samples.saturating_sub(1)) as f64;
            if value <= 0.04045 {
                (value / 12.92) as f32
            } else {
                pow_f64((value + 0.055) / 1.055, 2.4) as f32
            }
        })
        .collect()
}

fn srgb(samples: usize) -> ProfileDecode {
    let curve = srgb_curve(samples);
    ProfileDecode {
        curves: [curve.clone(), curve.clone(), curve],
        to_working: to_working(SRGB_COLORANTS_D50),
        identity: false,
    }
}

/// A scanner's own input profile describes the device, not the file's encoding.
#[must_use]
pub fn is_device_profile(icc: &[u8]) -> bool {
    icc.len() >= 16 && icc.get(12..16) == Some(b"scnr")
}

/// The decode for a Positive source, and the name of the rule that ran.
#[must_use]
pub fn resolve_decode(
    encoding: SourceEncoding,
    icc: Option<&[u8]>,
    depth: SampleDepth,
) -> ResolvedDecode {
    let samples = depth.samples();
    if encoding == SourceEncoding::Linear {
        return ResolvedDecode {
            ran: SourceEncoding::Linear.as_str().to_string(),
            decode: power(1.0, samples, None),
        };
    }
    if encoding == SourceEncoding::Gamma22 {
        return ResolvedDecode {
            ran: SourceEncoding::Gamma22.as_str().to_string(),
            decode: power(2.2, samples, None),
        };
    }
    let icc = icc.filter(|bytes| !is_device_profile(bytes));
    if let Some(profile) = icc.and_then(|bytes| parse_profile(bytes, samples)) {
        return ResolvedDecode {
            ran: PROFILE.to_string(),
            decode: profile,
        };
    }
    if let Some(name) = icc.and_then(identify_color_space_from_icc) {
        if name == "Adobe RGB" {
            return ResolvedDecode {
                ran: PROFILE.to_string(),
                decode: power(WORKING_GAMMA, samples, None),
            };
        }
        if name == "sRGB" {
            return ResolvedDecode {
                ran: PROFILE.to_string(),
                decode: srgb(samples),
            };
        }
    }
    if depth == SampleDepth::Eight {
        ResolvedDecode {
            ran: "srgb".to_string(),
            decode: srgb(samples),
        }
    } else {
        ResolvedDecode {
            ran: SourceEncoding::Linear.as_str().to_string(),
            decode: power(1.0, samples, None),
        }
    }
}

fn apply_curves(image: &ImageBuffer, curves: &[Vec<f32>; 3]) -> ImageBuffer {
    if let Some(source) = &image.source_u16 {
        let n = curves[0].len();
        let table = [curves[0].clone(), curves[1].clone(), curves[2].clone()].concat();
        let mut out = vec![0.0; image.width * image.height * 3];
        loaders::decode_curves_int(source, 3, &table, n, &mut out);
        return ImageBuffer::from_rgb(image.width, image.height, out);
    }
    let grid: Vec<f32> = (0..curves[0].len())
        .map(|index| index as f32 / (curves[0].len().saturating_sub(1)) as f32)
        .collect();
    let table = [curves[0].clone(), curves[1].clone(), curves[2].clone()].concat();
    let mut out = vec![0.0; image.pixels.len()];
    loaders::decode_curves_f32(&image.pixels, 3, &grid, &table, &mut out);
    ImageBuffer::from_rgb(image.width, image.height, out)
}

fn into_gamut(mut rgb: Vec<f32>) -> Vec<f32> {
    if rgb.iter().all(|value| *value >= 0.0 && *value <= 1.0) {
        return rgb;
    }
    let weights = WORKING_COLORANTS_D50[1];
    let mut out = vec![0.0; rgb.len()];
    for (dst, pixel) in out.chunks_exact_mut(3).zip(rgb.chunks_exact(3)) {
        let y = (pixel[0] as f64 * weights[0]
            + pixel[1] as f64 * weights[1]
            + pixel[2] as f64 * weights[2]) as f32;
        let y = y.clamp(0.0, 1.0);
        let lo = pixel[0].min(pixel[1]).min(pixel[2]);
        let hi = pixel[0].max(pixel[1]).max(pixel[2]);
        let t_lo = if lo < 0.0 { y / (y - lo) } else { 1.0 };
        let t_hi = if hi > 1.0 { (1.0 - y) / (hi - y) } else { 1.0 };
        let t = if t_lo.is_nan() || t_hi.is_nan() {
            0.0
        } else {
            t_lo.min(t_hi).clamp(0.0, 1.0)
        };
        for channel in 0..3 {
            dst[channel] = (y + t * (pixel[channel] - y)).clamp(0.0, 1.0);
        }
    }
    rgb.clear();
    out
}

/// Linear working-space RGB from a Positive source's samples.
#[must_use]
pub fn apply_decode(image: &ImageBuffer, decode: &ProfileDecode) -> ImageBuffer {
    let mut linear = apply_curves(image, &decode.curves);
    if let Some(matrix) = decode.to_working {
        let mut moved = vec![0.0; linear.pixels.len()];
        color::matmul_3x3(&linear.pixels, &mut moved, matrix);
        linear = ImageBuffer::from_rgb(image.width, image.height, into_gamut(moved));
    }
    linear.rgb_xyz_matrix = None;
    linear
}

/// Decode a Positive source, keeping its primaries when its gamut fits the working space.
#[must_use]
pub fn decode_positive(
    image: &ImageBuffer,
    decode: &ProfileDecode,
) -> (ImageBuffer, Option<[[f32; 3]; 3]>) {
    if let Some(matrix) = decode.to_working {
        if matrix
            .iter()
            .flatten()
            .copied()
            .fold(f32::INFINITY, f32::min)
            >= INSIDE_TOLERANCE
        {
            let mut linear = apply_curves(image, &decode.curves);
            linear.rgb_xyz_matrix = Some(matrix);
            return (linear, Some(matrix));
        }
    }
    (apply_decode(image, decode), None)
}
