//! Applies one resolved roll crop to a frame's stored config.

use negpy_backend::domain::models::{AutocropMode, WorkspaceConfig};
use negpy_backend::features::geometry::batch_autocrop::{ResolvedCrop, apply_resolved_crop};
use negpy_backend::features::geometry::logic::autocrop_detection_key;

fn crop(rect: [f64; 4], angle: f64) -> ResolvedCrop {
    ResolvedCrop {
        key: "0:hash".to_string(),
        crop_rect: rect,
        correction_angle: angle,
        confidence: 0.9,
        calibrated: true,
    }
}

#[test]
fn a_resolved_crop_keeps_auto_and_clears_the_measured_bounds() {
    let mut config = WorkspaceConfig::default();
    config.geometry.autocrop_mode = AutocropMode::Image;
    config.geometry.fine_rotation = 0.5;
    config.process.local_floors = [0.1, 0.2, 0.3];
    let updated = apply_resolved_crop(&config, &crop([0.1, 0.2, 0.9, 0.8], 1.0)).unwrap();
    assert!(updated.geometry.crop_from_auto);
    assert_eq!(updated.geometry.crop_rect, Some(vec![0.1, 0.2, 0.9, 0.8]));
    assert!((updated.geometry.fine_rotation - 1.5).abs() < 1e-9);
    assert_eq!(
        updated.geometry.crop_detect_key,
        autocrop_detection_key(&updated.geometry)
    );
    assert_eq!(updated.process.local_floors, [0.0; 3]);
}

#[test]
fn a_hand_drawn_crop_survives_a_batch_resolve() {
    let mut config = WorkspaceConfig::default();
    config.geometry.crop_rect = Some(vec![0.2, 0.2, 0.8, 0.8]);
    config.geometry.crop_from_auto = false;
    assert!(apply_resolved_crop(&config, &crop([0.1, 0.1, 0.9, 0.9], 0.0)).is_none());
}

#[test]
fn an_unordered_or_out_of_range_crop_is_refused() {
    let config = WorkspaceConfig::default();
    assert!(apply_resolved_crop(&config, &crop([0.9, 0.2, 0.1, 0.8], 0.0)).is_none());
    assert!(apply_resolved_crop(&config, &crop([0.1, 0.2, 1.1, 0.8], 0.0)).is_none());
}
