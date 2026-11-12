//! Ports tests/test_positive_auto_migration.py.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::domain::models::{ProcessMode, with_positive_source};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::positive_auto::migrate_auto_meter_for_positive_frames;
use negpy_backend::services::assets::rolls::{create_virtual_roll, set_roll_defaults};
use serde_json::{Map, Value};

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn positive_config() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::E6;
    with_positive_source(config, true)
}

fn save(repo: &StorageRepository, hash: &str, config: &WorkspaceConfig, path: &str) {
    repo.save_file_settings(hash, config, path).unwrap();
}

fn auto_values(repo: &StorageRepository, hash: &str) -> (bool, bool) {
    let config = repo.load_file_settings(hash).unwrap().unwrap();
    (
        config.exposure.auto_exposure,
        config.exposure.auto_normalize_contrast,
    )
}

#[test]
fn an_untouched_positive_frame_turns_both_toggles_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "h1", &positive_config(), "/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(auto_values(&repo, "h1"), (false, false));
}

#[test]
fn a_negative_frame_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::E6;
    save(&repo, "h1", &config, "/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(auto_values(&repo, "h1"), (true, true));
}

#[test]
fn a_deliberate_choice_on_a_positive_frame_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut config = positive_config();
    config.exposure.auto_exposure = false;
    config.exposure.auto_normalize_contrast = true;
    save(&repo, "h1", &config, "/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(auto_values(&repo, "h1"), (false, false));
}

#[test]
fn a_frame_inheriting_positive_from_its_roll_is_corrected() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = create_virtual_roll(&mut repo, "Roll", &["/roll/a.tif".to_string()]);
    let mut defaults = Map::new();
    defaults.insert("positive_source".to_string(), Value::Bool(true));
    set_roll_defaults(&mut repo, &roll_id, defaults);
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::E6;
    save(&repo, "h1", &config, "/roll/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(auto_values(&repo, "h1"), (false, false));
}

#[test]
fn a_frame_outside_any_positive_roll_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = create_virtual_roll(&mut repo, "Roll", &["/roll/a.tif".to_string()]);
    let mut defaults = Map::new();
    defaults.insert("positive_source".to_string(), Value::Bool(true));
    set_roll_defaults(&mut repo, &roll_id, defaults);
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::E6;
    save(&repo, "h1", &config, "/elsewhere/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(auto_values(&repo, "h1"), (true, true));
}

#[test]
fn the_flag_is_set_with_no_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert_eq!(
        repo.global_setting("auto_meter_positive_migrated_v1")
            .unwrap(),
        Some(Value::Bool(true))
    );
}

#[test]
fn a_second_run_keeps_a_choice_made_after_the_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "h1", &positive_config(), "/a.tif");
    migrate_auto_meter_for_positive_frames(&mut repo);
    let mut config = repo.load_file_settings("h1").unwrap().unwrap();
    config.exposure.auto_exposure = true;
    save(&repo, "h1", &config, "/a.tif");

    migrate_auto_meter_for_positive_frames(&mut repo);

    assert!(auto_values(&repo, "h1").0);
}
