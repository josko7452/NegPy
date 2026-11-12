//! The print kernel of `negpy/features/exposure/logic.py`, over
//! `negpy_core::print_curve`. The pure curve stays in the core; this module builds the
//! per-image values the Python wrapper derives from the paper profile and config.

use negpy_core::print_curve::{Lut, PrintCurve, apply_print_curve, tetra};

use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{
    compute_pivot, effective_midtone_gamma, grade_to_slope, reference_linear_value,
};
use crate::features::exposure::papers::{
    PaperProfile, compose_density_matrices, effective_constants, resolve_print_color,
    resolve_saturation_matrix,
};

/// The straight-line input of one print: grade, density and an optional metered anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintParams {
    pub slope: f64,
    pub pivot: f64,
    pub d_min: f64,
    pub midtone_gamma: f64,
}

impl PrintParams {
    /// The parameters the Python processor solves for a neutral print.
    #[must_use]
    pub fn from_grade(
        grade: f64,
        density: f64,
        d_min: f64,
        anchor: Option<f64>,
        paper: Option<&PaperProfile>,
        midtone_gamma: Option<f64>,
    ) -> Self {
        let slope = grade_to_slope(grade, None, paper.map_or(1.0, |p| p.slope_gain));
        Self {
            slope,
            pivot: compute_pivot(slope, density, d_min, anchor, paper, midtone_gamma),
            d_min,
            midtone_gamma: midtone_gamma.unwrap_or(effective_midtone_gamma(paper, 0.0)),
        }
    }
}

/// Every scalar the print kernel needs beyond the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintSettings {
    pub params: PrintParams,
    pub toe: f64,
    pub toe_width: f64,
    pub shoulder: f64,
    pub shoulder_width: f64,
    pub cmy_offsets: [f32; 3],
    pub shadow_cmy: [f32; 3],
    pub highlight_cmy: [f32; 3],
    pub shadow_density: f64,
    pub highlight_density: f64,
    pub bpc: bool,
    pub black_hold: f64,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            params: PrintParams {
                slope: 3.0,
                pivot: 0.5,
                d_min: 0.0,
                midtone_gamma: c::PAPER_MIDTONE_GAMMA,
            },
            toe: 0.0,
            toe_width: 2.5,
            shoulder: 0.0,
            shoulder_width: 2.5,
            cmy_offsets: [0.0; 3],
            shadow_cmy: [0.0; 3],
            highlight_cmy: [0.0; 3],
            shadow_density: 0.0,
            highlight_density: 0.0,
            bpc: false,
            black_hold: 0.0,
        }
    }
}

/// The paper values the kernel reads, including the base tint and dye coupling.
#[must_use]
pub fn paper_dmin_rgb(d_min: f64, paper: Option<&PaperProfile>) -> [f64; 3] {
    if d_min <= 0.0 {
        return [d_min.max(0.0); 3];
    }
    match paper {
        Some(profile) => std::array::from_fn(|ch| (d_min + profile.base_tint_cmy[ch]).max(0.0)),
        None => [d_min.max(0.0); 3],
    }
}

/// Per-layer effective toe/shoulder: grade-coupled global value plus the layer trim,
/// clamped to the slider domain.
#[must_use]
pub fn per_channel_toe_shoulder(
    toe_eff: f64,
    shoulder_eff: f64,
    toe_trims: [f64; 3],
    shoulder_trims: [f64; 3],
) -> ([f64; 3], [f64; 3]) {
    let clamp = |value: f64| value.clamp(-1.0, 1.0);
    (
        std::array::from_fn(|ch| clamp(toe_eff + toe_trims[ch])),
        std::array::from_fn(|ch| clamp(shoulder_eff + shoulder_trims[ch])),
    )
}

/// Per-layer effective toe/shoulder widths, clamped to the width slider domain.
#[must_use]
pub fn per_channel_widths(
    toe_width: f64,
    shoulder_width: f64,
    toe_trims: [f64; 3],
    shoulder_trims: [f64; 3],
) -> ([f64; 3], [f64; 3]) {
    let clamp = |value: f64| value.clamp(0.1, 5.0);
    (
        std::array::from_fn(|ch| clamp(toe_width + toe_trims[ch])),
        std::array::from_fn(|ch| clamp(shoulder_width + shoulder_trims[ch])),
    )
}

/// Per-layer dye-separation k, clamped to the matrix-coefficient range.
#[must_use]
pub fn per_channel_dye_separation(dye_separation: f64, trims: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|ch| (dye_separation + trims[ch]).clamp(0.0, 3.0))
}

