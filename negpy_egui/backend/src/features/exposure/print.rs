//! The print kernel of `negpy/features/exposure/logic.py`, over
//! `negpy_core::print_curve`. The pure curve stays in the core; this module builds the
//! per-image values the Python wrapper derives from the paper profile and config.

use negpy_core::print_curve::{Diffusion, Keys, Lut, Plane, PrintCurve, apply_print_curve, tetra};

use crate::domain::models::{ExposureConfig, ProcessConfig, ProcessMode, RenderIntent};
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{
    compute_pivot, effective_midtone_gamma, grade_to_slope, reference_linear_value,
};
use crate::features::exposure::papers::{
    PaperProfile, compose_density_matrices, effective_constants, resolve_print_color,
    resolve_saturation_matrix,
};
use crate::features::exposure::processor::NeutralAxisRefs;
use crate::features::exposure::{
    constants as c2,
    curve::{self, cast_solve_inputs},
    normalization::LogNegativeBounds,
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

/// The inputs of a Flat-master render: a fixed log curve over the normalized image.
#[derive(Clone, Copy, Debug)]
pub struct FlatRenderSettings<'a> {
    pub exposure: &'a ExposureConfig,
    pub process: &'a ProcessConfig,
    pub mode: ProcessMode,
    pub bounds: Option<LogNegativeBounds>,
}

/// Render the Flat master: the normalized log signal through a fixed low-contrast curve.
#[must_use]
pub fn render_flat(image: &[f32], settings: &FlatRenderSettings<'_>) -> Vec<f32> {
    let is_bw = settings.mode == ProcessMode::BW;
    let printed =
        crate::features::exposure::bw_filter::printed_exposure(settings.process, settings.exposure);
    let cmy = curve::filtration_offsets(
        [printed.wb_cyan, printed.wb_magenta, printed.wb_yellow],
        settings.bounds,
    )
    .map(|value| value as f32);
    let input = if is_bw {
        let weights = crate::features::exposure::bw_filter::collapse_weights(
            settings.process,
            settings.exposure,
        );
        let matrix = crate::features::exposure::bw_filter::collapse_matrix(weights);
        let mut out = vec![0.0; image.len()];
        negpy_core::color::matmul_3x3(image, &mut out, matrix);
        out
    } else {
        image.to_vec()
    };
    let paper = crate::features::exposure::papers::effective_paper_profile(
        &settings.exposure.paper_profile,
        Some(settings.mode),
    );
    let stock = if is_bw {
        None
    } else {
        crate::features::exposure::papers::resolve_flat_matrix(Some(paper))
            .map(|matrix| matrix.map(|row| row.map(|value| value as f32)))
    };
    let (gain, lift) = flat_curve_params();
    let mut out = vec![0.0; input.len()];
    negpy_core::color::flat_curve(&input, &mut out, cmy, stock, gain as f32, lift as f32);
    if is_bw {
        let mut lum = vec![0.0; input.len() / 3];
        negpy_core::analysis::luminance(&out, &mut lum);
        out = lum.iter().flat_map(|value| [*value; 3]).collect();
    }
    out
}

