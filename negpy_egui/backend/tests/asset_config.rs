//! Ports the asset-resolution rows of `tests/test_desktop_session.py`.

use negpy_backend::domain::models::{
    HdrConfig, ProcessMode, RgbScanConfig, StitchConfig, WorkspaceConfig,
};
use negpy_backend::domain::models::{scan_setup_rig, scan_setup_values};
use negpy_backend::session::asset_config::{
    CompositeKind, can_edit_triplet, composite_kind, is_plain_triplet, resolve_asset_capture,
    resolve_asset_hdr, resolve_asset_hdr_seed, resolve_asset_process_mode, resolve_asset_rgbscan,
    resolve_asset_stitch, trichrome_merge_indices,
};
use serde_json::json;

#[test]
fn a_stitch_is_tested_before_its_primary_triplet() {
    let asset = json!({
        "path": "/roll/part1.dng",
        "green_path": "/roll/g.dng",
        "blue_path": "/roll/b.dng",
        "stitch_paths": ["/roll/part2.dng"],
    });
    assert_eq!(composite_kind(&asset), CompositeKind::Stitch);
    assert!(!can_edit_triplet(&asset));
    assert!(!is_plain_triplet(&asset));
}

#[test]
fn a_plain_triplet_can_be_rebuilt_and_a_half_cannot() {
    let triplet = json!({
        "path": "/roll/r.dng",
        "green_path": "/roll/g.dng",
        "blue_path": "/roll/b.dng",
    });
    assert!(can_edit_triplet(&triplet));
    assert!(is_plain_triplet(&triplet));
    let half = json!({ "path": "/roll/h.dng", "half": 1 });
    assert!(!can_edit_triplet(&half));
    assert_eq!(composite_kind(&half), CompositeKind::Half);
    assert_eq!(
        composite_kind(&json!({"path": "/x", "diptych": true})),
        CompositeKind::Diptych
    );
}

#[test]
fn a_trichrome_merge_needs_three_distinct_plain_files() {
    let files = vec![
        json!({"path": "/roll/a.dng"}),
        json!({"path": "/roll/b.dng"}),
        json!({"path": "/roll/c.dng"}),
    ];
    assert_eq!(trichrome_merge_indices(&files, &[2, 0, 1]), vec![0, 1, 2]);
    assert!(trichrome_merge_indices(&files, &[0, 1]).is_empty());
    assert!(trichrome_merge_indices(&files, &[0, 0, 1]).is_empty());
    let composite = vec![
        json!({"path": "/roll/a.dng"}),
        json!({"path": "/roll/b.dng"}),
        json!({"path": "/roll/c.dng", "stitch_paths": ["/roll/d.dng"]}),
    ];
    assert!(trichrome_merge_indices(&composite, &[0, 1, 2]).is_empty());
}

#[test]
fn a_plain_asset_drops_a_leaked_triplet_bracket_and_registration() {
    let mut leaked = WorkspaceConfig::default();
    leaked.rgbscan.enabled = true;
    leaked.rgbscan.green_path = "/stale/g.dng".into();
    leaked.rgbscan.blue_path = "/stale/b.dng".into();
    leaked.hdr.hdr_enabled = true;
    leaked.hdr.hdr_paths = vec!["/stale/long.dng".into()];
    leaked.stitch.stitch_enabled = true;
    leaked.stitch.stitch_paths = vec!["/stale/part.dng".into()];
    let plain = json!({"path": "/roll/plain.dng"});
    let out = resolve_asset_stitch(
        &resolve_asset_hdr(&resolve_asset_rgbscan(&leaked, &plain), &plain),
        &plain,
    );
    assert_eq!(out.rgbscan, RgbScanConfig::default());
    assert_eq!(out.hdr, HdrConfig::default());
    assert_eq!(out.stitch, StitchConfig::default());
}