/// The black density per channel that black point compensation maps to black.
#[must_use]
pub fn black_density_rgb(d_max: f64, toe: [f64; 3], toe_height: f64) -> [f64; 3] {
    std::array::from_fn(|ch| {
        if toe[ch] < 0.0 {
            d_max + toe[ch] * toe_height
        } else {
            d_max
        }
    })
}

/// Luminance of a LUT's black corner, which BPC maps to display black.
#[must_use]
pub fn lut_black_luminance(
    table: &[f32],
    n: usize,
    e_max: [f32; 3],
    d_min_rgb: [f64; 3],
    black_rgb: [f64; 3],
) -> f64 {
    let lut = Lut {
        table,
        n,
        e_max,
        black: 0.0,
    };
    let rgb = tetra(
        &lut,
        std::array::from_fn(|ch| black_rgb[ch] - d_min_rgb[ch]),
    );
    (0..3)
        .map(|ch| [c::LUMA_R, c::LUMA_G, c::LUMA_B][ch] * 10f64.powf(-d_min_rgb[ch]) * rgb[ch])
        .sum()
}

/// The print curve of one image, as the Python wrapper passes it to the kernel.
#[must_use]
pub fn print_curve(settings: &PrintSettings, paper: Option<&PaperProfile>) -> PrintCurve<'static> {
    let tonal = effective_constants(paper);
    let strength = c::TOE_SHOULDER_STRENGTH;
    let toe = [settings.toe * strength; 3];
    let shoulder = [settings.shoulder * strength; 3];
    let widths = per_channel_widths(
        settings.toe_width,
        settings.shoulder_width,
        [0.0; 3],
        [0.0; 3],
    );
    let dye_separation = [1.0; 3];
    let damping = 0.0;
    let sat = if damping > 0.0 && dye_separation != [1.0; 3] {
        None
    } else {
        resolve_saturation_matrix(dye_separation)
    };
    let dye = resolve_print_color(paper).0;
    let composed = compose_density_matrices(dye, sat);
    let d_min_rgb = paper_dmin_rgb(settings.params.d_min, paper);
    PrintCurve {
        pivots: [settings.params.pivot as f32; 3],
        slopes: [settings.params.slope as f32; 3],
        curvatures: [0.0; 3],
        toe,
        shoulder,
        toe_width: widths.0,
        shoulder_width: widths.1,
        cmy_offsets: settings.cmy_offsets,
        shadow_cmy: settings.shadow_cmy,
        highlight_cmy: settings.highlight_cmy,
        d_min_rgb,
        d_max: tonal.d_max,
        a_toe_base: tonal.toe_sharpness_base,
        a_sh_base: tonal.shoulder_sharpness_base,
        width_ref: c::TOESHOULDER_WIDTH_REF,
        toe_height: tonal.toe_height,
        sh_height: tonal.shoulder_height,
        zone_center: c::REFERENCE_PRINT_DENSITY,
        shadow_density: settings.shadow_density,
        highlight_density: settings.highlight_density,
        shadow_grade: [0.0; 3],
        highlight_grade: [0.0; 3],
        zone_sh_center: c::REFERENCE_PRINT_DENSITY + c::ZONE_DENSITY_SHADOW_OFFSET,
        zone_hi_center: c::REFERENCE_PRINT_DENSITY + c::ZONE_DENSITY_HIGHLIGHT_OFFSET,
        zone_k: c::ZONE_DENSITY_SHARPNESS,
        v_star: reference_linear_value(settings.params.d_min, paper, None),
        midtone_gamma: [settings.params.midtone_gamma; 3],
        gamma_width: tonal.paper_gamma_width,
        dye_mix: composed,
        sep: None,
        ev: None,
        ev_scale: [0.0; 3],
        grade: None,
        keys: None,
        r0: c::ISO_R_MAX,
        r_min: c::ISO_R_MIN,
        r_max: c::ISO_R_MAX,
        flash: 0.0,
        flash_v: 0.0,
        flash_gamma: 1.0,
        flash_map: None,
        diffusion: None,
        lut: None,
        bpc: settings.bpc,
        black_hold: settings.black_hold,
    }
}

/// Apply the neutral print curve to an HxWx3 float32 image.
#[must_use]
pub fn render_print(
    image: &[f32],
    width: usize,
    settings: &PrintSettings,
    paper: Option<&PaperProfile>,
) -> Vec<f32> {
    let curve = print_curve(settings, paper);
    let mut out = vec![0.0; image.len()];
    apply_print_curve(image, width, &curve, &mut out);
    out
}

/// Fixed (gain, lift) for the flat log master.
#[must_use]
pub const fn flat_curve_params() -> (f64, f64) {
    (c::FLAT_LOG_GAIN, c::FLAT_LOG_LIFT)
}
