//! Ports of tests/test_rolls.py's fork rows.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::rolls::{
    ROLLS_KEY, fork_edit, forked_edit_hashes, is_forked, roll_edit_hash, saved_rolls, unfork_edit,
    unforked_hash,
};
use serde_json::json;

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn virtual_roll(repo: &mut StorageRepository, name: &str) -> String {
    let mut store = saved_rolls(repo);
    store.insert(
        "r1".to_string(),
        json!({"kind": "virtual", "name": name, "member_paths": ["/a.nef"]}),
    );
    let mut values = serde_json::Map::new();
    values.insert(ROLLS_KEY.to_string(), serde_json::Value::Object(store));
    repo.save_global_settings(&values).unwrap();
    "r1".to_string()
}

#[test]
fn a_fork_suffixes_the_roll_id_and_keeps_a_half_frame_suffix() {
    assert_eq!(roll_edit_hash("abc123", "r1"), "abc123#roll:r1");
    assert_eq!(roll_edit_hash("abc123#1", "r1"), "abc123#1#roll:r1");
}

#[test]
fn only_a_roll_suffix_is_stripped() {
    assert_eq!(unforked_hash("abc123#roll:r1"), "abc123");
    assert_eq!(unforked_hash("abc123#1#roll:r1"), "abc123#1");
    assert_eq!(unforked_hash("abc123#1"), "abc123#1");
    assert_eq!(unforked_hash("abc123"), "abc123");
}

#[test]
fn forking_seeds_the_forked_hash_and_marks_it_forked() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    let mut config = WorkspaceConfig::default();
    config.exposure.density = 0.42;

    let forked = fork_edit(&mut repo, &roll_id, "abc123", "/a.nef", &config).unwrap();

    assert_eq!(forked, "abc123#roll:r1");
    assert!(is_forked(&repo, &roll_id, "abc123"));
    assert_eq!(forked_edit_hashes(&repo, "abc123"), vec![forked.clone()]);
    assert_eq!(
        repo.load_file_settings(&forked)
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.42
    );
    assert!(repo.load_file_settings("abc123").unwrap().is_none());
}

#[test]
fn forking_twice_reseeds_without_duplicating_the_marker() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    let mut first = WorkspaceConfig::default();
    first.exposure.density = 0.1;
    let mut second = WorkspaceConfig::default();
    second.exposure.density = 0.9;

    fork_edit(&mut repo, &roll_id, "abc123", "/a.nef", &first).unwrap();
    fork_edit(&mut repo, &roll_id, "abc123", "/a.nef", &second).unwrap();

    let store = saved_rolls(&repo);
    assert_eq!(store[&roll_id]["forked_hashes"], json!(["abc123"]));
    assert_eq!(
        repo.load_file_settings("abc123#roll:r1")
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.9
    );
}

#[test]
fn forking_an_unknown_roll_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let forked = fork_edit(
        &mut repo,
        "not-a-real-id",
        "abc123",
        "/a.nef",
        &WorkspaceConfig::default(),
    )
    .unwrap();
    assert_eq!(forked, "abc123");
    assert!(repo.load_file_settings(&forked).unwrap().is_none());
}

#[test]
fn unforking_drops_the_marker_and_the_forked_edit() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    let forked = fork_edit(
        &mut repo,
        &roll_id,
        "abc123",
        "/a.nef",
        &WorkspaceConfig::default(),
    )
    .unwrap();
    repo.save_file_settings("abc123", &WorkspaceConfig::default(), "/a.nef")
        .unwrap();

    unfork_edit(&mut repo, &roll_id, "abc123").unwrap();

    assert!(!is_forked(&repo, &roll_id, "abc123"));
    assert!(repo.load_file_settings(&forked).unwrap().is_none());
    assert!(repo.load_file_settings("abc123").unwrap().is_some());
}
