//! Rehome edits saved under the pre-interior-sampling content hash.

use std::collections::HashMap;

use serde_json::Value;

use crate::infrastructure::storage::repository::StorageRepository;

/// An asset discovered with both its current and legacy content hashes.
#[derive(Clone, Copy, Debug)]
pub struct LegacyAsset<'a> {
    pub path: &'a str,
    pub hash: &'a str,
    pub legacy_hash: &'a str,
}

fn legacy_hash(asset: &Value) -> &str {
    asset
        .get("legacy_hash")
        .and_then(Value::as_str)
        .unwrap_or("")
}

/// Drop a legacy digest shared by more than one file.
///
/// The old digest cannot say whose edits it holds, and only one of a colliding pair was
/// ever openable, so neither file claims those edits.
pub fn blank_ambiguous_legacy_hashes(assets: &mut [Value]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for asset in assets.iter() {
        let legacy = legacy_hash(asset);
        if !legacy.is_empty() {
            *counts.entry(legacy.to_string()).or_default() += 1;
        }
    }
    for asset in assets.iter_mut() {
        let legacy = legacy_hash(asset).to_string();
        if counts.get(&legacy).copied().unwrap_or(0) > 1
            && let Some(object) = asset.as_object_mut()
        {
            log::warn!(
                "Ambiguous legacy hash for {}; starting it without migrated edits",
                object.get("path").and_then(Value::as_str).unwrap_or("")
            );
            object.insert("legacy_hash".into(), Value::String(String::new()));
        }
    }
}

/// Move an asset's saved edits from its legacy digest onto its current digest.
pub fn migrate_asset_hash(
    repo: &StorageRepository,
    asset: LegacyAsset<'_>,
) -> Result<(), crate::infrastructure::storage::repository::RepositoryError> {
    migrate_legacy_hash(repo, asset.path, asset.hash, asset.legacy_hash)
}

/// Move a legacy digest's saved edits onto the current digest.
pub fn migrate_legacy_hash(
    repo: &StorageRepository,
    path: &str,
    hash: &str,
    legacy_hash: &str,
) -> Result<(), crate::infrastructure::storage::repository::RepositoryError> {
    if !legacy_hash.is_empty() && legacy_hash != hash {
        repo.rehome_file_settings(legacy_hash, hash, path)?;
    }
    Ok(())
}
