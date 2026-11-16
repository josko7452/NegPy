//! The finish stage in `negpy/features/finish/processor.py` order.

use crate::domain::models::{AltProcess, FinishConfig, ProcessMode, RenderIntent, WorkspaceConfig};
use crate::features::exposure::bw_filter::printed_exposure;
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::print::{PrintRenderSettings, render_negative};
use crate::features::finish::logic::{apply_carrier, apply_vignette, carrier_tone_exposures};
use crate::features::lab::logic::apply_saturation;
use crate::features::process::path::is_mono;
use crate::features::toning::processor::process as toning_process;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// log10 by which the film base is clearer than the normalization's thin bound.
pub const REBATE_BASE_MARGIN: f64 = 0.15;

/// `carrier_width_px(carrier_width_mm, print_size_cm, long_edge_px)`.
#[must_use]
pub fn carrier_width_px(carrier_width_mm: f64, print_size_cm: f64, long_edge_px: f64) -> f64 {
    (carrier_width_mm / (print_size_cm * 10.0).max(1.0)) * long_edge_px
}

/// The scale bounds the rebate reads a paper-relative tone against.
#[derive(Clone, Copy, Debug, Default)]
pub struct RebateMetrics {
    pub final_bounds: Option<LogNegativeBounds>,
}

/// `rebate_tone`: the paper-relative print color of a neutral density ramp off the film
/// base, through this frame's curves, saturation and toning.
///
/// Plain light where there is no negative print model.
#[must_use]
pub fn rebate_tone(
    settings: &WorkspaceConfig,
    metrics: RebateMetrics,
) -> [[f32; 3]; crate::features::finish::logic::CARRIER_TONE_SAMPLES] {
    let plain = crate::features::finish::logic::linear_carrier_tone();
    let mode = settings.process.process_mode;
    let Some(bounds) = metrics.final_bounds else {
        return plain;
    };
    if (mode != ProcessMode::C41 && mode != ProcessMode::BW)
        || settings.process.positive_source
        || settings.exposure.render_intent == RenderIntent::Flat
        || !matches!(
            settings.altproc.alt_process,
            AltProcess::None | AltProcess::Sabattier
        )
    {
        return plain;
    }
    let exposures = carrier_tone_exposures();
    let mut strip = vec![0.0f32; exposures.len() * 3];
    for (index, exposure) in exposures.iter().enumerate() {
        let t = (*exposure).max(1e-6) as f64;
        let offset = (REBATE_BASE_MARGIN + negpy_core::mathf::log10_f64(t)) as f32;
        for channel in 0..3 {
            let span = (bounds.ceils[channel] - bounds.floors[channel]).max(1e-6) as f32;
            strip[index * 3 + channel] = 1.0 + offset / span;
        }
    }
    let mut exposure = printed_exposure(&settings.process, &settings.exposure);
    exposure.contrast_mask_strength = 0.0;
    exposure.diffusion = 0.0;
    let print = PrintRenderSettings {
        config: &exposure,
        process: &settings.process,
        density_range: None,
        textural_range: None,
        shadow_point: None,
        highlight_point: None,
        anchor: None,
        bounds: metrics.final_bounds,
        neutral_axis: None,
        mode: settings.process.process_mode,
        base_black_hold: 0.0,
        mask: None,
        diffusion: None,
    };
    let printed = render_negative(&strip, 1, &print);
    let mut image = ImageBuffer::from_rgb(1, exposures.len(), printed);
    if settings.lab.saturation != 1.0 || settings.lab.skin_protection > 0.0 {
        image = apply_saturation(
            &image,
            settings.lab.saturation,
            settings.lab.skin_protection,
        );
    }
    image = toning_process(
        &image,
        &settings.toning,
        settings.altproc.alt_process,
        is_mono(&settings.process),
    );
    let first = [
        image.pixels[0].max(1e-6),
        image.pixels[1].max(1e-6),
        image.pixels[2].max(1e-6),
    ];
    let mut out = [[0.0f32; 3]; crate::features::finish::logic::CARRIER_TONE_SAMPLES];
    for (index, row) in out.iter_mut().enumerate() {
        for channel in 0..3 {
            row[channel] = image.pixels[index * 3 + channel] / first[channel];
        }
    }
    out
}

/// The frame values the finishing stage reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct FinishContext {
    pub print_size_cm: f64,
    pub paper: [f32; 3],
    pub crop_preview_full: bool,
}

/// Vignette, then the carrier on a full crop preview.
#[must_use]
pub fn process(
    img: &ImageBuffer,
    config: &FinishConfig,
    context: FinishContext,
    tone: Option<&[[f32; 3]]>,
) -> ImageBuffer {
    let mut image = img.clone();
    if config.vignette_stops != 0.0 {
        image = apply_vignette(
            &image,
            config.vignette_stops,
            config.vignette_size,
            config.vignette_roundness,
        );
    }
    if config.carrier_width > 0.0 && !context.crop_preview_full {
        let long_edge = image.width.max(image.height) as f64;
        let width = carrier_width_px(config.carrier_width, context.print_size_cm, long_edge);
        image = apply_carrier(
            &image,
            width,
            config.carrier_rough,
            config.carrier_flare,
            config.carrier_corner,
            context.paper,
            tone,
        );
    }
    image
}
