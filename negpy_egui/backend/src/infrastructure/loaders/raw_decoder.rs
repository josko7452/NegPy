//! The camera RAW boundary.
//!
//! A backend that supports the format returns the undeveloped sample buffer. A
//! backend that cannot returns a reason a photographer can act on.

use std::path::Path;

use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use rawler::imgop::xyz::Illuminant;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};

/// A camera RAW decoder.
pub trait RawDecoder {
    /// Decode a camera RAW for the pipeline.
    fn decode(&self, path: &Path) -> Result<LoadedImage, String>;
}

/// The decoder this build provides. Camera RAW demosaic needs a LibRaw-compatible
/// crate, so this backend reports the unsupported case instead of guessing.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedRawDecoder;

impl RawDecoder for UnsupportedRawDecoder {
    fn decode(&self, path: &Path) -> Result<LoadedImage, String> {
        Err(unsupported_raw_reason(path).unwrap_or_else(|| {
            "This camera RAW format is not supported by this build.".to_string()
        }))
    }
}

/// A camera RAW decoder backed by `rawler`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RawlerDecoder;

impl RawDecoder for RawlerDecoder {
    fn decode(&self, path: &Path) -> Result<LoadedImage, String> {
        if let Some(reason) = unsupported_raw_reason(path) {
            return Err(reason);
        }
        let raw = rawler::decode_file(path).map_err(|error| error.to_string())?;
        let developed = RawDevelop::new_with(&[
            ProcessingStep::Rescale,
            ProcessingStep::Demosaic,
            ProcessingStep::FujiRotate,
            ProcessingStep::CropActiveArea,
            ProcessingStep::CropDefault,
        ])
        .develop_intermediate(&raw)
        .map_err(|error| error.to_string())?;

        let (width, height, mut pixels) = match developed {
            Intermediate::ThreeColor(image) => (image.width, image.height, image.into_flatten()),
            Intermediate::FourColor(image) => {
                let width = image.width;
                let height = image.height;
                let pixels = image
                    .into_inner()
                    .into_iter()
                    .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect();
                (width, height, pixels)
            }
            Intermediate::Monochrome(image) => {
                let width = image.width;
                let height = image.height;
                let pixels = image
                    .data
                    .into_iter()
                    .flat_map(|value| [value, value, value])
                    .collect();
                (width, height, pixels)
            }
        };

        let gains = white_balance(&raw.wb_coeffs);
        if let Some([red, green, blue]) = gains {
            for pixel in pixels.chunks_exact_mut(3) {
                pixel[0] = (pixel[0] * red).clamp(0.0, 1.0);
                pixel[1] = (pixel[1] * green).clamp(0.0, 1.0);
                pixel[2] = (pixel[2] * blue).clamp(0.0, 1.0);
            }
        }

        let mut image = ImageBuffer::from_rgb(width, height, pixels);
        image.rgb_xyz_matrix = camera_matrix(&raw);
        Ok(LoadedImage {
            image: Some(image),
            orientation: read_orientation(path),
            color_space: None,
            ..LoadedImage::default()
        })
    }
}

fn white_balance(coeffs: &[f32; 4]) -> Option<[f32; 3]> {
    let [red, green, blue, _] = *coeffs;
    [red, green, blue]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && green > 0.0)
        .then_some([red / green, 1.0, blue / green])
}

fn camera_matrix(raw: &rawler::RawImage) -> Option<[[f32; 3]; 3]> {
    let (_, matrix) = raw.color_matrix_find_first([
        Illuminant::D65,
        Illuminant::A,
        Illuminant::B,
        Illuminant::C,
        Illuminant::D50,
        Illuminant::D55,
        Illuminant::D75,
        Illuminant::Daylight,
        Illuminant::Flash,
    ])?;
    if matrix.len() < 9 {
        return None;
    }
    let mut out = [[0.0; 3]; 3];
    for (row, values) in out.iter_mut().enumerate() {
        values.copy_from_slice(&matrix[row * 3..row * 3 + 3]);
    }
    out.iter()
        .flatten()
        .all(|value| value.is_finite())
        .then_some(out)
}

const NEF_COMPRESSED: u16 = 34_713;
const TICORAW_MARKER: &[u8] = b"INTOPIX";
const STRIP_HEAD_BYTES: usize = 64;
const TAG_COMPRESSION: u16 = 259;
const TAG_STRIP_OFFSETS: u16 = 273;
const TAG_SUB_IFDS: u16 = 330;

