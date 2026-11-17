//! The asset facts that override a stored edit when a frame opens.
//!
//! Mirrors the `resolve_asset_*` helpers in `negpy/desktop/session.py`: the asset dict
//! is authoritative for which parts a composite holds, so a plain frame never inherits
//! the open frame's leaked triplet, bracket or registration.

use serde_json::Value;

use crate::domain::models::process::{SCAN_SETUP_PAPERS, scan_setup_values};
use crate::domain::models::{
    HdrConfig, ProcessMode, RgbScanConfig, StitchConfig, WorkspaceConfig, hdr_active,
    is_rgb_triplet, with_process_mode,
};
use crate::features::hdr::logic::{resolve_anchor, seed_shadow_density};

/// Which multi-file construction an asset is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositeKind {
    Plain,
    Stitch,
    Hdr,
    Rgb,
    Half,
    Diptych,
}

fn text<'a>(asset: &'a Value, key: &str) -> &'a str {
    asset.get(key).and_then(Value::as_str).unwrap_or_default()
}

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

/// The construction an asset names. Stitch is tested first because a stitch of
/// triplets also carries the primary part's green and blue pair.
#[must_use]
pub fn composite_kind(asset: &Value) -> CompositeKind {
    if asset
        .get("stitch_paths")
        .and_then(Value::as_array)
        .is_some_and(|paths| !paths.is_empty())
    {
        return CompositeKind::Stitch;
    }
    if asset
        .get("hdr_paths")
        .and_then(Value::as_array)
        .is_some_and(|paths| !paths.is_empty())
    {
        return CompositeKind::Hdr;
    }
    if !text(asset, "green_path").is_empty() && !text(asset, "blue_path").is_empty() {
        return CompositeKind::Rgb;
    }
    if asset.get("half").is_some() {
        return CompositeKind::Half;
    }
    if asset.get("diptych").is_some() {
        return CompositeKind::Diptych;
    }
    CompositeKind::Plain
}

/// Whether Edit Trichrome may rebuild this asset.
#[must_use]
pub fn can_edit_triplet(asset: &Value) -> bool {
    matches!(
        composite_kind(asset),
        CompositeKind::Plain | CompositeKind::Rgb
    ) && asset.get("half").is_none()
}

/// Whether the asset is a plain triplet with no half split.
#[must_use]
pub fn is_plain_triplet(asset: &Value) -> bool {
    composite_kind(asset) == CompositeKind::Rgb && asset.get("half").is_none()
}

/// The selected plain frames a trichrome merge joins: three distinct files, else none.
#[must_use]
pub fn trichrome_merge_indices(files: &[Value], indices: &[usize]) -> Vec<usize> {
    let mut picked: Vec<usize> = indices.to_vec();
    picked.sort_unstable();
    picked.dedup();
    if picked.len() != 3
        || picked.iter().any(|index| {
            *index >= files.len() || composite_kind(&files[*index]) != CompositeKind::Plain
        })
    {
        return Vec::new();
    }
    let paths: std::collections::HashSet<&str> = picked
        .iter()
        .map(|index| text(&files[*index], "path"))
        .collect();
    if paths.len() == 3 { picked } else { Vec::new() }
}

fn triplet_composition(config: &RgbScanConfig) -> (bool, &str, &str, bool) {
    if is_rgb_triplet(config) {
        (
            true,
            config.green_path.as_str(),
            config.blue_path.as_str(),
            config.align,
        )
    } else {
        (false, "", "", config.align)
    }
}

/// Overlay the asset's triplet paths. A changed composition drops the measured bounds,
/// which described the previous source.
#[must_use]
pub fn resolve_asset_rgbscan(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let green = text(asset, "green_path");
    let blue = text(asset, "blue_path");
    let resolved = if !green.is_empty() && !blue.is_empty() {
        RgbScanConfig {
            enabled: true,
            green_path: green.to_string(),
            blue_path: blue.to_string(),
            align: asset
                .get("align")
                .and_then(Value::as_bool)
                .unwrap_or(params.rgbscan.align),
        }
    } else {
        RgbScanConfig {
            align: params.rgbscan.align,
            ..RgbScanConfig::default()
        }
    };
    let mut out = params.clone();
    let changed = triplet_composition(&resolved) != triplet_composition(&params.rgbscan);
    out.rgbscan = resolved;
    if changed {
        let (invalidate, floors, ceils) =
            crate::domain::models::process::invalidate_local_bounds(&out.process);
        if invalidate {
            out.process.local_floors = floors;
            out.process.local_ceils = ceils;
        }
    }
    out
}

