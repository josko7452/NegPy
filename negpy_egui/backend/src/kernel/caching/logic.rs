//! Mirrors `negpy/kernel/caching/logic.py`: stable config hashes and stage entries.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The crop window a stage ran on, as `(x, y, width, height)` in frame pixels.
pub type Roi = Option<(usize, usize, usize, usize)>;

/// One intermediate pipeline result.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheEntry {
    pub config_hash: String,
    pub data: ImageBuffer,
    pub metrics: BTreeMap<String, Value>,
    pub active_roi: Roi,
}

/// The literal JSON form a Python config produces through `json.dumps(..., sort_keys=True)`.
fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_default(),
        Value::Array(values) => {
            let items: Vec<String> = values.iter().map(canonical_json).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(fields) => {
            let mut entries: Vec<(&String, &Value)> = fields.iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let items: Vec<String> = entries
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    )
                })
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

/// `calculate_config_hash`: the MD5 of the config's canonical JSON. Values are kept, so a
/// slider between -1 and -2 gets a new hash.
#[must_use]
pub fn calculate_config_hash(config: &crate::domain::models::WorkspaceConfig) -> String {
    let value = serde_json::to_value(config.to_dict()).unwrap_or(Value::Null);
    let serialized = canonical_json(&value);
    format!("{:x}", md5::compute(serialized.as_bytes()))
}

/// `calculate_config_hash` for a bare value, such as one slider's setting.
#[must_use]
pub fn calculate_value_hash<T: serde::Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).unwrap_or(Value::Null);
    format!("{:x}", md5::compute(canonical_json(&value).as_bytes()))
}