/// Every scalar the print kernel needs beyond the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintSettings {
    pub params: PrintParams,
    pub toe: f64,
    pub toe_width: f64,
    pub shoulder: f64,
    pub shoulder_width: f64,
    pub toe_trims: [f64; 3],
    pub shoulder_trims: [f64; 3],
    pub toe_width_trims: [f64; 3],
    pub shoulder_width_trims: [f64; 3],
    pub midtone_gamma_trims: [f64; 3],
    pub cmy_offsets: [f32; 3],
    pub shadow_cmy: [f32; 3],
    pub highlight_cmy: [f32; 3],
    pub shadow_density: f64,
    pub highlight_density: f64,
    pub bpc: bool,
    pub black_hold: f64,
    pub preflash: f64,
    pub grade: f64,
    pub dye_separation: [f64; 3],
    pub separation_damping: f64,
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
            toe_trims: [0.0; 3],
            shoulder_trims: [0.0; 3],
            toe_width_trims: [0.0; 3],
            shoulder_width_trims: [0.0; 3],
            midtone_gamma_trims: [0.0; 3],
            cmy_offsets: [0.0; 3],
            shadow_cmy: [0.0; 3],
            highlight_cmy: [0.0; 3],
            shadow_density: 0.0,
            highlight_density: 0.0,
            bpc: false,
            black_hold: 0.0,
            preflash: 0.0,
            grade: 115.0,
            dye_separation: [1.0; 3],
            separation_damping: 0.0,
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
    let (toe, shoulder) = per_channel_toe_shoulder(
        settings.toe,
        settings.shoulder,
        settings.toe_trims,
        settings.shoulder_trims,
    );
    let toe = toe.map(|value| value * strength);
    let shoulder = shoulder.map(|value| value * strength);
    let widths = per_channel_widths(
        settings.toe_width,
        settings.shoulder_width,
        settings.toe_width_trims,
        settings.shoulder_width_trims,
    );
    let damping = settings.separation_damping;
    let sat = if damping > 0.0 && settings.dye_separation != [1.0; 3] {
        None
    } else {
        resolve_saturation_matrix(settings.dye_separation)
    };
    let dye = resolve_print_color(paper).0;
    let composed = compose_density_matrices(dye, sat);
    let d_min_rgb = paper_dmin_rgb(settings.params.d_min, paper);
    let (flash_v, flash_gamma) = crate::features::exposure::curve::preflash_params(
        settings.grade,
        settings.params.d_min,
        paper,
        Some(settings.params.midtone_gamma),
    );
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
        midtone_gamma: std::array::from_fn(|ch| {
            settings.params.midtone_gamma + settings.midtone_gamma_trims[ch]
        }),
        gamma_width: tonal.paper_gamma_width,
        dye_mix: composed,
        sep: (damping > 0.0 && settings.dye_separation != [1.0; 3]).then_some((
            settings.dye_separation,
            damping,
            c::SEPARATION_DAMPING_REF_SPREAD,
        )),
        ev: None,
        ev_scale: [0.0; 3],
        grade: None,
        keys: None,
        r0: c::ISO_R_MAX,
        r_min: c::ISO_R_MIN,
        r_max: c::ISO_R_MAX,
        flash: settings.preflash.max(0.0),
        flash_v,
        flash_gamma,
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

/// The config and measured values a print render solves from.
#[derive(Clone, Copy, Debug)]
pub struct PrintRenderSettings<'a> {
    pub config: &'a crate::domain::models::ExposureConfig,
    pub process: &'a crate::domain::models::ProcessConfig,
    pub density_range: Option<f64>,
    pub textural_range: Option<f64>,
    pub shadow_point: Option<f64>,
    pub highlight_point: Option<f64>,
    pub anchor: Option<f64>,
    pub bounds: Option<LogNegativeBounds>,
    pub neutral_axis: Option<NeutralAxisRefs>,
    pub mode: crate::domain::models::ProcessMode,
    pub base_black_hold: f64,
    /// The Contrast Mask plane, expanded to the render size.
    pub mask: Option<MaskPlane<'a>>,
    /// The Diffusion plane on its analysis grid, with the rect it covers.
    pub diffusion: Option<DiffusionPlane<'a>>,
    /// The local exposure map, in print stops.
    pub local_exposure: Option<&'a [f32]>,
    /// The local grade factor map, one slope multiplier per pixel.
    pub local_grade: Option<&'a [f32]>,
    /// The local preflash map, a fraction of the threshold exposure.
    pub local_flash: Option<&'a [f32]>,
    /// The tone-limited mask parameters, `(stops, ISO-R delta, e0, e1)` per mask.
    pub key_params: Option<&'a [[f64; 4]]>,
    /// The tone-limited shape alphas, `(alpha, mask count)`, interleaved by pixel.
    pub key_alpha: Option<(&'a [f32], usize)>,
    /// The unlimited masks' raw ISO-R deltas, so a keyed grade starts from their sum.
    pub grade_deltas: Option<&'a [f32]>,
}

/// A Contrast Mask plane and the value it was centred on.
#[derive(Clone, Copy, Debug)]
pub struct MaskPlane<'a> {
    pub data: &'a [f32],
    pub centre: f32,
}

