//! Ports of the `WorkspaceConfig` assertions behind tests/test_config_deserialization.py,
//! tests/test_sidecar.py and tests/test_flat_master.py.

use std::path::PathBuf;

use negpy_backend::domain::models::export::{ExportFormat, ExportPreset};
use negpy_backend::domain::models::process::ProcessMode;
use negpy_backend::domain::models::workspace::{
    WorkspaceConfig, flat_export_config, flat_master_config, resolve_preset_export,
};
use negpy_backend::domain::models::{ExposureConfig, RenderIntent};
use serde_json::{Value, json};

fn from_fields(fields: &[(&str, Value)]) -> WorkspaceConfig {
    let map: serde_json::Map<String, Value> = fields
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect();
    WorkspaceConfig::from_flat_dict(&map)
}

#[test]
fn the_defaults_flatten_to_one_namespace() {
    let flat = WorkspaceConfig::default().to_dict();
    // A spot check across sections and the local-mask key.
    assert_eq!(flat.get("process_mode"), Some(&json!("Color Negative")));
    assert_eq!(flat.get("density"), Some(&json!(1.0)));
    assert_eq!(flat.get("crop_rect"), Some(&Value::Null));
    assert_eq!(flat.get("export_fmt"), Some(&json!("JPEG")));
    assert_eq!(flat.get("userDir"), Some(&json!("")));
    assert!(flat.contains_key("local_masks"));
    assert!(flat.contains_key("stitch_enabled"));
    assert!(flat.contains_key("hdr_enabled"));
}

#[test]
fn a_basic_deserialization_reads_each_section() {
    let config = from_fields(&[
        ("process_mode", json!("B&W Negative")),
        ("density", json!(1.2)),
        ("grade", json!(3.0)),
        ("export_fmt", json!("TIFF")),
    ]);
    assert_eq!(config.process.process_mode, ProcessMode::BW);
    assert_eq!(config.exposure.density, 1.2);
    // Legacy 0-5 paper grade migrates to ISO R (150 - 20*G).
    assert_eq!(config.exposure.grade, 90.0);
    assert_eq!(config.export.export_fmt, ExportFormat::Tiff);
}

#[test]
fn an_unknown_key_is_dropped_without_failing_the_load() {
    let config = from_fields(&[
        ("process_mode", json!("B&W Negative")),
        ("this_is_unknown", json!(42)),
        ("also_unknown", json!("hello")),
    ]);
    assert_eq!(config.process.process_mode, ProcessMode::BW);
    // The unknown keys never reach the flat namespace on the way back out.
    let flat = config.to_dict();
    assert!(!flat.contains_key("this_is_unknown"));
    assert!(!flat.contains_key("also_unknown"));
}

#[test]
fn a_retired_narrowband_scan_becomes_the_pakon_stock() {
    assert_eq!(
        WorkspaceConfig::default().to_dict().get("narrowband_scan"),
        None
    );
    assert_eq!(
        from_fields(&[("narrowband_scan", json!(false))]),
        WorkspaceConfig::default()
    );
    assert_eq!(
        from_fields(&[("narrowband_scan", json!(true))])
            .exposure
            .paper_profile,
        "pakon"
    );
    // A chosen stock, a B&W mode or a slide keeps its paper.
    let kept = from_fields(&[
        ("narrowband_scan", json!(true)),
        ("paper_profile", json!("kodak_endura")),
    ]);
    assert_eq!(kept.exposure.paper_profile, "kodak_endura");
    let bw = from_fields(&[
        ("narrowband_scan", json!(true)),
        ("process_mode", json!("B&W Negative")),
    ]);
    assert_eq!(bw.exposure.paper_profile, "neutral");
}

#[test]
fn a_merged_bracket_clears_the_auto_and_reconstruction_decisions() {
    let config = from_fields(&[
        ("hdr_enabled", json!(true)),
        ("hdr_paths", json!(["/frames/a.nef"])),
        ("highlight_reconstruction", json!(3)),
        ("auto_exposure", json!(true)),
        ("auto_normalize_contrast", json!(true)),
    ]);
    assert_eq!(config.process.highlight_reconstruction, 0);
    assert!(!config.exposure.auto_exposure);
    assert!(!config.exposure.auto_normalize_contrast);
    // Without an active bracket the autos are untouched.
    let plain = WorkspaceConfig::default();
    assert!(plain.exposure.auto_exposure);
}

