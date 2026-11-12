//! The exposure half of the render pipeline: normalize the negative, then print it.
//!
//! `negpy/services/rendering/engine.py` wires the later stages around these two. This
//! entry point owns their order and the metrics the GUI reads back.

use crate::domain::models::{ProcessConfig, WorkspaceConfig};
use crate::features::exposure::metering::{self, Roi};
use crate::features::exposure::print::{PrintRenderSettings, render_negative};
use crate::features::exposure::processor::{NormalizationSettings, normalize_negative};
use crate::features::finish::processor::{FinishContext, process as finish_process};
use crate::features::geometry::processor::{
    GeometryContext, crop_image, process as geometry_process,
};
use crate::features::lab::logic::apply_clahe;
use crate::features::lab::processor::{LabContext, process as lab_process};
use crate::features::toning::processor::process as toning_process;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// One frame's pixels and size.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub pixels: Vec<f32>,
    pub width: usize,
    pub height: usize,
}

/// The per-render context the Python engine reads from `PipelineContext`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExposureContext {
    pub active_roi: Option<Roi>,
    pub base_log: Option<[f64; 3]>,
}

/// The result of the exposure stages.
#[derive(Clone, Debug, PartialEq)]
pub struct ExposureResult {
    pub image: Frame,
    pub metrics: ExposureMetrics,
}

/// The metrics the normalizer measures and the print stage reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ExposureMetrics {
    pub anchor: f64,
    pub density_range: f64,
    pub textural_range: f64,
    pub shadow_point: f64,
    pub highlight_point: f64,
}

/// The print stage's black-hold taper from `black_hold_taper`.
#[must_use]
pub fn black_hold_taper(config: &crate::domain::models::ExposureConfig) -> f64 {
    if config.hold_black {
        crate::features::exposure::constants::ZONE_BLACK_HOLD_TAPER
    } else {
        0.0
    }
}

/// Run the normalization and print stages over one linear frame.
#[must_use]
pub fn render_exposure(
    frame: &Frame,
    config: &WorkspaceConfig,
    context: ExposureContext,
) -> ExposureResult {
    let normalized = normalize_negative(
        &frame.pixels,
        frame.width,
        frame.height,
        NormalizationSettings {
            config: &config.process,
            mode: config.process.process_mode,
            active_roi: context.active_roi,
            base_log: context.base_log,
        },
    );
    let metrics = &normalized.metrics;
    let anchor = config
        .exposure
        .auto_exposure
        .then_some(metrics.metered_anchor);
    let print = PrintRenderSettings {
        config: &config.exposure,
        process: &config.process,
        density_range: Some(metrics.norm_density_range),
        textural_range: Some(metrics.textural_range),
        shadow_point: Some(metrics.shadow_point),
        highlight_point: Some(metrics.highlight_point),
        anchor,
        bounds: Some(metrics.final_bounds),
        neutral_axis: metrics.neutral_axis,
        mode: config.process.process_mode,
        base_black_hold: black_hold_taper(&config.exposure),
    };
    let pixels = render_negative(&normalized.image, frame.width, &print);
    ExposureResult {
        image: Frame {
            pixels,
            width: frame.width,
            height: frame.height,
        },
        metrics: ExposureMetrics {
            anchor: metrics.metered_anchor,
            density_range: metrics.norm_density_range,
            textural_range: metrics.textural_range,
            shadow_point: metrics.shadow_point,
            highlight_point: metrics.highlight_point,
        },
    }
}

/// The exposure preview size: the full frame, or a long-edge budget while a slider moves.
#[must_use]
pub fn preview_frame(frame: &Frame, long_edge: u32) -> Frame {
    let current = frame.width.max(frame.height) as u32;
    if current <= long_edge {
        return frame.clone();
    }
    let scale = long_edge as f32 / current as f32;
    let width = ((frame.width as f32 * scale).round() as usize).max(1);
    let height = ((frame.height as f32 * scale).round() as usize).max(1);
    let mut pixels = vec![0.0; width * height * 3];
    for y in 0..height {
        let sy = (y * frame.height / height).min(frame.height - 1);
        for x in 0..width {
            let sx = (x * frame.width / width).min(frame.width - 1);
            let src = (sy * frame.width + sx) * 3;
            let dst = (y * width + x) * 3;
            pixels[dst..dst + 3].copy_from_slice(&frame.pixels[src..src + 3]);
        }
    }
    Frame {
        pixels,
        width,
        height,
    }
}

/// The render metrics as the event contract's key/value list.
#[must_use]
pub fn metrics_for_event(metrics: &ExposureMetrics) -> crate::event::RenderMetrics {
    crate::event::RenderMetrics(vec![
        ("metered_anchor".to_string(), metrics.anchor),
        ("norm_density_range".to_string(), metrics.density_range),
        ("textural_range".to_string(), metrics.textural_range),
        ("shadow_point".to_string(), metrics.shadow_point),
        ("highlight_point".to_string(), metrics.highlight_point),
    ])
}

/// The analysis region a frame reads with, for callers that build a context.
#[must_use]
pub fn analysis_roi(frame: &Frame, config: &ProcessConfig, active_roi: Option<Roi>) -> Option<Roi> {
    let rect = |values: Option<&Vec<f64>>| -> Option<[f64; 4]> {
        let values = values?;
        (values.len() >= 4).then(|| [values[0], values[1], values[2], values[3]])
    };
    metering::resolve_analysis_region(
        frame.width,
        frame.height,
        active_roi,
        config.analysis_buffer,
        rect(config.analysis_rect.as_ref()),
        rect(config.meter_rect.as_ref()),
    )
    .0
}

/// Whether a frame renders in black and white: a B&W negative, or a Slide with B&W on.
#[must_use]
pub fn is_mono(config: &WorkspaceConfig) -> bool {
    use crate::domain::models::ProcessMode;
    config.process.process_mode == ProcessMode::BW
        || (config.process.process_mode == ProcessMode::E6 && config.process.slide_bw)
}

/// The D8 tail stages: CLAHE, Lab, toning, crop and finish, in engine order.
#[must_use]
pub fn render_tail(
    frame: &Frame,
    config: &WorkspaceConfig,
    geometry: GeometryContext,
    preview_size: f64,
) -> Frame {
    let mono = is_mono(config);
    let mut image = ImageBuffer::from_rgb(frame.width, frame.height, frame.pixels.clone());
    if config.exposure.render_intent != crate::domain::models::RenderIntent::Flat {
        image = apply_clahe(&image, config.lab.clahe_strength);
        let lab = lab_process(&image, &config.lab, mono, LabContext { scale_factor: 1.0 });
        image = lab.image;
        image = toning_process(&image, &config.toning, config.altproc.alt_process, mono);
    }
    let geometry = geometry_process(&image, &config.geometry, geometry);
    let cropped = crop_image(&geometry.image, geometry.active_roi);
    let finished = if config.exposure.render_intent == crate::domain::models::RenderIntent::Flat {
        cropped
    } else {
        finish_process(
            &cropped,
            &config.finish,
            FinishContext {
                print_size_cm: 30.0,
                paper: [1.0; 3],
                crop_preview_full: false,
            },
            None,
        )
    };
    let _ = preview_size;
    Frame {
        pixels: finished.pixels,
        width: finished.width,
        height: finished.height,
    }
}
