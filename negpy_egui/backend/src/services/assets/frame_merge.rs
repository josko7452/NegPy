//! Carries an assembled frame's edit onto the TIFF merged from it.
//!
//! Mirrors `negpy/services/assets/frame_merge.py`: the merged file keeps the edit, card
//! locks, scene and roll membership, but loses the corrections it already bakes.

use std::path::Path;

use crate::domain::models::{ProcessConfig, RgbScanConfig, StitchConfig, WorkspaceConfig};
use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::rolls::{
    self, resolve_roll_config, roll_defaults, roll_edit_hash, rolls_containing_path,
    set_frame_override,
};
use crate::services::assets::sidecar::{sidecar_path_for, write_sidecar};

/// The cards a merged file bakes, per kind. A lock keeps an Apply from re-applying them.
#[must_use]
pub fn baked_cards(kind: &str) -> &'static [&'static str] {
    match kind {
        "rgb" => &["sensor"],
        "stitch" => &["sensor", "flatfield"],
        _ => &[],
    }
}

/// The frame's edit for its merged TIFF: no triplet inputs and no baked corrections.
#[must_use]
pub fn merged_edit(config: &WorkspaceConfig, kind: &str) -> WorkspaceConfig {
    let mut out = config.clone();
    out.rgbscan = RgbScanConfig::default();
    out.process = ProcessConfig {
        sensor_matrix: None,
        sensor_profile: "None".to_string(),
        ..config.process.clone()
    };
    if kind == "stitch" {
        out.stitch = StitchConfig::default();
        out.flatfield = crate::domain::models::FlatFieldConfig::default();
    }
    out
}

/// Whether a roll's defaults would re-apply a baked card on the merged frame.
#[must_use]
pub fn roll_would_reapply(
    defaults: &serde_json::Map<String, serde_json::Value>,
    card: &str,
) -> bool {
    match card {
        "sensor" => defaults
            .get("sensor_matrix")
            .is_some_and(|value| !value.is_null()),
        "flatfield" => defaults.get("apply").and_then(serde_json::Value::as_bool) == Some(true),
        _ => false,
    }
}

/// Copy the frame's edit, forks, card locks, scene and roll membership to the merged
/// file. `config` is the hydrated edit; `keep_source` keeps the source frame in its rolls.
pub fn carry_edit(
    repo: &mut StorageRepository,
    old_hash: &str,
    old_path: &str,
    new_hash: &str,
    new_path: &str,
    dropped_paths: &[String],
    config: &WorkspaceConfig,
    kind: &str,
    keep_source: bool,
) {
    let transform = |value: &WorkspaceConfig| merged_edit(value, kind);
    let owns_new = repo
        .copy_file_edits(old_hash, new_hash, new_path, &transform)
        .unwrap_or(false);
    let roll_ids = rolls::adopt_replacement(
        repo,
        old_hash,
        new_hash,
        old_path,
        new_path,
        dropped_paths,
        keep_source,
    );
    for roll_id in &roll_ids {
        let from = roll_edit_hash(old_hash, roll_id);
        let to = roll_edit_hash(new_hash, roll_id);
        let _ = repo.copy_file_edits(&from, &to, new_path, &transform);
    }
    let member_rolls = rolls_containing_path(repo, new_path);
    // A lock freezes the whole card, so each field must hold the roll's value. An
    // unopened composite needs the row too, or it hydrates with the rig's flat field on.
    if owns_new {
        if let Some(roll_id) = member_rolls.first() {
            let resolved = resolve_roll_config(repo, Some(roll_id), old_hash, config);
            let _ = repo.save_file_settings(new_hash, &merged_edit(&resolved, kind), new_path);
        } else {
            let _ = repo.save_file_settings(new_hash, &merged_edit(config, kind), new_path);
        }
    }
    for roll_id in &member_rolls {
        let defaults = roll_defaults(repo, roll_id);
        for card in baked_cards(kind) {
            if roll_would_reapply(&defaults, card) {
                set_frame_override(repo, roll_id, new_hash, card, true, None);
            }
        }
    }
}