/// A Diffusion plane on its analysis grid and the rect it covers in output pixels.
#[derive(Clone, Copy, Debug)]
pub struct DiffusionPlane<'a> {
    pub data: &'a [f32],
    pub height: usize,
    pub width: usize,
    pub rect: [f64; 4],
}

/// The per-channel curve and tone values of one print render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedPrint {
    pub slopes: [f64; 3],
    pub pivots: [f64; 3],
    pub curvatures: [f64; 3],
    pub toe: f64,
    pub shoulder: f64,
    pub shadow_density: f64,
    pub highlight_density: f64,
    pub shadow_grade: [f64; 3],
    pub highlight_grade: [f64; 3],
}

/// Solve the print curve from config and metrics, mirroring `PhotometricProcessor.process`.
#[must_use]
pub fn solve_print(settings: &PrintRenderSettings<'_>) -> SolvedPrint {
    let exposure = settings.config;
    let paper = crate::features::exposure::papers::effective_paper_profile(
        &exposure.paper_profile,
        Some(settings.mode),
    );
    let d_min = if exposure.paper_dmin {
        paper.tonal.d_min
    } else {
        0.0
    };
    let anchor = exposure.auto_exposure.then_some(settings.anchor).flatten();
    let (strength, neutral_axis) = cast_solve_inputs(
        settings
            .bounds
            .unwrap_or(LogNegativeBounds::new([-1.0; 3], [0.0; 3])),
        settings.neutral_axis,
        exposure.cast_removal_strength,
    );
    let midtone_gamma = curve::effective_midtone_gamma(Some(paper), exposure.midtone_gamma);
    let (slopes, pivots, curvatures) = curve::per_channel_curve_params(
        exposure.grade,
        exposure.density,
        exposure.auto_normalize_contrast,
        strength,
        settings.density_range,
        settings.textural_range,
        d_min,
        anchor,
        Some(paper),
        if strength > 0.0 { neutral_axis } else { None },
        [
            exposure.grade_trim_red,
            exposure.grade_trim_green,
            exposure.grade_trim_blue,
        ],
        settings.shadow_point,
        Some(midtone_gamma),
    );
    let highlight = settings
        .highlight_point
        .filter(|_| exposure.auto_normalize_contrast);
    let highlight_density = highlight.map_or(0.0, |point| {
        curve::highlight_hold_offset(
            slopes[1],
            pivots[1],
            point,
            d_min,
            Some(paper),
            exposure.preflash,
            exposure.grade,
            Some(midtone_gamma),
        )
    });
    let shadow = settings
        .shadow_point
        .filter(|_| exposure.auto_normalize_contrast);
    let shadow_density = shadow.map_or(0.0, |point| {
        curve::shadow_hold_offset(
            slopes[1],
            pivots[1],
            point,
            d_min,
            Some(paper),
            Some(midtone_gamma),
        )
    });
    let (toe, shoulder) = curve::grade_coupled_shape(slopes[1], exposure.toe, exposure.shoulder);
    let (shadow_grade, highlight_grade) = curve::split_grade_deltas(
        exposure.grade,
        exposure.shadow_grade,
        exposure.highlight_grade,
        [
            exposure.shadow_grade_trim_red,
            exposure.shadow_grade_trim_green,
            exposure.shadow_grade_trim_blue,
        ],
        [
            exposure.highlight_grade_trim_red,
            exposure.highlight_grade_trim_green,
            exposure.highlight_grade_trim_blue,
        ],
    );
    SolvedPrint {
        slopes,
        pivots,
        curvatures,
        toe,
        shoulder,
        shadow_density: exposure.shadow_density + shadow_density,
        highlight_density: exposure.highlight_density + highlight_density,
        shadow_grade,
        highlight_grade,
    }
}