/// The Nikon High Efficiency reason, when the payload is TicoRAW.
///
/// The Compression tag reads the same as a lossless NEF, so only the strip head
/// tells them apart.
#[must_use]
pub fn unsupported_raw_reason(path: &Path) -> Option<String> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("nef"))
    {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let tiff = Tiff::new(&data)?;
    let first = tiff.ifd(tiff.u32(4)? as usize)?;
    for subifd in first.subifds {
        let Some(info) = tiff.ifd(subifd as usize) else {
            continue;
        };
        if info.compression != Some(NEF_COMPRESSED) {
            continue;
        }
        let Some(offset) = info.strip_offsets.first() else {
            continue;
        };
        let Some(head) = data.get(*offset as usize..) else {
            continue;
        };
        let head = &head[..head.len().min(STRIP_HEAD_BYTES)];
        if head
            .windows(TICORAW_MARKER.len())
            .any(|window| window == TICORAW_MARKER)
        {
            return Some(
                "Nikon High Efficiency (HE) raw is not supported. Re-shoot as Lossless Compressed, or convert to DNG."
                    .to_string(),
            );
        }
    }
    None
}

struct Ifd {
    compression: Option<u16>,
    strip_offsets: Vec<u64>,
    subifds: Vec<u32>,
}

struct Tiff<'a> {
    data: &'a [u8],
    little: bool,
}

impl<'a> Tiff<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let little = match data.get(..2)? {
            b"II" => true,
            b"MM" => false,
            _ => return None,
        };
        Some(Self { data, little })
    }

    fn u16(&self, offset: usize) -> Option<u16> {
        let bytes = self.data.get(offset..offset.checked_add(2)?)?;
        Some(if self.little {
            u16::from_le_bytes(bytes.try_into().ok()?)
        } else {
            u16::from_be_bytes(bytes.try_into().ok()?)
        })
    }

    fn u32(&self, offset: usize) -> Option<u32> {
        let bytes = self.data.get(offset..offset.checked_add(4)?)?;
        Some(if self.little {
            u32::from_le_bytes(bytes.try_into().ok()?)
        } else {
            u32::from_be_bytes(bytes.try_into().ok()?)
        })
    }

    fn values(&self, entry: usize, kind: u16, count: u32) -> Vec<u64> {
        let width: usize = match kind {
            3 => 2,
            4 | 13 => 4,
            _ => return Vec::new(),
        };
        let Some(count) = usize::try_from(count).ok() else {
            return Vec::new();
        };
        let Some(value_bytes) = width.checked_mul(count) else {
            return Vec::new();
        };
        let start = if value_bytes <= 4 {
            entry.checked_add(8)
        } else {
            entry
                .checked_add(8)
                .and_then(|offset| self.u32(offset))
                .map(|offset| offset as usize)
        };
        let Some(start) = start else {
            return Vec::new();
        };
        (0..count)
            .filter_map(|index| {
                let offset = start.checked_add(index.checked_mul(width)?)?;
                match width {
                    2 => self.u16(offset).map(u64::from),
                    4 => self.u32(offset).map(u64::from),
                    _ => None,
                }
            })
            .collect()
    }

    fn ifd(&self, offset: usize) -> Option<Ifd> {
        let count = self.u16(offset)?;
        let mut compression = None;
        let mut strip_offsets = Vec::new();
        let mut subifds = Vec::new();
        for index in 0..count {
            let entry = offset
                .checked_add(2)?
                .checked_add(usize::from(index).checked_mul(12)?)?;
            let tag = self.u16(entry)?;
            let kind = self.u16(entry.checked_add(2)?)?;
            let count = self.u32(entry.checked_add(4)?)?;
            let values = self.values(entry, kind, count);
            match tag {
                TAG_COMPRESSION if count == 1 => {
                    compression = values.first().and_then(|value| u16::try_from(*value).ok());
                }
                TAG_STRIP_OFFSETS => strip_offsets = values,
                TAG_SUB_IFDS => {
                    subifds = values
                        .into_iter()
                        .filter_map(|value| u32::try_from(value).ok())
                        .collect();
                }
                _ => {}
            }
        }
        Some(Ifd {
            compression,
            strip_offsets,
            subifds,
        })
    }
}
