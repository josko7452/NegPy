//! Ports the small Lab and retouch helper cases added after the D8 tail.

use negpy_backend::domain::models::RetouchConfig;
use negpy_backend::features::lab::logic::{
    SHARPEN_SHADOW_FLOOR, apply_resize_sharpening, sharpen_shadow_gain,
};
use negpy_backend::features::retouch::clone::clone_token;
use negpy_backend::features::retouch::logic::{
    exclusion_token, hair_bake_token, manual_bake_token, split_hairs,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use serde_json::json;

#[test]
fn shadow_gain_is_the_floor_at_black_and_one_at_the_hi_point() {
    let gains = sharpen_shadow_gain(&[0.0, 17.5, 35.0, 70.0]);
    assert!((gains[0] - SHARPEN_SHADOW_FLOOR).abs() < 1e-6);
    assert!((gains[1] - 2.0 / 3.0).abs() < 1e-6);
    assert!((gains[2] - 1.0).abs() < 1e-6);
    assert!((gains[3] - 1.0).abs() < 1e-6);
}

#[test]
fn zero_sharpen_leaves_the_encoded_image_unchanged() {
    let img = ImageBuffer::from_rgb(2, 1, vec![0.2, 0.4, 0.6, 0.1, 0.3, 0.5]);
    let mut config = negpy_backend::domain::models::LabConfig::default();
    config.sharpen = 0.0;
    assert_eq!(apply_resize_sharpening(&img, &config), img);
}

#[test]
fn a_compact_mask_has_no_hairs() {
    let mut mask = vec![0u8; 80 * 80];
    for y in 20..60 {
        for x in 20..60 {
            mask[y * 80 + x] = 1;
        }
    }
    let (compact, hairs) = split_hairs(&mask, 80, 80);
    assert_eq!(compact, mask);
    assert!(hairs.is_none());
}

#[test]
fn a_one_pixel_snake_routes_to_hairs() {
    let mut mask = vec![0u8; 80 * 80];
    for x in 5..70 {
        mask[40 * 80 + x] = 1;
    }
    let (compact, hairs) = split_hairs(&mask, 80, 80);
    assert!(compact.iter().all(|value| *value == 0));
    assert!(hairs.is_some_and(|hairs| hairs.iter().any(|value| *value != 0)));
}

#[test]
fn no_retouch_strokes_means_no_tokens() {
    let config = RetouchConfig::default();
    assert_eq!(exclusion_token(&config), "");
    assert_eq!(manual_bake_token(&config), "");
    assert_eq!(clone_token(&config), "");
}

#[test]
fn exclusion_changes_the_hair_token() {
    let base = RetouchConfig::default();
    let mut excluded = base.clone();
    excluded.dust_exclusion_strokes = vec![json!([[[0.1, 0.2], [0.3, 0.4]], 0.5])];
    assert!(!exclusion_token(&excluded).is_empty());
    assert_ne!(hair_bake_token(&base), hair_bake_token(&excluded));
}

#[test]
fn manual_and_clone_tokens_track_their_own_strokes() {
    let base = RetouchConfig::default();
    let mut healed = base.clone();
    healed.manual_heal_strokes = vec![json!([[[0.1, 0.2]], 12])];
    let mut cloned = base.clone();
    cloned.clone_strokes = vec![json!([[[0.1, 0.2]], 12, 0.3, 0.4, 1.0, 0.5, true])];
    assert!(!manual_bake_token(&healed).is_empty());
    assert_eq!(clone_token(&healed), "");
    assert!(!clone_token(&cloned).is_empty());
    assert_eq!(manual_bake_token(&cloned), "");
}

#[test]
fn scratch_lines_enter_the_manual_token() {
    let base = RetouchConfig::default();
    let mut lined = base.clone();
    lined.scratch_lines = vec![json!([[0.1, 0.2], [0.3, 0.4]])];
    assert!(!manual_bake_token(&lined).is_empty());
    assert_ne!(manual_bake_token(&base), manual_bake_token(&lined));
}
