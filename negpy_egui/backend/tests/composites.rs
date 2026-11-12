//! Ports of tests/test_composites_persist.py.

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::composites::{
    COMPOSITES_KEY, composite_entry, forget_composite, frame_count, part_paths,
    remember_composites, restore_maps, saved_composites,
};
use serde_json::{Map, Value, json};

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn stitch_asset() -> Value {
    json!({
        "name": "a+b (Stitch)",
        "path": "/roll_a/a.nef",
        "hash": "digest#stitch",
        "stitch_paths": ["/roll_a/b.nef"],
        "stitch_transforms": [[1.0, 0.0, 0.0, 0.0, 1.0, 0.0], [1.0, 0.0, 40.0, 0.0, 1.0, 0.0]],
        "stitch_canvas": [120, 80],
        "stitch_sizes": [[80, 80], [80, 80]]
    })
}

fn hdr_asset() -> Value {
    json!({
        "name": "e1 +2 (HDR)",
        "path": "/roll_b/e1.nef",
        "hash": "digest#hdr",
        "hdr_paths": ["/roll_b/e2.nef", "/roll_b/e3.nef"],
        "hdr_ratios": [1.0, 2.0, 4.0]
    })
}

#[test]
fn loading_another_folder_keeps_the_composite() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    remember_composites(&mut repo, &[stitch_asset()]);
    remember_composites(&mut repo, &[json!({"path": "/roll_b/x.nef", "hash": "hx"})]);

    let (stitches, _) = restore_maps(&mut repo);
    assert_eq!(stitches["/roll_a/a.nef"]["paths"], json!(["/roll_a/b.nef"]));
}

#[test]
fn stitch_and_hdr_are_stored_apart() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    remember_composites(&mut repo, &[stitch_asset(), hdr_asset()]);
    let (stitches, merges) = restore_maps(&mut repo);
    assert_eq!(stitches.len(), 1);
    assert_eq!(merges.len(), 1);
    assert_eq!(merges["/roll_b/e1.nef"]["ratios"], json!([1.0, 2.0, 4.0]));
}

#[test]
fn dissolving_a_composite_forgets_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    remember_composites(&mut repo, &[stitch_asset(), hdr_asset()]);
    forget_composite(&mut repo, Some("/roll_a/a.nef"));
    let (stitches, merges) = restore_maps(&mut repo);
    assert!(stitches.is_empty());
    assert_eq!(merges.len(), 1);
}

#[test]
fn legacy_session_entries_are_promoted_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut values = Map::new();
    values.insert(
        "session_stitches".into(),
        json!({"/roll_a/a.nef": {"paths": ["/roll_a/b.nef"], "transforms": [], "canvas": [1, 1], "sizes": [], "hash": "d#stitch"}}),
    );
    values.insert(
        "session_hdr_merges".into(),
        json!({"/roll_b/e1.nef": {"paths": ["/roll_b/e2.nef"], "ratios": [1.0, 2.0], "hash": "d#hdr"}}),
    );
    repo.save_global_settings(&values).unwrap();

    let (stitches, merges) = restore_maps(&mut repo);
    assert_eq!(stitches.len(), 1);
    assert_eq!(merges.len(), 1);
    assert!(repo.global_setting(COMPOSITES_KEY).unwrap().is_some());

    let mut values = Map::new();
    values.insert("session_stitches".into(), Value::Null);
    values.insert("session_hdr_merges".into(), Value::Null);
    repo.save_global_settings(&values).unwrap();
    assert_eq!(restore_maps(&mut repo).0.len(), 1);
}

#[test]
fn a_dissolved_legacy_composite_stays_dissolved() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut values = Map::new();
    values.insert(
        "session_stitches".into(),
        json!({"/roll_a/a.nef": {"paths": ["/roll_a/b.nef"], "hash": "d#stitch"}}),
    );
    repo.save_global_settings(&values).unwrap();
    let _ = restore_maps(&mut repo);
    let mut values = Map::new();
    values.insert(COMPOSITES_KEY.into(), json!({}));
    repo.save_global_settings(&values).unwrap();
    assert_eq!(restore_maps(&mut repo), (Map::new(), Map::new()));
}

#[test]
fn a_forked_composite_keeps_its_shared_hash() {
    let mut asset = stitch_asset();
    asset["hash"] = json!("digest#stitch#roll:r1");
    assert_eq!(composite_entry(&asset).unwrap()["hash"], "digest#stitch");
}

#[test]
fn parts_are_dropped_from_a_frame_count() {
    let mut composites = Map::new();
    composites.insert(
        "/roll_a/a.nef".into(),
        composite_entry(&stitch_asset()).unwrap(),
    );
    let paths = vec!["/roll_a/a.nef".to_string(), "/roll_a/b.nef".to_string()];
    let entry = composite_entry(&stitch_asset()).unwrap();
    assert_eq!(part_paths([entry]).len(), 1);
    assert_eq!(frame_count(&paths, &composites, &Map::new()), 1);
    let dir = tempfile::tempdir().unwrap();
    assert!(saved_composites(&mut repository(dir.path())).is_empty());
}
