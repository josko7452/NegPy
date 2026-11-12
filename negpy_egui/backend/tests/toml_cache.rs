//! Ports of tests/test_profile_caches.py.

use std::fs;

use negpy_backend::services::assets::toml_cache::load_toml_cached;

#[test]
fn a_changed_file_is_reparsed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.toml");
    fs::write(&path, "name = \"one\"\n").unwrap();
    assert_eq!(
        load_toml_cached(&path)
            .unwrap()
            .get("name")
            .unwrap()
            .as_str(),
        Some("one")
    );

    fs::write(&path, "name = \"two\"\n").unwrap();
    assert_eq!(
        load_toml_cached(&path)
            .unwrap()
            .get("name")
            .unwrap()
            .as_str(),
        Some("two")
    );
}

#[test]
fn an_invalid_file_is_cached_as_none_until_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.toml");
    fs::write(&path, "not valid toml ][{\n").unwrap();
    assert!(load_toml_cached(&path).is_none());

    fs::write(&path, "name = \"fixed\"\n").unwrap();
    assert_eq!(
        load_toml_cached(&path)
            .unwrap()
            .get("name")
            .unwrap()
            .as_str(),
        Some("fixed")
    );
}

#[test]
fn a_missing_file_has_no_entry() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_toml_cached(&dir.path().join("missing.toml")).is_none());
}
