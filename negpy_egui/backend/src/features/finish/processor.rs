//! The finish stage in `negpy/features/finish/processor.py` order.

use crate::domain::models::FinishConfig;
use crate::features::finish::logic::{apply_carrier, apply_vignette};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// `carrier_width_px(carrier_width_mm, print_size_cm, long_edge_px)`.
#[must_use]
pub fn carrier_width_px(carrier_width_mm: f64, print_size_cm: f64, long_edge_px: f64) -> f64 {
    (carrier_width_mm / (print_size_cm * 10.0).max(1.0)) * long_edge_px
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
