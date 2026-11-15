//! Mirrors the path and rule half of `negpy/services/export/frame_merge.py`: which files
//! form an assembled frame, what it merges to, and the params its decode drops.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::models::{HdrConfig, StitchConfig, WorkspaceConfig};
use crate::features::process::path::{RenderPath, render_path};

/// The assembled kinds whose parts a merge can replace. An HDR bracket keeps its detail
/// below a 16-bit step, so it has no merged file.
pub const MERGEABLE_KINDS: [&str; 2] = ["rgb", "stitch"];

/// The basename marker a merged frame carries.
#[must_use]
pub fn merged_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "rgb" => Some("_RGB"),
        "stitch" => Some("_STITCH"),
        _ => None,
    }
}

/// The extension every merged frame takes.
pub const MERGED_EXT: &str = ".tif";

/// The labels `source_format_label` gives a merged file.
pub const MERGED_FORMATS: [&str; 2] = ["camera RAW (RGB triplet)", "camera RAW (stitch "];

fn strings(asset: &Value, key: &str) -> Vec<String> {
    asset
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn text(asset: &Value, key: &str) -> Option<String> {
    asset
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Every source file of `asset` but its primary, in a stable order.
#[must_use]
pub fn part_files(asset: &Value, kind: &str) -> Vec<String> {
    match kind {
        "rgb" => ["green_path", "blue_path"]
            .iter()
            .filter_map(|key| text(asset, key))
            .collect(),
        "stitch" => {
            let primary = text(asset, "path").unwrap_or_default();
            let mut paths = strings(asset, "stitch_paths");
            if let Some(triplets) = asset.get("stitch_triplets").and_then(Value::as_array) {
                for pair in triplets {
                    let Some(pair) = pair.as_array() else { continue };
                    let green = pair.first().and_then(Value::as_str).unwrap_or_default();
                    let blue = pair.get(1).and_then(Value::as_str).unwrap_or_default();
                    if !green.is_empty() && !blue.is_empty() {
                        paths.extend([green.to_string(), blue.to_string()]);
                    }
                }
            }
            dedup(paths, &primary)
        }
        _ => Vec::new(),
    }
}

fn dedup(paths: Vec<String>, primary: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter(|path| path != primary && seen.insert(path.clone()))
        .collect()
}

/// The sources that were frames in their own right, so can carry a `.negpy` sidecar.
#[must_use]
pub fn frame_files(asset: &Value, kind: &str) -> Vec<String> {
    let primary = text(asset, "path").unwrap_or_default();
    if kind == "stitch" {
        let mut out = vec![primary];
        out.extend(strings(asset, "stitch_paths"));
        return out;
    }
    vec![primary]
}

/// Whether the stitch registration names every part and a canvas size.
#[must_use]
pub fn registration_complete(asset: &Value) -> bool {
    let parts = 1 + strings(asset, "stitch_paths").len();
    let canvas = asset.get("stitch_canvas").and_then(Value::as_array);
    let canvas_ok = canvas.is_some_and(|values| {
        values.len() >= 2 && values.iter().all(|value| value.as_f64().is_some_and(|v| v != 0.0))
    });
    canvas_ok
        && strings(asset, "stitch_transforms").len() == parts
        && strings(asset, "stitch_sizes").len() == parts
}

/// Whether the merge label marks an assembled source.
#[must_use]
pub fn is_merge_description(label: &str) -> bool {
    MERGED_FORMATS.iter().any(|mark| label.contains(mark))
}

/// Whether the written file would carry the merge label `is_merged_source` reads back.
/// A TIFF or LinearRaw DNG source is labeled for its own format, so it cannot merge.
#[must_use]
pub fn describes_a_merge(primary_path: &Path, params: &WorkspaceConfig) -> bool {
    let label = crate::services::export::linear_output::source_format_label_for(
        primary_path,
        Some(&params.rgbscan),
        Some(&params.stitch),
    );
    is_merge_description(&label)
}

/// Whether a frame renders through the RAW's camera color matrix, which a TIFF cannot carry.
#[must_use]
pub fn needs_camera_matrix(params: &WorkspaceConfig) -> bool {
    matches!(
        render_path(&params.process),
        RenderPath::Transfer | RenderPath::Positive
    )
}

/// `<primary stem><suffix>.tif` beside the primary, numbered past any existing file and
/// any path the caller already took.
#[must_use]
pub fn merged_path_for(primary_path: &Path, kind: &str, taken: &BTreeSet<String>) -> Option<PathBuf> {
    let suffix = merged_suffix(kind)?;
    let folder = primary_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = format!(
        "{}{suffix}",
        primary_path.file_stem().unwrap_or_default().to_string_lossy()
    );
    let mut candidate = folder.join(format!("{stem}{MERGED_EXT}"));
    let mut counter = 2;
    while taken.contains(&candidate.to_string_lossy().to_string()) || candidate.exists() {
        candidate = folder.join(format!("{stem}_{counter}{MERGED_EXT}"));
        counter += 1;
    }
    Some(candidate)
}

/// The params to decode `kind` with. A stitch bakes its flat field: the registration is
/// valid only on flat-fielded parts.
pub fn decode_params(params: &WorkspaceConfig, kind: &str) -> Option<WorkspaceConfig> {
    let mut updated = params.clone();
    match kind {
        "rgb" => {
            updated.flatfield = crate::domain::models::FlatFieldConfig::default();
            updated.stitch = StitchConfig::default();
            updated.hdr = HdrConfig::default();
        }
        "stitch" => {
            updated.hdr = HdrConfig::default();
        }
        _ => return None,
    }
    Some(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset() -> Value {
        serde_json::json!({
            "path": "/scans/frame.dng",
            "green_path": "/scans/frame_g.dng",
            "blue_path": "/scans/frame_b.dng",
        })
    }

    #[test]
    fn an_rgb_triplet_lists_its_two_parts() {
        assert_eq!(
            part_files(&asset(), "rgb"),
            vec!["/scans/frame_g.dng", "/scans/frame_b.dng"]
        );
        assert_eq!(frame_files(&asset(), "rgb"), vec!["/scans/frame.dng"]);
    }

    #[test]
    fn an_hdr_bracket_has_no_merged_file() {
        assert!(merged_suffix("hdr").is_none());
        assert!(decode_params(&WorkspaceConfig::default(), "hdr").is_none());
    }

    #[test]
    fn a_stitch_lists_every_part_once() {
        let asset = serde_json::json!({
            "path": "/scans/a.dng",
            "stitch_paths": ["/scans/b.dng"],
            "stitch_triplets": [["/scans/c.dng", "/scans/d.dng"]],
        });
        assert_eq!(
            part_files(&asset, "stitch"),
            vec!["/scans/b.dng", "/scans/c.dng", "/scans/d.dng"]
        );
    }

    #[test]
    fn a_merged_frame_takes_the_suffix_and_the_first_free_number() {
        let path = merged_path_for(Path::new("/scans/frame.dng"), "stitch", &BTreeSet::new());
        assert_eq!(path, Some(PathBuf::from("/scans/frame_STITCH.tif")));
    }

    #[test]
    fn an_rgb_merge_drops_the_flat_field_and_both_composites() {
        let params = WorkspaceConfig::default();
        let decode = decode_params(&params, "rgb").expect("an RGB merge");
        assert!(decode.flatfield.profile_id.is_empty());
        assert!(!decode.stitch.stitch_enabled);
        assert!(!decode.hdr.hdr_enabled);
    }
}
