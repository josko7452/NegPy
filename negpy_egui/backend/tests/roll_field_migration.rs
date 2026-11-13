//! Ports the new-field lock rows of tests/test_roll_field_migration.py.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::roll_fields::migrate_new_roll_field_locks;
use negpy_backend::services::assets::rolls::{
    create_virtual_roll, frame_override_cards, roll_edit_hash,
};
use serde_json::Value;

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn crop(rect: Option<Vec<f64>>, auto: bool) -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.geometry.crop_rect = rect;
    config.geometry.crop_from_auto = auto;
    config
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

#[test]
fn a_frame_that_followed_its_roll_keeps_the_rolls_cast_removal() {
    use negpy_backend::services::assets::migrations::roll_fields::migrate_cast_removal_to_frames;
    use negpy_backend::services::assets::rolls::{
        frame_seed, roll_defaults, set_frame_override, set_roll_defaults,
    };

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(
        &mut repo,
        "Roll",
        &["/r/a.tif", "/r/b.tif", "/r/c.tif"].map(str::to_string),
    );
    for name in ["a", "b"] {
        save(
            &repo,
            name,
            &WorkspaceConfig::default(),
            &format!("/r/{name}.tif"),
        );
    }
    let mut defaults = serde_json::Map::new();
    defaults.insert("cast_removal_strength".to_string(), serde_json::json!(0.4));
    set_roll_defaults(&mut repo, &roll, defaults);
    set_frame_override(&mut repo, &roll, "b", "cast_removal", true, None);
    set_frame_override(&mut repo, &roll, "b", "sensor", true, None);

    migrate_cast_removal_to_frames(&mut repo);
    let mut later = serde_json::Map::new();
    later.insert("cast_removal_strength".to_string(), serde_json::json!(0.7));
    set_roll_defaults(&mut repo, &roll, later);
    migrate_cast_removal_to_frames(&mut repo);

    assert_eq!(
        repo.load_file_settings("a")
            .unwrap()
            .unwrap()
            .exposure
            .cast_removal_strength,
        0.4
    );
    assert_eq!(
        repo.load_file_settings("b")
            .unwrap()
            .unwrap()
            .exposure
            .cast_removal_strength,
        WorkspaceConfig::default().exposure.cast_removal_strength
    );
    assert_eq!(cards(&repo, &roll, "b"), ["sensor"]);
    assert_eq!(
        roll_defaults(&repo, &roll).get("cast_removal_strength"),
        Some(&serde_json::json!(0.7))
    );
    assert_eq!(
        frame_seed(&repo, &roll).get("cast_removal_strength"),
        Some(&serde_json::json!(0.4))
    );
}

#[test]
fn section_pushes_are_dropped_once() {
    use negpy_backend::services::assets::migrations::roll_fields::migrate_drop_section_pushes;
    use negpy_backend::services::assets::rolls::{ROLLS_KEY, roll_for_id, saved_rolls};

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(&mut repo, "Roll", &["/r/a.tif".to_string()]);
    let mut store = saved_rolls(&repo);
    store[&roll].as_object_mut().unwrap().insert(
        "section_pushes".to_string(),
        serde_json::json!({"tone": {"density": 1.2}}),
    );
    let mut values = serde_json::Map::new();
    values.insert(ROLLS_KEY.to_string(), Value::Object(store));
    repo.save_global_settings(&values).unwrap();

    migrate_drop_section_pushes(&mut repo);
    let mut store = saved_rolls(&repo);
    store[&roll].as_object_mut().unwrap().insert(
        "section_pushes".to_string(),
        serde_json::json!({"tone": {"density": 1.2}}),
    );
    let mut values = serde_json::Map::new();
    values.insert(ROLLS_KEY.to_string(), Value::Object(store));
    repo.save_global_settings(&values).unwrap();
    migrate_drop_section_pushes(&mut repo);

    assert_eq!(
        roll_for_id(&repo, &roll)
            .unwrap()
            .get("section_pushes")
            .cloned(),
        Some(serde_json::json!({"tone": {"density": 1.2}})),
        "runs once"
    );
    let mut values = serde_json::Map::new();
    values.insert("section_pushes_dropped_v1".to_string(), Value::Bool(false));
    repo.save_global_settings(&values).unwrap();
    migrate_drop_section_pushes(&mut repo);
    assert!(
        roll_for_id(&repo, &roll)
            .unwrap()
            .get("section_pushes")
            .is_none()
    );
}

#[test]
fn a_calibration_lock_becomes_both_halves_of_the_raw_decode_split() {
    use negpy_backend::services::assets::migrations::roll_fields::migrate_linear_raw_card_split;
    use negpy_backend::services::assets::rolls::set_frame_override;

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(&mut repo, "Roll", &["/r/a.tif".to_string()]);
    set_frame_override(&mut repo, &roll, "a", "sensor", true, None);

    migrate_linear_raw_card_split(&mut repo);

    assert_eq!(cards(&repo, &roll, "a"), ["demosaic", "sensor"]);
}