#[test]
fn a_flat_master_bypasses_the_creative_print_decisions() {
    let base = from_fields(&[
        ("grade", json!(130.0)),
        ("cast_removal_strength", json!(0.8)),
        ("toe", json!(0.4)),
        ("paper_black", json!(false)),
    ]);
    let flat = flat_master_config(&base);
    assert_eq!(flat.exposure.render_intent, RenderIntent::Flat);
    assert!(!flat.exposure.auto_exposure);
    assert_eq!(flat.exposure.cast_removal_strength, 0.0);
    assert_eq!(flat.exposure.toe, 0.0);
    assert!(flat.exposure.paper_black);
    // The grade and paper profile are untouched.
    assert_eq!(flat.exposure.grade, 130.0);
}

#[test]
fn a_flat_export_forces_sixteen_bit_lossless() {
    let config: negpy_backend::domain::models::export::ExportConfig =
        serde_json::from_value(json!({
            "export_fmt": "JPEG",
            "export_bit_depth": 8,
            "export_resolution_mode": "print"
        }))
        .unwrap();
    let flat = flat_export_config(&config);
    // JPEG is not a flat deliverable, so it becomes TIFF; a chosen Print sizing survives.
    assert_eq!(flat.export_fmt, ExportFormat::Tiff);
    assert_eq!(flat.export_bit_depth, 16);
    assert_eq!(
        flat.export_resolution_mode,
        negpy_backend::domain::models::export::ExportResolutionMode::Print
    );
}

#[test]
fn a_print_intent_preset_returns_the_params_unchanged() {
    let params = WorkspaceConfig::default();
    let preset = ExportPreset::default();
    let (config, delivery) = resolve_preset_export(&preset, &params);
    assert_eq!(config, params);
    assert_eq!(delivery, preset);
}

#[test]
fn the_0_63_0_sidecar_fixture_reloads_and_reemits() {
    // The fixture predates several fields; loading runs the migrations and drops retired
    // keys, then the config re-emits a stable flat object.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sidecars/0.63.0.negpy");
    let text = std::fs::read_to_string(&path).expect("fixture reads");
    let raw: serde_json::Map<String, Value> = serde_json::from_str(&text).expect("fixture parses");

    let config = WorkspaceConfig::from_flat_dict(&raw);
    // A couple of values the fixture sets.
    assert_eq!(config.exposure.density, 0.42);
    assert_eq!(config.exposure.grade, 80.0);
    assert_eq!(config.exposure.cast_removal_strength, 1.0);
    assert_eq!(config.local.masks.len(), 1);

    // Re-emitting then reloading is a fixed point.
    let emitted = config.to_dict();
    let again = WorkspaceConfig::from_flat_dict(&emitted);
    assert_eq!(config, again);
    // Retired keys the fixture still carries are gone.
    assert!(!emitted.contains_key("narrowband_scan"));
    assert!(!emitted.contains_key("mask_spacer"));
    assert!(!emitted.contains_key("ir_method"));
}

#[test]
fn the_crossover_paper_black_trims_round_trip() {
    let config = from_fields(&[
        ("white_point_trim_red", json!(0.05)),
        ("black_point_trim_blue", json!(0.11)),
    ]);
    assert_eq!(config.process.white_point_trim_red, 0.05);
    assert_eq!(config.process.black_point_trim_blue, 0.11);
    let flat = config.to_dict();
    assert_eq!(flat.get("white_point_trim_red"), Some(&json!(0.05)));
    assert_eq!(flat.get("black_point_trim_blue"), Some(&json!(0.11)));
}

#[test]
fn the_exposure_default_helper_stays_consistent() {
    // A programmatic default and a deserialized `{}` must agree.
    let programmatic = ExposureConfig::default();
    let deserialized: ExposureConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(programmatic, deserialized);
}
