//! Mirrors the pure encode path of `negpy/services/export/linear_output.py`.

use std::path::Path;

use crate::domain::models::ProcessConfig;
use crate::infrastructure::loaders::constants::{SUPPORTED_TIFF_EXTENSIONS, has_extension};
use crate::infrastructure::loaders::factory::is_camera_raw;
use crate::infrastructure::loaders::fff_loader::is_flextight_fff;
use crate::infrastructure::loaders::nef_loader::is_coolscan_nef;
use crate::infrastructure::loaders::source_kind::{
    LoaderKind, is_noritsu, is_pakon, select_loader,
};
use crate::services::export::encoders::{Resolution, TiffCompression, encode_tiff};

/// The only decodes that read Input gamma.
pub const GAMMA_SOURCE_TYPES: [&str; 2] = ["tiff", "nef"];
pub const PAKON_EXPANSION: f64 = 4.0;
pub const NORITSU_EXPANSION: f64 = 16.0;
pub const NOMINAL_DPI: u32 = 300;

/// The Input gamma choices the Linear Output panel offers.
pub const TIFF_GAMMA_OPTIONS: [(&str, &str); 8] = [
    ("linear", "Linear (1.0)"),
    ("1.8", "Gamma 1.8"),
    ("2.2", "Gamma 2.2"),
    ("2.4", "Gamma 2.4"),
    ("2.6", "Gamma 2.6"),
    ("srgb", "sRGB"),
    ("lstar", "L*"),
    ("rec709", "Rec.709"),
];

/// Camera white-balance multipliers, as the RAW decoder reports them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraWb {
    pub as_shot: [f64; 4],
    pub daylight: [f64; 4],
}

/// Device and timestamp metadata read from the source file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceMeta {
    pub make: Option<String>,
    pub model: Option<String>,
    pub datetime: Option<String>,
    pub demosaic: Option<String>,
    pub lens_corrected: bool,
}

fn is_dng(path: &Path) -> bool {
    has_extension(path, &[".dng"])
}

fn is_tiff(path: &Path) -> bool {
    has_extension(path, SUPPORTED_TIFF_EXTENSIONS)
}

fn is_pakon_f335(path: &Path) -> bool {
    std::fs::metadata(path)
        .ok()
        .is_some_and(|meta| meta.len().abs_diff(72_000_000) < 1024)
}

/// `is_linear_output_supported`: whether the source can be dumped.
#[must_use]
pub fn is_linear_output_supported(path: &Path) -> bool {
    if is_pakon(path) {
        return true;
    }
    if is_dng(path) {
        return is_camera_raw(path) || {
            let kind = select_loader(path);
            kind == LoaderKind::CameraRaw
        };
    }
    if is_coolscan_nef(path) || is_flextight_fff(path) || is_noritsu(path) {
        return true;
    }
    is_camera_raw(path) || is_tiff(path)
}

/// Classify a file for Linear Output expansion options.
#[must_use]
pub fn linear_output_source_type(path: &Path) -> &'static str {
    if is_pakon(path) {
        return if is_pakon_f335(path) {
            "pakon_f335"
        } else {
            "pakon"
        };
    }
    if is_dng(path) {
        return "dng";
    }
    if is_coolscan_nef(path) {
        return "nef";
    }
    if is_flextight_fff(path) {
        return "fff";
    }
    if is_noritsu(path) {
        return "noritsu";
    }
    if is_camera_raw(path) {
        return "camera";
    }
    if is_tiff(path) {
        return "tiff";
    }
    "unsupported"
}

/// The default expansion for a source that stores fewer than 16 bits of range.
#[must_use]
pub fn effective_expansion(path: &Path, expansion: Option<f64>) -> f64 {
    if is_pakon(path) {
        let factor = expansion.unwrap_or(if is_pakon_f335(path) {
            1.0
        } else {
            PAKON_EXPANSION
        });
        return if factor > 1.0 { factor } else { 1.0 };
    }
    if is_noritsu(path) {
        let factor = expansion.unwrap_or(NORITSU_EXPANSION);
        return if factor > 1.0 { factor } else { 1.0 };
    }
    if is_dng(path) || is_tiff(path) {
        return expansion.filter(|factor| *factor > 1.0).unwrap_or(1.0);
    }
    1.0
}

/// The source label stamped into the output description.
#[must_use]
pub fn source_format_label(path: &Path) -> &'static str {
    match linear_output_source_type(path) {
        "pakon" | "pakon_f335" => "Pakon",
        "dng" => "DNG LinearRaw",
        "nef" => "Coolscan NEF",
        "fff" => "Flextight FFF",
        "noritsu" => "Noritsu RAW",
        "camera" => "camera RAW",
        "tiff" => "TIFF",
        _ => "unknown",
    }
}

/// Normalize RGGB multipliers to green = 1.
///
/// A three-color CFA reports the second green multiplier as 0; using it as-is would halve
/// the green normalizer and double the red and blue gains.
#[must_use]
pub fn normalize_wb_rgb(wb: [f64; 4]) -> [f64; 3] {
    let green2 = if wb[3] > 0.0 { wb[3] } else { wb[1] };
    let green = if wb[1] + green2 > 0.0 {
        (wb[1] + green2) / 2.0
    } else {
        1.0
    };
    [wb[0] / green, 1.0, wb[2] / green]
}

