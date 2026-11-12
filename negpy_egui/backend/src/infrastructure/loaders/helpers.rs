//! Embedded profile names and EXIF orientation.
//!
//! Mirrors the metadata half of `negpy/infrastructure/loaders/helpers.py`.

use std::path::Path;

fn utf16be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn tag_table(icc: &[u8]) -> Vec<([u8; 4], usize, usize)> {
    if icc.len() < 132 {
        return Vec::new();
    }
    let count = u32::from_be_bytes(icc[128..132].try_into().unwrap()) as usize;
    (0..count)
        .filter_map(|index| {
            let start = 132 + index * 12;
            let entry = icc.get(start..start + 12)?;
            let kind = entry[0..4].try_into().ok()?;
            let offset = u32::from_be_bytes(entry[4..8].try_into().ok()?) as usize;
            let size = u32::from_be_bytes(entry[8..12].try_into().ok()?) as usize;
            Some((kind, offset, size))
        })
        .collect()
}

fn profile_description(icc: &[u8]) -> Option<String> {
    let (_, offset, size) = tag_table(icc)
        .into_iter()
        .find(|(kind, _, _)| kind == b"desc")?;
    let tag = icc.get(offset..offset.checked_add(size)?)?;
    match &tag[0..4] {
        b"desc" => {
            let end = tag[8..].iter().position(|byte| *byte == 0)?;
            Some(String::from_utf8_lossy(&tag[8..8 + end]).into_owned())
        }
        b"text" => {
            let end = tag[8..].iter().position(|byte| *byte == 0)?;
            Some(String::from_utf8_lossy(&tag[8..8 + end]).into_owned())
        }
        b"mluc" => {
            let count = u32::from_be_bytes(tag[8..12].try_into().ok()?) as usize;
            for index in 0..count {
                let record = tag.get(16 + index * 12..28 + index * 12)?;
                let length = u32::from_be_bytes(record[4..8].try_into().ok()?) as usize;
                let start = u32::from_be_bytes(record[8..12].try_into().ok()?) as usize;
                if let Some(text) = tag.get(start..start.checked_add(length)?) {
                    return Some(utf16be(text));
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve a known color-space name from an embedded ICC profile.
#[must_use]
pub fn identify_color_space_from_icc(icc: &[u8]) -> Option<&'static str> {
    let description = profile_description(icc)?.to_lowercase();
    let description = description.trim();
    match description {
        "a98c" => return Some("Adobe RGB"),
        "romm" => return Some("ProPhoto RGB"),
        "2020" => return Some("Rec 2020"),
        "sp3" => return Some("P3 D65"),
        _ => {}
    }
    if description.contains("prophoto") {
        Some("ProPhoto RGB")
    } else if description.contains("rec. 2020")
        || description.contains("rec2020")
        || description.contains("bt.2020")
    {
        Some("Rec 2020")
    } else if description.contains("display p3") || description.contains("p3 d65") {
        Some("P3 D65")
    } else if description.contains("aces") {
        Some("ACES")
    } else if description.contains("adobe rgb") || description.contains("adobe compat") {
        Some("Adobe RGB")
    } else if description.contains("srgb")
        || description.contains("iec 61966")
        || description.contains("iec61966")
    {
        Some("sRGB")
    } else {
        None
    }
}

fn tiff_orientation(data: &[u8]) -> Option<u8> {
    let little = data.starts_with(b"II");
    if !little && !data.starts_with(b"MM") {
        return None;
    }
    let u16_at = |offset: usize| -> Option<u16> {
        let bytes = data.get(offset..offset + 2)?;
        Some(if little {
            u16::from_le_bytes(bytes.try_into().ok()?)
        } else {
            u16::from_be_bytes(bytes.try_into().ok()?)
        })
    };
    let u32_at = |offset: usize| -> Option<u32> {
        let bytes = data.get(offset..offset + 4)?;
        Some(if little {
            u32::from_le_bytes(bytes.try_into().ok()?)
        } else {
            u32::from_be_bytes(bytes.try_into().ok()?)
        })
    };
    let first_ifd = u32_at(4)? as usize;
    let count = u16_at(first_ifd)? as usize;
    for index in 0..count {
        let entry = first_ifd + 2 + index * 12;
        if u16_at(entry)? == 274 && u16_at(entry + 2)? == 3 && u32_at(entry + 4)? == 1 {
            let value = u16_at(entry + 8)?;
            return (1..=8).contains(&value).then_some(value as u8);
        }
    }
    None
}

fn jpeg_exif(data: &[u8]) -> Option<&[u8]> {
    if !data.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut position = 2;
    while position + 4 <= data.len() {
        if data[position] != 0xff {
            return None;
        }
        let marker = data[position + 1];
        if marker == 0xd9 {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            position += 2;
            continue;
        }
        let length = u16::from_be_bytes([data[position + 2], data[position + 3]]) as usize;
        if length < 2 || position + 2 + length > data.len() {
            return None;
        }
        let payload = &data[position + 4..position + 2 + length];
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            return Some(&payload[6..]);
        }
        if marker == 0xda {
            return None;
        }
        position += 2 + length;
    }
    None
}

/// Read the EXIF orientation (1-8) from TIFF, JPEG or JPEG XL. Other files stay upright.
#[must_use]
pub fn read_orientation(path: &Path) -> u8 {
    let Ok(data) = std::fs::read(path) else {
        return 1;
    };
    if let Some(orientation) = tiff_orientation(&data) {
        return orientation;
    }
    if let Some(exif) = jpeg_exif(&data).and_then(tiff_orientation) {
        return exif;
    }
    crate::infrastructure::loaders::jxl_boxes::read_jxl_exif(&data)
        .and_then(|exif| tiff_orientation(&exif))
        .unwrap_or(1)
}
