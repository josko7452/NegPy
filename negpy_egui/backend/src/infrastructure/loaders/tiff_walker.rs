//! A shared TIFF directory walk, mirroring the tag reads of the Python NEF, FFF and
//! TIFF loaders.

use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::path::Path;

use tiff::Directory;
use tiff::decoder::ifd::Value;
use tiff::decoder::{Decoder, IfdDecoder};
use tiff::tags::{IfdPointer, Tag};

/// The tags a loader needs before it decodes an image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfdInfo {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u16,
    pub bits_per_sample: u16,
    pub photometric: u16,
    pub compression: u16,
    pub subfile_type: u32,
    pub is_subifd: bool,
}

impl IfdInfo {
    /// Pixel count, with the TIFF width and height.
    #[must_use]
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

fn unsigned(tags: &mut IfdDecoder<'_>, tag: Tag) -> Option<u64> {
    match tags.find_tag(tag).ok().flatten()? {
        Value::List(values) => values
            .first()
            .and_then(|value| value.clone().into_u64().ok()),
        value => value.into_u64().ok(),
    }
}

fn read_info(
    decoder: &mut Decoder<File>,
    directory: &Directory,
    is_subifd: bool,
) -> Option<IfdInfo> {
    let mut tags = decoder.read_directory_tags(directory);
    let width = u32::try_from(unsigned(&mut tags, Tag::ImageWidth)?).ok()?;
    let height = u32::try_from(unsigned(&mut tags, Tag::ImageLength)?).ok()?;
    let samples = u32::try_from(unsigned(&mut tags, Tag::SamplesPerPixel).unwrap_or(1)).ok()?;
    let bits = u32::try_from(unsigned(&mut tags, Tag::BitsPerSample).unwrap_or(8)).ok()?;
    let photometric =
        u32::try_from(unsigned(&mut tags, Tag::PhotometricInterpretation).unwrap_or(0)).ok()?;
    let compression = u32::try_from(unsigned(&mut tags, Tag::Compression).unwrap_or(1)).ok()?;
    let subfile = u32::try_from(unsigned(&mut tags, Tag::NewSubfileType).unwrap_or(0)).ok()?;
    Some(IfdInfo {
        width,
        height,
        samples_per_pixel: u16::try_from(samples).ok()?,
        bits_per_sample: u16::try_from(bits).ok()?,
        photometric: u16::try_from(photometric).ok()?,
        compression: u16::try_from(compression).ok()?,
        subfile_type: subfile,
        is_subifd,
    })
}

fn child_pointers(decoder: &mut Decoder<File>, directory: &Directory) -> Vec<IfdPointer> {
    let mut tags = decoder.read_directory_tags(directory);
    tags.find_tag(Tag::SubIfd)
        .ok()
        .flatten()
        .and_then(|value| value.into_ifd_vec().ok())
        .unwrap_or_default()
}

/// Read every IFD reachable from the first directory, depth first, with SubIFDs marked.
#[must_use]
pub fn read_ifds(path: &Path) -> Option<Vec<IfdInfo>> {
    let mut decoder = Decoder::new(File::open(path).ok()?).ok()?;
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(decoder.ifd_pointer()?, false)]);
    let mut out = Vec::new();
    while let Some((pointer, is_subifd)) = queue.pop_front() {
        if !seen.insert(pointer.0) {
            continue;
        }
        let Ok(directory) = decoder.read_directory(pointer) else {
            continue;
        };
        if let Some(info) = read_info(&mut decoder, &directory, is_subifd) {
            out.push(info);
        }
        for child in child_pointers(&mut decoder, &directory) {
            queue.push_back((child, true));
        }
        if let Some(next) = directory.next() {
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
