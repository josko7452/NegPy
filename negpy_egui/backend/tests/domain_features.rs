//! Defaults and coercion for the editing feature configs, checked against the Python
//! dataclasses in negpy/features/*/models.py.

use negpy_backend::domain::models::altprocess::{AltProcess, AltProcessConfig, Sensitizer};
use negpy_backend::domain::models::finish::FinishConfig;
use negpy_backend::domain::models::flatfield::FlatFieldConfig;
use negpy_backend::domain::models::geometry::{AspectRatio, AutocropMode, GeometryConfig};
use negpy_backend::domain::models::lab::{LabConfig, SharpenMethod};
use negpy_backend::domain::models::local::{LocalAdjustmentsConfig, LocalMask, MaskKey, MaskShape};
use negpy_backend::domain::models::retouch::{
    HEAL_SIZE_MAX, HEAL_SIZE_MIN, HEAL_SIZE_REF, RetouchConfig,
};
use negpy_backend::domain::models::toning::ToningConfig;

#[test]
fn the_geometry_defaults_and_coercion_match() {
    let config = GeometryConfig::default();
    assert_eq!(config.rotation, 0);
    assert_eq!(config.autocrop_ratio, AspectRatio::Free);
    assert_eq!(config.autocrop_mode, AutocropMode::Image);
    assert_eq!(config.autocrop_rebate_trim, 1.0);
    assert_eq!(config.crop_rect, None);
    // A retired ratio degrades to Free, not a load failure.
    assert_eq!(AspectRatio::coerce("2:3"), AspectRatio::R2x3);
    assert_eq!(AspectRatio::coerce("not a ratio"), AspectRatio::Free);
    // Lens metadata on clears the manual coefficient.
    let config = GeometryConfig {
        lens_distortion_from_metadata: true,
        distortion_k1: 0.1,
        ..GeometryConfig::default()
    }
    .normalize();
    assert_eq!(config.distortion_k1, 0.0);
}

#[test]
fn the_lab_defaults_match() {
    let config = LabConfig::default();
    assert_eq!(config.saturation, 1.0);
    assert_eq!(config.skin_protection, 0.5);
    assert_eq!(config.sharpen, 0.25);
    assert_eq!(config.sharpen_method, SharpenMethod::Usm);
    assert_eq!(config.sharpen_masking, 0.1);
    assert_eq!(config.chroma_denoise, 2.5);
}

#[test]
fn the_local_defaults_match() {
    let config = LocalAdjustmentsConfig::default();
    assert!(config.masks.is_empty());
    let mask = LocalMask::default();
    assert_eq!(mask.stops, 0.0);
    assert_eq!(mask.feather, 0.04);
    assert_eq!(mask.shape, MaskShape::Polygon);
    assert!(mask.enabled);
    assert_eq!(mask.key, MaskKey::Off);
    assert_eq!(mask.key_zone, 6.0);
    assert_eq!(mask.key_softness, 1.0);
}

#[test]
fn a_local_mask_round_trips_through_the_flat_namespace() {
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
        r#"{"masks":[{"vertices":[[0.1,0.1],[0.5,0.5]],"stops":-0.7,"feather":0.05}]}"#,
    )
    .unwrap();
    let config: LocalAdjustmentsConfig =
        serde_json::from_value(serde_json::Value::Object(map)).unwrap();
    assert_eq!(config.masks.len(), 1);
    assert_eq!(config.masks[0].stops, -0.7);
    assert_eq!(config.masks[0].vertices, vec![[0.1, 0.1], [0.5, 0.5]]);
}

#[test]
fn the_retouch_defaults_and_brush_bounds_match() {
    let config = RetouchConfig::default();
    assert_eq!(config.dust_threshold, 0.66);
    assert_eq!(config.dust_size, 4);
    assert_eq!(config.scratch_threshold, 0.5);
    assert_eq!(config.clone_strength, 1.0);
    assert_eq!(config.clone_feather, 0.5);
    assert!(config.clone_match_tone);
    assert_eq!(config.ir_threshold, 0.66);
    assert!(config.ir_attenuation);
    assert_eq!(HEAL_SIZE_REF, 1600.0);
    assert_eq!(HEAL_SIZE_MIN, 2.0);
    assert_eq!(HEAL_SIZE_MAX, 64.0);
}

#[test]
fn the_altprocess_defaults_and_coercion_match() {
    let config = AltProcessConfig::default();
    assert_eq!(config.alt_process, AltProcess::None);
    assert_eq!(config.lith_exposure, 2.0);
    assert_eq!(config.lith_snatch, 0.55);
    assert_eq!(config.cyano_sensitizer, Sensitizer::Classic);
    assert_eq!(config.cyano_scale, 1.4);
    assert_eq!(config.sabattier_strength, 1.3);
    assert_eq!(AltProcess::coerce("cyanotype"), AltProcess::Cyanotype);
    assert_eq!(AltProcess::coerce("stale"), AltProcess::None);
}

#[test]
fn the_toning_and_finish_defaults_match() {
    let toning = ToningConfig::default();
    assert_eq!(toning.selenium_strength, 0.0);
    assert_eq!(toning.shadow_tint_hue, 0.0);
    let finish = FinishConfig::default();
    assert_eq!(finish.vignette_size, 0.5);
    assert_eq!(finish.carrier_rough, 0.25);
    assert_eq!(finish.carrier_flare, 0.5);
    assert_eq!(finish.border_color, "#ffffff");
    assert_eq!(finish.border_bottom_weight, 1.0);
}

#[test]
fn the_flatfield_defaults_match() {
    let config = FlatFieldConfig::default();
    assert!(!config.apply);
    assert_eq!(config.profile_id, "");
}
