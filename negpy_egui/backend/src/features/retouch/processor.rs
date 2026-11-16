//! Detection entry points that the source baker and the GUI read.

use crate::domain::models::RetouchConfig;
use crate::features::retouch::logic::{
    detect_luma_score, downsample_ir, ir_bake_token, ir_detect_target, luma_bake_token,
};
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

/// `_downsample_to_long_edge`: shrink so the long edge is at most `long_px`; never upscales.
#[must_use]
pub fn downsample_to_long_edge(image: &ImageBuffer, long_px: usize) -> ImageBuffer {
    let long_edge = image.height.max(image.width);
    if long_px == 0 || long_edge <= long_px {
        return image.clone();
    }
    let scale = long_px as f64 / long_edge as f64;
    let width = ((image.width as f64 * scale).round() as usize).max(1);
    let height = ((image.height as f64 * scale).round() as usize).max(1);
    let pixels =
        negpy_core::resize::resize_area(&image.pixels, image.height, image.width, 3, height, width)
            .expect("a positive source and destination size");
    ImageBuffer::from_rgb(width, height, pixels)
}

/// `_detection_downsample`: the plane optical dust detection reads.
///
/// It min-pools like the IR plane, since a speck is a minimum in transmittance that area
/// averaging dilutes below the grain, and lands at the resolution the IR path detects at
/// for a buffer this size (`ir_detect_target`).
#[must_use]
pub fn detection_downsample(image: &ImageBuffer, preview_long_edge: usize) -> Vec<f32> {
    let mut gray = vec![0.0f32; image.width * image.height];
    negpy_core::analysis::luminance(&image.pixels, &mut gray);
    let target = ir_detect_target(image.height.max(image.width), preview_long_edge);
    downsample_ir(&gray, image.height, image.width, target, None)
}

/// `_resize_mask`: a boolean mask at another plane's shape, nearest-neighbour.
#[must_use]
pub fn resize_mask(
    mask: &[bool],
    height: usize,
    width: usize,
    target: (usize, usize),
) -> Vec<bool> {
    let (target_height, target_width) = target;
    if (height, width) == target {
        return mask.to_vec();
    }
    let source: Vec<u8> = mask.iter().map(|value| u8::from(*value)).collect();
    let mut out = vec![0u8; target_height * target_width];
    negpy_core::resize::resize_nearest(
        &source,
        (height, width, 1),
        (target_height, target_width),
        &mut out,
    );
    out.into_iter().map(|value| value > 0).collect()
}

/// `_without_ir`: optical detections with the IR-corrected pixels released to clean.
#[must_use]
pub fn without_ir(
    score: Option<&[f32]>,
    hairs: &[Option<Vec<u8>>],
    mask_height: usize,
    mask_width: usize,
    ir_mask: &[bool],
    target: (usize, usize),
) -> (Option<Vec<f32>>, Vec<Option<Vec<u8>>>) {
    let covered = resize_mask(ir_mask, mask_height, mask_width, target);
    let score = score.map(|score| {
        score
            .iter()
            .zip(&covered)
            .map(|(value, hit)| if *hit { 1.0 } else { *value })
            .collect::<Vec<f32>>()
    });
    let score = score.filter(|values| values.iter().any(|value| *value < 1.0));
    let mut released = Vec::new();
    for mask in hairs.iter() {
        let some = mask.as_ref();
        let kept = {
            let Some(mask) = some else {
                continue;
            };
            let kept: Vec<u8> = mask
                .iter()
                .zip(&covered)
                .map(|(value, hit)| if *hit { 0 } else { *value })
                .collect();
            kept
        };
        if kept.iter().any(|value| *value != 0) {
            released.push(Some(kept));
        }
    }
    (score, released)
}
