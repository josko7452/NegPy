//! Ports of tests/test_rolls.py's fork rows.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::rolls::{
    DISMISSED_FOLDERS_KEY, ROLLS_KEY, add_frame_seed, add_members, all_rolls_sorted,
    clear_frame_overrides, convert_to_folder_roll, copy_roll_settings, create_folder_roll,
    create_virtual_roll, delete_roll, discovery_filters, excluded_paths, folder_roll_id_for_path,
    folder_scope, forget_import_sources, fork_edit, forked_edit_hashes, frame_baseline,
    frame_override_cards, frame_roll, frame_seed, home_roll, import_sources,
    import_subfolders_as_rolls, is_excluded_key, is_forked, matches_discovery_filter,
    next_roll_name, outside_folder, prune_filtered_rolls, recognize_folder, remove_members,
    resolve_roll_config, roll_defaults, roll_edit_hash, roll_folder_name, roll_for_id,
    rolls_containing_path, saved_rolls, set_discovery_filters, set_frame_override,
    set_roll_defaults, settle_card_lock, unfork_edit, unforked_hash, virtual_rolls,
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

fn folder_roll(repo: &mut StorageRepository, id: &str, folder: &str) {
    let mut store = saved_rolls(repo);
    store.insert(
        id.into(),
        json!({"kind": "folder", "name": "roll_a", "folder_path": folder, "extra_paths": []}),
    );
    let mut values = Map::new();
    values.insert(ROLLS_KEY.into(), Value::Object(store));
    repo.save_global_settings(&values).unwrap();
}

#[test]
fn a_virtual_roll_stores_its_member_list_and_extends_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let roll_id = create_virtual_roll(
        &mut repo,
        "Portra",
        &["/a.nef".to_string(), "/b.nef".to_string()],
    );

    add_members(
        &mut repo,
        &roll_id,
        &["/b.nef".to_string(), "/c.nef".to_string()],
        &[],
    );

    let entry = roll_for_id(&repo, &roll_id).unwrap();
    assert_eq!(entry["kind"], json!("virtual"));
    assert_eq!(entry["name"], json!("Portra"));
    assert_eq!(entry["member_paths"], json!(["/a.nef", "/b.nef", "/c.nef"]));
}

#[test]
fn membership_answers_by_path_and_prefers_a_folder_roll() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("roll_a");
    let frame = folder.join("frame001.tif");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(&frame, b"x").unwrap();
    folder_roll(&mut repo, "folder", &folder.display().to_string());
    let virtual_id = create_virtual_roll(&mut repo, "Picks", &[frame.display().to_string()]);

    let path = frame.display().to_string();
    let mut containing = rolls_containing_path(&repo, &path);
    containing.sort();
    let mut expected = vec!["folder".to_string(), virtual_id];
    expected.sort();

    assert_eq!(containing, expected);
    assert_eq!(home_roll(&repo, &path), Some("folder".to_string()));
    assert_eq!(frame_roll(&repo, None, &path), Some("folder".to_string()));
    assert_eq!(
        frame_roll(&repo, Some("active"), "/elsewhere.tif"),
        Some("active".to_string())
    );
}

#[test]
fn a_subfolder_is_not_a_member_of_its_parents_roll() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("roll_a");
    let sub = folder.join("picks");
    std::fs::create_dir_all(&sub).unwrap();
    let file = sub.join("p1.tif");
    std::fs::write(&file, b"x").unwrap();
    folder_roll(&mut repo, "parent", &folder.display().to_string());
    assert!(rolls_containing_path(&repo, &file.display().to_string()).is_empty());
    folder_roll(&mut repo, "picks", &sub.display().to_string());

    assert_eq!(
        rolls_containing_path(&repo, &file.display().to_string()),
        vec!["picks".to_string()]
    );
    assert_eq!(
        rolls_containing_path(&repo, &folder.join("a1.tif").display().to_string()),
        vec!["parent".to_string()]
    );
}

