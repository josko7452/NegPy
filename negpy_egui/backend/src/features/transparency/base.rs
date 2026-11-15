//! The slide base stage: camera primaries to a fixed log-density window.
//!
//! Mirrors `TransparencyBaseProcessor` in `negpy/features/transparency/processor.py`.
//! No meter shapes the window: a raw slide was exposed deliberately, so every frame of
//! one capture series renders the same. The meters only move the render when Auto
//! Density or Auto Grade is on, and a slide starts with both off.

use crate::domain::models::{ProcessConfig, ProcessMode};
use crate::features::exposure::metering::{
    measure_anchor, measure_clip_fractions, measure_highlight_point, measure_shadow_point,
    measure_textural_range, resolve_analysis_region,
};
use crate::features::exposure::normalization::{
    LogNegativeBounds, luminance_density_range, prefilter_log_grid, to_log_density_image,
};
use crate::features::process::capture_color::apply_camera_matrix;
use crate::features::process::fade::{
    MeasuredWhite, apply_crossover, crossover_offsets, crossover_space, fade_density,
    render_white_space, resolve_crossover, with_fade_white,
};
use crate::features::process::fade_estimate::measure_white;
use crate::features::process::path::{RenderPath, render_path};
use crate::features::transparency::logic::{
    DensityStage, TRANSFER_DENSITY_RANGE, density_stage, fade_white_grid,
    measure_transfer_neutral_axis, relog_matrix_image, transfer_bounds, transfer_point_offsets,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The meters the base stage stored for the exposure stage.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferMetrics {
    pub bounds: LogNegativeBounds,
    pub norm_density_range: f64,
    pub metered_anchor: f64,
    pub textural_range: f64,
    pub shadow_point: f64,
    pub highlight_point: f64,
    pub neutral_axis: Option<crate::features::exposure::processor::NeutralAxisRefs>,
    pub fade_white: Option<[f64; 3]>,
    pub fade_crossover: Option<Vec<f64>>,
    pub scan_clip_fractions: [f64; 3],
}

impl Default for TransferMetrics {
    fn default() -> Self {
        Self {
            bounds: LogNegativeBounds::new([0.0; 3], [-TRANSFER_DENSITY_RANGE; 3]),
            norm_density_range: TRANSFER_DENSITY_RANGE,
            metered_anchor: 0.0,
            textural_range: 0.0,
            shadow_point: 0.0,
            highlight_point: 0.0,
            neutral_axis: None,
            fade_white: None,
            fade_crossover: None,
            scan_clip_fractions: [0.0; 3],
        }
    }
}

/// The normalized slide and its meters.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferBaseResult {
    pub image: ImageBuffer,
    pub metrics: TransferMetrics,
    /// A white the stage measured for itself, which the frame stores.
    pub measured_white: Option<MeasuredWhite>,
}

/// The per-frame inputs the slide base reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransferBaseSettings<'a> {
    pub cam_xyz: Option<[[f64; 3]; 3]>,
    pub camera_wb: Option<&'a [f64]>,
    /// The crop's pixel window; None meters the frame or the config's meter rect.
    pub active_roi: Option<(usize, usize, usize, usize)>,
    /// The Cast Removal strength; the axis is metered only when it can act.
    pub cast_strength: f64,
}

fn rect(values: Option<&Vec<f64>>) -> Option<[f64; 4]> {
    let values = values?;
    (values.len() >= 4).then(|| [values[0], values[1], values[2], values[3]])
}

/// The slide's transfer anchor: where a well-exposed frame's midtone is expected.
#[must_use]
pub fn transfer_assumed_anchor() -> f64 {
    crate::features::transparency::logic::TRANSFER_CONTRAST_PIVOT / TRANSFER_DENSITY_RANGE
}

