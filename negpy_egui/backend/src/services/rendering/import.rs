//! Mirrors the headless half of `ImageProcessor._decode_oriented_f32` and
//! `_load_source_f32`: decode a source, orient it, flat-field it, optionally unmix the
//! sensor, then report the film process and the clear base the render will read.

use std::path::Path;
use std::sync::Mutex;

use crate::domain::models::{ProcessMode, WorkspaceConfig, is_rgb_triplet, stitch_has_triplets};
use crate::features::flatfield::logic::GainEntry;
use crate::features::process::detect::{detect_process_and_source, detect_process_mode};
use crate::features::process::path::positive_decode;
use crate::features::process::sensor::{apply_sensor_correction, effective_sensor_matrix};
use crate::infrastructure::loaders::factory::{self, LoadOptions};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::services::assets::half_frame::HalfSlice;
use crate::services::rendering::base_read::base_log_for;
use crate::services::rendering::lens::flatfield_source;

/// The last corrected source, keyed by everything its decode read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SourceCacheKey {
    path: String,
    modified_nanos: u128,
    source_token: String,
    sensor_token: String,
    flatfield_token: String,
    gain_slices: Vec<(i32, u64, Option<[u64; 4]>, u64, char)>,
}

/// The corrected source and the facts the pipeline reads beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedSource {
    pub image: ImageBuffer,
    pub orientation: u8,
    pub color_space: Option<String>,
    pub source_encoding: Option<String>,
    /// The oriented IR plane, when the file carried one.
    pub ir: Option<Vec<f32>>,
    pub ir_width: usize,
    pub ir_height: usize,
}

/// One-slot memo of the decoded and pre-corrected export source.
#[derive(Default)]
pub struct SourceCache {
    entry: Mutex<Option<(SourceCacheKey, CachedSource)>>,
}

impl SourceCache {
    /// A source cache with no entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop the memo, as releasing the active source does.
    pub fn clear(&self) {
        if let Ok(mut entry) = self.entry.lock() {
            *entry = None;
        }
    }

    /// Whether an entry is memoized.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.entry
            .lock()
            .is_ok_and(|entry| entry.as_ref().is_some())
    }

    /// Decode, or return the memoized source when nothing it reads changed.
    pub fn load(
        &self,
        path: &Path,
        config: &WorkspaceConfig,
        entry: Option<&GainEntry>,
        gain_slices: &[HalfSlice],
    ) -> Result<CachedSource, String> {
        let key = cache_key(path, config, entry, gain_slices)?;
        if let Ok(guard) = self.entry.lock()
            && let Some((cached_key, cached)) = guard.as_ref()
            && *cached_key == key
        {
            return Ok(cached.clone());
        }
        let loaded = factory::load(
            path,
            LoadOptions {
                linear_raw: config.process.linear_raw,
                positive_decode: positive_decode(&config.process),
                load_ir: true,
            },
        )?;
        let decoded = loaded
            .image
            .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
        let image = oriented(&decoded, loaded.orientation);
        let image = precorrect_source(&image, config, entry, gain_slices, loaded.orientation);
        let (ir, ir_width, ir_height) = loaded.ir.map_or((None, 0, 0), |ir| {
            let oriented = orient_ir(&ir, loaded.ir_width, loaded.ir_height, loaded.orientation);
            (Some(oriented), loaded.ir_width, loaded.ir_height)
        });
        let cached = CachedSource {
            image,
            orientation: loaded.orientation,
            color_space: loaded.color_space,
            source_encoding: loaded.source_encoding,
            ir,
            ir_width,
            ir_height,
        };
        if let Ok(mut guard) = self.entry.lock() {
            *guard = Some((key, cached.clone()));
        }
        Ok(cached)
    }
}

fn cache_key(
    path: &Path,
    config: &WorkspaceConfig,
    entry: Option<&GainEntry>,
    gain_slices: &[HalfSlice],
) -> Result<SourceCacheKey, String> {
    let modified_nanos = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    Ok(SourceCacheKey {
        path: path.display().to_string(),
        modified_nanos,
        source_token: crate::services::rendering::source_identity::source_token(config, entry),
        sensor_token: crate::features::process::sensor::sensor_token(&config.process),
        flatfield_token: crate::features::flatfield::logic::flatfield_token(
            &config.flatfield,
            entry,
        ),
        gain_slices: gain_slices
            .iter()
            .map(|slice| {
                (
                    slice.half,
                    slice.split_x.to_bits(),
                    slice
                        .crop_rect
                        .map(|rect| rect.map(|value| value.to_bits())),
                    slice.gutter_thickness.to_bits(),
                    slice.split_axis,
                )
            })
            .collect(),
    })
}

/// What a decode produced: the oriented, pre-corrected linear frame and its source facts.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSource {
    pub image: ImageBuffer,
    pub process_mode: ProcessMode,
    pub source_color: bool,
    pub base_log: Option<[f64; 3]>,
    pub orientation: u8,
    pub color_space: Option<String>,
    pub source_encoding: Option<String>,
    /// The oriented IR plane, when the file carried one.
    pub ir: Option<Vec<f32>>,
    pub ir_width: usize,
    pub ir_height: usize,
}

/// Whether a preview may take the sensor unmix. Triplet composites take each channel from
/// its own single-band exposure, so unmixing them injects crosstalk that was never captured.
#[must_use]
pub fn preview_takes_unmix(config: &WorkspaceConfig) -> bool {
    !is_rgb_triplet(&config.rgbscan) && !stitch_has_triplets(&config.stitch)
}