#[test]
fn a_triplet_takes_the_assets_own_paths() {
    let asset = json!({
        "path": "/roll/r.dng",
        "green_path": "/roll/g.dng",
        "blue_path": "/roll/b.dng",
        "align": false,
    });
    let out = resolve_asset_rgbscan(&WorkspaceConfig::default(), &asset);
    assert_eq!(
        out.rgbscan,
        RgbScanConfig {
            enabled: true,
            green_path: "/roll/g.dng".into(),
            blue_path: "/roll/b.dng".into(),
            align: false,
        }
    );
}

#[test]
fn a_changed_triplet_composition_clears_the_measured_bounds() {
    let mut params = WorkspaceConfig::default();
    params.process.local_floors = [1.0, 2.0, 3.0];
    params.process.local_ceils = [4.0, 5.0, 6.0];
    let out = resolve_asset_rgbscan(
        &params,
        &json!({"green_path": "/g.dng", "blue_path": "/b.dng"}),
    );
    assert_eq!(out.process.local_floors, [0.0; 3]);
    assert_eq!(out.process.local_ceils, [0.0; 3]);
}

#[test]
fn a_stitch_registration_rides_on_the_params() {
    let asset = json!({
        "stitch_paths": ["/roll/p2.dng"],
        "stitch_transforms": [[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]],
        "stitch_canvas": [120, 80],
        "stitch_sizes": [[80, 80]],
    });
    let out = resolve_asset_stitch(&WorkspaceConfig::default(), &asset);
    assert!(out.stitch.stitch_enabled);
    assert_eq!(out.stitch.stitch_canvas, [120, 80]);
    assert_eq!(out.stitch.stitch_sizes, vec![[80, 80]]);
}

#[test]
fn a_fresh_bracket_opens_at_its_stored_anchor() {
    let asset = json!({
        "path": "/roll/ref.dng",
        "hdr_paths": ["/roll/long.dng"],
        "hdr_ratios": [1.0, 4.0],
        "hdr_anchor": "/roll/long.dng",
    });
    let out = resolve_asset_hdr_seed(&WorkspaceConfig::default(), &asset);
    assert!(out.hdr.hdr_enabled);
    assert_eq!(out.hdr.hdr_anchor, "/roll/long.dng");
    assert!(out.exposure.shadow_density < 0.0);
}

#[test]
fn a_composite_process_mode_overrides_a_stale_sticky_mode() {
    let params = WorkspaceConfig::default();
    assert_eq!(params.process.process_mode, ProcessMode::C41);
    let asset = json!({"process_mode": "Transparency"});
    assert_eq!(
        resolve_asset_process_mode(&params, &asset)
            .process
            .process_mode,
        ProcessMode::E6
    );
    assert_eq!(
        resolve_asset_process_mode(&params, &json!({}))
            .process
            .process_mode,
        ProcessMode::C41
    );
}

#[test]
fn a_fresh_narrowband_capture_opens_as_c41_on_the_pakon_stock() {
    let mut params = WorkspaceConfig::default();
    params.exposure.paper_profile = "neutral".into();
    let out = resolve_asset_capture(&params, &json!({"capture": "blend"}));
    assert_eq!(out.process.process_mode, ProcessMode::C41);
    assert!(out.process.linear_raw);
    assert_eq!(out.exposure.paper_profile, "pakon");
    assert_eq!(
        resolve_asset_capture(&params, &json!({"capture": "other"})),
        params
    );
}

#[test]
fn the_scanning_setup_values_match_the_wizard_rules() {
    assert_eq!(scan_setup_values("scanner", "white"), (true, "neutral"));
    assert_eq!(scan_setup_values("camera", "white"), (false, "neutral"));
    assert_eq!(scan_setup_values("auto", "narrowband"), (true, "pakon"));
    assert_eq!(scan_setup_values("camera", "narrowband"), (true, "pakon"));
    assert_eq!(scan_setup_rig(None), ("auto", "white"));
    assert_eq!(
        scan_setup_rig(Some(&json!({"capture": "scanner", "light": "narrowband"}))),
        ("scanner", "narrowband")
    );
    assert_eq!(
        scan_setup_rig(Some(&json!({"capture": "bad", "light": "bad"}))),
        ("auto", "white")
    );
}
