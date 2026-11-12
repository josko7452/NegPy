//! Detection entry points that the source baker and the GUI read.

use crate::domain::models::RetouchConfig;
use crate::features::retouch::logic::{detect_luma_score, ir_bake_token, luma_bake_token};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The speck score and hair mask for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct LumaDetection {
    pub score: Option<Vec<f32>>,
    pub hairs: Option<Vec<u8>>,
}

/// Run luma dust detection when the toggle is on.
#[must_use]
pub fn detect_dust(img: &ImageBuffer, config: &RetouchConfig) -> Option<LumaDetection> {
    if !config.dust_remove {
        return None;
    }
    detect_luma_score(
        img,
        config.dust_threshold,
        config.dust_size,
        None,
        Some(config.dust_hair_threshold),
    )
    .map(|(score, hairs)| LumaDetection { score, hairs })
}

/// The source-hash tokens the retouch bakes contribute.
#[must_use]
pub fn source_tokens(config: &RetouchConfig, has_ir: bool) -> String {
    format!(
        "{}{}",
        luma_bake_token(config),
        ir_bake_token(config, has_ir)
    )
}
