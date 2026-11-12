//! The camera RAW boundary.
//!
//! A backend that supports the format returns the undeveloped sample buffer. A
//! backend that cannot returns a reason a photographer can act on.

use std::path::Path;

use crate::infrastructure::loaders::image_buffer::LoadedImage;

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
