//! Mirrors the cache-key and positive-preview half of `negpy/services/assets/thumbnails.py`.

use serde_json::Value;

use crate::domain::models::ProcessMode;
use crate::features::process::detect::detect_process_mode;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The version that retires thumbnails written by an earlier pipeline.
pub const THUMBNAIL_CACHE_VERSION: &str = "v3";

/// The one key an asset's thumbnail lives under, in memory and on disk.
#[must_use]
pub fn asset_thumbnail_key(asset: &Value) -> String {
    let file_hash = asset
        .get("hash")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let triplet = ["green_path", "blue_path"]
        .iter()
        .all(|key| asset.get(*key).and_then(Value::as_str).is_some_and(|p| !p.is_empty()));
    thumbnail_cache_key(file_hash, triplet)
}

/// The disk cache key for a frame's thumbnail. A triplet takes its own key, so a red-only
/// thumbnail cannot shadow the merged one, and the batch and rendered paths agree.
#[must_use]
pub fn thumbnail_cache_key(file_hash: &str, is_triplet: bool) -> String {
    let base = if is_triplet {
        format!("{file_hash}-rgb")
    } else {
        file_hash.to_string()
    };
    format!("{base}-{THUMBNAIL_CACHE_VERSION}")
}

/// Invert a negative preview so the filmstrip reads as photographs before a frame has
/// ever been opened. The frame's stored film process decides; the heuristic runs only
/// when nothing has decided yet.
#[must_use]
pub fn preview_positive(img: &ImageBuffer, process_mode: Option<ProcessMode>) -> ImageBuffer {
    let mode = process_mode.unwrap_or_else(|| detect_process_mode(&img.pixels, img.width, img.height));
    if mode == ProcessMode::E6 {
        return img.clone();
    }
    let mut bits = vec![0u8; img.width * img.height * 3];
    negpy_core::capture::log_stretch_u8(&img.pixels, &mut bits);
    let mut values = vec![0.0f32; bits.len()];
    negpy_core::convert::uint8_to_float32(&bits, &mut values);
    ImageBuffer::from_rgb(img.width, img.height, values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_triplet_takes_its_own_thumbnail_key() {
        assert_eq!(thumbnail_cache_key("abc", false), "abc-v3");
        assert_eq!(thumbnail_cache_key("abc", true), "abc-rgb-v3");
    }

    #[test]
    fn the_asset_key_reads_the_hash_and_the_parts() {
        let asset = serde_json::json!({"hash": "abc", "green_path": "/g.dng", "blue_path": "/b.dng"});
        assert_eq!(asset_thumbnail_key(&asset), "abc-rgb-v3");
        let asset = serde_json::json!({"hash": "abc"});
        assert_eq!(asset_thumbnail_key(&asset), "abc-v3");
    }

    #[test]
    fn a_slide_preview_is_left_alone() {
        let img = ImageBuffer::from_rgb(2, 2, vec![0.5; 12]);
        assert_eq!(preview_positive(&img, Some(ProcessMode::E6)), img);
    }
}