/// Rewrite the merged file's sidecar when its source had one.
#[must_use]
pub fn carry_sidecar(
    old_path: &Path,
    new_path: &Path,
    config: &WorkspaceConfig,
    kind: &str,
) -> bool {
    if !sidecar_path_for(old_path, 0).exists() {
        return false;
    }
    write_sidecar(new_path, &merged_edit(config, kind), 0).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::FlatFieldConfig;

    #[test]
    fn a_merge_drops_the_triplet_inputs_and_the_baked_cards() {
        let mut config = WorkspaceConfig::default();
        config.process.sensor_profile = "Rig".to_string();
        config.process.sensor_matrix = Some(vec![1.0; 9]);
        config.flatfield = FlatFieldConfig {
            apply: true,
            ..FlatFieldConfig::default()
        };

        let rgb = merged_edit(&config, "rgb");
        assert_eq!(rgb.process.sensor_profile, "None");
        assert!(rgb.process.sensor_matrix.is_none());
        assert!(rgb.flatfield.apply, "an rgb merge keeps the flat field");

        let stitch = merged_edit(&config, "stitch");
        assert!(!stitch.flatfield.apply);
        assert!(!stitch.stitch.stitch_enabled);
    }

    #[test]
    fn a_roll_default_reapplies_only_the_card_it_sets() {
        let mut defaults = serde_json::Map::new();
        defaults.insert("sensor_matrix".into(), serde_json::json!([1.0]));
        defaults.insert("apply".into(), serde_json::json!(true));
        assert!(roll_would_reapply(&defaults, "sensor"));
        assert!(roll_would_reapply(&defaults, "flatfield"));
        assert!(!roll_would_reapply(&serde_json::Map::new(), "sensor"));
        assert!(!roll_would_reapply(&defaults, "lab"));
    }

    #[test]
    fn a_merge_copies_the_edit_and_its_roll_membership() {
        let temp = std::env::temp_dir().join(format!("negpy-merge-{}", std::process::id()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let mut repo = StorageRepository::new(
            temp.join("edits.db").to_string_lossy().into_owned(),
            temp.join("settings.db").to_string_lossy().into_owned(),
        );
        repo.initialize().expect("schema");

        let old_path = temp.join("old.tif").to_string_lossy().into_owned();
        let new_path = temp.join("merged.tif").to_string_lossy().into_owned();
        let mut config = WorkspaceConfig::default();
        config.process.sensor_profile = "Rig".to_string();
        repo.save_file_settings("old", &config, &old_path)
            .expect("save edit");
        repo.save_file_mark("old", Some("keeper"), &old_path)
            .expect("save mark");

        let mut defaults = serde_json::Map::new();
        defaults.insert("sensor_matrix".into(), serde_json::json!([1.0]));
        let roll_id = rolls::create_virtual_roll(&mut repo, "Merge test", &[]);
        rolls::add_members(&mut repo, &roll_id, &[old_path.clone()], &[]);
        rolls::set_roll_defaults(&mut repo, &roll_id, defaults);

        carry_edit(
            &mut repo,
            "old",
            &old_path,
            "new",
            &new_path,
            &[],
            &config,
            "rgb",
            false,
        );

        let merged = repo
            .load_file_settings("new")
            .expect("load")
            .expect("copied edit");
        assert_eq!(merged.process.sensor_profile, "None");
        assert_eq!(
            repo.file_marks().expect("marks").get("new"),
            Some(&"keeper".to_string())
        );
        let members = rolls::rolls_containing_path(&repo, &new_path);
        assert_eq!(members, [roll_id.clone()]);
        let overrides = rolls::frame_override_cards(&repo, &roll_id, "new");
        assert_eq!(overrides, ["sensor"]);
        let _ = std::fs::remove_dir_all(&temp);
    }
}