/// Overlay the film process a composite inherited from its frames.
#[must_use]
pub fn resolve_asset_process_mode(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let mode = text(asset, "process_mode");
    if mode.is_empty() {
        return params.clone();
    }
    with_process_mode(params.clone(), ProcessMode::coerce(mode))
}

/// Open a fresh narrowband capture as its import recorded it.
#[must_use]
pub fn resolve_asset_capture(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let capture = text(asset, "capture");
    if capture != "blend" && capture != "single" {
        return params.clone();
    }
    let (linear_raw, paper) = scan_setup_values("camera", "narrowband");
    let mut out = with_process_mode(params.clone(), ProcessMode::C41);
    out.process.linear_raw = linear_raw;
    if SCAN_SETUP_PAPERS.contains(&out.exposure.paper_profile.as_str()) {
        out.exposure.paper_profile = paper.to_string();
    }
    out
}

/// Overlay the stored bracket onto the params. A non-HDR asset drops a leaked bracket.
#[must_use]
pub fn resolve_asset_hdr(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let mut out = params.clone();
    let paths = strings(asset, "hdr_paths");
    if paths.is_empty() {
        out.hdr = HdrConfig::default();
        return out;
    }
    out.hdr = HdrConfig {
        hdr_enabled: true,
        hdr_paths: paths,
        hdr_ratios: asset
            .get("hdr_ratios")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default(),
        hdr_align: asset
            .get("hdr_align")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        hdr_anchor: text(asset, "hdr_anchor").to_string(),
        hdr_anchor_ev: asset
            .get("hdr_anchor_ev")
            .and_then(Value::as_f64)
            .unwrap_or(crate::domain::models::hdr::ANCHOR_EV_UNSET),
    };
    out
}

/// Seed a fresh merge with the shadow range its ratios recovered.
#[must_use]
pub fn resolve_asset_hdr_seed(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let mut out = resolve_asset_hdr(params, asset);
    if !hdr_active(&out.hdr) {
        return out;
    }
    let paths = hdr_frame_paths(asset);
    let ratios = &out.hdr.hdr_ratios;
    let anchor = resolve_anchor(&paths, ratios, &out.hdr);
    let seed = seed_shadow_density(ratios, anchor);
    if seed != 0.0 {
        out.exposure.shadow_density = seed;
    }
    out
}

/// Every frame of a merged asset, reference first.
#[must_use]
pub fn hdr_frame_paths(asset: &Value) -> Vec<String> {
    let others = strings(asset, "hdr_paths");
    if others.is_empty() {
        return Vec::new();
    }
    let mut out = vec![text(asset, "path").to_string()];
    out.extend(others);
    out
}

