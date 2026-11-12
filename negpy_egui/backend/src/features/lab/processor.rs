//! The Lab stage in `negpy/features/lab/processor.py` order.

use crate::domain::models::LabConfig;
use crate::features::lab::logic::{
    apply_chroma_denoise, apply_glow_and_halation, apply_lab_sharpening, apply_saturation,
    auto_masking, chroma_denoise_radius, clip_frame,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The per-frame values the lab stage reads and writes.
#[derive(Clone, Copy, Debug, Default)]
pub struct LabContext {
    pub scale_factor: f64,
}

/// The lab stage's output and the auto-masking value it measured.
#[derive(Clone, Debug, PartialEq)]
pub struct LabResult {
    pub image: ImageBuffer,
    pub auto_masking: Option<f32>,
}

/// Run chroma denoise, saturation, sharpening and glow in the Python order.
#[must_use]
pub fn process(
    img: &ImageBuffer,
    config: &LabConfig,
    mono: bool,
    context: LabContext,
) -> LabResult {
    let mut image = img.clone();
    let radius = chroma_denoise_radius(config.chroma_denoise, mono);
    if radius > 0.0 {
        image = apply_chroma_denoise(&image, radius, context.scale_factor);
    }
    if config.saturation != 1.0 || config.skin_protection > 0.0 {
        image = apply_saturation(&image, config.saturation, config.skin_protection);
    }
    let mut auto = None;
    if config.sharpen > 0.0 {
        auto = auto_masking(&image, config);
        image = apply_lab_sharpening(&image, config, auto);
    }
    if config.glow_amount > 0.0 || config.halation_strength > 0.0 {
        image = apply_glow_and_halation(&image, config.glow_amount, config.halation_strength, mono);
    }
    LabResult {
        image: clip_frame(image),
        auto_masking: auto,
    }
}
