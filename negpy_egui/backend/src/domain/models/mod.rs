//! Mirrors `negpy/domain/models.py`: the per-feature config DTOs and the flat edit
//! namespace `WorkspaceConfig` folds them into.
//!
//! Each sub-config is a frozen value with serde `Serialize`/`Deserialize`, so a saved
//! sidecar, preset or DB row round-trips through the flat JSON object the Python layer
//! writes. Field declaration order is the Python field order and drives the emitted key
//! order, because the sidecar writer compares bytes.

mod string_enum;

pub mod altprocess;
pub mod export;
pub mod exposure;
pub mod finish;
pub mod flatfield;
pub mod geometry;
pub mod hdr;
pub mod lab;
pub mod lens;
pub mod local;
pub mod metadata;
pub mod process;
pub mod retouch;
pub mod rgbscan;
pub mod stitch;
pub mod toning;
pub mod workspace;

pub use altprocess::{AltProcess, AltProcessConfig, Sensitizer};
pub use export::{
    EXPORT_COLOR_SPACES, ExportConfig, ExportFormat, ExportPreset, ExportPresetOutputMode,
    ExportResolutionMode, JXL_TAGGABLE_SPACES, TiffCompression, export_blocked, migrate_export_fmt,
    preset_display_name,
};
pub use exposure::{BwFilter, ExposureConfig, RenderIntent};
pub use finish::FinishConfig;
pub use flatfield::FlatFieldConfig;
pub use geometry::{AspectRatio, AutocropMode, GeometryConfig};
pub use hdr::{HdrConfig, hdr_active, hdr_merge_token, hdr_name, hdr_stem, hdr_token};
pub use lab::{LabConfig, SharpenMethod};
pub use lens::{LensCorrections, LensMetadata, LensWarp};
pub use local::{LocalAdjustmentsConfig, LocalMask, MaskKey, MaskShape};
pub use metadata::{MetadataConfig, normalize_description_fields};
pub use process::{
    DemosaicMode, ProcessConfig, ProcessMode, SensorUnmix, SourceEncoding, auto_meter_for_mode,
    cast_removal_for_mode, with_film_fields, with_positive_source, with_process_mode,
    with_slide_bw, with_source_encoding,
};
pub use retouch::RetouchConfig;
pub use rgbscan::{RgbScanConfig, is_rgb_triplet, rgbscan_token};
pub use stitch::{StitchConfig, stitch_active, stitch_has_triplets, stitch_name, stitch_token};
pub use toning::ToningConfig;
pub use workspace::{
    WorkspaceConfig, flat_export_config, flat_export_preset, flat_master_config,
    resolve_preset_export,
};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// One JSON value the app wrote earlier may no longer fit its field. Python drops that one
/// field and keeps the edit; this mirrors it.
///
/// A whole-object deserialize is tried first, which is the common case. If it fails, every
/// key is tried alone to find the ones that cannot build, those are removed, and the object
/// is rebuilt. A map that still fails falls back to `Default`: a corrupt stored row must
/// not stop the app.
pub fn build_lenient<T: DeserializeOwned + Default>(map: &Map<String, Value>) -> T {
    let whole = Value::Object(map.clone());
    if let Ok(config) = serde_json::from_value::<T>(whole) {
        return config;
    }
    let mut bad: Vec<String> = Vec::new();
    for (key, value) in map {
        let mut single = Map::new();
        single.insert(key.clone(), value.clone());
        if serde_json::from_value::<T>(Value::Object(single)).is_err() {
            bad.push(key.clone());
        }
    }
    if !bad.is_empty() {
        log::warn!("Dropping invalid config values: {:?}", bad);
    }
    let mut filtered = map.clone();
    for key in &bad {
        filtered.remove(key);
    }
    serde_json::from_value::<T>(Value::Object(filtered)).unwrap_or_default()
}

/// A JSON number or bool for a field Python typed as `float`. A legacy bool toggle coerces
/// to `1.0`/`0.0`, the way the Python `__post_init__` does after a key rename.
pub fn bool_or_float<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct BoolOrFloat;

    impl Visitor<'_> for BoolOrFloat {
        type Value = f64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a number or a bool")
        }

        fn visit_bool<E: de::Error>(self, value: bool) -> Result<f64, E> {
            Ok(if value { 1.0 } else { 0.0 })
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<f64, E> {
            Ok(value as f64)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<f64, E> {
            Ok(value as f64)
        }

        fn visit_f64<E: de::Error>(self, value: f64) -> Result<f64, E> {
            Ok(value)
        }
    }

    deserializer.deserialize_any(BoolOrFloat)
}

/// A default of `[0.0, 0.0, 0.0]` for the per-channel triple fields.
#[must_use]
pub const fn zero_triple() -> [f64; 3] {
    [0.0, 0.0, 0.0]
}