/// Render the print stage over an HxWx3 normalized image.
#[must_use]
pub fn render_negative(
    image: &[f32],
    width: usize,
    settings: &PrintRenderSettings<'_>,
) -> Vec<f32> {
    let exposure = settings.config;
    if exposure.render_intent == RenderIntent::Flat {
        return render_flat(
            image,
            &FlatRenderSettings {
                exposure,
                process: settings.process,
                mode: settings.mode,
                bounds: settings.bounds,
            },
        );
    }
    let (settings_out, solved, paper) = print_settings(settings);
    let is_bw = settings.mode == crate::domain::models::ProcessMode::BW;
    let input = if is_bw {
        let weights =
            crate::features::exposure::bw_filter::collapse_weights(settings.process, exposure);
        let matrix = crate::features::exposure::bw_filter::collapse_matrix(weights);
        let mut out = vec![0.0; image.len()];
        negpy_core::analysis::unmix_log(image, matrix, &mut out);
        out
    } else {
        image.to_vec()
    };
    let mut curve = print_curve(&settings_out, Some(paper));
    curve.pivots = solved.pivots.map(|value| value as f32);
    curve.slopes = solved.slopes.map(|value| value as f32);
    curve.curvatures = solved.curvatures.map(|value| value as f32);
    curve.shadow_grade = solved.shadow_grade;
    curve.highlight_grade = solved.highlight_grade;
    curve.dye_mix = if is_bw { None } else { curve.dye_mix };
    let mut ev_map = settings
        .local_exposure
        .map(<[f32]>::to_vec)
        .unwrap_or_default();
    if let Some(mask) = settings.mask {
        let offsets = settings_out.cmy_offsets.map(f64::from);
        if let Some(coeffs) = contrast_mask_coeffs(
            exposure.contrast_mask_strength,
            f64::from(mask.centre),
            solved.slopes,
            solved.pivots,
            offsets,
            settings.density_range.unwrap_or(0.0),
        ) {
            if ev_map.is_empty() {
                ev_map = vec![0.0; image.len() / 3];
            }
            let mut contrast = vec![0.0; image.len() / 3];
            negpy_core::analysis::contrast_mask_stops(
                mask.data,
                &mut contrast,
                coeffs.map(|value| value as f32),
            );
            for (value, add) in ev_map.iter_mut().zip(&contrast) {
                *value += *add;
            }
        }
    }
    if !ev_map.is_empty() {
        curve.ev = Some(Plane {
            data: &ev_map,
            w: width,
        });
        curve.ev_scale = local_ev_scale(settings.bounds);
    }
    if let Some(grade) = settings.local_grade {
        if grade.iter().any(|value| (*value - 1.0).abs() > 1e-6) {
            curve.grade = Some(Plane {
                data: grade,
                w: width,
            });
        }
    }
    if let Some(flash) = settings.local_flash {
        if flash.iter().any(|value| *value > 0.0) {
            curve.flash_map = Some(Plane {
                data: flash,
                w: width,
            });
        }
    }
    if let (Some(params), Some((alpha, _))) = (settings.key_params, settings.key_alpha) {
        let deltas = settings
            .grade_deltas
            .unwrap_or(settings.local_grade.unwrap_or(&[]));
        if !params.is_empty() && !alpha.is_empty() && deltas.len() == image.len() / 3 {
            curve.keys = Some(Keys {
                alpha,
                params,
                deltas: Plane {
                    data: deltas,
                    w: width,
                },
            });
            curve.r0 = exposure.grade.clamp(c::ISO_R_MIN, c::ISO_R_MAX);
        }
    }
    if let Some(diffusion) = settings.diffusion {
        if !diffusion.data.is_empty() {
            curve.diffusion = Some(Diffusion {
                plane: diffusion.data,
                gh: diffusion.height,
                gw: diffusion.width,
                rect: diffusion.rect,
                mix: diffusion_mix(exposure.diffusion),
                range: curve::channel_density_ranges(settings.bounds),
            });
        }
    }
    let mut out = vec![0.0; input.len()];
    apply_print_curve(&input, width, &curve, &mut out);
    if is_bw {
        let mut lum = vec![0.0; input.len() / 3];
        negpy_core::analysis::luminance(&out, &mut lum);
        out = lum.iter().flat_map(|value| [*value; 3]).collect();
    }
    out
}