#[test]
fn removing_a_folder_file_adds_an_exclusion_and_returns_its_forks() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("roll_a");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("frame001.tif");
    std::fs::write(&path, b"x").unwrap();
    let path = path.display().to_string();
    folder_roll(&mut repo, "roll", &folder.display().to_string());
    set_frame_override(&mut repo, "roll", "h1", "film", true, None);
    fork_edit(&mut repo, "roll", "h1", &path, &WorkspaceConfig::default()).unwrap();

    let forked = remove_members(&mut repo, "roll", &[path.clone()], &["h1".to_string()]);

    assert_eq!(forked, vec!["h1"]);
    let entry = roll_for_id(&repo, "roll").unwrap();
    assert!(entry.get("frame_overrides").is_none());
    assert!(
        entry["excluded_paths"]
            .as_array()
            .unwrap()
            .contains(&json!(path))
    );
    let excluded = excluded_paths(&repo, &[folder.display().to_string()]);
    assert!(is_excluded_key(&path, &excluded));
    assert!(rolls_containing_path(&repo, &path).is_empty());
}

#[test]
fn deleting_a_folder_roll_dismisses_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("roll_a");
    std::fs::create_dir_all(&folder).unwrap();
    folder_roll(&mut repo, "roll", &folder.display().to_string());

    delete_roll(&mut repo, "roll");

    assert!(roll_for_id(&repo, "roll").is_none());
    assert_eq!(
        repo.global_setting(DISMISSED_FOLDERS_KEY).unwrap().unwrap(),
        json!([folder.display().to_string()])
    );
}

#[test]
fn rolls_sort_by_name_and_virtual_lists_only_virtual_ones() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    folder_roll(&mut repo, "folder", "/scans/roll_a");
    create_virtual_roll(&mut repo, "Zebra", &[]);
    create_virtual_roll(&mut repo, "apple", &[]);

    let all: Vec<String> = all_rolls_sorted(&repo)
        .into_iter()
        .map(|(_, entry)| entry["name"].as_str().unwrap().to_string())
        .collect();
    let virtual_names: Vec<String> = virtual_rolls(&repo)
        .into_iter()
        .map(|(_, entry)| entry["name"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(all, vec!["apple", "roll_a", "Zebra"]);
    assert_eq!(virtual_names, vec!["apple", "Zebra"]);
}

fn image_folder(path: &std::path::Path) -> &std::path::Path {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("f1.tif"), b"x").unwrap();
    path
}

#[test]
fn recognize_folder_is_idempotent_and_names_from_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let first = recognize_folder(&mut repo, "/scans/2024-10-portra", None);
    let second = recognize_folder(&mut repo, "/scans/2024-10-portra", None);
    let entry = roll_for_id(&repo, &first).unwrap();

    assert_eq!(first, second);
    assert_eq!(entry["kind"], json!("folder"));
    assert_eq!(entry["name"], json!("2024-10-portra"));
    assert_eq!(entry["extra_paths"], json!([]));
    assert_eq!(
        folder_roll_id_for_path(&repo, "/scans/2024-10-portra"),
        Some(first)
    );
}

#[test]
fn recognizing_an_under_an_import_source_names_it_from_that_source() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("scans");
    image_folder(&parent.join("old"));
    let mut repo = repository(dir.path());
    import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), false);
    let roll_id = recognize_folder(
        &mut repo,
        &parent.join("2026/new").display().to_string(),
        None,
    );

    assert_eq!(
        roll_for_id(&repo, &roll_id).unwrap()["name"],
        json!("scans/2026/new")
    );
}

#[test]
fn discovery_filters_default_to_export_and_replace_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    assert_eq!(discovery_filters(&repo), vec!["export"]);

    set_discovery_filters(
        &mut repo,
        &["  raw_* ".to_string(), String::new(), "tmp".to_string()],
    );

    assert_eq!(discovery_filters(&repo), vec!["raw_*", "tmp"]);
    assert!(matches_discovery_filter("Exports_TIFF", &["export".into()]));
    assert!(matches_discovery_filter("raw_scans", &["raw_*".into()]));
    assert!(!matches_discovery_filter("my_raw_scans", &["raw_*".into()]));
}

