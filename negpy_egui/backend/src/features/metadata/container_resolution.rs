//! Resolution held by the container rather than by EXIF.
//!
//! Mirrors the container half of `negpy/features/metadata/resolution.py`: a JPEG's
//! JFIF density and a PNG's pHYs chunk, in the file's own unit.

use std::path::Path;

use super::resolution::{CENTIMETRE, INCH, Resolution};

const JFIF_INCH: u8 = 1;
const JFIF_CENTIMETRE: u8 = 2;

/// The resolution stated by a container header, or None.
#[must_use]
pub fn from_container(path: &Path) -> Option<Resolution> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return from_jfif(&bytes);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return from_png(&bytes);
    }
    None
}

/// A JPEG's JFIF density, in the numbers and unit it states.
#[must_use]
pub fn from_jfif(data: &[u8]) -> Option<Resolution> {
    let mut index = 2usize;
    while index + 4 <= data.len() && data[index] == 0xFF {
        let marker = data[index + 1];
        let length = u16::from_be_bytes(data.get(index + 2..index + 4)?.try_into().ok()?) as usize;
        if marker == 0xE0 && data.get(index + 4..index + 9) == Some(b"JFIF\0") {
            let unit = *data.get(index + 11)?;
            let x = u16::from_be_bytes(data.get(index + 12..index + 14)?.try_into().ok()?);
            let y = u16::from_be_bytes(data.get(index + 14..index + 16)?.try_into().ok()?);
            let unit = match unit {
                JFIF_INCH => INCH,
                JFIF_CENTIMETRE => CENTIMETRE,
                _ => return None,
            };
            if x == 0 || y == 0 {
                return None;
            }
            return Some(Resolution {
                x: (i64::from(x), 1),
                y: (i64::from(y), 1),
                unit,
            });
        }
        if marker == 0xDA {
            break;
        }
        index = index.checked_add(2 + length)?;
    }
    None
}

/// A PNG's pHYs chunk, in pixels per metre or as an aspect ratio.
#[must_use]
pub fn from_png(data: &[u8]) -> Option<Resolution> {
    let mut index = 8usize;
    while index + 12 <= data.len() {
        let length = u32::from_be_bytes(data.get(index..index + 4)?.try_into().ok()?) as usize;
        let kind = data.get(index + 4..index + 8)?;
        let body = data.get(index + 8..index + 8 + length)?;
        if kind == b"pHYs" && body.len() >= 9 {
            let unit = body[8];
            if unit != 1 {
                return None;
            }
            let x = u32::from_be_bytes(body[0..4].try_into().ok()?);
            let y = u32::from_be_bytes(body[4..8].try_into().ok()?);
            if x == 0 || y == 0 {
                return None;
            }
            // Pixels per metre, with unitless resolution's placeholder unit until a
            // caller asks for inches or centimetres.
            return Some(Resolution {
                x: (i64::from(x), 1),
                y: (i64::from(y), 1),
                unit: 0,
            });
        }
        if kind == b"IDAT" {
            break;
        }
        index = index.checked_add(12 + length)?;
    }
    None
}
