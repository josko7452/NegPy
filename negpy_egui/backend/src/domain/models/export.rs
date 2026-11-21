//! Mirrors the export half of `negpy/domain/models.py`: `ExportConfig` and the enums it
//! uses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::string_enum;

string_enum! {
    /// The output file format.
    pub enum ExportFormat {
        Jpeg => "JPEG",
        Tiff => "TIFF",
        Png => "PNG",
        Jxl => "JXL",
        Webp => "WEBP",
    }
    default Jpeg
}

string_enum! {
    /// Where a preset writes its output relative to the source.
    pub enum ExportPresetOutputMode {
        SubfolderOfSource => "subfolder_of_source",
        SameAsSource => "same_as_source",
        Absolute => "absolute",
    }
    default SubfolderOfSource
}

string_enum! {
    /// How the export sizes the output.
    pub enum ExportResolutionMode {
        Original => "original",
        Print => "print",
        TargetPx => "target_px",
    }
    default Original
}

string_enum! {
    /// The literal `tifffile` compression arguments. JXL-in-TIFF is deliberately absent; a
    /// saved config naming it coerces to ZIP.
    pub enum TiffCompression {
        None => "none",
        Lzw => "lzw",
        Zip => "zlib",
    }
    default Zip
}

/// Color spaces offered for export. ACES and XYZ are rawpy decode spaces with no bundled ICC
/// profile, so they are excluded.
pub const EXPORT_COLOR_SPACES: &[&str] = &[
    "Same as Source",
    "sRGB",
    "Adobe RGB",
    "ProPhoto RGB",
    "P3 D65",
    "Rec 2020",
    "Greyscale",
];

/// Color spaces JPEG XL can tag. Same as Source is excluded: it resolves per file, usually
/// to the Adobe RGB working space, which is not JXL-taggable.
pub const JXL_TAGGABLE_SPACES: &[&str] = &["sRGB", "P3 D65", "Rec 2020", "Greyscale"];

/// True when the format + color-space pairing cannot be encoded.
#[must_use]
pub fn export_blocked(fmt: ExportFormat, color_space: &str) -> bool {
    fmt == ExportFormat::Jxl && !JXL_TAGGABLE_SPACES.contains(&color_space)
}

/// Export parameters (path, format, sizing).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportConfig {
    /// The Python field is `userDir`, so its flat key keeps that exact spelling.
    #[serde(default, rename = "userDir")]
    pub user_dir: String,
    #[serde(default)]
    pub export_path: String,
    #[serde(default)]
    pub export_fmt: ExportFormat,
    /// 8 or 16; JPEG and WebP ignore it.
    #[serde(default = "default_bit_depth")]
    pub export_bit_depth: i64,
    #[serde(default = "default_quality")]
    pub jpeg_quality: i64,
    #[serde(default)]
    pub jpeg_progressive: bool,
    #[serde(default)]
    pub tiff_compression: TiffCompression,
    #[serde(default = "default_png_level")]
    pub png_compress_level: i64,
    #[serde(default = "default_true")]
    pub jxl_lossless: bool,
    #[serde(default = "default_jxl_distance")]
    pub jxl_distance: f64,
    #[serde(default = "default_jxl_effort")]
    pub jxl_effort: i64,
    #[serde(default = "default_quality")]
    pub webp_quality: i64,
    #[serde(default)]
    pub webp_lossless: bool,
    #[serde(default = "default_webp_method")]
    pub webp_method: i64,
    #[serde(default = "default_color_space")]
    pub export_color_space: String,
    #[serde(default = "default_paper_ratio")]
    pub paper_aspect_ratio: String,
    #[serde(default = "default_print_size")]
    pub export_print_size: f64,
    #[serde(default = "default_dpi")]
    pub export_dpi: i64,
    #[serde(default)]
    pub export_resolution_mode: ExportResolutionMode,
    #[serde(default = "default_long_edge")]
    pub export_target_long_edge_px: i64,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub output_mode: ExportPresetOutputMode,
    #[serde(default = "default_output_subfolder")]
    pub output_subfolder: String,
    #[serde(default)]
    pub icc_input_path: Option<String>,
    #[serde(default)]
    pub icc_output_path: Option<String>,
    /// Empty = follow the export destination rules.
    #[serde(default)]
    pub contact_sheet_output_path: String,
    #[serde(default)]
    pub export_sidecars_enabled: bool,
}

