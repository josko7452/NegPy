//! Ports of tests/test_presets_service.py.

use negpy_backend::services::assets::presets::{PresetStore, sanitize_preset_name};
use serde_json::{Map, Value, json};

fn fields(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

#[test]
fn a_preset_round_trips_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let store = PresetStore::new(dir.path(), "");
    store
        .save("Portra Look", &fields(&[("density", json!(1.5))]))
        .unwrap();
    assert!(store.list().contains(&"Portra Look".to_string()));
    assert_eq!(
        store.load("Portra Look").unwrap().get("density"),
        Some(&json!(1.5))
    );

    assert!(store.delete("Portra Look"));
    assert!(store.list().is_empty());
}

#[test]
fn deleting_a_missing_preset_is_false() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!PresetStore::new(dir.path(), "").delete("Nope"));
}

#[test]
fn names_are_sanitized_and_validated() {
    assert_eq!(sanitize_preset_name("../escaped"), "escaped");
    assert!(!sanitize_preset_name("../escaped").eq("../escaped"));
    assert!(
        !PresetStore::new("/tmp", "")
            .save("../bad", &Map::new())
            .is_ok()
    );
}

#[test]
fn notes_are_reserved_and_omitted_when_empty() {
    let data = fields(&[("density", json!(1.5)), ("__notes", json!("look"))]);
    let notes = negpy_backend::services::assets::presets::preset_notes(&data);
    assert_eq!(notes, "look");
    assert_eq!(
        negpy_backend::services::assets::presets::preset_fields(&data).len(),
        1
    );
    assert!(
        negpy_backend::services::assets::presets::with_preset_notes(&data, "  ")
            .get("__notes")
            .is_none()
    );
}
