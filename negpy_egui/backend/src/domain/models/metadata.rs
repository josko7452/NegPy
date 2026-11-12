//! Mirrors `negpy/features/metadata/models.py`: the custom analog metadata written to
//! exported files.

use serde::{Deserialize, Serialize};

/// A format nobody set, shown as its own dropdown row. It is not "Other".
pub const FORMAT_UNSET: &str = "—";

/// The format dropdown rows, in panel order.
pub const FORMAT_OPTIONS: &[&str] = &[FORMAT_UNSET, "35mm", "120", "4×5", "8×10", "110", "Other"];

/// The dropdown row for a stored format.
#[must_use]
pub fn format_label(value: &str) -> &str {
    if FORMAT_OPTIONS.contains(&value) {
        value
    } else {
        FORMAT_UNSET
    }
}

/// The stored format for a dropdown row.
#[must_use]
pub fn format_value(label: &str) -> String {
    if label == FORMAT_UNSET {
        String::new()
    } else {
        label.to_string()
    }
}

/// Panel order: strongest push first.
pub const PUSH_PULL_VALUES: [i64; 7] = [3, 2, 1, 0, -1, -2, -3];

/// Ordered keys for the EXIF ImageDescription; only non-empty values are joined.
pub const DESCRIPTION_FIELD_ORDER: &[&str] = &[
    "camera",
    "lens",
    "film",
    "iso",
    "format",
    "developer",
    "push_pull",
    "scanning",
];

/// Preserve pre-selector behavior: gear only.
pub const DEFAULT_DESCRIPTION_FIELDS: &[&str] = &["camera", "lens", "film", "iso"];

/// Keep known keys in canonical order. `None` yields the gear-only defaults.
#[must_use]
pub fn normalize_description_fields(fields: Option<&[String]>) -> Vec<String> {
    let Some(fields) = fields else {
        return DEFAULT_DESCRIPTION_FIELDS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    };
    DESCRIPTION_FIELD_ORDER
        .iter()
        .filter(|key| fields.iter().any(|field| field == *key))
        .map(|key| (*key).to_string())
        .collect()
}

/// Custom analog photography metadata written to exported files. Empty strings = field not
/// set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MetadataConfig {
    #[serde(default)]
    pub camera_id: String,
    #[serde(default)]
    pub lens_id: String,
    #[serde(default)]
    pub film_stock_id: String,
    #[serde(default)]
    pub camera_make: String,
    #[serde(default)]
    pub camera_model: String,
    #[serde(default)]
    pub lens_make: String,
    #[serde(default)]
    pub lens_model: String,
    #[serde(default)]
    pub focal_length_mm: Option<f64>,
    #[serde(default)]
    pub max_aperture: Option<f64>,
    #[serde(default)]
    pub film_iso: Option<i64>,
    #[serde(default)]
    pub film_manufacturer: String,
    #[serde(default)]
    pub film_color_type: String,
    #[serde(default)]
    pub film: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub format_other: String,
    #[serde(default)]
    pub process_id: String,
    #[serde(default)]
    pub scanning_id: String,
    #[serde(default)]
    pub developer: String,
    #[serde(default)]
    pub process_dilution: String,
    #[serde(default)]
    pub push_pull: i64,
    #[serde(default)]
    pub process_time_seconds: Option<i64>,
    #[serde(default)]
    pub process_temperature_c: Option<f64>,
    #[serde(default)]
    pub scanning: String,
    #[serde(default)]
    pub capture_date: String,
    #[serde(default)]
    pub gps_latitude: Option<f64>,
    #[serde(default)]
    pub gps_longitude: Option<f64>,
    #[serde(default)]
    pub location_city: String,
    #[serde(default)]
    pub location_state: String,
    #[serde(default)]
    pub location_country: String,
    #[serde(default)]
    pub capture_roll: String,
    #[serde(default)]
    pub capture_frame: Option<i64>,
    #[serde(default)]
    pub protect_original_metadata: bool,
    #[serde(default)]
    pub exposure_override: String,
    /// The EXIF ImageDescription field set. `None` inherits the sticky roll choice on open.
    #[serde(default)]
    pub description_fields: Option<Vec<String>>,
}

impl MetadataConfig {
    /// Apply the Python `__post_init__`: a set field list is normalized to canonical order.
    /// `None` (the default) stays `None` and inherits the sticky choice.
    #[must_use]
    pub fn normalize(mut self) -> Self {
        if let Some(fields) = &self.description_fields {
            self.description_fields = Some(normalize_description_fields(Some(fields)));
        }
        self
    }
}
