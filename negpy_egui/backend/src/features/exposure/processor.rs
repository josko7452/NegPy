//! The normalization half of `negpy/features/exposure/processor.py`: measured bounds,
//! manual picks, point trims and the meters the print stage reads back.

use negpy_core::analysis::{log_density_open, normalize_log_image};

use crate::domain::models::{ProcessConfig, ProcessMode};
use crate::features::exposure::metering::{
    Roi, analysis_grid, density_above_base, effective_crosstalk_matrix, measure_anchor,
    measure_clip_fractions, measure_highlight_point, measure_neutral_axis, measure_shadow_point,
    measure_textural_range, resolve_analysis_region, unmix_log_image,
};
use crate::features::exposure::normalization::{
    LogNegativeBounds, analyze_log_exposure_bounds, luminance_density_range, mix_luma_color_bounds,
};

/// The neutral-axis references the print stage's cast solve reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NeutralAxisRefs {
    pub mid: [f64; 3],
    pub shadow: [f64; 3],
    pub highlight: Option<[f64; 3]>,
    pub confidence: f64,
}

/// Everything the normalization pass measures for the later stages.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationMetrics {
    pub scan_clip_fractions: [f64; 3],
    pub norm_density_range: f64,
    pub density_above_base: Option<[f64; 3]>,
    pub final_bounds: LogNegativeBounds,
    pub pre_trim_bounds: LogNegativeBounds,
    pub base_bounds: LogNegativeBounds,
    pub metered_anchor: f64,
    pub textural_range: f64,
    pub shadow_point: f64,
    pub highlight_point: f64,
    pub neutral_axis: Option<NeutralAxisRefs>,
}

/// The normalized image and its measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationResult {
    pub image: Vec<f32>,
    pub metrics: NormalizationMetrics,
}

/// The per-frame inputs the normalization pass reads.
#[derive(Clone, Copy, Debug)]
pub struct NormalizationSettings<'a> {
    pub config: &'a ProcessConfig,
    pub mode: ProcessMode,
    /// The crop's pixel window; None meters the frame, or the config's meter rect.
    pub active_roi: Option<Roi>,
    /// The clear base the engine read for this source.
    pub base_log: Option<[f64; 3]>,
}

fn triple(values: Option<&Vec<f64>>) -> Option<[f64; 3]> {
    let values = values?;
    (values.len() >= 3).then(|| [values[0], values[1], values[2]])
}

fn rect(values: Option<&Vec<f64>>) -> Option<[f64; 4]> {
    let values = values?;
    (values.len() >= 4).then(|| [values[0], values[1], values[2], values[3]])
}

fn point_offsets(config: &ProcessConfig) -> ([f64; 3], [f64; 3]) {
    let white = config.white_point_offset;
    let black = config.black_point_offset;
    (
        [
            white + config.white_point_trim_red,
            white + config.white_point_trim_green,
            white + config.white_point_trim_blue,
        ],
        [
            black + config.black_point_trim_red,
            black + config.black_point_trim_green,
            black + config.black_point_trim_blue,
        ],
    )
}

fn locked_bounds(config: &ProcessConfig) -> LogNegativeBounds {
    LogNegativeBounds::new(config.locked_floors, config.locked_ceils)
}

fn local_bounds(config: &ProcessConfig) -> LogNegativeBounds {
    LogNegativeBounds::new(config.local_floors, config.local_ceils)
}

fn apply_manual_picks(config: &ProcessConfig, base: LogNegativeBounds) -> LogNegativeBounds {
    let floors = triple(config.manual_dmax.as_ref()).unwrap_or(base.floors);
    let ceils = triple(config.manual_dmin.as_ref()).unwrap_or(base.ceils);
    LogNegativeBounds::new(floors, ceils)
}

