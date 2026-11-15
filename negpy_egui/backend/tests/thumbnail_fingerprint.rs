//! Pins the fingerprint against `negpy/services/assets/thumbnail_fingerprint.py`.
//!
//! The stored comment is a cache key, so a fingerprint that disagrees with Python would
//! re-render every frame once.

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::services::assets::thumbnail_fingerprint::{
    QUICK, decode_comment, encode_comment, is_current, thumbnail_fingerprint,
};

const WORKSPACE: &str = "Adobe RGB";

fn fingerprint(config: &WorkspaceConfig) -> String {
    thumbnail_fingerprint(config, WORKSPACE, None)
}

#[test]
fn the_default_config_fingerprint_matches_python() {
    assert_eq!(
        fingerprint(&WorkspaceConfig::default()),
        "2c204f52cf743ae5bef91b4c55f23e3e0f3eda916b7f92d34390ca5f6cd3de0c"
    );
}

#[test]
fn a_saturation_change_moves_the_fingerprint() {
    let mut config = WorkspaceConfig::default();
    config.lab.saturation = 1.2;
    assert_eq!(
        fingerprint(&config),
        "f5ccced337e1e23f67748e1fd62286c5f16ee174203fe2913715bc846a3db9a0"
    );
}

#[test]
fn a_density_change_moves_the_fingerprint() {
    let mut config = WorkspaceConfig::default();
    config.exposure.density = 0.35;
    assert_eq!(
        fingerprint(&config),
        "324844841ace0ee2a3675bfb42475d9f819472edd9a3c63c3a46fbb7c827debe"
    );
}

#[test]
fn the_workspace_color_space_moves_the_fingerprint() {
    let config = WorkspaceConfig::default();
    assert_eq!(
        thumbnail_fingerprint(&config, "sRGB", None),
        "3bd77d3050feafedc946ee68570f4bd82d475c2c840d3c26733793efbaea9574"
    );
}

#[test]
fn a_live_carrier_folds_in_the_export_print_size() {
    let mut config = WorkspaceConfig::default();
    config.finish.carrier_width = 2.0;
    assert_eq!(
        fingerprint(&config),
        "c81830d99b9ffb614530b56b5e48d609544affa9c274b1cf4c10a67b38453ac7"
    );
}

#[test]
fn an_export_only_change_keeps_the_fingerprint() {
    let mut config = WorkspaceConfig::default();
    config.export.jpeg_quality = 55;
    config.export.export_fmt = negpy_backend::domain::models::ExportFormat::Png;
    config.metadata.capture_date = "2020-01-01".to_string();
    assert_eq!(fingerprint(&config), fingerprint(&WorkspaceConfig::default()));
}

#[test]
fn a_sharpen_change_is_below_the_thumbnail() {
    let mut config = WorkspaceConfig::default();
    config.lab.sharpen = 1.4;
    config.lab.sharpen_radius = 2.2;
    config.lab.chroma_denoise = 6.0;
    assert_eq!(fingerprint(&config), fingerprint(&WorkspaceConfig::default()));
}

#[test]
fn the_jpeg_comment_round_trips_and_rejects_a_quick_mark() {
    let value = "abc123";
    let encoded = encode_comment(Some(value)).expect("comment");
    assert_eq!(decode_comment(Some(&encoded)), Some(value.to_string()));
    assert_eq!(decode_comment(Some(b"other")), None);
    assert_eq!(encode_comment(None), None);
    assert!(is_current(Some(value), value));
    assert!(!is_current(Some(QUICK), QUICK));
    assert!(!is_current(None, value));
}