impl ExportConfig {
    /// Apply the Python `__post_init__` format migration a programmatic build skips.
    #[must_use]
    pub fn normalize(mut self) -> Self {
        self.export_fmt = ExportFormat::coerce(&migrate_export_fmt(self.export_fmt.as_str()));
        self
    }

    /// Every stored field name, including the Python `userDir` spelling.
    #[must_use]
    pub fn field_names() -> Vec<String> {
        serde_json::to_value(Self::default())
            .ok()
            .and_then(|value| value.as_object().cloned())
            .map(|fields| fields.into_iter().map(|(key, _)| key).collect())
            .unwrap_or_default()
    }
}

impl Default for ExportConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the export defaults deserialize")
    }
}

/// A retired export format mapped to the closest surviving one. DNG maps to TIFF, the other
/// 16-bit master.
#[must_use]
pub fn migrate_export_fmt(fmt: &str) -> String {
    if fmt == "DNG" {
        "TIFF".to_string()
    } else {
        fmt.to_string()
    }
}

/// A single export preset: format, sizing, destination and color settings. The sizing/color
/// field names match `ExportConfig`, so `PrintService` accepts either.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportPreset {
    #[serde(default = "new_preset_id")]
    pub id: String,
    #[serde(default = "default_preset_name")]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Fixed at creation; not exposed in the preset editor form.
    #[serde(default = "default_print_intent")]
    pub render_intent: String,
    #[serde(default)]
    pub export_fmt: ExportFormat,
    #[serde(default = "default_bit_depth")]
    pub export_bit_depth: i64,
    #[serde(default = "default_quality")]
    pub jpeg_quality: i64,
    #[serde(default)]
    pub jpeg_progressive: bool,
    #[serde(default)]
    pub tiff_compression: TiffCompression,
    #[serde(default = "default_png_level")]
    pub png_compress_level: i64,
    #[serde(default = "default_true")]
    pub jxl_lossless: bool,
    #[serde(default = "default_jxl_distance")]
    pub jxl_distance: f64,
    #[serde(default = "default_jxl_effort")]
    pub jxl_effort: i64,
    #[serde(default = "default_quality")]
    pub webp_quality: i64,
    #[serde(default)]
    pub webp_lossless: bool,
    #[serde(default = "default_webp_method")]
    pub webp_method: i64,
    #[serde(default)]
    pub export_resolution_mode: ExportResolutionMode,
    #[serde(default = "default_paper_ratio")]
    pub paper_aspect_ratio: String,
    #[serde(default = "default_print_size")]
    pub export_print_size: f64,
    #[serde(default = "default_dpi")]
    pub export_dpi: i64,
    #[serde(default = "default_long_edge")]
    pub export_target_long_edge_px: i64,
    #[serde(default = "default_same_as_source")]
    pub output_mode: ExportPresetOutputMode,
    #[serde(default)]
    pub output_subfolder: String,
    #[serde(default)]
    pub output_path: String,
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    #[serde(default = "default_color_space")]
    pub export_color_space: String,
    #[serde(default)]
    pub icc_input_path: Option<String>,
    #[serde(default)]
    pub icc_output_path: Option<String>,
}

impl Default for ExportPreset {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the export preset defaults deserialize")
    }
}