/// The final bounds and the per-frame base, mirroring `resolve_bounds_detailed`.
#[must_use]
pub fn resolve_bounds(
    config: &ProcessConfig,
    analyze: impl FnOnce() -> LogNegativeBounds,
) -> (LogNegativeBounds, LogNegativeBounds) {
    let locked_initialized = config.is_local_initialized();
    let roll_luma = config.use_luma_average && locked_initialized;
    let roll_color = config.use_color_average && locked_initialized;
    let locked = locked_bounds(config);
    if roll_luma && roll_color {
        return (locked, locked);
    }
    let base = if config.is_local_initialized() {
        local_bounds(config)
    } else {
        analyze()
    };
    let base = apply_manual_picks(config, base);
    let luma = if roll_luma { locked } else { base };
    let color = if roll_color { locked } else { base };
    (mix_luma_color_bounds(luma, color), base)
}

/// The bounds the luma meters read: the roll baseline under luma averaging, else the base.
#[must_use]
pub fn luma_source_bounds(config: &ProcessConfig, base: LogNegativeBounds) -> LogNegativeBounds {
    if config.use_luma_average && config.is_local_initialized() {
        locked_bounds(config)
    } else {
        base
    }
}

/// Run the normalization pass over an HxWx3 linear image.
#[must_use]
pub fn normalize_negative(
    image: &[f32],
    width: usize,
    height: usize,
    settings: NormalizationSettings<'_>,
) -> NormalizationResult {
    let config = settings.config;
    let mut log = vec![0.0; image.len()];
    log_density_open(image, &mut log);

    let (roi, buffer) = resolve_analysis_region(
        width,
        height,
        settings.active_roi,
        config.analysis_buffer,
        rect(config.analysis_rect.as_ref()),
        rect(config.meter_rect.as_ref()),
    );
    let (grid, grid_width, grid_height) = analysis_grid(image, width, height, roi, buffer);
    let scan_clip_fractions = measure_clip_fractions(image, width, height, roi, buffer);

    let unmix = effective_crosstalk_matrix(config, Some(settings.mode));
    let log = unmix_log_image(&log, unmix);
    let grid = unmix_log_image(&grid, unmix);

    let (mut bounds, base_bounds) = resolve_bounds(config, || {
        analyze_log_exposure_bounds(&grid, config.luma_range_clip, config.color_range_clip)
    });
    let pre_trim_bounds = bounds;
    let norm_density_range = luminance_density_range(bounds);
    let density_above_base = density_above_base(settings.base_log, pre_trim_bounds, &grid, unmix);

    let (white, black) = point_offsets(config);
    if white.iter().chain(black.iter()).any(|value| *value != 0.0) {
        bounds = LogNegativeBounds::new(
            std::array::from_fn(|ch| bounds.floors[ch] + white[ch]),
            std::array::from_fn(|ch| bounds.ceils[ch] + black[ch]),
        );
    }

    let mut normalized = vec![0.0; log.len()];
    normalize_log_image(
        &log,
        bounds.floors.map(|value| value as f32),
        bounds.ceils.map(|value| value as f32),
        &mut normalized,
    );

    let neutral_axis = if settings.mode == ProcessMode::BW {
        None
    } else {
        measure_neutral_axis(&grid, grid_width, grid_height, pre_trim_bounds, true).map(|axis| {
            NeutralAxisRefs {
                mid: axis.mid,
                shadow: axis.shadow,
                highlight: axis.highlight,
                confidence: axis.confidence,
            }
        })
    };

    let anchor_bounds = luma_source_bounds(config, base_bounds);
    let metrics = NormalizationMetrics {
        scan_clip_fractions,
        norm_density_range,
        density_above_base,
        final_bounds: bounds,
        pre_trim_bounds,
        base_bounds,
        metered_anchor: measure_anchor(&grid, grid_width, grid_height, anchor_bounds),
        textural_range: measure_textural_range(&grid, grid_width, grid_height),
        shadow_point: measure_shadow_point(&grid, grid_width, grid_height, anchor_bounds),
        highlight_point: measure_highlight_point(&grid, grid_width, grid_height, anchor_bounds),
        neutral_axis,
    };
    NormalizationResult {
        image: normalized,
        metrics,
    }
}