/// The print settings and solved curve one render uses.
pub(crate) fn print_settings(
    settings: &PrintRenderSettings<'_>,
) -> (PrintSettings, SolvedPrint, &'static PaperProfile) {
    let exposure = settings.config;
    let paper = crate::features::exposure::papers::effective_paper_profile(
        &exposure.paper_profile,
        Some(settings.mode),
    );
    let d_min = if exposure.paper_dmin {
        paper.tonal.d_min
    } else {
        0.0
    };
    let cmy = curve::filtration_offsets(
        [exposure.wb_cyan, exposure.wb_magenta, exposure.wb_yellow],
        settings.bounds,
    );
    let cmy_max = c2::CMY_MAX_DENSITY;
    let shadow_cmy = [
        exposure.shadow_cyan * cmy_max,
        exposure.shadow_magenta * cmy_max,
        exposure.shadow_yellow * cmy_max,
    ];
    let highlight_cmy = [
        exposure.highlight_cyan * cmy_max,
        exposure.highlight_magenta * cmy_max,
        exposure.highlight_yellow * cmy_max,
    ];
    let is_bw = settings.mode == crate::domain::models::ProcessMode::BW;
    let midtone_gamma = curve::effective_midtone_gamma(Some(paper), exposure.midtone_gamma);
    let (dye_separation, separation_damping) = if is_bw {
        ([1.0; 3], 0.0)
    } else {
        (
            per_channel_dye_separation(
                exposure.dye_separation,
                [
                    exposure.dye_separation_trim_red,
                    exposure.dye_separation_trim_green,
                    exposure.dye_separation_trim_blue,
                ],
            ),
            exposure.separation_damping,
        )
    };
    let solved = solve_print(settings);
    let settings_out = PrintSettings {
        params: PrintParams {
            slope: solved.slopes[1],
            pivot: solved.pivots[1],
            d_min,
            midtone_gamma,
        },
        toe: solved.toe,
        toe_width: exposure.toe_width,
        shoulder: solved.shoulder,
        shoulder_width: exposure.shoulder_width,
        toe_trims: [
            exposure.toe_trim_red,
            exposure.toe_trim_green,
            exposure.toe_trim_blue,
        ],
        shoulder_trims: [
            exposure.shoulder_trim_red,
            exposure.shoulder_trim_green,
            exposure.shoulder_trim_blue,
        ],
        toe_width_trims: [
            exposure.toe_width_trim_red,
            exposure.toe_width_trim_green,
            exposure.toe_width_trim_blue,
        ],
        shoulder_width_trims: [
            exposure.shoulder_width_trim_red,
            exposure.shoulder_width_trim_green,
            exposure.shoulder_width_trim_blue,
        ],
        midtone_gamma_trims: [
            exposure.midtone_gamma_trim_red,
            exposure.midtone_gamma_trim_green,
            exposure.midtone_gamma_trim_blue,
        ],
        cmy_offsets: cmy.map(|value| value as f32),
        shadow_cmy: shadow_cmy.map(|value| value as f32),
        highlight_cmy: highlight_cmy.map(|value| value as f32),
        shadow_density: solved.shadow_density,
        highlight_density: solved.highlight_density,
        bpc: !exposure.paper_black,
        black_hold: settings.base_black_hold,
        preflash: exposure.preflash,
        grade: exposure.grade,
        dye_separation,
        separation_damping,
    };
    (settings_out, solved, paper)
}

/// The achromatic print curve sampled from `x_min` to `x_max` into display codes.
///
/// The curve is the same solved curve the render uses, so the chart cannot drift from
/// the preview.
#[must_use]
pub fn sample_curve(
    settings: &PrintRenderSettings<'_>,
    x_min: f64,
    x_max: f64,
    samples: usize,
) -> Vec<[f64; 2]> {
    if samples < 2 || x_max <= x_min {
        return Vec::new();
    }
    if settings.config.render_intent == RenderIntent::Flat {
        let (gain, lift) = flat_curve_params();
        return (0..samples)
            .map(|index| {
                let t = index as f64 / (samples - 1) as f64;
                let x = x_min + t * (x_max - x_min);
                [x, (lift + gain * (1.0 - x)).clamp(0.0, 1.0)]
            })
            .collect();
    }
    let (settings_out, solved, paper) = print_settings(settings);
    let mut curve = print_curve(&settings_out, Some(paper));
    curve.pivots = solved.pivots.map(|value| value as f32);
    curve.slopes = solved.slopes.map(|value| value as f32);
    curve.curvatures = solved.curvatures.map(|value| value as f32);
    curve.shadow_grade = solved.shadow_grade;
    curve.highlight_grade = solved.highlight_grade;
    let points: Vec<f32> = (0..samples)
        .map(|index| {
            let t = index as f64 / (samples - 1) as f64;
            (x_min + t * (x_max - x_min)) as f32
        })
        .collect();
    let mut density = vec![0.0f32; samples * 3];
    for (index, value) in points.iter().enumerate() {
        density[index * 3] = *value;
        density[index * 3 + 1] = *value;
        density[index * 3 + 2] = *value;
    }
    let mut display = vec![0.0f32; density.len()];
    apply_print_curve(&density, samples, &curve, &mut display);
    (0..samples)
        .map(|index| {
            let encoded = crate::features::exposure::analysis::working_oetf_encode(&[
                display[index * 3],
                display[index * 3 + 1],
                display[index * 3 + 2],
            ]);
            [points[index] as f64, encoded[1] as f64]
        })
        .collect()
}

