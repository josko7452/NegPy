//! Mirrors the path and rule half of `negpy/services/export/frame_merge.py`: which files
//! form an assembled frame, what it merges to, and the params its decode drops.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use blake2::digest::consts::U16;
use blake2::{Blake2b, Digest};
use serde_json::Value;

use crate::domain::models::{HdrConfig, StitchConfig, WorkspaceConfig};
use crate::features::process::path::{RenderPath, render_path};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::kernel::system::paths::{apply_umask_mode, replace_file, safe_unlink};
use crate::services::export::encoders::{TiffCompression, encode_tiff_with_description};
use crate::services::export::linear_output::{
    LinearBakes, LinearRecord, linear_description, source_format_label_for,
};

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
                    let Some(pair) = pair.as_array() else {
                        continue;
                    };
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
        values.len() >= 2
            && values
                .iter()
                .all(|value| value.as_f64().is_some_and(|v| v != 0.0))
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
pub fn merged_path_for(
    primary_path: &Path,
    kind: &str,
    taken: &BTreeSet<String>,
) -> Option<PathBuf> {
    let suffix = merged_suffix(kind)?;
    let folder = primary_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = format!(
        "{}{suffix}",
        primary_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
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

/// The recovery error a failed merge reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeError {
    /// The bytes read back do not match the frame that was written.
    Verify(String),
    /// The user cancelled before the rename.
    Cancelled(String),
    /// The destination already exists.
    Exists(PathBuf),
    Io(String),
}

/// Whether `path` is a TIFF whose description names a merged source.
#[must_use]
pub fn is_merged_source(path: &Path) -> bool {
    if !crate::infrastructure::loaders::constants::has_extension(
        path,
        crate::infrastructure::loaders::constants::SUPPORTED_TIFF_EXTENSIONS,
    ) {
        return false;
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut decoder = match tiff::decoder::Decoder::new(std::io::BufReader::new(file)) {
        Ok(decoder) => decoder,
        Err(_) => return false,
    };
    let description = decoder
        .get_tag_ascii_string(tiff::tags::Tag::ImageDescription)
        .unwrap_or_default();
    description.starts_with("NegPy Linear Output") && is_merge_description(&description)
}

/// Write `image` as an untagged 16-bit TIFF, verify it under a temporary name, then rename
/// to `out_path`. An existing destination is never replaced.
pub fn write_merged_frame(
    image: &ImageBuffer,
    primary_path: &Path,
    out_path: &Path,
    params: &WorkspaceConfig,
    compression: TiffCompression,
    should_cancel: Option<&dyn Fn() -> bool>,
) -> Result<(), MergeError> {
    let expected = uint16_bytes(image);
    let folder = out_path.parent().unwrap_or_else(|| Path::new("."));
    let name = out_path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = folder.join(format!(".{name}.{}.part", std::process::id()));
    let source_name = primary_path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let source_format =
        source_format_label_for(primary_path, Some(&params.rgbscan), Some(&params.stitch));
    let description = linear_description(
        LinearRecord {
            source_name: &source_name,
            camera_wb: None,
            expansion: 1.0,
            source_format: &source_format,
            sensor_applied: None,
            gamma_key: "linear",
            meta: None,
        },
        LinearBakes::default(),
    );
    let bytes = encode_tiff_with_description(
        &expected,
        image.width,
        image.height,
        3,
        16,
        compression,
        None,
        None,
        Some(&description),
    )
    .ok_or_else(|| MergeError::Io("the merge buffer does not match its size".to_string()))?;

    let result = std::fs::write(&temporary, bytes)
        .and_then(|()| apply_umask_mode(&temporary))
        .map_err(|error| MergeError::Io(error.to_string()));
    if let Err(error) = result {
        safe_unlink(&temporary);
        return Err(error);
    }

    let verified = verify_tiff(&temporary, &expected, image.width, image.height);
    if !verified {
        safe_unlink(&temporary);
        return Err(MergeError::Verify(format!(
            "{name} did not read back as written"
        )));
    }
    if should_cancel.is_some_and(|cancel| cancel()) {
        safe_unlink(&temporary);
        return Err(MergeError::Cancelled(name.into_owned()));
    }
    if out_path.exists() {
        safe_unlink(&temporary);
        return Err(MergeError::Exists(out_path.to_path_buf()));
    }
    if let Err(error) = replace_file(&temporary, out_path) {
        safe_unlink(&temporary);
        return Err(MergeError::Io(error.to_string()));
    }
    Ok(())
}

fn uint16_bytes(image: &ImageBuffer) -> Vec<u8> {
    let mut codes = vec![0u16; image.pixels.len()];
    negpy_core::convert::to_uint(&image.pixels, &mut codes);
    codes.iter().flat_map(|code| code.to_ne_bytes()).collect()
}

fn verify_tiff(path: &Path, expected: &[u8], width: usize, height: usize) -> bool {
    let decoded = crate::infrastructure::loaders::tiff_loader::decode_page(path);
    let Some((image, _, _)) = decoded else {
        return false;
    };
    if image.width != width || image.height != height || image.source_u16.is_none() {
        return false;
    }
    let Some(codes) = image.source_u16 else {
        return false;
    };
    let written: Vec<u8> = codes.iter().flat_map(|code| code.to_ne_bytes()).collect();
    let mut hasher = Blake2b::<U16>::new();
    hasher.update(expected);
    let expected_digest = hasher.finalize();
    let mut hasher = Blake2b::<U16>::new();
    hasher.update(&written);
    hasher.finalize() == expected_digest
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

    #[test]
    fn a_written_merge_reads_back_with_its_label() {
        let folder = std::env::temp_dir().join(format!("negpy-merge-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let primary = folder.join("frame.arw");
        std::fs::write(&primary, b"raw").unwrap();
        let out = folder.join("frame_RGB.tif");
        let image = ImageBuffer::from_rgb(4, 3, vec![0.25; 4 * 3 * 3]);
        let mut params = WorkspaceConfig::default();
        params.rgbscan.enabled = true;
        params.rgbscan.green_path = "/scans/frame_g.dng".to_string();
        params.rgbscan.blue_path = "/scans/frame_b.dng".to_string();
        write_merged_frame(
            &image,
            &primary,
            &out,
            &params,
            TiffCompression::Deflate,
            None,
        )
        .expect("a merged frame");
        assert!(out.exists());
        assert!(is_merged_source(&out));
        let (decoded, _, _) =
            crate::infrastructure::loaders::tiff_loader::decode_page(&out).expect("read back");
        assert_eq!(decoded.source_u16.as_ref().map(Vec::len), Some(4 * 3 * 3));
        assert!(
            !folder
                .join(format!(".frame_RGB.tif.{}.part", std::process::id()))
                .exists()
        );
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_cancelled_merge_leaves_no_file() {
        let folder = std::env::temp_dir().join(format!("negpy-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let primary = folder.join("frame.dng");
        std::fs::write(&primary, b"raw").unwrap();
        let out = folder.join("frame_RGB.tif");
        let image = ImageBuffer::from_rgb(2, 2, vec![0.5; 2 * 2 * 3]);
        let error = write_merged_frame(
            &image,
            &primary,
            &out,
            &WorkspaceConfig::default(),
            TiffCompression::Deflate,
            Some(&|| true),
        )
        .expect_err("cancelled");
        assert!(matches!(error, MergeError::Cancelled(_)));
        assert!(!out.exists());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn an_existing_merge_is_refused() {
        let folder = std::env::temp_dir().join(format!("negpy-exists-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let primary = folder.join("frame.dng");
        std::fs::write(&primary, b"raw").unwrap();
        let out = folder.join("frame_RGB.tif");
        std::fs::write(&out, b"keep").unwrap();
        let image = ImageBuffer::from_rgb(2, 2, vec![0.5; 2 * 2 * 3]);
        let error = write_merged_frame(
            &image,
            &primary,
            &out,
            &WorkspaceConfig::default(),
            TiffCompression::Deflate,
            None,
        )
        .expect_err("exists");
        assert!(matches!(error, MergeError::Exists(_)));
        assert_eq!(std::fs::read(&out).unwrap(), b"keep");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