impl ExportPreset {
    /// The export config this preset asks for, over `base`.
    ///
    /// A preset stores only the export fields it owns. The destination follows its
    /// output mode: a fixed folder wins, else the subfolder of the source.
    #[must_use]
    pub fn apply_to(&self, base: &ExportConfig, source_dir: &std::path::Path) -> ExportConfig {
        let mut config = base.clone();
        config.export_fmt = self.export_fmt;
        config.export_bit_depth = self.export_bit_depth;
        config.jpeg_quality = self.jpeg_quality;
        config.jpeg_progressive = self.jpeg_progressive;
        config.tiff_compression = self.tiff_compression;
        config.png_compress_level = self.png_compress_level;
        config.jxl_lossless = self.jxl_lossless;
        config.jxl_distance = self.jxl_distance;
        config.jxl_effort = self.jxl_effort;
        config.webp_quality = self.webp_quality;
        config.webp_lossless = self.webp_lossless;
        config.webp_method = self.webp_method;
        config.export_resolution_mode = self.export_resolution_mode;
        config.paper_aspect_ratio = self.paper_aspect_ratio.clone();
        config.export_print_size = self.export_print_size;
        config.export_dpi = self.export_dpi;
        config.export_target_long_edge_px = self.export_target_long_edge_px;
        config.filename_pattern = self.filename_pattern.clone();
        config.overwrite = self.overwrite;
        config.output_mode = self.output_mode;
        config.output_subfolder = self.output_subfolder.clone();
        config.export_color_space = self.export_color_space.clone();
        config.icc_input_path = self.icc_input_path.clone();
        config.icc_output_path = self.icc_output_path.clone();
        config.export_path = match self.output_mode {
            ExportPresetOutputMode::Absolute => self.output_path.clone(),
            ExportPresetOutputMode::SubfolderOfSource if !self.output_subfolder.is_empty() => {
                source_dir
                    .join(&self.output_subfolder)
                    .display()
                    .to_string()
            }
            _ => source_dir.display().to_string(),
        };
        config
    }
}

/// The preset list/checkbox label, flagging flat-master presets.
#[must_use]
pub fn preset_display_name(preset: &ExportPreset) -> String {
    if preset.render_intent == "flat" {
        format!("{} (flat)", preset.name)
    } else {
        preset.name.clone()
    }
}

/// Keep only the keys the preset declares, mirroring the Python `from_dict`.
#[must_use]
pub fn preset_from_dict(value: &Value) -> ExportPreset {
    let map = value.as_object().cloned().unwrap_or_default();
    crate::domain::models::build_lenient(&map)
}

#[cfg(test)]
mod preset_export_tests {
    use super::*;

    #[test]
    fn a_preset_overlays_its_export_fields_and_its_folder() {
        let base = ExportConfig::default();
        let preset = ExportPreset {
            export_fmt: ExportFormat::Webp,
            jpeg_quality: 70,
            output_mode: ExportPresetOutputMode::SubfolderOfSource,
            output_subfolder: "web".to_string(),
            ..ExportPreset::default()
        };
        let applied = preset.apply_to(&base, std::path::Path::new("/roll"));
        assert_eq!(applied.export_fmt, ExportFormat::Webp);
        assert_eq!(applied.jpeg_quality, 70);
        assert_eq!(applied.export_path, "/roll/web");
    }

    #[test]
    fn a_fixed_folder_preset_keeps_its_path() {
        let preset = ExportPreset {
            output_mode: ExportPresetOutputMode::Absolute,
            output_path: "/prints".to_string(),
            ..ExportPreset::default()
        };
        let applied = preset.apply_to(&ExportConfig::default(), std::path::Path::new("/roll"));
        assert_eq!(applied.export_path, "/prints");
    }
}

fn new_preset_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_preset_name() -> String {
    "Untitled Preset".to_string()
}

fn default_print_intent() -> String {
    "print".to_string()
}

const fn default_bit_depth() -> i64 {
    16
}

const fn default_quality() -> i64 {
    90
}

const fn default_png_level() -> i64 {
    6
}

const fn default_true() -> bool {
    true
}

const fn default_jxl_distance() -> f64 {
    1.0
}

const fn default_jxl_effort() -> i64 {
    7
}

const fn default_webp_method() -> i64 {
    4
}

fn default_color_space() -> String {
    "sRGB".to_string()
}

fn default_paper_ratio() -> String {
    "Original".to_string()
}

const fn default_print_size() -> f64 {
    30.0
}

const fn default_dpi() -> i64 {
    300
}

const fn default_long_edge() -> i64 {
    2000
}

fn default_filename_pattern() -> String {
    "{{ original_name }}".to_string()
}

fn default_output_subfolder() -> String {
    "export".to_string()
}

const fn default_same_as_source() -> ExportPresetOutputMode {
    ExportPresetOutputMode::SameAsSource
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_defaults_carry_the_sidecar_settings() {
        let export = ExportConfig::default();
        assert!(!export.export_sidecars_enabled);
        assert!(export.contact_sheet_output_path.is_empty());
    }
}