/// Run the slide base over one linear frame.
#[must_use]
pub fn process(
    image: &ImageBuffer,
    config: &ProcessConfig,
    mode: ProcessMode,
    settings: TransferBaseSettings<'_>,
) -> TransferBaseResult {
    let (width, height) = (image.width, image.height);
    let (roi, buffer) = resolve_analysis_region(
        width,
        height,
        settings.active_roi,
        config.analysis_buffer,
        rect(config.analysis_rect.as_ref()),
        rect(config.meter_rect.as_ref()),
    );
    let white_space = render_white_space(config, settings.cam_xyz, settings.camera_wb);
    let measured_white = if crate::features::process::fade::fade_white_stale(config, &white_space) {
        let white_grid = fade_white_grid(
            config,
            Some(mode),
            settings.cam_xyz,
            settings.camera_wb,
            image,
            roi,
            buffer as f32,
            None,
        );
        measure_white(&white_grid).map(|white| (white[0], white[1], white[2], white_space.clone()))
    } else {
        None
    };
    let density = fade_density(
        &with_fade_white(config, measured_white.clone()),
        &white_space,
    );
    let stage = density_stage(
        config,
        settings.cam_xyz,
        settings.camera_wb,
        density.matrix,
        density.anchor,
    );
    let linear = apply_camera_matrix(&image.pixels, stage.pre);
    let log = to_log_density_image(&linear);
    let (log_grid, grid_w, grid_h) = prefilter_log_grid(&linear, width, height, roi, buffer as f32);
    let faded = unmix_stage(&log_grid, grid_w, grid_h, &stage);
    let (table, _measured) = resolve_crossover(
        config,
        &crossover_space(&density, &white_space, config),
        Some(&faded),
        true,
    );
    let offsets = crossover_offsets(table.as_deref(), config.fade_crossover);
    let crossed = relog_matrix_image(
        &apply_crossover(&unmix_stage(&log, width, height, &stage), offsets.as_ref()),
        stage.post,
    );
    let bounds = transfer_bounds(TRANSFER_DENSITY_RANGE);
    let (white, black) = transfer_point_offsets(config);
    let bounds = if white.iter().chain(black.iter()).any(|value| *value != 0.0) {
        LogNegativeBounds::new(
            std::array::from_fn(|ch| bounds.floors[ch] + white[ch]),
            std::array::from_fn(|ch| bounds.ceils[ch] + black[ch]),
        )
    } else {
        bounds
    };
    let mut normalized = vec![0.0; crossed.pixels.len()];
    negpy_core::analysis::normalize_log_image(
        &crossed.pixels,
        bounds.floors.map(|value| value as f32),
        bounds.ceils.map(|value| value as f32),
        &mut normalized,
    );
    let pre_trim = transfer_bounds(TRANSFER_DENSITY_RANGE);
    let prefiltered = relog_matrix_image(&apply_crossover(&faded, offsets.as_ref()), stage.post);
    let positive = render_path(config) == RenderPath::Positive;
    let neutral_axis = if settings.cast_strength > 0.0 {
        measure_transfer_neutral_axis(
            &prefiltered.pixels,
            prefiltered.width,
            prefiltered.height,
            positive,
        )
        .map(
            |axis| crate::features::exposure::processor::NeutralAxisRefs {
                mid: axis.mid,
                shadow: axis.shadow,
                highlight: axis.highlight,
                confidence: axis.confidence,
            },
        )
    } else {
        None
    };
    let metrics = TransferMetrics {
        bounds,
        norm_density_range: luminance_density_range(bounds),
        metered_anchor: measure_anchor(
            &prefiltered.pixels,
            prefiltered.width,
            prefiltered.height,
            pre_trim,
        ),
        textural_range: measure_textural_range(
            &prefiltered.pixels,
            prefiltered.width,
            prefiltered.height,
        ),
        shadow_point: measure_shadow_point(
            &prefiltered.pixels,
            prefiltered.width,
            prefiltered.height,
            pre_trim,
        ),
        highlight_point: measure_highlight_point(
            &prefiltered.pixels,
            prefiltered.width,
            prefiltered.height,
            pre_trim,
        ),
        neutral_axis,
        fade_white: measured_white.as_ref().map(|(r, g, b, _)| [*r, *g, *b]),
        fade_crossover: table,
        scan_clip_fractions: measure_clip_fractions(
            image.pixels.as_slice(),
            width,
            height,
            roi,
            buffer,
        ),
    };
    TransferBaseResult {
        image: ImageBuffer::from_rgb(width, height, normalized),
        metrics,
        measured_white,
    }
}

/// The unmix a `DensityStage` applies, with its offset folded in.
fn unmix_stage(grid: &[f32], width: usize, height: usize, stage: &DensityStage) -> ImageBuffer {
    let Some(matrix) = stage.unmix else {
        return ImageBuffer::from_rgb(width, height, grid.to_vec());
    };
    let linear: [[f32; 3]; 3] = std::array::from_fn(|row| {
        [
            matrix[row][0] as f32,
            matrix[row][1] as f32,
            matrix[row][2] as f32,
        ]
    });
    let mut out = vec![0.0; grid.len()];
    if let Some(anchor) = stage.anchor {
        let offset: [f32; 3] = std::array::from_fn(|row| matrix[row][3] as f32);
        negpy_core::analysis::unmix_log_anchored(
            grid,
            linear,
            anchor.map(|value| value as f32),
            offset,
            &mut out,
        );
    } else {
        negpy_core::analysis::unmix_log(grid, linear, &mut out);
        let offset: [f32; 3] = std::array::from_fn(|row| matrix[row][3] as f32);
        for pixel in out.chunks_exact_mut(3) {
            for channel in 0..3 {
                pixel[channel] += offset[channel];
            }
        }
    }
    ImageBuffer::from_rgb(width, height, out)
}
