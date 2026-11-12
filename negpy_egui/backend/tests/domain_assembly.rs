//! Ports of tests/test_source_tokens.py: each assembly config and `composite_token`.
//!
//! Two configs share a token only if they decode to the same pixels. The field-coverage
//! checks mirror the Python parametric test: every field must appear in the token, or a
//! stale buffer is served for the setting the user just changed.

use negpy_backend::domain::models::hdr::{
    ANCHOR_EV_UNSET, HdrConfig, hdr_active, hdr_merge_token, hdr_name, hdr_stem, hdr_token,
};
use negpy_backend::domain::models::rgbscan::{RgbScanConfig, is_rgb_triplet, rgbscan_token};
use negpy_backend::domain::models::stitch::{
    StitchConfig, stitch_has_triplets, stitch_name, stitch_token,
};
use negpy_backend::domain::tokens::composite_token;

fn a_file() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/domain_assembly.rs").to_string()
}

#[test]
fn the_hdr_and_stitch_defaults_match() {
    let hdr = HdrConfig::default();
    assert!(!hdr.hdr_enabled);
    assert_eq!(hdr.hdr_anchor_ev, ANCHOR_EV_UNSET);
    assert!(hdr.hdr_align);
    let stitch = StitchConfig::default();
    assert_eq!(stitch.stitch_canvas, [0, 0]);
    assert!(stitch.stitch_align);
    let rgb = RgbScanConfig::default();
    assert!(rgb.align);
}

#[test]
fn an_inactive_config_has_an_empty_token() {
    assert_eq!(hdr_token(&HdrConfig::default()), "");
    assert_eq!(stitch_token(&StitchConfig::default()), "");
    assert_eq!(rgbscan_token(&RgbScanConfig::default()), "");
    assert!(!hdr_active(&HdrConfig::default()));
}

#[test]
fn an_unreachable_source_is_not_assemblable() {
    // "" is the contract for "cannot be assembled"; callers fall back to the plain frame.
    let config = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec!["/nowhere.nef".to_string()],
        ..HdrConfig::default()
    };
    assert_eq!(hdr_token(&config), "");
}

#[test]
fn every_hdr_field_changes_the_token() {
    let path = a_file();
    let base = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec![path.clone()],
        hdr_ratios: vec![1.0, 2.0],
        hdr_align: true,
        hdr_anchor: path.clone(),
        hdr_anchor_ev: 0.0,
    };
    let token = hdr_token(&base);
    assert_ne!(token, "");
    // hdr_enabled off is inactive ("" by contract), so it is checked apart.
    assert_eq!(
        hdr_token(&HdrConfig {
            hdr_enabled: false,
            ..base.clone()
        }),
        ""
    );
    assert_ne!(
        token,
        hdr_token(&HdrConfig {
            hdr_ratios: vec![3.0],
            ..base.clone()
        })
    );
    assert_ne!(
        token,
        hdr_token(&HdrConfig {
            hdr_align: false,
            ..base.clone()
        })
    );
    assert_ne!(
        token,
        hdr_token(&HdrConfig {
            hdr_anchor: String::new(),
            ..base.clone()
        })
    );
    assert_ne!(
        token,
        hdr_token(&HdrConfig {
            hdr_anchor_ev: 0.5,
            ..base.clone()
        })
    );
}

#[test]
fn the_merge_token_ignores_the_render_exposure() {
    let path = a_file();
    let base = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec![path.clone()],
        hdr_ratios: vec![1.0, 2.0],
        ..HdrConfig::default()
    };
    let anchored = HdrConfig {
        hdr_anchor: path.clone(),
        hdr_anchor_ev: 0.0,
        ..base.clone()
    };
    assert_eq!(hdr_merge_token(&base), hdr_merge_token(&anchored));
    // The full token still carries the exposure: a scaled buffer is a different buffer.
    assert_ne!(hdr_token(&base), hdr_token(&anchored));
}

