//! Ports the row identity, grouping and preset assertions in
//! `tests/test_settings_catalog_presets.py`.

use serde_json::{Map, Value, json};

use negpy_backend::domain::models::{
    DemosaicMode, ProcessMode, SourceEncoding, WorkspaceConfig, with_process_mode,
};
use negpy_backend::services::assets::rolls::card_fields;
use negpy_backend::session::settings_catalog::{
    all_rows, apply_selected_fields, default_sticky_ids, format_row, highlight_bucket,
    preset_config, preset_summary, roll_card_rows, rows_by_id, selected_flat_dict,
};

fn row(label: &str) -> negpy_backend::session::settings_catalog::SettingRow {
    all_rows()
        .into_iter()
        .find(|row| row.label == label)
        .expect("catalog row")
}

fn merge(config: &WorkspaceConfig, preset: &Map<String, Value>) -> WorkspaceConfig {
    let mut flat = config.to_dict();
    for (key, value) in preset {
        flat.insert(key.clone(), value.clone());
    }
    WorkspaceConfig::from_flat_dict(&flat)
}

#[test]
fn row_ids_are_unique_and_defaults_resolve() {
    let rows = all_rows();
    assert_eq!(rows_by_id().len(), rows.len());
    let by_id = rows_by_id();
    assert!(default_sticky_ids().iter().all(|id| by_id.contains_key(id)));
}

#[test]
fn a_channel_grouped_row_stores_every_channel() {
    let mut config = WorkspaceConfig::default();
    config.process.white_point_trim_red = 0.1;
    let data = selected_flat_dict(&config, &[row("White Point Trim")]);
    assert_eq!(data.len(), 3);
    assert_eq!(data["white_point_trim_red"], json!(0.1));
    assert_eq!(
        data["white_point_trim_green"],
        json!(config.process.white_point_trim_green)
    );
}

#[test]
fn crosstalk_copies_its_matrix_and_process_as_a_unit() {
    let data = selected_flat_dict(&WorkspaceConfig::default(), &[row("Crosstalk")]);
    for key in [
        "crosstalk_strength",
        "crosstalk_profile",
        "crosstalk_matrix",
        "crosstalk_process",
    ] {
        assert!(data.contains_key(key), "{key}");
    }
}

#[test]
fn every_roll_card_field_has_a_catalog_row() {
    for card in [
        "film",
        "sensor",
        "fade",
        "demosaic",
        "process",
        "baseline",
        "lens",
        "flatfield",
    ] {
        let listed: std::collections::HashSet<&str> = roll_card_rows(&[card])
            .iter()
            .flat_map(|row| row.fields.iter().copied())
            .collect();
        let (_, fields) = card_fields(card).expect("roll card");
        for field in fields {
            assert!(listed.contains(field), "{card} misses {field}");
        }
    }
}

#[test]
fn a_pasted_highlight_level_remeters_the_frame() {
    let mut source = WorkspaceConfig::default();
    source = with_process_mode(source, ProcessMode::E6);
    source.process.highlight_reconstruction = 5;
    let mut target = source.clone();
    target.process.local_floors = [0.1; 3];
    target.process.local_ceils = [0.9; 3];
    let out = apply_selected_fields(&source, &target, &[row("Highlight Recovery")]);
    assert_eq!(out.process.highlight_reconstruction, 5);
    assert_eq!(out.process.local_floors, [0.0; 3]);
    assert_eq!(out.process.local_ceils, [0.0; 3]);
}

#[test]
fn a_pasted_preview_demosaic_remeters_the_frame() {
    let mut source = WorkspaceConfig::default();
    source.process.demosaic_preview = DemosaicMode::Dcb;
    let mut target = WorkspaceConfig::default();
    target.process.local_floors = [0.1; 3];
    target.process.local_ceils = [0.9; 3];
    let out = apply_selected_fields(&source, &target, &[row("Demosaic Preview")]);
    assert_eq!(out.process.demosaic_preview, DemosaicMode::Dcb);
    assert_eq!(out.process.local_floors, [0.0; 3]);
}

#[test]
fn highlight_levels_show_the_card_choice() {
    let cases = [(0, 0), (2, 1), (5, 2), (7, 2), (1, 0), (42, 0)];
    for (level, bucket) in cases {
        assert_eq!(highlight_bucket(level), bucket);
    }
    assert_eq!(highlight_bucket(7), 2);
}

#[test]
fn a_preset_summary_lists_only_stored_rows() {
    let data = Map::from_iter([
        ("density".to_string(), json!(1.5)),
        ("wb_cyan".to_string(), json!(0.2)),
        ("bogus".to_string(), json!(1)),
    ]);
    assert_eq!(
        preset_summary(&data),
        "Tone: Print Density\nFiltration: Cyan"
    );
}

#[test]
fn a_preset_summary_lists_a_deliberately_default_value() {
    let data = Map::from_iter([(
        "density".to_string(),
        json!(WorkspaceConfig::default().exposure.density),
    )]);
    assert_eq!(preset_summary(&data), "Tone: Print Density");
}

#[test]
fn a_preset_summary_is_empty_without_fields() {
    assert_eq!(preset_summary(&Map::new()), "");
}

#[test]
fn preset_config_lays_stored_fields_over_defaults() {
    let data = Map::from_iter([("saturation".to_string(), json!(1.4))]);
    let config = preset_config(&data);
    assert_eq!(config.lab.saturation, 1.4);
    assert_eq!(
        config.exposure.density,
        WorkspaceConfig::default().exposure.density
    );
}

#[test]
fn a_full_snapshot_applies_without_changes() {
    let snapshot = WorkspaceConfig::default().to_dict();
    assert_eq!(
        merge(&WorkspaceConfig::default(), &snapshot),
        WorkspaceConfig::default()
    );
}

#[test]
fn a_legacy_preset_key_migrates() {
    let data = Map::from_iter([("true_black".to_string(), json!(false))]);
    assert!(
        merge(&WorkspaceConfig::default(), &data)
            .exposure
            .paper_black
    );
}

#[test]
fn a_row_at_default_still_formats() {
    let config = WorkspaceConfig::default();
    let formatted = format_row(&row("Film Mode"), &config);
    assert_eq!(formatted, ProcessMode::C41.as_str());
    assert_eq!(format_row(&row("Encoding"), &config), "Auto");
    assert_eq!(SourceEncoding::default(), SourceEncoding::Auto);
}