/// The Contrast Mask's seven kernel coefficients, or `None` when the slider is off.
#[must_use]
pub fn contrast_mask_coeffs(
    strength: f64,
    centre: f64,
    slopes: [f64; 3],
    pivots: [f64; 3],
    offsets: [f64; 3],
    density_range: f64,
) -> Option<[f64; 7]> {
    let slope = slopes.iter().sum::<f64>() / 3.0;
    if strength <= 0.0 || slope <= 0.0 || slopes[1] <= 0.0 || density_range <= 0.0 {
        return None;
    }
    let c = c::MASK_CODES_PER_DENSITY;
    let reference = c::REFERENCE_PRINT_DENSITY;
    let shift = (0..3)
        .map(|ch| slopes[ch] * (offsets[ch] - pivots[ch]))
        .sum::<f64>()
        / 3.0;
    let stops_per_code = density_range / (c * slopes[1] * std::f64::consts::LOG10_2);
    let gain = strength * stops_per_code / (4.0 * 256.0);
    Some([
        c * slope,
        c::MASK_AIM_CODE + c * (slope * centre + shift - reference) - c::MASK_MID_CODE,
        -c::MASK_MID_CODE,
        c::MASK_CODE_MAX - c::MASK_MID_CODE,
        gain,
        gain,
        c::MASK_TABLE_CAP / 4.0 * stops_per_code,
    ])
}

/// One dodge or burn stop per channel in the print's normalized space.
#[must_use]
pub fn local_ev_scale(bounds: Option<LogNegativeBounds>) -> [f32; 3] {
    let step = std::f64::consts::LOG10_2;
    std::array::from_fn(|ch| {
        let range = bounds.map_or(1.0, |bounds| {
            (bounds.ceils[ch] - bounds.floors[ch]).abs().max(1e-6)
        });
        (step / range) as f32
    })
}

/// The fraction of blurred light the paper sees at a Diffusion slider value.
#[must_use]
pub fn diffusion_mix(diffusion: f64) -> f64 {
    c::DIFFUSION_MIX_MAX * diffusion.clamp(0.0, 1.0)
}

/// Fixed (gain, lift) for the flat log master.
#[must_use]
pub const fn flat_curve_params() -> (f64, f64) {
    (c::FLAT_LOG_GAIN, c::FLAT_LOG_LIFT)
}

#[cfg(test)]
mod curve_tests {
    use super::*;
    use crate::domain::models::{ExposureConfig, ProcessConfig, ProcessMode};

    #[test]
    fn a_sampled_curve_is_monotone_over_the_chart_domain() {
        let exposure = ExposureConfig::default();
        let process = ProcessConfig::default();
        let settings = PrintRenderSettings {
            config: &exposure,
            process: &process,
            density_range: Some(1.0),
            textural_range: Some(1.0),
            shadow_point: None,
            highlight_point: None,
            anchor: None,
            bounds: None,
            neutral_axis: None,
            mode: ProcessMode::C41,
            base_black_hold: 0.0,
            mask: None,
            diffusion: None,
            local_exposure: None,
            local_grade: None,
            local_flash: None,
            key_params: None,
            key_alpha: None,
            grade_deltas: None,
        };
        let points = sample_curve(&settings, -0.1, 1.1, 33);
        assert_eq!(points.len(), 33);
        assert!(points[0][1] >= points[32][1]);
        assert!(points.iter().all(|point| point[1].is_finite()));
    }
}
