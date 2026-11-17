//! An EXIF block reader over a TIFF directory tree.
//!
//! Mirrors `read_exif_from_file` in `negpy/infrastructure/loaders/helpers.py`: a JPEG's
//! APP1 payload, a TIFF-based RAW or a JPEG XL `Exif` box parses into the five
//! piexif-shaped directories. The tag table is the writer's own, so a read then a write
//! carries the same field type.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::features::metadata::exif_read::{ExifTags, ExifValue};
use crate::features::metadata::writer::{ExifData, tiff_type};

const TIFF_ASCII: u16 = 2;
const TIFF_UNDEFINED: u16 = 7;
const TIFF_BYTE: u16 = 1;
const TIFF_SHORT: u16 = 3;
const TIFF_LONG: u16 = 4;
const TIFF_RATIONAL: u16 = 5;
const TIFF_SRATIONAL: u16 = 10;
const TAG_EXIF_IFD: u16 = 34665;
const TAG_GPS_IFD: u16 = 34853;
const TAG_INTEROP_IFD: u16 = 40965;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8]) -> u16 {
        let pair = [bytes[0], bytes[1]];
        if self == Self::Little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    }

    fn u32(self, bytes: &[u8]) -> u32 {
        let quad: [u8; 4] = bytes[..4].try_into().unwrap();
        if self == Self::Little {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        }
    }

    fn u64(self, bytes: &[u8]) -> u64 {
        let octet: [u8; 8] = bytes[..8].try_into().unwrap();
        if self == Self::Little {
            u64::from_le_bytes(octet)
        } else {
            u64::from_be_bytes(octet)
        }
    }
}

struct Tiff<'a> {
    data: &'a [u8],
    endian: Endian,
    big: bool,
}

