//! Ports of tests/test_rolls.py's fork rows.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::rolls::{
    ROLLS_KEY, add_frame_seed, clear_frame_overrides, copy_roll_settings, fork_edit,
    forked_edit_hashes, frame_baseline, frame_override_cards, frame_seed, is_forked,
    resolve_roll_config, roll_defaults, roll_edit_hash, saved_rolls, set_frame_override,
    set_roll_defaults, settle_card_lock, unfork_edit, unforked_hash,
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

#[test]
fn a_roll_default_overrides_an_unlocked_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    set_roll_defaults(&mut repo, &roll_id, map([("linear_raw", json!(true))]));
    let frame = WorkspaceConfig {
        process: ProcessConfig {
            linear_raw: false,
            ..ProcessConfig::default()
        },
        ..WorkspaceConfig::default()
    };

    let resolved = resolve_roll_config(&repo, Some(&roll_id), "h1", &frame);

    assert!(resolved.process.linear_raw);
    assert_eq!(roll_defaults(&repo, &roll_id)["linear_raw"], json!(true));
}

#[test]
fn a_locked_card_keeps_the_frames_own_value() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    set_roll_defaults(
        &mut repo,
        &roll_id,
        map([("linear_raw", json!(true)), ("hue_trim", json!(4.0))]),
    );
    set_frame_override(&mut repo, &roll_id, "h1", "demosaic", true, None);
    let frame = WorkspaceConfig {
        process: ProcessConfig {
            linear_raw: false,
            ..ProcessConfig::default()
        },
        ..WorkspaceConfig::default()
    };

    let resolved = resolve_roll_config(&repo, Some(&roll_id), "h1", &frame);

    assert!(!resolved.process.linear_raw);
    assert_eq!(resolved.process.hue_trim, 4.0);
    assert_eq!(
        frame_override_cards(&repo, &roll_id, "h1"),
        vec!["demosaic"]
    );

    clear_frame_overrides(&mut repo, &roll_id, "h1");
    assert!(
        resolve_roll_config(&repo, Some(&roll_id), "h1", &frame)
            .process
            .linear_raw
    );
}

#[test]
fn a_slide_roll_default_moves_the_cast_and_auto_meters() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Velvia");
    set_roll_defaults(
        &mut repo,
        &roll_id,
        map([
            ("process_mode", json!("Transparency")),
            ("positive_source", json!(true)),
        ]),
    );
    let frame = WorkspaceConfig::default();

    let resolved = resolve_roll_config(&repo, Some(&roll_id), "h1", &frame);

    assert_eq!(resolved.process.process_mode, ProcessMode::E6);
    assert!(resolved.process.positive_source);
    assert_eq!(resolved.exposure.cast_removal_strength, 0.0);
    assert!(!resolved.exposure.auto_exposure);
}

#[test]
fn a_camera_raw_white_stays_on_its_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "R");
    let scanner = map([
        ("fade_white_r", json!(-0.1)),
        ("fade_white_g", json!(-0.2)),
        ("fade_white_b", json!(-0.3)),
        ("fade_white_space", json!("")),
    ]);
    set_roll_defaults(
        &mut repo,
        &roll_id,
        map([("fade_ratio_g", json!(2.0))])
            .into_iter()
            .chain(scanner.clone())
            .collect(),
    );
    let raw = map([
        ("fade_ratio_g", json!(1.5)),
        ("fade_white_r", json!(-0.4)),
        ("fade_white_g", json!(-0.5)),
        ("fade_white_b", json!(-0.6)),
        ("fade_white_space", json!("abc123")),
    ]);
    set_roll_defaults(&mut repo, &roll_id, raw);

    let defaults = roll_defaults(&repo, &roll_id);

    assert_eq!(defaults["fade_ratio_g"], json!(1.5));
    assert_eq!(defaults["fade_white_r"], json!(-0.1));
    let measured = WorkspaceConfig {
        process: ProcessConfig {
            fade_white_r: -0.7,
            fade_white_space: "abc123".into(),
            ..ProcessConfig::default()
        },
        ..WorkspaceConfig::default()
    };
    let resolved = resolve_roll_config(&repo, Some(&roll_id), "h1", &measured);
    assert_eq!(resolved.process.fade_ratio_g, 1.5);
    assert_eq!(resolved.process.fade_white_r, -0.7);
}

#[test]
fn a_lock_keeps_its_first_baseline_until_it_unlocks() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    let first = map([("linear_raw", json!(true))]);
    set_frame_override(&mut repo, &roll_id, "h1", "demosaic", true, Some(first));
    set_frame_override(
        &mut repo,
        &roll_id,
        "h1",
        "demosaic",
        true,
        Some(map([("linear_raw", json!(false))])),
    );
    assert_eq!(
        frame_baseline(&repo, &roll_id, "h1", "demosaic")["linear_raw"],
        json!(true)
    );

    set_frame_override(&mut repo, &roll_id, "h1", "demosaic", false, None);
    assert!(frame_baseline(&repo, &roll_id, "h1", "demosaic").is_empty());
    assert!(
        saved_rolls(&repo)[&roll_id]
            .get("frame_baselines")
            .is_none()
    );
}

#[test]
fn copying_a_roll_takes_its_defaults_seed_and_named_locks() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let source = virtual_roll(&mut repo, "Portra");
    let mut store = saved_rolls(&repo);
    store.insert(
        "picks".into(),
        json!({"kind": "virtual", "member_paths": []}),
    );
    let mut values = Map::new();
    values.insert(ROLLS_KEY.into(), Value::Object(store));
    repo.save_global_settings(&values).unwrap();
    set_roll_defaults(
        &mut repo,
        &source,
        map([("film_stock_id", json!("portra400"))]),
    );
    add_frame_seed(
        &mut repo,
        &source,
        map([("cast_removal_strength", json!(0.5))]),
    );
    set_frame_override(&mut repo, &source, "h1", "film", true, None);
    set_frame_override(&mut repo, &source, "h2", "film", true, None);

    copy_roll_settings(&mut repo, &source, "picks", &["h1".to_string()]);
    let target = &saved_rolls(&repo)["picks"];

    assert_eq!(target["defaults"], json!({"film_stock_id": "portra400"}));
    assert_eq!(target["frame_seed"], json!({"cast_removal_strength": 0.5}));
    assert_eq!(target["frame_overrides"], json!({"h1": ["film"]}));
    assert_eq!(
        frame_seed(&repo, "picks")["cast_removal_strength"],
        json!(0.5)
    );
}

#[test]
fn a_card_locks_when_its_values_diverge_and_unlocks_when_they_match() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = virtual_roll(&mut repo, "Portra");
    set_roll_defaults(&mut repo, &roll_id, map([("linear_raw", json!(true))]));

    settle_card_lock(
        &mut repo,
        &roll_id,
        "h1",
        "demosaic",
        &map([("linear_raw", json!(false))]),
        None,
        None,
    );
    assert_eq!(
        frame_override_cards(&repo, &roll_id, "h1"),
        vec!["demosaic"]
    );

    settle_card_lock(
        &mut repo,
        &roll_id,
        "h1",
        "demosaic",
        &map([("linear_raw", json!(true))]),
        None,
        None,
    );
    assert!(frame_override_cards(&repo, &roll_id, "h1").is_empty());
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}
