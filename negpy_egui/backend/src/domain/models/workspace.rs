//! Mirrors the `WorkspaceConfig` half of `negpy/domain/models.py`: the per-image edit and
//! its flat serialization.
//!
//! Every sub-config flattens into one JSON key namespace, so a duplicate field name across
//! two of them would silently clobber on load. The sections are listed here in the Python
//! merge order, which is the emitted key order.

use serde_json::{Map, Value};

/// The measured-bounds version a sidecar records. Mirrors `BOUNDS_VERSION` in
/// `negpy/features/exposure/normalization.py`.
pub const BOUNDS_VERSION: i64 = 5;

use crate::domain::migrations::{FlatConfig, migrate_flat_config};
use crate::domain::models::export::{ExportConfig, ExportPreset};
use crate::domain::models::{
    AltProcessConfig, ExposureConfig, FinishConfig, FlatFieldConfig, GeometryConfig, HdrConfig,
    LabConfig, LocalAdjustmentsConfig, MetadataConfig, ProcessConfig, RenderIntent, RetouchConfig,
    RgbScanConfig, StitchConfig, ToningConfig, build_lenient,
};

/// Complete state for a single image edit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceConfig {
    pub process: ProcessConfig,
    pub exposure: ExposureConfig,
    pub flatfield: FlatFieldConfig,
    pub rgbscan: RgbScanConfig,
    pub stitch: StitchConfig,
    pub hdr: HdrConfig,
    pub geometry: GeometryConfig,
    pub lab: LabConfig,
    pub local: LocalAdjustmentsConfig,
    pub retouch: RetouchConfig,
    pub altproc: AltProcessConfig,
    pub toning: ToningConfig,
    pub finish: FinishConfig,
    pub metadata: MetadataConfig,
    pub export: ExportConfig,
}

impl WorkspaceConfig {
    /// Apply the `__post_init__` rule a merged bracket needs.
    ///
    /// The merge decides the same thing as Auto Density, Auto Grade and Highlight
    /// Reconstruction, and the merge wins: the autos meter the merged buffer and divide the
    /// render exposure's choice straight back out, and per-frame reconstruction feeds the
    /// merge's clip detection a guess instead of a recovered highlight. Held here rather
    /// than at the render, because these fields are read down through both engines, the
    /// decode and the sidebars.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if !crate::domain::models::hdr_active(&self.hdr) {
            return self;
        }
        if self.process.highlight_reconstruction != 0 {
            self.process.highlight_reconstruction = 0;
        }
        if self.exposure.auto_exposure || self.exposure.auto_normalize_contrast {
            self.exposure.auto_exposure = false;
            self.exposure.auto_normalize_contrast = false;
        }
        self
    }

    /// Flatten every sub-config into the one JSON key namespace the app persists.
    #[must_use]
    pub fn to_dict(&self) -> FlatConfig {
        let mut result = Map::new();
        extend_object(&mut result, &self.process);
        extend_object(&mut result, &self.exposure);
        extend_object(&mut result, &self.flatfield);
        extend_object(&mut result, &self.rgbscan);
        extend_object(&mut result, &self.stitch);
        extend_object(&mut result, &self.hdr);
        extend_object(&mut result, &self.geometry);
        extend_object(&mut result, &self.lab);
        // Local masks live under their own key, not flattened.
        if let Ok(value) = serde_json::to_value(&self.local) {
            result.insert("local_masks".to_string(), value);
        }
        extend_object(&mut result, &self.retouch);
        extend_object(&mut result, &self.altproc);
        extend_object(&mut result, &self.toning);
        extend_object(&mut result, &self.finish);
        extend_object(&mut result, &self.metadata);
        extend_object(&mut result, &self.export);
        result
    }

    /// Rebuild an edit from the flat namespace, running the migrations and dropping any
    /// single value an older version wrote that this one rejects.
    #[must_use]
    pub fn from_flat_dict(data: &FlatConfig) -> Self {
        let mut data = data.clone();
        let local_value = data.remove("local_masks");
        let local_map = local_value
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        migrate_flat_config(&mut data);

        let known = known_keys();
        let unknown: Vec<&String> = data
            .keys()
            .filter(|key| !known.contains(key.as_str()))
            .collect();
        if !unknown.is_empty() {
            log::warn!("Dropping unknown config keys: {unknown:?}");
        }

        let config = Self {
            process: build_lenient(&data),
            exposure: build_lenient::<ExposureConfig>(&data).normalize(),
            flatfield: build_lenient(&data),
            rgbscan: build_lenient(&data),
            stitch: build_lenient(&data),
            hdr: build_lenient(&data),
            geometry: build_lenient::<GeometryConfig>(&data).normalize(),
            lab: build_lenient(&data),
            local: build_lenient(&local_map),
            retouch: build_lenient(&data),
            altproc: build_lenient(&data),
            toning: build_lenient(&data),
            finish: build_lenient(&data),
            metadata: build_lenient::<MetadataConfig>(&data).normalize(),
            export: build_lenient::<ExportConfig>(&data).normalize(),
        };
        config.normalized()
    }
}