impl<'a> Tiff<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let endian = match data.get(..2)? {
            b"II" => Endian::Little,
            b"MM" => Endian::Big,
            _ => return None,
        };
        let big = match endian.u16(data.get(2..4)?) {
            42 => false,
            43 => true,
            _ => return None,
        };
        Some(Self { data, endian, big })
    }

    fn first_ifd(&self) -> Option<u64> {
        if self.big {
            Some(self.endian.u64(self.data.get(8..16)?))
        } else {
            Some(u64::from(self.endian.u32(self.data.get(4..8)?)))
        }
    }

    fn entry_size(&self) -> usize {
        if self.big { 20 } else { 12 }
    }

    fn count_size(&self) -> usize {
        if self.big { 8 } else { 2 }
    }

    fn inline_bytes(&self) -> usize {
        if self.big { 8 } else { 4 }
    }

    fn type_size(kind: u16) -> Option<usize> {
        Some(match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 => 8,
            _ => return None,
        })
    }

    fn count_at(&self, offset: usize) -> Option<u64> {
        if self.big {
            Some(self.endian.u64(self.data.get(offset..offset + 8)?))
        } else {
            Some(u64::from(
                self.endian.u16(self.data.get(offset..offset + 2)?),
            ))
        }
    }

    fn next_at(&self, offset: usize) -> Option<u64> {
        if self.big {
            Some(self.endian.u64(self.data.get(offset..offset + 8)?))
        } else {
            Some(u64::from(
                self.endian.u32(self.data.get(offset..offset + 4)?),
            ))
        }
    }

    fn entry_payload(&self, entry: &'a [u8], kind: u16, count: u64) -> Option<&'a [u8]> {
        let size = Self::type_size(kind)?;
        let total = usize::try_from(count).ok()?.checked_mul(size)?;
        if total <= self.inline_bytes() {
            let start = if self.big { 12 } else { 8 };
            return entry.get(start..start + total);
        }
        let start = if self.big {
            usize::try_from(self.endian.u64(&entry[4 + 8..])).ok()?
        } else {
            usize::try_from(self.endian.u32(&entry[4 + 4..])).ok()?
        };
        self.data.get(start..start.checked_add(total)?)
    }

    fn value(&self, kind: u16, count: u64, raw: &[u8]) -> Option<ExifValue> {
        let count = usize::try_from(count).ok()?;
        Some(match kind {
            TIFF_ASCII => {
                let text = raw
                    .iter()
                    .take_while(|byte| **byte != 0)
                    .copied()
                    .collect::<Vec<u8>>();
                ExifValue::Text(String::from_utf8_lossy(&text).into_owned())
            }
            TIFF_BYTE | TIFF_UNDEFINED => ExifValue::Bytes(raw.to_vec()),
            TIFF_SHORT => {
                let value = self.endian.u16(raw);
                if count == 1 {
                    ExifValue::UInt(u64::from(value))
                } else {
                    ExifValue::Bytes(
                        raw.chunks_exact(2)
                            .map(|value| self.endian.u16(value) as u8)
                            .collect(),
                    )
                }
            }
            TIFF_LONG => {
                let value = self.endian.u32(raw);
                ExifValue::UInt(u64::from(value))
            }
            TIFF_SRATIONAL => {
                let numerator = self.endian.u32(raw) as i32;
                let denominator = self.endian.u32(&raw[4..]) as i32;
                ExifValue::Rational(i64::from(numerator), i64::from(denominator))
            }
            TIFF_RATIONAL => {
                let numerator = self.endian.u32(raw);
                let denominator = self.endian.u32(&raw[4..]);
                if count == 1 {
                    ExifValue::Rational(i64::from(numerator), i64::from(denominator))
                } else {
                    ExifValue::Rationals(
                        raw.chunks_exact(8)
                            .take(count)
                            .map(|value| {
                                (
                                    i64::from(self.endian.u32(value)),
                                    i64::from(self.endian.u32(&value[4..])),
                                )
                            })
                            .collect(),
                    )
                }
            }
            _ => return None,
        })
    }

    fn read_directory(&self, offset: u64) -> (ExifTags, Option<u64>) {
        let mut tags = ExifTags::new();
        let Some(start) = usize::try_from(offset).ok() else {
            return (tags, None);
        };
        let Some(count) = self.count_at(start) else {
            return (tags, None);
        };
        for index in 0..usize::try_from(count).unwrap_or(0) {
            let at = start + self.count_size() + index * self.entry_size();
            let Some(entry) = self.data.get(at..at + self.entry_size()) else {
                break;
            };
            let tag = self.endian.u16(entry);
            let kind = self.endian.u16(&entry[2..]);
            let count = if self.big {
                self.endian.u64(&entry[4..])
            } else {
                u64::from(self.endian.u32(&entry[4..]))
            };
            let Some(raw) = self.entry_payload(entry, kind, count) else {
                continue;
            };
            if let Some(value) = self.value(kind, count, raw) {
                tags.insert(tag, value);
            }
        }
        let next_at =
            start + self.count_size() + usize::try_from(count).unwrap_or(0) * self.entry_size();
        (tags, self.next_at(next_at))
    }

    fn pointer(&self, tags: &ExifTags, tag: u16) -> Option<u64> {
        match tags.get(&tag) {
            Some(ExifValue::UInt(value)) => Some(*value),
            Some(ExifValue::SInt(value)) if *value >= 0 => Some(*value as u64),
            _ => None,
        }
    }
}

/// The five IFD names an EXIF block can carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawExif {
    pub zeroth: ExifTags,
    pub exif: ExifTags,
    pub gps: ExifTags,
    pub interop: ExifTags,
    pub first: ExifTags,
}

impl RawExif {
    /// Whether the block carries any tag at all.
    #[must_use]
    pub fn has_any(&self) -> bool {
        !self.zeroth.is_empty()
            || !self.exif.is_empty()
            || !self.gps.is_empty()
            || !self.interop.is_empty()
            || !self.first.is_empty()
    }
}

