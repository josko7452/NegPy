//! Thumbnail fingerprints, stored in the JPEG comment so the record lives and dies with the image.
//!
//! Mirrors `negpy/services/assets/thumbnail_fingerprint.py`. Anything that cannot be
//! fingerprinted reads as stale: a false stale costs one render, a false current leaves a
//! wrong thumbnail up.

use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::domain::models::{
    AltProcessConfig, ExposureConfig, FinishConfig, FlatFieldConfig, GeometryConfig, HdrConfig, LabConfig,
    LocalAdjustmentsConfig, ProcessConfig, RetouchConfig, RgbScanConfig, StitchConfig, ToningConfig,
    WorkspaceConfig,
};

/// Bump in any change that alters rendered pixels for unchanged settings.
pub const THUMBNAIL_RENDER_VERSION: u32 = 35;

/// Marks a thumbnail made from the source preview, which runs none of the frame's settings.
pub const QUICK: &str = "quick";

const COMMENT_PREFIX: &str = "negpy-thumb:";

/// Sections that never reach the pixels.
pub const NON_PIXEL_SECTIONS: [&str; 2] = ["metadata", "export"];

/// Fields left out of a hashed section.
fn unhashed_fields(section: &str) -> &'static [&'static str] {
    match section {
        "process" => &[
            "demosaic_export",
            "roll_name",
            "baseline_source",
            "demosaic_preview",
            "crosstalk_profile",
            "meter_rect",
            "meter_key",
            "film_base",
            "manual_dmin_at",
            "manual_dmax_at",
            "fade_auto_red",
            "fade_neutrals_mix",
        ],
        "lab" => &[
            "sharpen",
            "sharpen_method",
            "sharpen_radius",
            "sharpen_masking",
            "sharpen_auto_masking",
            "chroma_denoise",
        ],
        _ => &[],
    }
}

const BELOW_THUMBNAIL_SECTIONS: [&str; 1] = ["retouch"];

