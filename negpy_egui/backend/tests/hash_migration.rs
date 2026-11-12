//! Ports tests/test_hash_migration.py's rehome and ambiguity cases.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::hash::{
    LegacyAsset, blank_ambiguous_legacy_hashes, migrate_asset_hash,
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

fn config(density: f64) -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.exposure.density = density;
    config
}

#[test]
fn rehome_moves_settings_history_and_mark() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let cfg = config(0.42);
    repo.save_file_settings("v1", &cfg, "/p/a.dng").unwrap();
    repo.save_history_step("v1", 0, &cfg).unwrap();
    repo.save_history_step("v1", 1, &cfg).unwrap();
    repo.save_file_mark("v1", Some("keeper"), "/p/a.dng")
        .unwrap();

    migrate_asset_hash(
        &repo,
        LegacyAsset {
            path: "/p/a.dng",
            hash: "v2",
            legacy_hash: "v1",
        },
    )
    .unwrap();

    let moved = repo.load_file_settings("v2").unwrap().unwrap();
    assert_eq!(moved.exposure.density, 0.42);
    assert!(repo.load_file_settings("v1").unwrap().is_none());
    assert_eq!(repo.all_history("v2").unwrap().len(), 2);
    assert!(repo.all_history("v1").unwrap().is_empty());
    assert_eq!(
        repo.file_marks().unwrap().get("v2").map(String::as_str),
        Some("keeper")
    );
}

#[test]
fn rehome_never_clobbers_a_live_edit() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("v1", &config(0.11), "/p/a.dng")
        .unwrap();
    repo.save_file_settings("v2", &config(0.99), "/p/a.dng")
        .unwrap();

    migrate_asset_hash(
        &repo,
        LegacyAsset {
            path: "/p/a.dng",
            hash: "v2",
            legacy_hash: "v1",
        },
    )
    .unwrap();

    assert_eq!(
        repo.load_file_settings("v2")
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.99
    );
}

#[test]
fn rehome_of_one_half_leaves_the_sibling_alone() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("v1#1", &config(0.11), "/p/a.tif")
        .unwrap();
    repo.save_file_settings("v1#2", &config(0.22), "/p/a.tif")
        .unwrap();

    for (legacy, current) in [("v1#1", "v2#1"), ("v1#2", "v2#2")] {
        migrate_asset_hash(
            &repo,
            LegacyAsset {
                path: "/p/a.tif",
                hash: current,
                legacy_hash: legacy,
            },
        )
        .unwrap();
    }

    assert_eq!(
        repo.load_file_settings("v2#1")
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.11
    );
    assert_eq!(
        repo.load_file_settings("v2#2")
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.22
    );
}

#[test]
fn blanking_ambiguous_legacy_hashes_claims_none() {
    let mut assets = vec![
        json!({"path": "/p/a.dng", "hash": "v2a", "legacy_hash": "shared"}),
        json!({"path": "/p/b.dng", "hash": "v2b", "legacy_hash": "shared"}),
        json!({"path": "/p/c.dng", "hash": "v2c", "legacy_hash": "own"}),
    ];

    blank_ambiguous_legacy_hashes(&mut assets);

    assert_eq!(assets[0]["legacy_hash"], json!(""));
    assert_eq!(assets[1]["legacy_hash"], json!(""));
    assert_eq!(assets[2]["legacy_hash"], json!("own"));
}

#[test]
fn a_blank_or_unchanged_legacy_hash_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("v1", &config(0.42), "/p/a.dng")
        .unwrap();

    migrate_asset_hash(
        &repo,
        LegacyAsset {
            path: "/p/a.dng",
            hash: "v1",
            legacy_hash: "v1",
        },
    )
    .unwrap();
    migrate_asset_hash(
        &repo,
        LegacyAsset {
            path: "/p/a.dng",
            hash: "v2",
            legacy_hash: "",
        },
    )
    .unwrap();

    assert!(repo.load_file_settings("v2").unwrap().is_none());
    assert_eq!(
        repo.load_file_settings("v1")
            .unwrap()
            .unwrap()
            .exposure
            .density,
        0.42
    );
}
