//! The toning stage in `negpy/features/toning/processor.py` order.

use crate::domain::models::{AltProcess, ToningConfig};
use crate::features::toning::logic::{apply_chemical_toning, apply_split_toning};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// Chemical baths on silver, then the split tint on every print.
#[must_use]
pub fn process(
    img: &ImageBuffer,
    config: &ToningConfig,
    alt_process: AltProcess,
    mono: bool,
) -> ImageBuffer {
    let mut image = img.clone();
    // A cyanotype holds no silver, so no bath can react; split tinting is a dye stain.
    if mono && alt_process != AltProcess::Cyanotype {
        image = apply_chemical_toning(
            &image,
            config.selenium_strength,
            config.sepia_strength,
            config.gold_strength,
            config.blue_strength,
            config.copper_strength,
            config.vanadium_strength,
            alt_process == AltProcess::Lith,
        );
    }
    apply_split_toning(
        &image,
        config.shadow_tint_hue,
        config.shadow_tint_strength,
        config.highlight_tint_hue,
        config.highlight_tint_strength,
    )
}
