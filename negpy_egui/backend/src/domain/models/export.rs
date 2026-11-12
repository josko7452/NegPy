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