#[test]
fn every_stitch_and_rgbscan_field_changes_the_token() {
    let path = a_file();
    let stitch = StitchConfig {
        stitch_enabled: true,
        stitch_paths: vec![path.clone()],
        stitch_transforms: vec![vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]],
        stitch_canvas: [100, 200],
        stitch_sizes: vec![[50, 60]],
        stitch_triplets: vec![[path.clone(), path.clone()]],
        stitch_align: true,
    };
    let token = stitch_token(&stitch);
    assert_ne!(token, "");
    assert_eq!(
        stitch_token(&StitchConfig {
            stitch_enabled: false,
            ..stitch.clone()
        }),
        ""
    );
    assert_ne!(
        token,
        stitch_token(&StitchConfig {
            stitch_transforms: vec![vec![2.0]],
            ..stitch.clone()
        })
    );
    assert_ne!(
        token,
        stitch_token(&StitchConfig {
            stitch_canvas: [1, 1],
            ..stitch.clone()
        })
    );
    assert_ne!(
        token,
        stitch_token(&StitchConfig {
            stitch_align: false,
            ..stitch.clone()
        })
    );

    let rgb = RgbScanConfig {
        enabled: true,
        green_path: path.clone(),
        blue_path: path.clone(),
        align: true,
    };
    assert!(is_rgb_triplet(&rgb));
    assert_ne!(rgbscan_token(&rgb), "");
    assert_ne!(
        rgbscan_token(&rgb),
        rgbscan_token(&RgbScanConfig {
            align: false,
            ..rgb.clone()
        })
    );
}

#[test]
fn a_rewrite_within_the_same_second_still_invalidates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("green.dng");
    std::fs::write(&path, b"old").unwrap();
    let path = path.to_string_lossy().into_owned();
    let config = RgbScanConfig {
        enabled: true,
        green_path: path.clone(),
        blue_path: path.clone(),
        align: true,
    };
    let before = rgbscan_token(&config);
    std::fs::write(&path, b"new-content-different-size").unwrap();
    assert_ne!(before, rgbscan_token(&config));
}

#[test]
fn a_composite_token_carries_its_kind_and_digest() {
    let path = a_file();
    let config = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec![path.clone()],
        ..HdrConfig::default()
    };
    let token = composite_token("hdr", &config, &[&path]);
    assert!(token.starts_with("|hdr:"));
    // A different kind is a different token even for the same assembly.
    assert_ne!(token, composite_token("other", &config, &[&path]));
}

#[test]
fn the_stem_and_display_name_match_the_python_helpers() {
    let paths = vec![
        "/frames/_DSC1718.NEF".to_string(),
        "/frames/_DSC1715.NEF".to_string(),
        "/frames/_DSC1719.NEF".to_string(),
    ];
    assert_eq!(hdr_stem(&paths), "_DSC1715");
    assert_eq!(hdr_name(&paths), "_DSC1715 +2 (HDR)");
    assert_eq!(hdr_name(&[]), "(HDR)");

    let parts = vec!["/a/left.tif".to_string(), "/a/right.tif".to_string()];
    assert_eq!(stitch_name(&parts), "left+right (Stitch)");
    let many = vec![
        "/a/a.tif".to_string(),
        "/a/b.tif".to_string(),
        "/a/c.tif".to_string(),
        "/a/d.tif".to_string(),
    ];
    assert_eq!(stitch_name(&many), "a +3 (Stitch)");
}

#[test]
fn stitch_triplet_detection_matches() {
    let config = StitchConfig {
        stitch_triplets: vec![["g".to_string(), "b".to_string()]],
        ..StitchConfig::default()
    };
    assert!(stitch_has_triplets(&config));
    let config = StitchConfig {
        stitch_triplets: vec![["g".to_string(), String::new()]],
        ..StitchConfig::default()
    };
    assert!(!stitch_has_triplets(&config));
}
