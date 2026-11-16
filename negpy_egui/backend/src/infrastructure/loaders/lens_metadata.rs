//! Embedded lens metadata readers for Sony ARW, DNG and Panasonic RW2.
//!
//! Mirrors `negpy/infrastructure/loaders/lens_metadata.py`. Reading is bounded by the
//! existing loader limits: no pixel decode and no unbounded opcode list.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::domain::models::{IDENTITY, LensMetadata, LensWarp};
use crate::infrastructure::loaders::tiff_walker::{self, DirectoryTags, TagValue};

const MAX_OPCODE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_OPCODES: u32 = 32;
const WARP_RECTILINEAR: u32 = 1;
const WARP_VERSION: u32 = 0x0103_0000;

/// The DNG OpcodeList3 tag.
pub const OPCODE_LIST_3: u16 = 51022;
/// Active image area.
pub const ACTIVE_AREA: u16 = 50829;
/// Default crop origin and size.
pub const DEFAULT_CROP_ORIGIN: u16 = 50719;
pub const DEFAULT_CROP_SIZE: u16 = 50720;

/// Read DNG WarpRectilinear opcodes. An identity list is empty; a malformed or
/// unsupported list is an error, because a partial correction is worse than none.
pub fn parse_opcodes(data: &[u8]) -> Result<Vec<LensWarp>, String> {
    if data.len() < 4 || data.len() as u64 > MAX_OPCODE_BYTES {
        return Err("Invalid DNG correction data size.".to_string());
    }
    let count = u32::from_be_bytes(data[..4].try_into().unwrap());
    if count > MAX_OPCODES {
        return Err("Too many DNG correction instructions.".to_string());
    }
    let mut offset = 4usize;
    let mut warps = Vec::new();
    for _ in 0..count {
        let header = data
            .get(offset..offset + 16)
            .ok_or_else(|| "Incomplete DNG correction header.".to_string())?;
        let opcode = u32::from_be_bytes(header[0..4].try_into().unwrap());
        let version = u32::from_be_bytes(header[4..8].try_into().unwrap());
        let flags = u32::from_be_bytes(header[8..12].try_into().unwrap());
        let size = u32::from_be_bytes(header[12..16].try_into().unwrap());
        offset += 16;
        let end = offset
            .checked_add(size as usize)
            .ok_or_else(|| "Invalid DNG correction instruction.".to_string())?;
        if end > data.len() || flags & !3 != 0 {
            return Err("Invalid DNG correction instruction.".to_string());
        }
        if opcode != WARP_RECTILINEAR || version > WARP_VERSION {
            return Err(
                "Unsupported DNG correction instruction (requires WarpRectilinear).".to_string(),
            );
        }
        if size < 4 {
            return Err("Incomplete DNG warp.".to_string());
        }
        let planes = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        if !matches!(planes, 1 | 3) || size != 4 + 48 * planes + 16 {
            return Err("Unsupported DNG warp plane count or size.".to_string());
        }
        let value_count = (6 * planes + 2) as usize;
        let mut values = Vec::with_capacity(value_count);
        for index in 0..value_count {
            let at = offset + 4 + index * 8;
            values.push(f64::from_be_bytes(data[at..at + 8].try_into().unwrap()));
        }
        if !values.iter().all(|value| value.is_finite()) {
            return Err("Invalid DNG warp coefficients or optical center.".to_string());
        }
        let center = (values[value_count - 2], values[value_count - 1]);
        if !centre_valid(center) {
            return Err("Invalid DNG warp coefficients or optical center.".to_string());
        }
        let mut coefficients = Vec::with_capacity(planes as usize);
        for plane in 0..planes as usize {
            coefficients.push([
                values[plane * 6],
                values[plane * 6 + 1],
                values[plane * 6 + 2],
                values[plane * 6 + 3],
                values[plane * 6 + 4],
                values[plane * 6 + 5],
            ]);
        }
        check_warp(center, &coefficients)?;
        if coefficients
            .iter()
            .any(|coefficient| *coefficient != IDENTITY)
        {
            warps.push(LensWarp::Rectilinear {
                coefficients,
                center,
            });
        }
        offset = end;
    }
    if offset != data.len() {
        return Err("Trailing data in DNG correction list.".to_string());
    }
    Ok(warps)
}

fn centre_valid(center: (f64, f64)) -> bool {
    (0.0..=1.0).contains(&center.0) && (0.0..=1.0).contains(&center.1)
}