#[test]
fn auto_crop_locks_only_a_frame_saved_with_it_on() {
    use negpy_backend::domain::models::workspace::WorkspaceConfig;
    use negpy_backend::services::assets::migrations::roll_fields::migrate_auto_crop_roll_locks;

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(
        &mut repo,
        "Roll",
        &["/r/a.tif", "/r/b.tif", "/r/c.tif"].map(str::to_string),
    );

    let mut a = WorkspaceConfig::default();
    a.geometry.crop_rect = Some(vec![0.1, 0.1, 0.9, 0.9]);
    a.geometry.crop_from_auto = true;
    let mut b = WorkspaceConfig::default();
    b.geometry.crop_rect = Some(vec![0.1, 0.1, 0.9, 0.9]);
    save(&repo, "a", &a, "/r/a.tif");
    save(&repo, "b", &b, "/r/b.tif");
    save(&repo, "c", &WorkspaceConfig::default(), "/r/c.tif");

    migrate_auto_crop_roll_locks(&mut repo);

    assert_eq!(cards(&repo, &roll, "a"), ["auto_crop"]);
    assert!(cards(&repo, &roll, "b").is_empty());
    assert!(cards(&repo, &roll, "c").is_empty());
}

#[test]
fn each_frame_keeps_the_crop_its_roll_gave_it() {
    use negpy_backend::domain::models::AspectRatio;
    use negpy_backend::services::assets::migrations::roll_fields::migrate_crop_to_frames;
    use negpy_backend::services::assets::rolls::{
        roll_defaults, set_frame_override, set_roll_defaults,
    };

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let rect = vec![0.1, 0.1, 0.9, 0.9];
    let roll = create_virtual_roll(
        &mut repo,
        "Roll",
        &["/r/a.tif", "/r/b.tif", "/r/c.tif", "/r/d.tif", "/r/e.tif"].map(str::to_string),
    );
    save(&repo, "a", &crop(None, false), "/r/a.tif");
    save(&repo, "b", &crop(Some(rect.clone()), false), "/r/b.tif");
    let mut c = crop(None, false);
    c.geometry.autocrop_ratio = AspectRatio::R5x4;
    save(&repo, "c", &c, "/r/c.tif");
    save(&repo, "d", &crop(Some(rect.clone()), true), "/r/d.tif");

    let mut defaults = serde_json::Map::new();
    defaults.insert("crop_from_auto".to_string(), serde_json::json!(true));
    defaults.insert("autocrop_ratio".to_string(), serde_json::json!("3:2"));
    defaults.insert("autocrop_offset".to_string(), serde_json::json!(8));
    defaults.insert("hue_trim".to_string(), serde_json::json!(2.0));
    set_roll_defaults(&mut repo, &roll, defaults);
    set_frame_override(&mut repo, &roll, "c", "autocrop", true, None);
    set_frame_override(&mut repo, &roll, "d", "auto_crop", true, None);
    set_frame_override(&mut repo, &roll, "d", "sensor", true, None);

    migrate_crop_to_frames(&mut repo);

    let a = repo.load_file_settings("a").unwrap().unwrap().geometry;
    assert!(a.crop_from_auto);
    assert_eq!(a.crop_rect, None);
    assert_eq!(a.autocrop_ratio, AspectRatio::R3x2);
    assert_eq!(a.autocrop_offset, 8);
    let b = repo.load_file_settings("b").unwrap().unwrap().geometry;
    assert!(!b.crop_from_auto);
    assert_eq!(b.crop_rect, Some(rect.clone()));
    assert_eq!(b.autocrop_ratio, AspectRatio::R3x2);
    let c = repo.load_file_settings("c").unwrap().unwrap().geometry;
    assert!(c.crop_from_auto);
    assert_eq!(c.autocrop_ratio, AspectRatio::R5x4);
    assert_eq!(c.autocrop_offset, 0);
    let d = repo.load_file_settings("d").unwrap().unwrap().geometry;
    assert!(d.crop_from_auto);
    assert_eq!(d.crop_rect, Some(rect));
    assert!(repo.load_file_settings("e").unwrap().is_none());
    assert_eq!(
        roll_defaults(&repo, &roll),
        [("hue_trim".to_string(), serde_json::json!(2.0))]
            .into_iter()
            .collect()
    );
    assert!(cards(&repo, &roll, "c").is_empty());
    assert_eq!(cards(&repo, &roll, "d"), ["sensor"]);
}

#[test]
fn the_crop_migration_runs_once() {
    use negpy_backend::domain::models::AspectRatio;
    use negpy_backend::services::assets::migrations::roll_fields::migrate_crop_to_frames;
    use negpy_backend::services::assets::rolls::set_roll_defaults;

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(&mut repo, "Roll", &["/r/a.tif".to_string()]);
    save(&repo, "a", &crop(None, false), "/r/a.tif");
    migrate_crop_to_frames(&mut repo);
    let mut defaults = serde_json::Map::new();
    defaults.insert("autocrop_ratio".to_string(), serde_json::json!("4:3"));
    set_roll_defaults(&mut repo, &roll, defaults);

    migrate_crop_to_frames(&mut repo);

    assert_eq!(
        repo.load_file_settings("a")
            .unwrap()
            .unwrap()
            .geometry
            .autocrop_ratio,
        AspectRatio::Free
    );
}

#[test]
fn a_roll_with_film_defaults_gets_auto_encoding() {
    use negpy_backend::services::assets::migrations::roll_fields::migrate_film_card_source_encoding;
    use negpy_backend::services::assets::rolls::{roll_defaults, set_roll_defaults};

    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll = create_virtual_roll(&mut repo, "Roll", &["/r/a.tif".to_string()]);
    let mut defaults = serde_json::Map::new();
    defaults.insert(
        "process_mode".to_string(),
        Value::String("Transparency".to_string()),
    );
    set_roll_defaults(&mut repo, &roll, defaults);

    migrate_film_card_source_encoding(&mut repo);

    assert_eq!(
        roll_defaults(&repo, &roll).get("source_encoding"),
        Some(&Value::String("auto".to_string()))
    );
}
