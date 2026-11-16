//! Capture recipes and framing levels over the settings database.

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::capture::presets::{
    PRESETS_KEY, PresetStore, ScanlightPreset, default_framing_levels, framing_levels,
};

fn store(dir: &tempfile::TempDir) -> PresetStore {
    let repo = StorageRepository::new(
        dir.path().join("edits.db").to_string_lossy(),
        dir.path().join("settings.db").to_string_lossy(),
    );
    repo.initialize().unwrap();
    PresetStore::new(repo)
}

#[test]
fn save_get_list_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    assert!(store.names().unwrap().is_empty());
    let mut preset = ScanlightPreset::default();
    preset.r_level = 200;
    preset.g_level = 180;
    preset.shutter_b = "1/4".to_string();
    store.save("Portra 400", &preset).unwrap();
    assert_eq!(store.names().unwrap(), ["Portra 400"]);
    assert_eq!(store.get("Portra 400").unwrap(), Some(preset));
    let mut ektar = ScanlightPreset::default();
    ektar.r_level = 210;
    store.save("Ektar 100", &ektar).unwrap();
    assert_eq!(store.names().unwrap(), ["Ektar 100", "Portra 400"]);
    store.delete("Portra 400").unwrap();
    assert!(store.get("Portra 400").unwrap().is_none());
}

#[test]
fn garbage_and_unknown_fields_are_tolerated() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = StorageRepository::new(
        dir.path().join("edits.db").to_string_lossy(),
        dir.path().join("settings.db").to_string_lossy(),
    );
    repo.initialize().unwrap();
    let mut settings = serde_json::Map::new();
    settings.insert(
        PRESETS_KEY.to_string(),
        serde_json::json!({
            "x": "not a dict",
            "y": {"r_level": 100, "bogus": 5},
        }),
    );
    repo.save_global_settings(&settings).unwrap();
    let store = PresetStore::new(repo);
    assert!(store.get("x").unwrap().is_none());
    assert_eq!(store.get("y").unwrap().unwrap().r_level, 100);
}

#[test]
fn legacy_presets_default_the_new_fields() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = StorageRepository::new(
        dir.path().join("edits.db").to_string_lossy(),
        dir.path().join("settings.db").to_string_lossy(),
    );
    repo.initialize().unwrap();
    let mut settings = serde_json::Map::new();
    settings.insert(
        PRESETS_KEY.to_string(),
        serde_json::json!({
            "Old": {"r_level": 200, "shutter_r": "1/5"},
        }),
    );
    repo.save_global_settings(&settings).unwrap();
    let preset = PresetStore::new(repo).get("Old").unwrap().unwrap();
    assert_eq!(preset.iso, "");
    assert_eq!(preset.aperture, "");
    assert!(!preset.single_capture);
    assert!(!preset.blend_capture);
}

#[test]
fn iso_and_aperture_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let mut preset = ScanlightPreset::default();
    preset.r_level = 200;
    preset.shutter_r = "1/5".to_string();
    preset.iso = "100".to_string();
    preset.aperture = "f/8".to_string();
    store.save("Portra 400", &preset).unwrap();
    assert_eq!(store.get("Portra 400").unwrap(), Some(preset));
}

#[test]
fn the_store_knows_which_sensor_profiles_it_owns() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let mut profiled = ScanlightPreset::default();
    profiled.sensor_profile = "Portra".to_string();
    store.save("Portra", &profiled).unwrap();
    store.save("Triplet", &ScanlightPreset::default()).unwrap();
    assert!(store.owns_sensor_profile("Portra").unwrap());
    assert!(!store.owns_sensor_profile("Hand Made").unwrap());
    assert!(!store.owns_sensor_profile("").unwrap());
}

#[test]
fn framing_dims_three_stops_and_keeps_channels_lit() {
    assert_eq!(default_framing_levels(210, 95, 80, false), [26, 11, 10]);
    assert_eq!(default_framing_levels(255, 4, 0, false), [31, 1, 0]);
    assert_eq!(default_framing_levels(180, 185, 155, false), [22, 23, 19]);
}

#[test]
fn rgb_only_framing_raises_to_its_floor_but_not_above_the_recipe() {
    assert_eq!(default_framing_levels(180, 185, 155, true), [115, 115, 115]);
    assert_eq!(default_framing_levels(255, 230, 90, true), [115, 115, 90]);
    assert_eq!(default_framing_levels(100, 90, 0, true), [100, 90, 0]);
    assert_eq!(default_framing_levels(0, 0, 0, true), [0, 0, 0]);
    assert_eq!(framing_levels(210, 95, 80, 3, false), [26, 11, 10]);
}