/// `apply_exif_orientation` for a one-band plane.
#[must_use]
pub fn orient_ir(ir: &[f32], width: usize, height: usize, orientation: u8) -> Vec<f32> {
    if orientation < 2 || width == 0 || height == 0 {
        return ir.to_vec();
    }
    let bytes: Vec<u8> = ir.iter().flat_map(|value| value.to_ne_bytes()).collect();
    let (out_height, out_width) = negpy_core::orient::oriented_dims(height, width, orientation);
    let mut dst = vec![0u8; out_height * out_width * 4];
    negpy_core::orient::orient(&bytes, height, width, 4, orientation, &mut dst);
    dst.chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// `apply_exif_orientation` for a float frame.
#[must_use]
pub fn oriented(image: &ImageBuffer, orientation: u8) -> ImageBuffer {
    if orientation < 2 {
        return image.clone();
    }
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect();
    let (height, width) = negpy_core::orient::oriented_dims(image.height, image.width, orientation);
    let mut dst = vec![0u8; height * width * 12];
    negpy_core::orient::orient(&bytes, image.height, image.width, 12, orientation, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}

/// Flat-field, then the sensor unmix: the source pre-corrections, before geometry and crop.
#[must_use]
pub fn precorrect_source(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    entry: Option<&GainEntry>,
    gain_slices: &[HalfSlice],
    orientation: u8,
) -> ImageBuffer {
    let image = flatfield_source(image, config, entry, gain_slices, Some(orientation));
    if preview_takes_unmix(config) {
        return apply_sensor_correction(
            &image,
            effective_sensor_matrix(&config.process),
            config.process.sensor_unmix,
        );
    }
    image
}

/// Decode one file, orient it, then flat-field and unmix it as the render source reads.
pub fn import_source(
    path: &Path,
    config: &WorkspaceConfig,
    entry: Option<&GainEntry>,
    gain_slices: &[HalfSlice],
) -> Result<ImportedSource, String> {
    let loaded = factory::load(
        path,
        LoadOptions {
            linear_raw: config.process.linear_raw,
            positive_decode: positive_decode(&config.process),
            load_ir: true,
        },
    )?;
    let decoded = loaded
        .image
        .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
    let image = oriented(&decoded, loaded.orientation);
    let image = precorrect_source(&image, config, entry, gain_slices, loaded.orientation);
    let (process_mode, source_color) =
        detect_process_and_source(&image.pixels, image.width, image.height);
    let base_log = if crate::features::process::path::render_path(&config.process)
        == crate::features::process::path::RenderPath::Print
    {
        base_log_for(Some(&source_key(path, config)), &image)
    } else {
        None
    };
    let (ir, ir_width, ir_height) = loaded.ir.map_or((None, 0, 0), |ir| {
        (
            Some(orient_ir(
                &ir,
                loaded.ir_width,
                loaded.ir_height,
                loaded.orientation,
            )),
            loaded.ir_width,
            loaded.ir_height,
        )
    });
    Ok(ImportedSource {
        image,
        process_mode,
        source_color,
        base_log,
        orientation: loaded.orientation,
        color_space: loaded.color_space,
        source_encoding: loaded.source_encoding,
        ir,
        ir_width,
        ir_height,
    })
}

fn source_key(path: &Path, config: &WorkspaceConfig) -> String {
    format!(
        "{}|{}|{}",
        path.display(),
        crate::services::rendering::source_identity::source_token(config, None),
        u8::from(config.process.sensor_unmix != crate::domain::models::SensorUnmix::Linear),
    )
}

/// The process mode a stored frame reads, else the scan's own detection.
#[must_use]
pub fn effective_process_mode(stored: Option<ProcessMode>, image: &ImageBuffer) -> ProcessMode {
    stored.unwrap_or_else(|| detect_process_mode(&image.pixels, image.width, image.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: f32) -> ImageBuffer {
        ImageBuffer::from_rgb(8, 6, vec![value; 8 * 6 * 3])
    }

    #[test]
    fn an_upright_frame_keeps_its_pixels() {
        let image = frame(0.4);
        assert_eq!(oriented(&image, 1), image);
    }

    #[test]
    fn a_triplet_skips_the_unmix() {
        let mut config = WorkspaceConfig::default();
        config.rgbscan.enabled = true;
        config.rgbscan.green_path = "/g.dng".to_string();
        config.rgbscan.blue_path = "/b.dng".to_string();
        assert!(!preview_takes_unmix(&config));
    }

    #[test]
    fn a_plain_scan_takes_the_unmix() {
        assert!(preview_takes_unmix(&WorkspaceConfig::default()));
    }

    #[test]
    fn a_decoded_frame_reports_its_process_and_base() {
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
        if !fixture.exists() {
            return;
        }
        let config = WorkspaceConfig::default();
        let source = import_source(&fixture, &config, None, &[]).expect("a decodable fixture");
        assert!(source.image.width > 0);
        assert!(source.image.height > 0);
        let detected = detect_process_mode(
            &source.image.pixels,
            source.image.width,
            source.image.height,
        );
        assert_eq!(source.process_mode, detected);
    }

    #[test]
    fn a_stored_mode_wins_over_detection() {
        let image = frame(0.4);
        assert_eq!(
            effective_process_mode(Some(ProcessMode::BW), &image),
            ProcessMode::BW
        );
    }
}
