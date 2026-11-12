//! ISOBMFF box access for JPEG XL files: the container that holds Exif and XMP.
//!
//! Mirrors `negpy/infrastructure/loaders/jxl_boxes.py`.

use std::io::Read;

/// The JPEG XL container signature.
pub const JXL_SIGNATURE: &[u8] = b"\x00\x00\x00\x0cJXL \x0d\x0a\x87\x0a";
/// A bare JPEG XL codestream signature.
pub const JXL_CODESTREAM: &[u8] = b"\xff\x0a";
pub const JXL_EXIF_BOX: &[u8; 4] = b"Exif";
pub const JXL_XMP_BOX: &[u8; 4] = b"xml ";

/// A box's type and payload range, with the header excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JxlBox {
    pub kind: [u8; 4],
    pub start: usize,
    pub end: usize,
}

/// Whether `data` starts with a JPEG XL container or codestream signature.
#[must_use]
pub fn is_jxl(data: &[u8]) -> bool {
    data.starts_with(JXL_SIGNATURE) || data.starts_with(JXL_CODESTREAM)
}

/// Split a JPEG XL container into boxes. The boxes must tile the file exactly.
pub fn jxl_boxes(data: &[u8]) -> Result<Vec<JxlBox>, &'static str> {
    let mut boxes = Vec::new();
    let mut position = 0usize;
    while position < data.len() {
        if position + 8 > data.len() {
            return Err("truncated JPEG XL box header");
        }
        let size = u32::from_be_bytes(data[position..position + 4].try_into().unwrap()) as usize;
        let kind = data[position + 4..position + 8].try_into().unwrap();
        let (size, header) = if size == 1 {
            if position + 16 > data.len() {
                return Err("truncated JPEG XL box header");
            }
            let size = u64::from_be_bytes(data[position + 8..position + 16].try_into().unwrap());
            let size = usize::try_from(size).map_err(|_| "malformed JPEG XL box size")?;
            (size, 16)
        } else if size == 0 {
            (data.len() - position, 8)
        } else {
            (size, 8)
        };
        if size < header
            || position
                .checked_add(size)
                .is_none_or(|end| end > data.len())
        {
            return Err("malformed JPEG XL box size");
        }
        boxes.push(JxlBox {
            kind,
            start: position + header,
            end: position + size,
        });
        position += size;
    }
    Ok(boxes)
}

fn brotli_decode(data: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut input = data;
    brotli_decompressor::BrotliDecompress(&mut input, &mut output).ok()?;
    Some(output)
}

fn box_payload(data: &[u8], wanted: &[u8; 4]) -> Option<Vec<u8>> {
    let boxes = jxl_boxes(data).ok()?;
    for entry in boxes {
        if entry.kind == *wanted {
            return Some(data[entry.start..entry.end].to_vec());
        }
        if entry.kind == *b"brob" && data.get(entry.start..entry.start + 4) == Some(wanted) {
            return brotli_decode(&data[entry.start + 4..entry.end]);
        }
    }
    None
}

/// The EXIF payload of a JPEG XL file, from the TIFF header on.
#[must_use]
pub fn read_jxl_exif(data: &[u8]) -> Option<Vec<u8>> {
    let payload = box_payload(data, JXL_EXIF_BOX)?;
    if payload.len() < 8 {
        return None;
    }
    let offset = u32::from_be_bytes(payload[0..4].try_into().ok()?) as usize;
    let tiff = payload.get(4 + offset..)?;
    (tiff.starts_with(b"MM") || tiff.starts_with(b"II")).then(|| tiff.to_vec())
}

/// The XMP payload of a JPEG XL file.
#[must_use]
pub fn read_jxl_xmp(data: &[u8]) -> Option<Vec<u8>> {
    box_payload(data, JXL_XMP_BOX)
}

/// Read a whole JPEG XL file and return its EXIF payload.
#[must_use]
pub fn read_jxl_exif_file(path: &std::path::Path) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .read_to_end(&mut data)
        .ok()?;
    read_jxl_exif(&data)
}