/// Reverse a gamma encoding to recover linear-light values.
#[must_use]
pub fn linearize(image: &[f32], gamma_key: &str) -> Vec<f32> {
    if gamma_key == "linear" {
        return image.to_vec();
    }
    let mut clipped = vec![0.0; image.len()];
    negpy_core::convert::clip_unit(image, &mut clipped);
    let mut out = vec![0.0; clipped.len()];
    match gamma_key {
        "1.8" | "2.2" | "2.4" | "2.6" => {
            negpy_core::color::oetf_decode(&clipped, &mut out, gamma_key.parse().unwrap_or(1.0));
        }
        "srgb" => negpy_core::color::srgb_to_linear(&clipped, &mut out),
        "rec709" => negpy_core::color::piecewise_to_linear(
            &clipped,
            &mut out,
            negpy_core::color::Piecewise {
                cut: 0.081,
                slope: 4.5,
                offset: 0.099,
                scale: 1.099,
                gamma: 1.0 / 0.45,
            },
        ),
        "lstar" => negpy_core::color::piecewise_to_linear(
            &clipped,
            &mut out,
            negpy_core::color::Piecewise {
                cut: 0.08,
                slope: 9.0329,
                offset: 0.16,
                scale: 1.16,
                gamma: 3.0,
            },
        ),
        _ => return clipped,
    }
    out
}

/// Multiply every channel by one gain and clip to 0..1.
#[must_use]
pub fn expand(image: &[f32], factor: f64) -> Vec<f32> {
    let gains = [factor as f32; 3];
    let mut out = vec![0.0; image.len()];
    negpy_core::convert::scale_clip_unit(image, &gains, &mut out);
    out
}

/// Multiply a linear RGB buffer by the as-shot white-balance gains.
#[must_use]
pub fn apply_white_balance(image: &[f32], wb: CameraWb) -> Vec<f32> {
    let gains = normalize_wb_rgb(wb.as_shot).map(|value| value as f32);
    let mut out = vec![0.0; image.len()];
    negpy_core::convert::scale_clip_unit(image, &gains, &mut out);
    out
}

/// Why baking the as-shot gains cannot apply: `"trichrome"`, `"narrowband"`, or `""`.
#[must_use]
pub fn wb_bake_block_reason(trichrome: bool, process: Option<&ProcessConfig>) -> &'static str {
    if trichrome {
        return "trichrome";
    }
    if process.is_some_and(|process| process.sensor_matrix.is_some()) {
        return "narrowband";
    }
    ""
}

/// Quantize a float buffer to the 16-bit code values a linear dump stores.
#[must_use]
pub fn to_uint16(image: &[f32]) -> Vec<u16> {
    let mut out = vec![0u16; image.len()];
    negpy_core::convert::to_uint(image, &mut out);
    out
}

fn sensor_record(process: Option<&ProcessConfig>) -> Option<&'static str> {
    let process = process?;
    process.sensor_matrix.as_ref()?;
    Some(match process.sensor_unmix.as_str() {
        "linear" => "Linear",
        "density" => "Density",
        _ => "Two-Scale",
    })
}

/// The corrections a linear dump records in its description.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearBakes {
    pub wb_applied: bool,
    pub flatfield: bool,
    pub ice: bool,
}

/// The source and render choices one linear description records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearRecord<'a> {
    pub source_name: &'a str,
    pub camera_wb: Option<CameraWb>,
    pub expansion: f64,
    pub source_format: &'a str,
    pub sensor_applied: Option<&'a ProcessConfig>,
    pub gamma_key: &'a str,
    pub meta: Option<&'a SourceMeta>,
}

/// The processing record both linear writers stamp on their output.
#[must_use]
pub fn linear_description(record: LinearRecord<'_>, bakes: LinearBakes) -> String {
    let mut parts = vec![format!("source: {}", record.source_format)];
    if record.expansion > 1.0 {
        parts.push(format!("expansion: x{}", record.expansion));
    } else {
        parts.push("no scaling".to_string());
    }
    if record.gamma_key != "linear" {
        let label = TIFF_GAMMA_OPTIONS
            .iter()
            .find(|(key, _)| *key == record.gamma_key)
            .map_or(record.gamma_key, |(_, label)| label);
        parts.push(format!("linearized from {label}"));
    }
    if let Some(demosaic) = record.meta.and_then(|meta| meta.demosaic.as_deref()) {
        parts.push(format!("demosaic: {demosaic}"));
    }
    if let Some(wb) = record.camera_wb {
        let [r, g, b] = normalize_wb_rgb(wb.as_shot);
        let state = if bakes.wb_applied {
            "applied"
        } else {
            "no WB applied"
        };
        parts.push(format!("WB {state} (as-shot: {r:.3} {g:.3} {b:.3})"));
    }
    let sensor = sensor_record(record.sensor_applied);
    let mut corrections = Vec::new();
    if bakes.flatfield {
        corrections.push("flatfield");
    }
    if record.meta.is_some_and(|meta| meta.lens_corrected) {
        corrections.push("lens");
    }
    if let Some(method) = sensor {
        corrections.push(method);
    }
    if bakes.ice {
        corrections.push("ICE");
    }
    if !corrections.is_empty() {
        parts.push(format!("corrections: {}", corrections.join(", ")));
    }
    parts.push("no color management".to_string());
    let _ = record.source_name;
    format!("NegPy Linear Output -- {}.", parts.join(", "))
}

/// Write a float buffer as an untagged 16-bit TIFF.
#[must_use]
pub fn encode_linear_tiff(
    image: &[f32],
    width: usize,
    height: usize,
    resolution: Option<Resolution>,
    compression: TiffCompression,
) -> Option<Vec<u8>> {
    let codes = to_uint16(image);
    let mut bytes = vec![0u8; codes.len() * 2];
    for (chunk, code) in bytes.as_chunks_mut::<2>().0.iter_mut().zip(codes) {
        chunk.copy_from_slice(&code.to_ne_bytes());
    }
    encode_tiff(&bytes, width, height, 3, 16, compression, resolution, None)
}