fn check_warp(center: (f64, f64), coefficients: &[[f64; 6]]) -> Result<(), String> {
    for coefficient in coefficients {
        if coefficient.iter().any(|value| value.abs() > 16.0) {
            return Err("DNG warp coefficients are out of range.".to_string());
        }
        let [k0, k1, k2, k3, t0, t1] = *coefficient;
        let tangential = 6.0 * (t0.abs() + t1.abs());
        for index in 0..257 {
            let r = index as f64 / 256.0;
            let derivative = k0 + r * (3.0 * k1 + r * (5.0 * k2 + 7.0 * k3 * r));
            if derivative <= tangential {
                return Err("DNG warp may fold the image.".to_string());
            }
        }
    }
    if !centre_valid(center) {
        return Err("Invalid DNG warp coefficients or optical center.".to_string());
    }
    Ok(())
}

fn signed_values(tag: &TagValue) -> Vec<i64> {
    tag.signed.clone().unwrap_or_else(|| {
        tag.bytes
            .iter()
            .map(|value| i64::from(*value as i8))
            .collect()
    })
}

fn sony_values(
    tags: &HashMap<u16, TagValue>,
    code: u16,
    channels: usize,
) -> Result<Vec<f64>, String> {
    let Some(tag) = tags.get(&code) else {
        return Ok(Vec::new());
    };
    if tag.kind != 8 || tag.count > 33 {
        return Err("Unsupported Sony correction data type.".to_string());
    }
    let values = signed_values(tag);
    let n = *values.first().unwrap_or(&0);
    if n < (2 * channels) as i64
        || n > (16 * channels) as i64
        || n % channels as i64 != 0
        || !((values.len() as i64) == n + 1 || (values.len() as i64) == (16 * channels + 1) as i64)
    {
        return Err("Invalid Sony correction coefficient count.".to_string());
    }
    Ok(values[1..=n as usize]
        .iter()
        .map(|value| *value as f64)
        .collect())
}

fn read_sony(page: &DirectoryTags) -> Result<LensMetadata, String> {
    let distortion = sony_values(&page.tags, 0x7037, 1)?;
    let ca = sony_values(&page.tags, 0x7035, 2)?;
    for (code, values) in [(0x7036, &distortion), (0x7034, &ca)] {
        if !values.is_empty()
            && page
                .tags
                .get(&code)
                .and_then(|tag| tag.unsigned.first())
                .is_some_and(|value| *value == 255)
        {
            return Err("Sony marks the correction coefficients as unavailable.".to_string());
        }
    }
    if !distortion.is_empty() && !ca.is_empty() && ca.len() != 2 * distortion.len() {
        return Err("Sony correction arrays have different lengths.".to_string());
    }
    let half = ca.len() / 2;
    if distortion.iter().any(|value| 1.0 + value / 16384.0 <= 0.0) {
        return Err("Invalid Sony distortion coefficients.".to_string());
    }
    Ok(LensMetadata {
        source: "Sony ARW".to_string(),
        warps: vec![LensWarp::Sony {
            distortion,
            ca_red: ca[..half].to_vec(),
            ca_blue: ca[half..].to_vec(),
        }],
        reason: "No nonzero Sony lens correction coefficients.".to_string(),
        ..LensMetadata::default()
    })
}

