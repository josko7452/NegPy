//! Ports the new-field lock rows of tests/test_roll_field_migration.py.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::roll_fields::migrate_new_roll_field_locks;
use negpy_backend::services::assets::rolls::{
    create_virtual_roll, frame_override_cards, roll_edit_hash,
};

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn save(repo: &StorageRepository, hash: &str, config: &WorkspaceConfig, path: &str) {
    repo.save_file_settings(hash, config, path).unwrap();
}

fn cards(repo: &StorageRepository, roll_id: &str, hash: &str) -> Vec<String> {
    frame_override_cards(repo, roll_id, hash)
}

#[test]
fn only_the_cards_with_changed_fields_are_locked() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(
        &mut repo,
        "Roll",
        &["/r/a.tif", "/r/b.tif", "/r/c.tif"].map(str::to_string),
    );
    let mut a = WorkspaceConfig::default();
    a.process.white_point_trim_red = 0.02;
    let mut b = WorkspaceConfig::default();
    b.process.highlight_reconstruction = 2;
    let mut c = WorkspaceConfig::default();
    c.process.analysis_buffer = 0.1;
    save(&repo, "a", &a, "/r/a.tif");
    save(&repo, "b", &b, "/r/b.tif");
    save(&repo, "c", &c, "/r/c.tif");

    migrate_new_roll_field_locks(&mut repo);

    assert_eq!(cards(&repo, &roll, "a"), ["process"]);
    assert_eq!(cards(&repo, &roll, "b"), ["demosaic"]);
    assert!(cards(&repo, &roll, "c").is_empty());
}

#[test]
fn a_shared_edit_locks_every_roll_that_holds_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let first = create_virtual_roll(&mut repo, "One", &["/r/a.tif".to_string()]);
    let second = create_virtual_roll(&mut repo, "Two", &["/r/a.tif".to_string()]);
    let mut config = WorkspaceConfig::default();
    config.process.black_point_offset = -0.03;
    save(&repo, "a", &config, "/r/a.tif");

    migrate_new_roll_field_locks(&mut repo);

    assert_eq!(cards(&repo, &first, "a"), ["process"]);
    assert_eq!(cards(&repo, &second, "a"), ["process"]);
}

#[test]
fn a_fork_locks_only_its_own_roll_under_the_unforked_hash() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let own = create_virtual_roll(&mut repo, "Own", &["/r/a.tif".to_string()]);
    let other = create_virtual_roll(&mut repo, "Other", &["/r/a.tif".to_string()]);
    let mut config = WorkspaceConfig::default();
    config.process.white_point_offset = 0.05;
    save(&repo, &roll_edit_hash("a", &own), &config, "/r/a.tif");

    migrate_new_roll_field_locks(&mut repo);

    assert_eq!(cards(&repo, &own, "a"), ["process"]);
    assert!(cards(&repo, &other, "a").is_empty());
}

#[test]
fn the_migration_runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(&mut repo, "Roll", &["/r/a.tif".to_string()]);
    migrate_new_roll_field_locks(&mut repo);
    let mut config = WorkspaceConfig::default();
    config.process.white_point_offset = 0.05;
    save(&repo, "a", &config, "/r/a.tif");

    migrate_new_roll_field_locks(&mut repo);

    assert!(cards(&repo, &roll, "a").is_empty());
}

#[test]
fn a_normalization_lock_becomes_both_halves_of_the_split() {
    use negpy_backend::services::assets::migrations::roll_fields::migrate_baseline_card_split;
    use negpy_backend::services::assets::rolls::set_frame_override;

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(
        &mut repo,
        "Roll",
        &["/r/a.tif", "/r/b.tif"].map(str::to_string),
    );
    set_frame_override(&mut repo, &roll, "a", "process", true, None);
    set_frame_override(&mut repo, &roll, "b", "lens", true, None);

    migrate_baseline_card_split(&mut repo);
    migrate_baseline_card_split(&mut repo);

    assert_eq!(cards(&repo, &roll, "a"), ["baseline", "process"]);
    assert_eq!(cards(&repo, &roll, "b"), ["lens"]);
}
