//! Ports of tests/test_source_tokens.py: `source_token` and `calculate_config_hash`.

use negpy_backend::domain::models::LensCorrections;
use negpy_backend::domain::models::{
    DemosaicMode, HdrConfig, RgbScanConfig, SourceEncoding, WorkspaceConfig,
};
use negpy_backend::features::flatfield::logic::{GainEntry, flatfield_token};
use negpy_backend::features::process::path::render_path;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::kernel::caching::logic::{calculate_config_hash, calculate_value_hash};
use negpy_backend::kernel::system::config::default_workspace_config;
use negpy_backend::services::rendering::source_identity::{lens_decode_token, source_token};

fn a_file() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/source_identity.rs").to_string()
}

fn gain() -> GainEntry {
    GainEntry {
        gain: ImageBuffer::from_rgb(1, 1, vec![1.0, 1.0, 1.0]),
        token: "gain-token".to_string(),
        sensor_layout: true,
    }
}

#[test]
fn the_default_source_token_matches_python() {
    let token = source_token(&WorkspaceConfig::default(), None);
    assert_eq!(token, "|lr0|hr0|hrwb0|dm:Auto");
}

#[test]
fn the_default_config_hash_matches_python() {
    let home = negpy_backend::kernel::system::paths::default_user_dir();
    let config = default_workspace_config(home.join("export").display().to_string());
    let hash = calculate_config_hash(&config);
    assert_eq!(hash, "f01ad94187497e26676d976369e18414");
}

#[test]
fn a_slider_value_keeps_its_hash() {
    assert_ne!(
        calculate_value_hash(&(-1.0,)),
        calculate_value_hash(&(-2.0,))
    );
    let mut config = WorkspaceConfig::default();
    assert_ne!(
        calculate_config_hash(&config),
        calculate_config_hash(&WorkspaceConfig {
            geometry: negpy_backend::domain::models::GeometryConfig {
                fine_rotation: -2.0,
                ..config.geometry.clone()
            },
            ..config.clone()
        })
    );
    config.geometry.fine_rotation = -1.0;
    assert_ne!(
        calculate_config_hash(&config),
        calculate_config_hash(&WorkspaceConfig {
            geometry: negpy_backend::domain::models::GeometryConfig {
                fine_rotation: -2.0,
                ..config.geometry.clone()
            },
            ..config
        })
    );
}

#[test]
fn the_decode_settings_move_the_source_token() {
    let base = WorkspaceConfig::default();
    let mut linear = base.clone();
    linear.process.linear_raw = true;
    assert_ne!(source_token(&base, None), source_token(&linear, None));

    let mut vng = base.clone();
    vng.process.demosaic_preview = DemosaicMode::Vng;
    assert_ne!(source_token(&base, None), source_token(&vng, None));

    let mut export = base.clone();
    export.process.demosaic_export = DemosaicMode::Vng;
    assert_eq!(source_token(&base, None), source_token(&export, None));
}

#[test]
fn a_positive_adds_its_source_encoding() {
    let mut positive = WorkspaceConfig::default();
    positive.process.process_mode = negpy_backend::domain::models::ProcessMode::E6;
    positive.process.positive_source = true;
    positive.process.source_encoding = SourceEncoding::Linear;
    let token = source_token(&positive, None);
    assert!(token.contains("|pdlinear"));
    assert_eq!(
        render_path(&positive.process),
        negpy_backend::features::process::path::RenderPath::Positive
    );
}

#[test]
fn composites_and_assemblies_appear_in_the_token() {
    let base = WorkspaceConfig::default();
    let merged = WorkspaceConfig {
        hdr: HdrConfig {
            hdr_enabled: true,
            hdr_paths: vec![a_file()],
            hdr_ratios: vec![1.0, 2.0],
            ..HdrConfig::default()
        },
        ..base.clone()
    };
    assert_ne!(source_token(&base, None), source_token(&merged, None));
    let rgb = WorkspaceConfig {
        rgbscan: RgbScanConfig {
            enabled: true,
            green_path: a_file(),
            blue_path: a_file(),
            ..RgbScanConfig::default()
        },
        ..base.clone()
    };
    assert_ne!(source_token(&base, None), source_token(&rgb, None));
}

#[test]
fn a_render_only_setting_does_not_re_decode() {
    let base = WorkspaceConfig::default();
    let mut edited = base.clone();
    edited.exposure.density = 1.7;
    edited.flatfield.apply = true;
    edited.flatfield.profile_id = "missing".to_string();
    assert_eq!(source_token(&base, None), source_token(&edited, None));

    let mut stitched = base.clone();
    stitched.stitch.stitch_enabled = true;
    stitched.flatfield.apply = true;
    stitched.flatfield.profile_id = "reference".to_string();
    assert!(
        source_token(&stitched, Some(&gain()))
            .contains(&flatfield_token(&stitched.flatfield, Some(&gain())))
    );
}

#[test]
fn embedded_lens_identity_appears_only_when_a_correction_is_enabled() {
    let off = LensCorrections::default();
    assert_eq!(
        lens_decode_token(off, &WorkspaceConfig::default().flatfield, None),
        ""
    );
    let on = LensCorrections {
        distortion: true,
        ca: false,
    };
    assert_eq!(
        lens_decode_token(on, &WorkspaceConfig::default().flatfield, None),
        "|embedded-lens-v3-d1-ca0"
    );
}

#[test]
fn a_lens_correction_and_its_flat_field_move_the_token() {
    let base = WorkspaceConfig::default();
    let mut enabled = base.clone();
    enabled.geometry.lens_distortion_from_metadata = true;
    enabled.flatfield.apply = true;
    enabled.flatfield.profile_id = "reference".to_string();
    assert_ne!(
        source_token(&base, None),
        source_token(&enabled, Some(&gain()))
    );
    assert_eq!(
        source_token(&base, None),
        source_token(&base, Some(&gain()))
    );

    let mut composite = enabled.clone();
    composite.hdr.hdr_enabled = true;
    composite.hdr.hdr_paths = vec![a_file()];
    composite.hdr.hdr_ratios = vec![1.0, 2.0];
    assert!(!source_token(&composite, Some(&gain())).contains("embedded-lens"));
}