fn read_dng(page: &DirectoryTags) -> Result<LensMetadata, String> {
    let Some(opcode) = page.tags.get(&OPCODE_LIST_3) else {
        return Ok(LensMetadata {
            reason: "No embedded DNG lens warp (OpcodeList3).".to_string(),
            ..LensMetadata::default()
        });
    };
    if opcode.count > MAX_OPCODE_BYTES || !matches!(opcode.kind, 1 | 7) {
        return Err("Invalid DNG correction data size or type.".to_string());
    }
    if let Some(scale) = page.tags.get(&50718)
        && scale.unsigned.as_slice() != [1, 1, 1, 1]
        && scale.unsigned.as_slice() != [1, 1]
    {
        return Err("DNG lens correction requires square source pixels.".to_string());
    }
    if let Some(colors) = page.tags.get(&50710)
        && colors.unsigned.as_slice() != [0, 1, 2]
    {
        return Err("Unsupported DNG color plane order.".to_string());
    }
    let active = page
        .tags
        .get(&ACTIVE_AREA)
        .map(|tag| {
            tag.unsigned
                .iter()
                .map(|value| *value as i64)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![0, 0, page.height as i64, page.width as i64]);
    if active.len() != 4
        || !(0 <= active[0]
            && active[0] < active[2]
            && active[2] <= page.height as i64
            && 0 <= active[1]
            && active[1] < active[3]
            && active[3] <= page.width as i64)
    {
        return Err("Invalid DNG active image area.".to_string());
    }
    let mut buffer = active.clone();
    if let (Some(origin), Some(size)) = (
        page.tags.get(&DEFAULT_CROP_ORIGIN),
        page.tags.get(&DEFAULT_CROP_SIZE),
    ) {
        let numbers = |tag: &TagValue| -> Vec<i64> {
            if matches!(tag.kind, 5 | 10) {
                tag.floats
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|value| value.round() as i64)
                    .collect()
            } else {
                tag.unsigned.iter().map(|value| *value as i64).collect()
            }
        };
        let origin = numbers(origin);
        let size = numbers(size);
        if origin.len() < 2 || size.len() < 2 {
            return Err("Invalid DNG default crop.".to_string());
        }
        let (ox, oy) = (origin[0], origin[1]);
        let (width, height) = (size[0], size[1]);
        buffer = vec![
            active[0] + oy,
            active[1] + ox,
            active[0] + oy + height,
            active[1] + ox + width,
        ];
        if !(active[0] <= buffer[0]
            && buffer[0] < buffer[2]
            && buffer[2] <= active[2]
            && active[1] <= buffer[1]
            && buffer[1] < buffer[3]
            && buffer[3] <= active[3])
        {
            return Err("Invalid DNG default crop.".to_string());
        }
    }
    Ok(LensMetadata {
        source: "DNG WarpRectilinear".to_string(),
        warps: parse_opcodes(&opcode.bytes)?,
        reason: "Embedded DNG warp is an identity.".to_string(),
        active_area: Some([active[0], active[1], active[2], active[3]]),
        buffer_area: Some([buffer[0], buffer[1], buffer[2], buffer[3]]),
        ..LensMetadata::default()
    })
}

fn read_panasonic(path: &Path) -> Result<LensMetadata, String> {
    let data = std::fs::read(path)
        .map_err(|_| "Cannot read embedded lens correction data.".to_string())?;
    if data.len() < 10 || data[..2] != *b"II" || data[2..4] != *b"\x55\x00" {
        return Err("Not a Panasonic RW2 file.".to_string());
    }
    let ifd = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    if ifd + 2 > data.len() {
        return Err("Panasonic RW2 IFD0 offset is out of range.".to_string());
    }
    let entries = u16::from_le_bytes(data[ifd..ifd + 2].try_into().unwrap()) as usize;
    if entries > 16384 || ifd + 2 + entries * 12 > data.len() {
        return Err("Panasonic RW2 IFD0 is malformed.".to_string());
    }
    let mut value = None;
    for index in 0..entries {
        let base = ifd + 2 + index * 12;
        let tag = u16::from_le_bytes(data[base..base + 2].try_into().unwrap());
        let kind = u16::from_le_bytes(data[base + 2..base + 4].try_into().unwrap());
        let count = u32::from_le_bytes(data[base + 4..base + 8].try_into().unwrap());
        if tag != 0x0119 {
            continue;
        }
        if kind != 7 || count != 32 {
            return Err("Panasonic distortion tag has an unsupported type or count.".to_string());
        }
        let offset = u32::from_le_bytes(data[base + 8..base + 12].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(32)
            .ok_or_else(|| "Panasonic distortion data offset is out of range.".to_string())?;
        if end > data.len() {
            return Err("Panasonic distortion data offset is out of range.".to_string());
        }
        value = Some(&data[offset..end]);
        break;
    }
    let Some(data) = value else {
        return Ok(LensMetadata {
            reason: "Panasonic RW2 carries no embedded distortion correction.".to_string(),
            ..LensMetadata::default()
        });
    };
    let mut words = [0i16; 16];
    for (index, word) in words.iter_mut().enumerate() {
        *word = i16::from_le_bytes(data[index * 2..index * 2 + 2].try_into().unwrap());
    }
    if words[7] & 0x0f != 1 {
        return Ok(LensMetadata {
            reason: "Panasonic distortion correction is off in this file.".to_string(),
            ..LensMetadata::default()
        });
    }
    let denominator = 1.0 + f64::from(words[5]) / 32768.0;
    if denominator <= 0.0 {
        return Err("Panasonic distortion scale folds the image.".to_string());
    }
    let scale = 1.0 / denominator;
    let a = f64::from(words[8]) / 32768.0;
    let b = f64::from(words[4]) / 32768.0;
    let c = f64::from(words[11]) / 32768.0;
    if !scale.is_finite() || !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return Err("Panasonic distortion coefficients are not finite.".to_string());
    }
    let mut previous = 0.0;
    for index in 0..257 {
        let r = index as f64 * 1.5 / 256.0;
        let mapped = r + scale * (a * r.powi(3) + b * r.powi(5) + c * r.powi(7));
        if index > 0 && mapped <= previous {
            return Err("Panasonic distortion may fold the image.".to_string());
        }
        previous = mapped;
    }
    Ok(LensMetadata {
        source: "Panasonic RW2".to_string(),
        warps: vec![LensWarp::Panasonic { scale, a, b, c }],
        ..LensMetadata::default()
    })
}