/// Fold one sub-config's fields into the flat map. A sub-config serializes to a JSON object
/// because it is a struct, so a non-object result is impossible and is skipped.
fn extend_object<T: serde::Serialize>(map: &mut Map<String, Value>, config: &T) {
    if let Value::Object(fields) = serde_json::to_value(config).unwrap_or(Value::Null) {
        map.extend(fields);
    }
}

/// Every key the sub-configs declare, plus the local-mask key. Derived from a default's
/// serialization, so a field added to a config appears here without a second list.
fn known_keys() -> std::collections::HashSet<String> {
    let mut keys = std::collections::HashSet::new();
    let mut add = |value: Value| {
        if let Value::Object(fields) = value {
            for key in fields.into_iter().map(|(key, _)| key) {
                keys.insert(key);
            }
        }
    };
    add(serde_json::to_value(ProcessConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(ExposureConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(FlatFieldConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(RgbScanConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(StitchConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(HdrConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(GeometryConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(LabConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(RetouchConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(AltProcessConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(ToningConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(FinishConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(MetadataConfig::default()).unwrap_or(Value::Null));
    add(serde_json::to_value(ExportConfig::default()).unwrap_or(Value::Null));
    keys.insert("local_masks".to_string());
    keys
}

/// Derive a flat digital-intermediate render config from an edit, without mutating it.
///
/// Keeps the framing, process mode and normalization bounds, but switches the Print stage to
/// the flat intent and turns off every automatic and creative print decision, so the result
/// is neutral and consistent across a roll.
#[must_use]
pub fn flat_master_config(config: &WorkspaceConfig) -> WorkspaceConfig {
    let flat_exposure = ExposureConfig {
        render_intent: RenderIntent::Flat,
        auto_exposure: false,
        auto_normalize_contrast: false,
        cast_removal_strength: 0.0,
        paper_dmin: false,
        toe: 0.0,
        shoulder: 0.0,
        toe_trim_red: 0.0,
        toe_trim_green: 0.0,
        toe_trim_blue: 0.0,
        shoulder_trim_red: 0.0,
        shoulder_trim_green: 0.0,
        shoulder_trim_blue: 0.0,
        toe_width_trim_red: 0.0,
        toe_width_trim_green: 0.0,
        toe_width_trim_blue: 0.0,
        shoulder_width_trim_red: 0.0,
        shoulder_width_trim_green: 0.0,
        shoulder_width_trim_blue: 0.0,
        grade_trim_red: 0.0,
        grade_trim_green: 0.0,
        grade_trim_blue: 0.0,
        paper_black: true,
        midtone_gamma: 0.0,
        midtone_gamma_trim_red: 0.0,
        midtone_gamma_trim_green: 0.0,
        midtone_gamma_trim_blue: 0.0,
        preflash: 0.0,
        ..config.exposure.clone()
    };
    WorkspaceConfig {
        exposure: flat_exposure,
        ..config.clone()
    }
}

/// Override export settings for a flat master. Delivers 16-bit lossless in TIFF, or JXL when
/// the user chose it and the color space is JXL-taggable.
#[must_use]
pub fn flat_export_config(export: &ExportConfig) -> ExportConfig {
    let mut result = export.clone();
    if result.export_fmt == crate::domain::models::ExportFormat::Jxl
        && !crate::domain::models::export_blocked(result.export_fmt, &result.export_color_space)
    {
        result.jxl_lossless = true;
    } else {
        result.export_fmt = crate::domain::models::ExportFormat::Tiff;
    }
    result.export_bit_depth = 16;
    if !matches!(
        result.export_resolution_mode,
        crate::domain::models::ExportResolutionMode::Print
            | crate::domain::models::ExportResolutionMode::TargetPx
    ) {
        result.export_resolution_mode = crate::domain::models::ExportResolutionMode::Original;
        result.paper_aspect_ratio = "Original".to_string();
    }
    result
}

/// Override a flat-master preset's export settings, mirroring `flat_export_config` on the
/// preset's shared sizing fields.
#[must_use]
pub fn flat_export_preset(preset: &ExportPreset) -> ExportPreset {
    let mut result = preset.clone();
    if result.export_fmt == crate::domain::models::ExportFormat::Jxl
        && !crate::domain::models::export_blocked(result.export_fmt, &result.export_color_space)
    {
        result.jxl_lossless = true;
    } else {
        result.export_fmt = crate::domain::models::ExportFormat::Tiff;
    }
    result.export_bit_depth = 16;
    if !matches!(
        result.export_resolution_mode,
        crate::domain::models::ExportResolutionMode::Print
            | crate::domain::models::ExportResolutionMode::TargetPx
    ) {
        result.export_resolution_mode = crate::domain::models::ExportResolutionMode::Original;
        result.paper_aspect_ratio = "Original".to_string();
    }
    result
}

/// Return `(pipeline_config, delivery_preset)` for one preset-driven export task.
#[must_use]
pub fn resolve_preset_export(
    preset: &ExportPreset,
    params: &WorkspaceConfig,
) -> (WorkspaceConfig, ExportPreset) {
    if preset.render_intent != "flat" {
        return (params.clone(), preset.clone());
    }
    (flat_master_config(params), flat_export_preset(preset))
}