/// Path plus size and mtime: a file replaced under the same name changes the render.
fn file_identity(path: Option<&str>) -> Option<String> {
    let path = path?;
    let metadata = std::fs::metadata(path);
    let Ok(metadata) = metadata else {
        return Some(format!("{path}|missing"));
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    Some(format!("{path}|{}|{modified}", metadata.len()))
}

fn section_value<T: serde::Serialize>(config: &T) -> Value {
    serde_json::to_value(config).unwrap_or(Value::Null)
}

/// The fields of one section that differ from their defaults.
fn changed_fields(section: &Map<String, Value>, defaults: &Map<String, Value>, skip: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    for (key, value) in section {
        if skip.contains(&key.as_str()) {
            continue;
        }
        // A field at its default is left out, so a new defaulted field leaves the
        // fingerprint as it was.
        if defaults.get(key) == Some(value) {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }
    out
}

fn sections(config: &WorkspaceConfig) -> Map<String, Value> {
    let mut out = Map::new();
    let mut push = |name: &str, section: Value, default: Value| {
        if NON_PIXEL_SECTIONS.contains(&name) || BELOW_THUMBNAIL_SECTIONS.contains(&name) {
            return;
        }
        let (Value::Object(section), Value::Object(default)) = (section, default) else {
            return;
        };
        let value = Value::Object(changed_fields(&section, &default, unhashed_fields(name)));
        out.insert(name.to_string(), value);
    };
    push(
        "process",
        section_value(&config.process),
        section_value(&ProcessConfig::default()),
    );
    push(
        "exposure",
        section_value(&config.exposure),
        section_value(&ExposureConfig::default()),
    );
    push(
        "flatfield",
        section_value(&config.flatfield),
        section_value(&FlatFieldConfig::default()),
    );
    push(
        "rgbscan",
        section_value(&config.rgbscan),
        section_value(&RgbScanConfig::default()),
    );
    push(
        "stitch",
        section_value(&config.stitch),
        section_value(&StitchConfig::default()),
    );
    push("hdr", section_value(&config.hdr), section_value(&HdrConfig::default()));
    push(
        "geometry",
        section_value(&config.geometry),
        section_value(&GeometryConfig::default()),
    );
    push("lab", section_value(&config.lab), section_value(&LabConfig::default()));
    push(
        "local",
        section_value(&config.local),
        section_value(&LocalAdjustmentsConfig::default()),
    );
    push(
        "retouch",
        section_value(&config.retouch),
        section_value(&RetouchConfig::default()),
    );
    push(
        "altproc",
        section_value(&config.altproc),
        section_value(&AltProcessConfig::default()),
    );
    push(
        "toning",
        section_value(&config.toning),
        section_value(&ToningConfig::default()),
    );
    push(
        "finish",
        section_value(&config.finish),
        section_value(&FinishConfig::default()),
    );
    out
}

/// Files a composite reads besides the frame's own, which its hash already covers.
fn companion_paths(config: &WorkspaceConfig) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    paths.extend(config.hdr.hdr_paths.iter().cloned());
    if !config.rgbscan.green_path.is_empty() {
        paths.push(config.rgbscan.green_path.clone());
    }
    if !config.rgbscan.blue_path.is_empty() {
        paths.push(config.rgbscan.blue_path.clone());
    }
    paths.extend(config.stitch.stitch_paths.iter().cloned());
    for pair in &config.stitch.stitch_triplets {
        paths.extend(pair.iter().cloned());
    }
    paths.retain(|path| !path.is_empty());
    paths
}

/// The fingerprint the resolved config renders to.
#[must_use]
pub fn thumbnail_fingerprint(
    config: &WorkspaceConfig,
    workspace_color_space: &str,
    input_icc_path: Option<&str>,
) -> String {
    let mut payload = Map::new();
    payload.insert(
        "v".to_string(),
        Value::from(THUMBNAIL_RENDER_VERSION),
    );
    payload.insert("config".to_string(), Value::Object(sections(config)));
    payload.insert(
        "workspace".to_string(),
        Value::from(workspace_color_space),
    );
    payload.insert(
        "input_icc".to_string(),
        file_identity(input_icc_path).map_or(Value::Null, Value::from),
    );
    payload.insert(
        "companions".to_string(),
        Value::Array(
            companion_paths(config)
                .iter()
                .map(|path| file_identity(Some(path)).map_or(Value::Null, Value::from))
                .collect(),
        ),
    );
    if config.finish.carrier_width > 0.0 {
        // The carrier's width is in print units, so the export print size reaches its pixels.
        payload.insert(
            "carrier_print_size".to_string(),
            Value::from(config.export.export_print_size),
        );
    }
    let blob = py_json(&Value::Object(payload));
    let digest = Sha256::digest(blob.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `json.dumps(value, sort_keys=True)`: sorted keys and Python's default separators.
///
/// The fingerprint hashes this text, so the spacing and a float's `.0` must match Python's.
fn py_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => match number.as_f64() {
            Some(value) if number.is_f64() => format!("{value:?}"),
            _ => number.to_string(),
        },
        Value::String(text) => serde_json::to_string(text).unwrap_or_default(),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(py_json).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap_or_default(),
                        py_json(&fields[key])
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// The JPEG comment bytes a stored fingerprint writes.
#[must_use]
pub fn encode_comment(fingerprint: Option<&str>) -> Option<Vec<u8>> {
    let fingerprint = fingerprint.filter(|value| !value.is_empty())?;
    Some(format!("{COMMENT_PREFIX}{fingerprint}").into_bytes())
}

/// The fingerprint a JPEG comment holds, if any.
#[must_use]
pub fn decode_comment(raw: Option<&[u8]>) -> Option<String> {
    let text = std::str::from_utf8(raw?).ok()?;
    let value = text.strip_prefix(COMMENT_PREFIX)?;
    (!value.is_empty()).then(|| value.to_string())
}

/// Whether a stored fingerprint still names the current render.
#[must_use]
pub fn is_current(stored: Option<&str>, current: &str) -> bool {
    stored.is_some_and(|value| value != QUICK && value == current)
}

/// The file identity of a path, exposed for callers that key a cache on it.
#[must_use]
pub fn identity(path: Option<&str>) -> Option<String> {
    file_identity(path)
}

/// Whether a path exists, as the fingerprint treats a missing companion.
#[must_use]
pub fn is_missing(path: &str) -> bool {
    !Path::new(path).exists()
}