impl From<RawExif> for ExifData {
    fn from(raw: RawExif) -> Self {
        Self {
            zeroth: raw.zeroth,
            exif: raw.exif,
            gps: raw.gps,
            interop: raw.interop,
            first: raw.first,
        }
    }
}

/// Parse one TIFF-structured EXIF payload. Tags the writer has no type for are dropped, so
/// a later dump cannot write them with a guessed type.
#[must_use]
pub fn parse_exif_block(data: &[u8]) -> Option<RawExif> {
    let tiff = Tiff::new(data)?;
    let (mut zeroth, mut next) = tiff.read_directory(tiff.first_ifd()?);
    let exif_at = tiff.pointer(&zeroth, TAG_EXIF_IFD);
    let gps_at = tiff.pointer(&zeroth, TAG_GPS_IFD);
    let exif = exif_at.map_or_else(ExifTags::new, |offset| tiff.read_directory(offset).0);
    let gps = gps_at.map_or_else(ExifTags::new, |offset| tiff.read_directory(offset).0);
    let interop_at = tiff.pointer(&exif, TAG_INTEROP_IFD);
    let interop = interop_at.map_or_else(ExifTags::new, |offset| tiff.read_directory(offset).0);
    let next_offset = next.take();
    let first = next_offset.map_or_else(ExifTags::new, |offset| tiff.read_directory(offset).0);
    zeroth.remove(&TAG_EXIF_IFD);
    zeroth.remove(&TAG_GPS_IFD);
    let keep = |name: &str, tags: ExifTags| {
        tags.into_iter()
            .filter(|(tag, _)| tiff_type(name, *tag).is_some())
            .collect::<ExifTags>()
    };
    Some(RawExif {
        zeroth: keep("0th", zeroth),
        exif: keep("Exif", exif),
        gps: keep("GPS", gps),
        interop: keep("Interop", interop),
        first: keep("1st", first),
    })
}

fn jpeg_exif_ref(data: &[u8]) -> Option<&[u8]> {
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
        let length = usize::from(u16::from_be_bytes([data[position + 2], data[position + 3]]));
        if length < 2 || position + 2 + length > data.len() {
            return None;
        }
        if marker == 0xda {
            return None;
        }
        let payload = &data[position + 4..position + 2 + length];
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            return Some(&payload[6..]);
        }
        position += 2 + length;
    }
    None
}

fn exif_payload(data: &[u8]) -> Option<Vec<u8>> {
    if let Some(exif) = jpeg_exif_ref(data) {
        return Some(exif.to_vec());
    }
    if data.starts_with(b"II") || data.starts_with(b"MM") {
        return Some(data.to_vec());
    }
    crate::infrastructure::loaders::jxl_boxes::read_jxl_exif(data)
}

/// One cached file's EXIF, invalidated by modification time and size.
#[derive(Clone)]
struct CacheEntry {
    modified: Option<SystemTime>,
    size: u64,
    exif: Option<RawExif>,
}

static CACHE: Mutex<Option<HashMap<PathBuf, CacheEntry>>> = Mutex::new(None);
const MAX_ENTRIES: usize = 64;

/// Read a file's EXIF as the writer's five directories; `None` when it carries none.
///
/// A camera RAW whose metadata sits outside a TIFF parse falls back to `rawler`.
pub fn read_exif_file(path: &Path) -> Option<RawExif> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok();
    let size = metadata.len();
    {
        let mut guard = CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache = guard.get_or_insert_with(HashMap::new);
        if let Some(entry) = cache.get(path)
            && entry.modified == modified
            && entry.size == size
        {
            return entry.exif.clone();
        }
    }
    let exif = std::fs::read(path)
        .ok()
        .and_then(|data| exif_payload(&data))
        .and_then(|payload| parse_exif_block(&payload))
        .or_else(|| raw_exif(path));
    let mut guard = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    if cache.len() >= MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(
        path.to_path_buf(),
        CacheEntry {
            modified,
            size,
            exif: exif.clone(),
        },
    );
    exif
}