/// Overlay a stored registration. A non-stitch asset drops a leaked registration.
#[must_use]
pub fn resolve_asset_stitch(params: &WorkspaceConfig, asset: &Value) -> WorkspaceConfig {
    let mut out = params.clone();
    let paths = strings(asset, "stitch_paths");
    if paths.is_empty() {
        out.stitch = StitchConfig::default();
        return out;
    }
    let canvas = asset
        .get("stitch_canvas")
        .and_then(Value::as_array)
        .map(|values| {
            let read = |index: usize| values.get(index).and_then(Value::as_i64).unwrap_or(0);
            [read(0), read(1)]
        })
        .unwrap_or([0, 0]);
    out.stitch = StitchConfig {
        stitch_enabled: true,
        stitch_paths: paths,
        stitch_transforms: asset
            .get("stitch_transforms")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.as_array()
                            .map(|values| values.iter().filter_map(Value::as_f64).collect())
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default(),
        stitch_canvas: canvas,
        stitch_sizes: asset
            .get("stitch_sizes")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let values = row.as_array().cloned().unwrap_or_default();
                        let read =
                            |index: usize| values.get(index).and_then(Value::as_i64).unwrap_or(0);
                        [read(0), read(1)]
                    })
                    .collect()
            })
            .unwrap_or_default(),
        stitch_triplets: asset
            .get("stitch_triplets")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        let values = row.as_array()?;
                        let green = values.first()?.as_str()?.to_string();
                        let blue = values.get(1)?.as_str()?.to_string();
                        Some([green, blue])
                    })
                    .collect()
            })
            .unwrap_or_default(),
        stitch_align: asset
            .get("stitch_align")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    };
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_stitch_wins_over_its_primary_triplet() {
        let asset = json!({
            "path": "/roll/part1.dng",
            "green_path": "/roll/g.dng",
            "blue_path": "/roll/b.dng",
            "stitch_paths": ["/roll/part2.dng"],
        });
        assert_eq!(composite_kind(&asset), CompositeKind::Stitch);
        assert!(!can_edit_triplet(&asset));
    }

    #[test]
    fn a_plain_triplet_can_be_rebuilt() {
        let asset = json!({
            "path": "/roll/r.dng",
            "green_path": "/roll/g.dng",
            "blue_path": "/roll/b.dng",
        });
        assert!(can_edit_triplet(&asset));
        assert!(is_plain_triplet(&asset));
    }

    #[test]
    fn three_distinct_plain_files_merge() {
        let files = vec![
            json!({"path": "/roll/a.dng"}),
            json!({"path": "/roll/b.dng"}),
            json!({"path": "/roll/c.dng"}),
        ];
        assert_eq!(trichrome_merge_indices(&files, &[0, 1, 2]), vec![0, 1, 2]);
        assert!(trichrome_merge_indices(&files, &[0, 1]).is_empty());
    }

    #[test]
    fn a_plain_asset_drops_a_leaked_triplet_bracket_and_registration() {
        let mut leaked = WorkspaceConfig::default();
        leaked.rgbscan.enabled = true;
        leaked.rgbscan.green_path = "/stale/g.dng".into();
        leaked.rgbscan.blue_path = "/stale/b.dng".into();
        leaked.hdr.hdr_enabled = true;
        leaked.hdr.hdr_paths = vec!["/stale/long.dng".into()];
        leaked.stitch.stitch_enabled = true;
        leaked.stitch.stitch_paths = vec!["/stale/part.dng".into()];
        let plain = json!({"path": "/roll/plain.dng"});
        let out = resolve_asset_stitch(
            &resolve_asset_hdr(&resolve_asset_rgbscan(&leaked, &plain), &plain),
            &plain,
        );
        assert_eq!(out.rgbscan, RgbScanConfig::default());
        assert_eq!(out.hdr, HdrConfig::default());
        assert_eq!(out.stitch, StitchConfig::default());
    }

    #[test]
    fn a_triplet_takes_the_assets_own_paths() {
        let asset = json!({
            "path": "/roll/r.dng",
            "green_path": "/roll/g.dng",
            "blue_path": "/roll/b.dng",
            "align": false,
        });
        let out = resolve_asset_rgbscan(&WorkspaceConfig::default(), &asset);
        assert_eq!(
            out.rgbscan,
            RgbScanConfig {
                enabled: true,
                green_path: "/roll/g.dng".into(),
                blue_path: "/roll/b.dng".into(),
                align: false,
            }
        );
    }

    #[test]
    fn a_changed_triplet_composition_clears_the_measured_bounds() {
        let mut params = WorkspaceConfig::default();
        params.process.local_floors = [1.0, 2.0, 3.0];
        params.process.local_ceils = [4.0, 5.0, 6.0];
        let asset = json!({"green_path": "/g.dng", "blue_path": "/b.dng"});
        let out = resolve_asset_rgbscan(&params, &asset);
        assert_eq!(out.process.local_floors, [0.0; 3]);
        assert_eq!(out.process.local_ceils, [0.0; 3]);
    }

    #[test]
    fn a_stitch_registration_rides_on_the_params() {
        let asset = json!({
            "stitch_paths": ["/roll/p2.dng"],
            "stitch_transforms": [[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]],
            "stitch_canvas": [120, 80],
            "stitch_sizes": [[80, 80]],
        });
        let out = resolve_asset_stitch(&WorkspaceConfig::default(), &asset);
        assert!(out.stitch.stitch_enabled);
        assert_eq!(out.stitch.stitch_canvas, [120, 80]);
        assert_eq!(out.stitch.stitch_sizes, vec![[80, 80]]);
    }

    #[test]
    fn a_fresh_bracket_opens_at_its_stored_anchor() {
        let asset = json!({
            "path": "/roll/ref.dng",
            "hdr_paths": ["/roll/long.dng"],
            "hdr_ratios": [1.0, 4.0],
            "hdr_anchor": "/roll/long.dng",
        });
        let out = resolve_asset_hdr_seed(&WorkspaceConfig::default(), &asset);
        assert!(out.hdr.hdr_enabled);
        assert_eq!(out.hdr.hdr_anchor, "/roll/long.dng");
    }
}
