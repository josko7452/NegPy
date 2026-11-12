//! A shared TIFF directory walk, mirroring the tag reads of the Python NEF, FFF and
//! TIFF loaders.
//!
//! The image crate's enum rejects the SGI LogLuv photometric value that a
//! Flextight raw uses, so directories are read from their entries directly.

use std::collections::{HashSet, VecDeque};
use std::path::Path;

/// The tags a loader needs before it decodes an image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfdInfo {
    pub offset: u64,
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u16,
    pub bits_per_sample: u16,
    pub photometric: u16,
    pub compression: u16,
    pub subfile_type: u32,
    pub is_subifd: bool,
    /// Rows in each strip, the last one shorter; 0 when the directory has no strips.
    pub rows_per_strip: u32,
    pub strip_offsets: Vec<u64>,
    pub strip_byte_counts: Vec<u64>,
}

impl IfdInfo {
    /// Pixel count, with the TIFF width and height.
    #[must_use]
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

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
        let quad = bytes[..4].try_into().unwrap();
        if self == Self::Little {
            u32::from_le_bytes(quad)
        } else {
            u32::from_be_bytes(quad)
        }
    }

    fn u64(self, bytes: &[u8]) -> u64 {
        let octet = bytes[..8].try_into().unwrap();
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
        let magic = endian.u16(data.get(2..4)?);
        let big = match magic {
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

    fn inline_bytes(&self) -> usize {
        if self.big { 8 } else { 4 }
    }

    fn entry_size(&self) -> usize {
        if self.big { 20 } else { 12 }
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

    fn type_size(kind: u16) -> Option<usize> {
        Some(match kind {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 => 8,
            16 | 17 | 18 => 8,
            _ => return None,
        })
    }

    fn unsigned_values(&self, kind: u16, count: u64, raw: &[u8]) -> Option<Vec<u64>> {
        let count = usize::try_from(count).ok()?;
        let values = match kind {
            1 | 7 => raw.iter().take(count).map(|v| u64::from(*v)).collect(),
            3 => raw
                .chunks_exact(2)
                .take(count)
                .map(|v| u64::from(self.endian.u16(v)))
                .collect(),
            4 | 13 => raw
                .chunks_exact(4)
                .take(count)
                .map(|v| u64::from(self.endian.u32(v)))
                .collect(),
            16 | 18 => raw
                .chunks_exact(8)
                .take(count)
                .map(|v| self.endian.u64(v))
                .collect(),
            _ => return None,
        };
        Some(values)
    }

    fn entry_values(&self, entry: &[u8]) -> Option<(u16, Vec<u64>)> {
        let tag = self.endian.u16(entry);
        let kind = self.endian.u16(&entry[2..]);
        let count = if self.big {
            self.endian.u64(&entry[4..])
        } else {
            u64::from(self.endian.u32(&entry[4..]))
        };
        let size = Self::type_size(kind)?;
        let total = usize::try_from(count).ok()?.checked_mul(size)?;
        let inline = self.inline_bytes();
        let value_at = if total <= inline {
            inline_start(entry, self.big)
        } else {
            let start = if self.big {
                usize::try_from(self.endian.u64(&entry[4 + 8..])).ok()?
            } else {
                usize::try_from(self.endian.u32(&entry[4 + 4..])).ok()?
            };
            self.data.get(start..start.checked_add(total)?)?
        };
        Some((tag, self.unsigned_values(kind, count, value_at)?))
    }

    fn directory(&self, offset: u64) -> Option<Directory> {
        let start = usize::try_from(offset).ok()?;
        let count = self.count_at(start)?;
        let mut entries = Vec::new();
        for index in 0..usize::try_from(count).ok()? {
            let at = start
                .checked_add(if self.big { 8 } else { 2 })?
                .checked_add(index.checked_mul(self.entry_size())?)?;
            let entry = self.data.get(at..at.checked_add(self.entry_size())?)?;
            if let Some((tag, values)) = self.entry_values(entry) {
                entries.push((tag, values));
            }
        }
        let next_at = start
            .checked_add(if self.big { 8 } else { 2 })?
            .checked_add(
                usize::try_from(count)
                    .ok()?
                    .checked_mul(self.entry_size())?,
            )?;
        Some(Directory {
            entries,
            next: self.next_at(next_at),
        })
    }
}

fn inline_start(entry: &[u8], big: bool) -> &[u8] {
    if big { &entry[12..] } else { &entry[8..] }
}

struct Directory {
    entries: Vec<(u16, Vec<u64>)>,
    next: Option<u64>,
}

impl Directory {
    fn values(&self, tag: u16) -> Option<&[u64]> {
        self.entries
            .iter()
            .find(|(kind, _)| *kind == tag)
            .map(|(_, values)| values.as_slice())
    }

    fn value(&self, tag: u16) -> Option<u64> {
        self.values(tag)?.first().copied()
    }
}

fn info(directory: &Directory, offset: u64, is_subifd: bool) -> Option<IfdInfo> {
    Some(IfdInfo {
        offset,
        width: u32::try_from(directory.value(256)?).ok()?,
        height: u32::try_from(directory.value(257)?).ok()?,
        samples_per_pixel: u16::try_from(directory.value(277).unwrap_or(1)).ok()?,
        bits_per_sample: u16::try_from(directory.value(258).unwrap_or(8)).ok()?,
        photometric: u16::try_from(directory.value(262).unwrap_or(0)).ok()?,
        compression: u16::try_from(directory.value(259).unwrap_or(1)).ok()?,
        subfile_type: u32::try_from(directory.value(254).unwrap_or(0)).ok()?,
        is_subifd,
        rows_per_strip: u32::try_from(directory.value(278).unwrap_or(0)).ok()?,
        strip_offsets: directory
            .values(273)
            .map(<[u64]>::to_vec)
            .unwrap_or_default(),
        strip_byte_counts: directory
            .values(279)
            .map(<[u64]>::to_vec)
            .unwrap_or_default(),
    })
}

/// Read every IFD reachable from the first directory, depth first, with SubIFDs marked.
#[must_use]
pub fn read_ifds(path: &Path) -> Option<Vec<IfdInfo>> {
    let data = std::fs::read(path).ok()?;
    let tiff = Tiff::new(&data)?;
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(tiff.first_ifd()?, false)]);
    let mut out = Vec::new();
    while let Some((offset, is_subifd)) = queue.pop_front() {
        if !seen.insert(offset) {
            continue;
        }
        let Some(directory) = tiff.directory(offset) else {
            continue;
        };
        if let Some(found) = info(&directory, offset, is_subifd) {
            out.push(found);
        }
        if let Some(children) = directory.values(330) {
            for child in children {
                queue.push_back((*child, true));
            }
        }
        if let Some(next) = directory.next {
            queue.push_back((next, false));
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The first top-level IFD, which is the one a scanner NEF's SubIFDs hang from.
#[must_use]
pub fn first_ifd(path: &Path) -> Option<IfdInfo> {
    read_ifds(path)?.into_iter().find(|info| !info.is_subifd)
}