/// Inspect source metadata without decoding pixels. Results are cached by path, mtime
/// and size, so a replaced file reads again.
#[must_use]
pub fn read_lens_metadata(path: Option<&Path>) -> LensMetadata {
    let Some(path) = path else {
        return LensMetadata {
            reason: "Load a source file to check for embedded lens correction data.".to_string(),
            ..LensMetadata::default()
        };
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !matches!(extension.as_str(), "arw" | "dng" | "rw2") {
        return LensMetadata {
            reason: "Embedded lens correction is not supported for this file type.".to_string(),
            ..LensMetadata::default()
        };
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return LensMetadata {
            reason: "Cannot read source lens metadata.".to_string(),
            ..LensMetadata::default()
        };
    };
    let revision = format!(
        "{}|{}|{}",
        path.display(),
        metadata.len(),
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos())
    );
    let cache = cache();
    if let Some(hit) = cache
        .lock()
        .ok()
        .and_then(|store| store.get(&revision).cloned())
    {
        return hit;
    }
    let lens = match extension.as_str() {
        "rw2" => read_panasonic(path).unwrap_or_else(|error| LensMetadata {
            reason: error,
            ..LensMetadata::default()
        }),
        _ => tiff_walker::read_raw_directory(path).map_or_else(
            || LensMetadata {
                reason: "Cannot read embedded lens correction data.".to_string(),
                ..LensMetadata::default()
            },
            |page| {
                let read = if extension == "arw" {
                    read_sony(&page)
                } else {
                    read_dng(&page)
                };
                read.unwrap_or_else(|error| LensMetadata {
                    reason: error,
                    ..LensMetadata::default()
                })
            },
        ),
    };
    if let Ok(mut store) = cache.lock() {
        if store.len() >= 128 {
            store.clear();
        }
        store.insert(revision, lens.clone());
    }
    lens
}

fn cache() -> &'static Mutex<HashMap<String, LensMetadata>> {
    static CACHE: OnceLock<Mutex<HashMap<String, LensMetadata>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Locate LibRaw's visible pixels within the DNG active image.
#[must_use]
pub fn bind_decode(lens: &LensMetadata, raw_area: [i64; 4], fallback: bool) -> LensMetadata {
    let Some(active) = lens.active_area else {
        return lens.clone();
    };
    if !lens.available() {
        return lens.clone();
    }
    if fallback {
        let area = lens.buffer_area.unwrap_or(active);
        if [raw_area[2] - raw_area[0], raw_area[3] - raw_area[1]]
            != [area[2] - area[0], area[3] - area[1]]
        {
            return LensMetadata {
                reason: "DNG fallback crop does not match the lens correction area.".to_string(),
                ..LensMetadata::default()
            };
        }
        return LensMetadata {
            buffer_area: Some(area),
            ..lens.clone()
        };
    }
    let [top, left, bottom, right] = active;
    if !(top <= raw_area[0]
        && raw_area[0] < raw_area[2]
        && raw_area[2] <= bottom
        && left <= raw_area[1]
        && raw_area[1] < raw_area[3]
        && raw_area[3] <= right)
    {
        return LensMetadata {
            reason: "Decoded DNG area does not match the lens correction area.".to_string(),
            ..LensMetadata::default()
        };
    }
    LensMetadata {
        buffer_area: Some(raw_area),
        ..lens.clone()
    }
}
