//! Trichrome triplet membership, keyed by the red exposure's path.
//!
//! Mirrors `negpy/services/assets/triplets.py`: each record holds the three content
//! hashes, so a file changed on disk since the grouping does not re-attach.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::infrastructure::storage::repository::StorageRepository;

pub const TRIPLETS_KEY: &str = "triplets_by_path";

/// A saved triplet: green path, blue path, align flag, member hashes, made-by-hand flag.
pub type TripletRecord = Vec<Value>;

/// Every stored triplet, keyed by the red exposure's path.
#[must_use]
pub fn saved_triplets(repo: &StorageRepository) -> Map<String, Value> {
    match repo.global_setting(TRIPLETS_KEY).ok().flatten() {
        Some(Value::Object(store)) => store,
        _ => Map::new(),
    }
}

/// Whether the user grouped this triplet by hand.
#[must_use]
pub fn made_by_hand(record: &Value) -> bool {
    record
        .as_array()
        .is_some_and(|values| values.len() > 4 && truthy(&values[4]))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Null => false,
    }
}

/// The saved triplets a discovery re-attaches: all in Trichrome Mode, else those made by hand.
#[must_use]
pub fn applied_triplets(repo: &StorageRepository, trichrome_mode: bool) -> Map<String, Value> {
    let saved = saved_triplets(repo);
    if trichrome_mode {
        return saved;
    }
    saved
        .into_iter()
        .filter(|(_, record)| made_by_hand(record))
        .collect()
}

fn text(asset: &Value, key: &str) -> String {
    asset
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The storable record of an asset's triplet membership.
#[must_use]
pub fn triplet_record(asset: &Value) -> TripletRecord {
    let hash = text(asset, "hash");
    let red = crate::services::assets::half_frame::base_hash(Some(&hash)).unwrap_or_default();
    let align = asset.get("align").and_then(Value::as_bool).unwrap_or(true);
    let by_hand = asset
        .get("triplet_by_hand")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    vec![
        json!(text(asset, "green_path")),
        json!(text(asset, "blue_path")),
        json!(align),
        json!([red, text(asset, "green_hash"), text(asset, "blue_hash")]),
        json!(by_hand),
    ]
}

/// Drop the records for these primaries.
pub fn forget_triplets(repo: &mut StorageRepository, red_paths: &[String]) {
    let store = saved_triplets(repo);
    let drop: BTreeSet<&str> = red_paths.iter().map(String::as_str).collect();
    let updated: Map<String, Value> = store
        .into_iter()
        .filter(|(red, _)| !drop.contains(red.as_str()))
        .collect();
    save(repo, updated);
}

/// Upsert the triplets among `assets`; a stored triplet that shares a file is dropped.
pub fn remember_triplets(repo: &mut StorageRepository, assets: &[Value]) {
    let store = saved_triplets(repo);
    let mut updated = store.clone();
    for asset in assets {
        let composite = asset
            .get("stitch_paths")
            .or_else(|| asset.get("hdr_paths"))
            .is_some();
        if composite || text(asset, "green_path").is_empty() || text(asset, "blue_path").is_empty()
        {
            continue;
        }
        let path = text(asset, "path");
        let record = triplet_record(asset);
        let members: BTreeSet<&str> = [
            path.as_str(),
            record[0].as_str().unwrap_or_default(),
            record[1].as_str().unwrap_or_default(),
        ]
        .into_iter()
        .collect();
        let replaced: Vec<String> = updated
            .iter()
            .filter(|(red, other)| {
                red.as_str() != path
                    && (members.contains(red.as_str())
                        || members.contains(other[0].as_str().unwrap_or_default())
                        || members.contains(other[1].as_str().unwrap_or_default()))
            })
            .map(|(red, _)| red.clone())
            .collect();
        for red in replaced {
            updated.remove(&red);
        }
        updated.insert(path, Value::Array(record));
    }
    save(repo, updated);
}

fn save(repo: &mut StorageRepository, store: Map<String, Value>) {
    if store == saved_triplets(repo) {
        return;
    }
    let mut values = Map::new();
    values.insert(TRIPLETS_KEY.to_string(), Value::Object(store));
    let _ = repo.save_global_settings(&values);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> (tempfile::TempDir, StorageRepository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = StorageRepository::new(
            dir.path().join("edits.db").display().to_string(),
            dir.path().join("settings.db").display().to_string(),
        );
        repo.initialize().unwrap();
        (dir, repo)
    }

    fn triplet(red: &str, green: &str, blue: &str) -> Value {
        json!({
            "path": red,
            "hash": format!("h{}", red),
            "green_path": green,
            "blue_path": blue,
            "green_hash": format!("h{green}"),
            "blue_hash": format!("h{blue}"),
        })
    }

    #[test]
    fn a_triplet_is_stored_under_its_red_path() {
        let (_dir, mut repo) = repository();
        remember_triplets(&mut repo, &[triplet("/a/r", "/a/g", "/a/b")]);
        assert_eq!(
            saved_triplets(&repo)["/a/r"],
            json!(["/a/g", "/a/b", true, ["h/a/r", "h/a/g", "h/a/b"], false])
        );
    }

    #[test]
    fn a_new_triplet_replaces_one_that_shares_an_exposure() {
        let (_dir, mut repo) = repository();
        remember_triplets(&mut repo, &[triplet("/a/r1", "/a/g1", "/a/b1")]);
        remember_triplets(&mut repo, &[triplet("/a/r2", "/a/g1", "/a/b2")]);
        assert_eq!(saved_triplets(&repo).keys().collect::<Vec<_>>(), ["/a/r2"]);
    }

    #[test]
    fn a_stitch_of_triplets_is_not_stored() {
        let (_dir, mut repo) = repository();
        let mut asset = triplet("/a/r", "/a/g", "/a/b");
        asset["stitch_paths"] = json!(["/a/r2"]);
        remember_triplets(&mut repo, &[asset]);
        assert!(saved_triplets(&repo).is_empty());
    }

    #[test]
    fn only_hand_made_triplets_apply_with_trichrome_off() {
        let (_dir, mut repo) = repository();
        remember_triplets(&mut repo, &[triplet("/a/r", "/a/g", "/a/b")]);
        let mut by_hand = triplet("/b/r", "/b/g", "/b/b");
        by_hand["triplet_by_hand"] = json!(true);
        remember_triplets(&mut repo, &[by_hand]);
        assert_eq!(applied_triplets(&repo, true).len(), 2);
        assert_eq!(
            applied_triplets(&repo, false).keys().collect::<Vec<_>>(),
            ["/b/r"]
        );
    }

    #[test]
    fn forgetting_a_triplet_removes_only_that_primary() {
        let (_dir, mut repo) = repository();
        remember_triplets(
            &mut repo,
            &[
                triplet("/a/r", "/a/g", "/a/b"),
                triplet("/b/r", "/b/g", "/b/b"),
            ],
        );
        forget_triplets(&mut repo, &["/a/r".to_string()]);
        assert_eq!(saved_triplets(&repo).keys().collect::<Vec<_>>(), ["/b/r"]);
    }
}