#[test]
fn discovery_walks_nested_folders_but_not_into_a_roll_folder() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("scans");
    image_folder(&parent.join("2024/roll_a"));
    image_folder(&parent.join("2024/summer/roll_b"));
    image_folder(&parent.join("roll_a"));
    image_folder(&parent.join("roll_a/TIFF"));
    std::fs::create_dir_all(parent.join(".hidden")).unwrap();
    let mut repo = repository(dir.path());

    let rolls = import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), false);
    let names: Vec<String> = rolls
        .iter()
        .map(|roll_id| {
            roll_for_id(&repo, roll_id).unwrap()["name"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert_eq!(
        names,
        vec![
            "scans/2024/roll_a",
            "scans/2024/summer/roll_b",
            "scans/roll_a"
        ]
    );
    assert!(
        folder_roll_id_for_path(&repo, &parent.join("roll_a/TIFF").display().to_string()).is_none()
    );
    assert_eq!(import_sources(&repo), vec![parent.display().to_string()]);
}

#[test]
fn importing_the_picked_folder_names_a_roll_by_its_own_name() {
    let dir = tempfile::tempdir().unwrap();
    let picked = dir.path().join("kentmere_400_1");
    image_folder(&picked);
    let mut repo = repository(dir.path());

    let rolls = import_subfolders_as_rolls(&mut repo, &picked.display().to_string(), false);

    assert_eq!(rolls.len(), 1);
    assert_eq!(
        roll_for_id(&repo, &rolls[0]).unwrap()["name"],
        json!("kentmere_400_1")
    );
}

#[test]
fn a_dismissed_folder_is_skipped_until_recognized_by_hand() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("scans");
    let roll_path = parent.join("roll_a");
    let roll = image_folder(&roll_path);
    image_folder(&parent.join("roll_b"));
    let mut repo = repository(dir.path());
    import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), false);
    let roll_id = folder_roll_id_for_path(&repo, &roll.display().to_string()).unwrap();
    delete_roll(&mut repo, &roll_id);

    import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), true);
    assert!(folder_roll_id_for_path(&repo, &roll.display().to_string()).is_none());
    let recognized = recognize_folder(&mut repo, &roll.display().to_string(), None);
    let imported = import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), true);
    assert!(imported.contains(&recognized));
    assert_eq!(saved_rolls(&repo).len(), 2);
}

#[test]
fn discovery_filters_exclude_nested_rolls_and_prune_keeps_them_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    image_folder(&dir.path().join("roll_a"));
    image_folder(&dir.path().join("Export/nested_roll"));
    let mut repo = repository(dir.path());
    import_subfolders_as_rolls(&mut repo, &dir.path().display().to_string(), false);
    assert_eq!(saved_rolls(&repo).len(), 1);

    let scratch = dir.path().join("scratch");
    image_folder(&scratch.join("roll_b"));
    import_subfolders_as_rolls(&mut repo, &dir.path().display().to_string(), false);
    assert_eq!(saved_rolls(&repo).len(), 2);
    set_discovery_filters(&mut repo, &["scratch".to_string()]);
    assert_eq!(prune_filtered_rolls(&mut repo), 1);
    assert_eq!(saved_rolls(&repo).len(), 1);
}

#[test]
fn forgetting_a_source_keeps_one_a_remaining_roll_sits_under() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    for source in ["a/day", "b/day", "c/day"] {
        let parent = dir.path().join(source);
        image_folder(&parent.join("roll"));
        import_subfolders_as_rolls(&mut repo, &parent.display().to_string(), false);
    }
    let c_prefix = dir.path().join("c").display().to_string();
    let stale: Vec<String> = saved_rolls(&repo)
        .into_iter()
        .filter(|(_, entry)| {
            !entry["folder_path"]
                .as_str()
                .unwrap()
                .starts_with(&c_prefix)
        })
        .map(|(roll_id, _)| roll_id)
        .collect();
    for roll_id in stale {
        delete_roll(&mut repo, &roll_id);
    }

    forget_import_sources(
        &mut repo,
        &[
            dir.path().join("a").display().to_string(),
            dir.path().join("b/day").display().to_string(),
            dir.path().join("c").display().to_string(),
        ],
    );

    assert_eq!(
        import_sources(&repo),
        vec![dir.path().join("c/day").display().to_string()]
    );
}

