//! Mirrors `negpy/services/scanning/writer.py`: a scan written as a 16-bit TIFF, with an
//! IR sidecar and the mask of the IR pixels the scanner sampled.

use std::path::{Path, PathBuf};

use crate::infrastructure::loaders::constants::SUPPORTED_TIFF_EXTENSIONS;
use crate::services::export::encoders::{Resolution, TiffCompression, encode_tiff};
use crate::services::scanning::templating::{
    render_scan_filename, require_sequence_varying_scan_filename,
};

/// The extension used while a scan is written, so a watched folder does not index it.
pub const PART_SUFFIX: &str = ".part";
pub const MONO_TIFF: &str = "TIFF (mono)";

/// One scanner capture: 8- or 16-bit RGB, its IR plane and the IR validity mask.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanResult {
    pub rgb: Vec<u8>,
    pub ir: Option<Vec<u8>>,
    pub ir_valid_mask: Option<Vec<u8>>,
    pub dpi: i64,
    pub icc_profile: Option<Vec<u8>>,
}

fn to_uint16(samples: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for value in samples {
        let code = (*value as u16) << 8 | *value as u16;
        out.extend_from_slice(&code.to_ne_bytes());
    }
    out
}

fn to_grey(rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 2);
    for pixel in rgb.as_chunks::<3>().0 {
        let mean =
            ((pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) as f64 / 3.0).round() as u16;
        out.extend_from_slice(&mean.to_ne_bytes());
    }
    out
}

fn with_extension(path: &Path) -> PathBuf {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    if lower.ends_with(".tif") || lower.ends_with(".tiff") {
        return path.to_path_buf();
    }
    path.with_extension("tif")
}

fn ir_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}_IR.tif", path.with_extension("").display()))
}

fn valid_path(path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}_IR_VALID.tif",
        path.with_extension("").display()
    ))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut part = path.to_path_buf();
    part.set_extension(format!("tif{PART_SUFFIX}"));
    std::fs::write(&part, bytes)?;
    crate::kernel::system::paths::apply_umask_mode(&part)?;
    match crate::kernel::system::paths::replace_file(&part, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            crate::kernel::system::paths::safe_unlink(&part);
            Err(error)
        }
    }
}

/// Write one capture as a 16-bit TIFF. Zlib compression with a predictor, as the Python
/// writer sets. Returns the RGB path.
pub fn write_tiff_16bit(
    result: &ScanResult,
    path: &Path,
    width: usize,
    height: usize,
    mono: bool,
) -> Result<PathBuf, String> {
    if width == 0 || height == 0 {
        return Err("a scan needs a positive width and height".to_string());
    }
    let path = with_extension(path);
    let resolution = (result.dpi > 0).then_some(Resolution {
        x_dpi: result.dpi as f64,
        y_dpi: result.dpi as f64,
        unit: 2,
    });
    let (pixels, channels, icc) = if mono {
        (to_grey(&result.rgb), 1, None)
    } else {
        (to_uint16(&result.rgb), 3, result.icc_profile.as_deref())
    };
    let bytes = encode_tiff(
        &pixels,
        width,
        height,
        channels,
        16,
        TiffCompression::Deflate,
        resolution,
        icc,
    )
    .ok_or_else(|| "the scan buffer does not match its size".to_string())?;
    write_atomic(&path, &bytes).map_err(|error| error.to_string())?;
    if let Some(ir) = &result.ir {
        let bytes = encode_tiff(
            &to_uint16(ir),
            width,
            height,
            1,
            16,
            TiffCompression::Deflate,
            resolution,
            None,
        )
        .ok_or_else(|| "the IR buffer does not match its size".to_string())?;
        write_atomic(&ir_path(&path), &bytes).map_err(|error| error.to_string())?;
    }
    if let Some(mask) = &result.ir_valid_mask {
        let data: Vec<u8> = mask
            .iter()
            .map(|value| u8::from(*value != 0) * 255)
            .collect();
        let bytes = encode_tiff(
            &data,
            width,
            height,
            1,
            8,
            TiffCompression::Deflate,
            resolution,
            None,
        )
        .ok_or_else(|| "the IR mask does not match its size".to_string())?;
        write_atomic(&valid_path(&path), &bytes).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

fn sidecar_suffixes() -> Vec<String> {
    let mut suffixes: Vec<String> = SUPPORTED_TIFF_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_string())
        .collect();
    for token in ["_ir", "_ir_valid"] {
        for extension in SUPPORTED_TIFF_EXTENSIONS {
            suffixes.push(format!("{token}{extension}"));
        }
    }
    suffixes.sort();
    suffixes
}

/// `write_result`: choose a free scan basename and write the capture there.
pub fn write_scan(
    result: &ScanResult,
    output_folder: &Path,
    filename_pattern: &str,
    output_format: &str,
    date: &str,
    seq: Option<i64>,
    width: usize,
    height: usize,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(output_folder).map_err(|error| error.to_string())?;
    require_sequence_varying_scan_filename(filename_pattern, date, seq.unwrap_or(1))?;
    let suffixes = sidecar_suffixes();
    let mut current = seq.unwrap_or(1);
    let basename = loop {
        let basename = render_scan_filename(filename_pattern, date, current);
        let path = output_folder.join(&basename);
        let parent = path.parent().unwrap_or_else(|| Path::new(""));
        let taken: Vec<String> = std::fs::read_dir(parent)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.file_name().to_str().map(str::to_lowercase))
                    .collect()
            })
            .unwrap_or_default();
        let stem = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().to_lowercase());
        if !taken.iter().any(|name| {
            suffixes
                .iter()
                .any(|suffix| name == &format!("{stem}{suffix}"))
        }) {
            break basename;
        }
        current += 1;
    };
    let mono = output_format.eq_ignore_ascii_case(MONO_TIFF);
    write_tiff_16bit(result, &output_folder.join(basename), width, height, mono)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(width: usize, height: usize) -> ScanResult {
        let rgb = (0..width * height * 3)
            .map(|value| (value % 255) as u8)
            .collect();
        ScanResult {
            rgb,
            dpi: 300,
            ..ScanResult::default()
        }
    }

    #[test]
    fn an_eight_bit_sample_spans_the_sixteen_bit_range() {
        assert_eq!(to_uint16(&[0x12]), to_uint16(&[0x12]));
        assert_eq!(to_uint16(&[0x12]), vec![0x12, 0x12]);
    }

    #[test]
    fn a_grey_scan_averages_its_three_planes() {
        assert_eq!(to_grey(&[30, 40, 50]), vec![40, 0]);
    }

    #[test]
    fn a_scan_gains_its_tiff_extension() {
        assert_eq!(
            with_extension(Path::new("frame.psd")),
            PathBuf::from("frame.tif")
        );
        assert_eq!(
            with_extension(Path::new("frame.TIFF")),
            PathBuf::from("frame.TIFF")
        );
    }

    #[test]
    fn the_ir_sidecars_sit_beside_the_scan() {
        assert_eq!(
            ir_path(Path::new("/scans/frame.tif")),
            PathBuf::from("/scans/frame_IR.tif")
        );
        assert_eq!(
            valid_path(Path::new("/scans/frame.tif")),
            PathBuf::from("/scans/frame_IR_VALID.tif")
        );
    }

    #[test]
    fn a_written_scan_and_its_mask_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.tif");
        let mut capture = result(4, 3);
        capture.ir_valid_mask = Some(vec![1; 12]);
        let written = write_tiff_16bit(&capture, &path, 4, 3, false).unwrap();
        assert!(written.exists());
        assert!(dir.path().join("frame_IR_VALID.tif").exists());
    }
}