fn raw_exif(path: &Path) -> Option<RawExif> {
    let raw = rawler::decode_file(path).ok()?;
    let mut zeroth = ExifTags::new();
    if !raw.make.trim().is_empty() {
        zeroth.insert(271, ExifValue::Text(raw.make.clone()));
    }
    if !raw.model.trim().is_empty() {
        zeroth.insert(272, ExifValue::Text(raw.model.clone()));
    }
    if zeroth.is_empty() {
        return None;
    }
    Some(RawExif {
        zeroth,
        ..RawExif::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata::writer::dump_exif;

    #[test]
    fn a_dumped_block_reads_back_its_directories() {
        let mut data = ExifData::default();
        data.zeroth.insert(271, ExifValue::Text("Canon".into()));
        data.zeroth.insert(272, ExifValue::Text("EOS R5".into()));
        data.exif
            .insert(36867, ExifValue::Text("2024:05:12 14:30:00".into()));
        data.exif.insert(33437, ExifValue::Rational(28, 10));
        let bytes = dump_exif(&data);
        let raw = parse_exif_block(&bytes).expect("a readable block");
        assert_eq!(raw.zeroth.get(&271), Some(&ExifValue::Text("Canon".into())));
        assert_eq!(raw.exif.get(&33437), Some(&ExifValue::Rational(28, 10)));
    }

    #[test]
    fn a_jpeg_app1_payload_is_read() {
        let mut data = ExifData::default();
        data.zeroth.insert(271, ExifValue::Text("Plustek".into()));
        let block = dump_exif(&data);
        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
        let length = (block.len() + 8) as u16;
        jpeg.extend_from_slice(&length.to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&block);
        jpeg.extend_from_slice(&[0xff, 0xd9]);
        let raw = parse_exif_block(&exif_payload(&jpeg).unwrap()).unwrap();
        assert_eq!(
            raw.zeroth.get(&271),
            Some(&ExifValue::Text("Plustek".into()))
        );
    }

    #[test]
    fn a_file_without_exif_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.bin");
        std::fs::write(&path, b"not a container").unwrap();
        assert!(read_exif_file(&path).is_none());
    }
}

#[cfg(test)]
mod round_trip_tests {
    use super::*;
    use crate::features::metadata::writer::{ExifData, dump_exif};

    #[test]
    fn a_gps_directory_reads_back_its_rationals() {
        let mut data = ExifData::default();
        data.gps.insert(1, ExifValue::Text("N".into()));
        data.gps
            .insert(2, ExifValue::Rationals(vec![(48, 1), (8, 1), (1234, 100)]));
        let raw = parse_exif_block(&dump_exif(&data)).expect("a readable block");
        assert_eq!(
            raw.gps.get(&2),
            Some(&ExifValue::Rationals(vec![(48, 1), (8, 1), (1234, 100)]))
        );
    }

    #[test]
    fn an_exif_pointer_resolves_to_its_own_directory() {
        let mut data = ExifData::default();
        data.exif
            .insert(36867, ExifValue::Text("2024:05:12 14:30:00".into()));
        let raw = parse_exif_block(&dump_exif(&data)).expect("a readable block");
        assert_eq!(
            raw.exif.get(&36867),
            Some(&ExifValue::Text("2024:05:12 14:30:00".into()))
        );
        assert!(!raw.zeroth.contains_key(&TAG_EXIF_IFD));
    }

    #[test]
    fn an_unknown_tag_type_is_dropped_instead_of_guessed() {
        let mut data = ExifData::default();
        data.zeroth.insert(60000, ExifValue::Text("private".into()));
        let raw = parse_exif_block(&dump_exif(&data)).expect("a readable block");
        assert!(!raw.zeroth.contains_key(&60000));
    }
}