#[test]
fn folder_roll_names_and_counters_match_the_python_rules() {
    assert_eq!(roll_folder_name(""), Some("Roll001".into()));
    assert_eq!(
        roll_folder_name("  Summer 2026  "),
        Some("Summer 2026".into())
    );
    assert_eq!(roll_folder_name(".."), None);
    assert_eq!(roll_folder_name("a/b"), None);
    assert_eq!(roll_folder_name("a\\b"), None);
    assert_eq!(next_roll_name("Roll001", |_| false), "Roll002");
    assert_eq!(next_roll_name("portra400", |_| false), "portra400_2");
    assert_eq!(next_roll_name("portra400_1", |_| false), "portra400_2");
    assert_eq!(next_roll_name("hp5-9", |_| false), "hp5-10");
    assert_eq!(next_roll_name("Roll 7", |_| false), "Roll 8");
    assert_eq!(next_roll_name("12", |_| false), "13");
    assert_eq!(
        next_roll_name("Roll001", |name| ["Roll002", "Roll003"].contains(&name)),
        "Roll004"
    );
}

#[test]
fn folder_scope_finds_the_one_folder_and_the_files_it_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("500T");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    for name in ["a1.tif", "a2.tif", "a3.tif", "a3_IR.tif"] {
        std::fs::write(folder.join(name), b"1").unwrap();
    }
    std::fs::write(folder.join("sub/s.tif"), b"1").unwrap();
    let a1 = folder.join("a1.tif").display().to_string();
    let a2 = folder.join("a2.tif").display().to_string();

    let (found, mut missing) = folder_scope(&[a1.clone(), a2.clone()]);

    assert_eq!(found.unwrap(), folder.display().to_string());
    missing.sort();
    assert_eq!(missing, vec![folder.join("a3.tif").display().to_string()]);
    assert_eq!(
        folder_scope(&[a1, folder.join("sub/s.tif").display().to_string()]),
        (None, Vec::new())
    );
    assert_eq!(folder_scope(&[]), (None, Vec::new()));
}

#[test]
fn a_folder_roll_leaves_out_the_files_it_is_created_with() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("500T");
    let excluded = folder.join("a3.tif").display().to_string();

    let roll_id = create_folder_roll(
        &mut repo,
        &folder.display().to_string(),
        &[excluded.clone()],
    );

    let entry = roll_for_id(&repo, &roll_id).unwrap();
    assert_eq!(entry["kind"], json!("folder"));
    assert_eq!(entry["name"], json!("500T"));
    assert_eq!(entry["excluded_paths"], json!([excluded]));
}

#[test]
fn converting_a_virtual_roll_keeps_its_id_and_records() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("500T");
    std::fs::create_dir_all(&folder).unwrap();
    let roll_id = create_virtual_roll(
        &mut repo,
        "Picks",
        &[folder.join("a1.tif").display().to_string()],
    );
    set_roll_defaults(
        &mut repo,
        &roll_id,
        map([("film_stock_id", json!("portra400"))]),
    );
    set_frame_override(&mut repo, &roll_id, "h1", "film", true, None);
    let before = roll_for_id(&repo, &roll_id).unwrap();

    let name = convert_to_folder_roll(
        &mut repo,
        &roll_id,
        &folder.display().to_string(),
        &[folder.join("a2.tif").display().to_string()],
    );

    let entry = roll_for_id(&repo, &roll_id).unwrap();
    assert_eq!(name, "500T");
    assert_eq!(entry["kind"], json!("folder"));
    assert_eq!(entry["folder_path"], json!(folder.display().to_string()));
    assert_eq!(
        entry["excluded_paths"],
        json!([folder.join("a2.tif").display().to_string()])
    );
    assert!(entry.get("member_paths").is_none());
    assert_eq!(entry["defaults"], before["defaults"]);
    assert_eq!(entry["frame_overrides"], before["frame_overrides"]);
    assert_eq!(
        folder_roll_id_for_path(&repo, &folder.display().to_string()),
        Some(roll_id.clone())
    );
    assert_eq!(convert_to_folder_roll(&mut repo, &roll_id, "/x", &[]), "");
}

#[test]
fn outside_folder_names_the_files_a_folder_roll_cannot_hold() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let folder = dir.path().join("roll_a");
    let roll_id = recognize_folder(&mut repo, &folder.display().to_string(), None);
    let inside = folder.join("1.tif").display().to_string();
    let nested = folder.join("sub/2.tif").display().to_string();
    let outside = "/elsewhere/c.nef".to_string();

    assert_eq!(
        outside_folder(&repo, &roll_id, &[inside, nested.clone(), outside.clone()]),
        vec![nested, outside]
    );
    let virtual_id = create_virtual_roll(&mut repo, "Picks", &[]);
    assert!(outside_folder(&repo, &virtual_id, &["/a.tif".into()]).is_empty());
}
