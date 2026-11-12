//! TIFF scan loading.
//!
//! Mirrors `negpy/infrastructure/loaders/tiff_loader.py`: framed samples, an IR
//! plane from a fourth sample or a sidecar, and the Positive source decode.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

use crate::domain::models::SourceEncoding;
use crate::infrastructure::loaders::constants::SUPPORTED_TIFF_EXTENSIONS;
use crate::infrastructure::loaders::helpers::{identify_color_space_from_icc, read_orientation};
use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};
use crate::infrastructure::loaders::source_encoding::{
    PROFILE, SampleDepth, decode_positive, resolve_decode,
};

const PHOTOMETRIC_MINISWHITE: u16 = 0;
const PHOTOMETRIC_RGB: u16 = 2;
const PHOTOMETRIC_PALETTE: u16 = 3;
const PHOTOMETRIC_SEPARATED: u16 = 5;
const NEW_SUBFILE_MASK: u32 = 4;

pub(crate) type DecodedPage = (ImageBuffer, Option<Vec<u8>>, Option<Vec<f32>>);
type IrPlane = (Vec<f32>, usize, usize);

fn channels(color: ColorType) -> usize {
    match color {
        ColorType::Gray(_) | ColorType::Palette(_) => 1,
        ColorType::GrayA(_) => 2,
        ColorType::RGB(_) => 3,
        ColorType::RGBA(_) | ColorType::CMYK(_) => 4,
        _ => 3,
    }
}

fn unpack_u8(values: &[u8], channels: usize) -> Vec<u8> {
    values
        .chunks_exact(channels)
        .flat_map(|pixel| {
            [
                pixel[0],
                pixel[1.min(channels - 1)],
                pixel[2.min(channels - 1)],
            ]
        })
        .collect()
}

fn unpack_u16(values: &[u16], channels: usize) -> Vec<u16> {
    values
        .chunks_exact(channels)
        .flat_map(|pixel| {
            [
                pixel[0],
                pixel[1.min(channels - 1)],
                pixel[2.min(channels - 1)],
            ]
        })
        .collect()
}

fn rgb_from_gray_u8(values: &[u8]) -> Vec<f32> {
    values
        .iter()
        .flat_map(|v| [*v, *v, *v])
        .map(|v| v as f32 / 255.0)
        .collect()
}

fn rgb_from_gray_u16(values: &[u16]) -> Vec<u16> {
    values.iter().flat_map(|v| [*v, *v, *v]).collect()
}

fn separated_u8(values: &[u8]) -> Vec<u8> {
    values
        .chunks_exact(4)
        .flat_map(|pixel| {
            let top = 255.0;
            let k = top - f64::from(pixel[3]);
            [pixel[0], pixel[1], pixel[2]].map(|c| (((top - f64::from(c)) * k) / top) as u8)
        })
        .collect()
}

fn separated_u16(values: &[u16]) -> Vec<u16> {
    values
        .chunks_exact(4)
        .flat_map(|pixel| {
            let top = 65535.0;
            let k = top - f64::from(pixel[3]);
            [pixel[0], pixel[1], pixel[2]].map(|c| (((top - f64::from(c)) * k) / top) as u16)
        })
        .collect()
}

fn palette_u8(values: &[u8], colormap: &[u16]) -> Vec<u8> {
    let size = colormap.len() / 3;
    values
        .iter()
        .flat_map(|index| {
            let index = *index as usize;
            [
                colormap[index],
                colormap[index + size],
                colormap[index + 2 * size],
            ]
            .map(|v| (u32::from(v) * 255 / 65535) as u8)
        })
        .collect()
}

fn palette_u16(values: &[u8], colormap: &[u16]) -> Vec<u16> {
    let size = colormap.len() / 3;
    values
        .iter()
        .flat_map(|index| {
            let index = *index as usize;
            [
                colormap[index],
                colormap[index + size],
                colormap[index + 2 * size],
            ]
        })
        .collect()
}

pub(crate) fn decode_page(path: &Path) -> Option<DecodedPage> {
    decode_page_at(path, 0)
}

fn decode_page_at(path: &Path, ifd_index: usize) -> Option<DecodedPage> {
    let mut decoder = Decoder::new(File::open(path).ok()?).ok()?;
    if ifd_index > 0 {
        decoder.seek_to_image(ifd_index).ok()?;
    }
    decode_with(decoder)
}

/// Decode a TIFF directory that is not in the top-level image chain.
pub(crate) fn decode_ifd(path: &Path, offset: u64) -> Option<DecodedPage> {
    let reader = TiffAt::new(File::open(path).ok()?, offset)?;
    decode_with(Decoder::new(reader).ok()?)
}

fn decode_with(mut decoder: Decoder<impl Read + Seek>) -> Option<DecodedPage> {
    let (width, height) = decoder.dimensions().ok()?;
    let color = decoder.colortype().ok()?;
    let photometric = decoder
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .ok()
        .flatten()
        .unwrap_or(PHOTOMETRIC_RGB);
    let extra = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ExtraSamples)
        .ok()
        .flatten()
        .and_then(|values| values.first().copied());
    let colormap = decoder
        .find_tag_unsigned_vec::<u16>(Tag::ColorMap)
        .ok()
        .flatten();
    let icc = decoder
        .find_tag_unsigned_vec::<u8>(Tag::IccProfile)
        .ok()
        .flatten();
    let mut result = DecodingResult::U8(Vec::new());
    decoder.read_image_to_buffer(&mut result).ok()?;
    let width = width as usize;
    let height = height as usize;
    let count = channels(color);
    let mut ir = None;

    let image = match result {
        DecodingResult::U8(mut values) => {
            if photometric == PHOTOMETRIC_PALETTE {
                values = palette_u8(&values, colormap.as_deref()?);
            } else if photometric == PHOTOMETRIC_MINISWHITE {
                values = values.into_iter().map(|v| 255 - v).collect();
            } else if photometric == PHOTOMETRIC_SEPARATED && count == 4 {
                values = separated_u8(&values);
            }
            if photometric == PHOTOMETRIC_RGB && count == 4 {
                if extra == Some(0) || extra.is_none() {
                    ir = Some(
                        values
                            .chunks_exact(4)
                            .map(|p| p[3] as f32 / 255.0)
                            .collect(),
                    );
                }
                ImageBuffer::from_rgb(
                    width,
                    height,
                    values
                        .chunks_exact(4)
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .map(|v| v as f32 / 255.0)
                        .collect(),
                )
            } else if count == 2 {
                ir = Some(
                    values
                        .chunks_exact(2)
                        .map(|p| p[1] as f32 / 255.0)
                        .collect(),
                );
                ImageBuffer::from_rgb(
                    width,
                    height,
                    values
                        .chunks_exact(2)
                        .flat_map(|p| [p[0], p[0], p[0]])
                        .map(|v| v as f32 / 255.0)
                        .collect(),
                )
            } else if count == 1 {
                ImageBuffer::from_rgb(width, height, rgb_from_gray_u8(&values))
            } else {
                let rgb = unpack_u8(&values, count.min(3));
                ImageBuffer::from_rgb(
                    width,
                    height,
                    rgb.into_iter().map(|v| v as f32 / 255.0).collect(),
                )
            }
        }
        DecodingResult::U16(mut values) => {
            if photometric == PHOTOMETRIC_PALETTE {
                let indices: Vec<u8> = values.iter().map(|v| *v as u8).collect();
                values = palette_u16(&indices, colormap.as_deref()?);
            } else if photometric == PHOTOMETRIC_MINISWHITE {
                values = values.into_iter().map(|v| 65535 - v).collect();
            } else if photometric == PHOTOMETRIC_SEPARATED && count == 4 {
                values = separated_u16(&values);
            }
            if photometric == PHOTOMETRIC_RGB && count == 4 {
                if extra == Some(0) || extra.is_none() {
                    ir = Some(
                        values
                            .chunks_exact(4)
                            .map(|p| p[3] as f32 / 65535.0)
                            .collect(),
                    );
                }
                ImageBuffer::from_rgb16(
                    width,
                    height,
                    values
                        .chunks_exact(4)
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                        .collect(),
                )
            } else if count == 2 {
                ir = Some(
                    values
                        .chunks_exact(2)
                        .map(|p| p[1] as f32 / 65535.0)
                        .collect(),
                );
                ImageBuffer::from_rgb16(
                    width,
                    height,
                    values
                        .chunks_exact(2)
                        .flat_map(|p| [p[0], p[0], p[0]])
                        .collect(),
                )
            } else if count == 1 {
                ImageBuffer::from_rgb16(width, height, rgb_from_gray_u16(&values))
            } else {
                ImageBuffer::from_rgb16(width, height, unpack_u16(&values, count.min(3)))
            }
        }
        _ => return None,
    };
    Some((image, icc, ir))
}

struct TiffAt {
    file: File,
    header: [u8; 8],
    position: u64,
}

impl TiffAt {
    fn new(mut file: File, offset: u64) -> Option<Self> {
        let mut order = [0u8; 2];
        file.read_exact(&mut order).ok()?;
        let offset = u32::try_from(offset).ok()?;
        let mut header = [0u8; 8];
        header[..2].copy_from_slice(&order);
        match &order {
            b"II" => {
                header[2..4].copy_from_slice(&42u16.to_le_bytes());
                header[4..8].copy_from_slice(&offset.to_le_bytes());
            }
            b"MM" => {
                header[2..4].copy_from_slice(&42u16.to_be_bytes());
                header[4..8].copy_from_slice(&offset.to_be_bytes());
            }
            _ => return None,
        }
        file.seek(SeekFrom::Start(0)).ok()?;
        Some(Self {
            file,
            header,
            position: 0,
        })
    }
}

impl Read for TiffAt {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let position = usize::try_from(self.position).unwrap_or(usize::MAX);
        let from_header = (8usize.saturating_sub(position)).min(buffer.len());
        if from_header > 0 {
            buffer[..from_header].copy_from_slice(&self.header[position..position + from_header]);
            self.position += from_header as u64;
        }
        if self.position < 8 {
            return Ok(from_header);
        }
        self.file.seek(SeekFrom::Start(self.position))?;
        let mut total = from_header;
        if total < buffer.len() {
            let read = self.file.read(&mut buffer[total..])?;
            total += read;
            self.position += read as u64;
        }
        Ok(total)
    }
}

impl Seek for TiffAt {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let target = match position {
            SeekFrom::Start(position) => position,
            SeekFrom::Current(delta) => u64::try_from(self.position as i128 + i128::from(delta))
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "negative seek")
                })?,
            SeekFrom::End(delta) => self.file.seek(SeekFrom::End(delta))?,
        };
        self.position = target;
        if self.position <= 8 {
            self.file.seek(SeekFrom::Start(0))?;
        } else {
            self.file.seek(SeekFrom::Start(target))?;
        }
        Ok(target)
    }
}

fn tiff_extensions(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            SUPPORTED_TIFF_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(&known[1..]))
        })
}

#[cfg(test)]
mod tests {
    use super::TiffAt;
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn a_redirected_header_does_not_skip_payload_bytes() {
        let path = std::env::temp_dir().join("negpy-tiff-at.bin");
        let mut bytes = b"II\x2a\x00\x10\x00\x00\x00PAYLOAD".to_vec();
        bytes.resize(32, 0);
        std::fs::write(&path, bytes).unwrap();
        let mut reader = TiffAt::new(std::fs::File::open(&path).unwrap(), 16).unwrap();
        let mut header = [0u8; 8];
        reader.read_exact(&mut header).unwrap();
        assert_eq!(&header[4..], &16u32.to_le_bytes());
        let mut payload = [0u8; 7];
        reader.read_exact(&mut payload).unwrap();
        assert_eq!(&payload, b"PAYLOAD");
        reader.seek(SeekFrom::Start(16)).unwrap();
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).unwrap();
        assert_eq!(byte[0], 0);
        std::fs::remove_file(path).unwrap();
    }
}

fn find_sidecar(path: &Path, stem: &str, suffix: &str) -> Option<PathBuf> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_str()?.to_lowercase();
        if !tiff_extensions(&name) {
            continue;
        }
        if name == format!("{stem}{suffix}.tif") || name == format!("{stem}{suffix}.tiff") {
            return Some(entry.path());
        }
    }
    None
}

fn read_ir(path: &Path) -> Option<IrPlane> {
    let mut decoder = Decoder::new(File::open(path).ok()?).ok()?;
    let (width, height) = decoder.dimensions().ok()?;
    let count = channels(decoder.colortype().ok()?);
    let values = match decoder.read_image().ok()? {
        DecodingResult::U8(values) => values
            .chunks_exact(count)
            .map(|pixel| pixel[0] as f32 / 255.0)
            .collect(),
        DecodingResult::U16(values) => values
            .chunks_exact(count)
            .map(|pixel| pixel[0] as f32 / 65535.0)
            .collect(),
        _ => return None,
    };
    Some((values, width as usize, height as usize))
}

fn ir_sidecar(path: &Path) -> Option<IrPlane> {
    let stem = path.file_stem()?.to_str()?.to_lowercase();
    let candidate = find_sidecar(path, &stem, "_ir")?;
    let (mut ir, width, height) = read_ir(&candidate)?;
    if let Some(mask) = find_sidecar(path, &stem, "_ir_valid").and_then(|mask| read_ir(&mask)) {
        if mask.1 != width || mask.2 != height || mask.0.len() != ir.len() {
            return None;
        }
        for (value, valid) in ir.iter_mut().zip(mask.0) {
            if valid != 0.0 && valid != 1.0 / 255.0 && valid != 255.0 / 255.0 {
                return None;
            }
            if valid == 0.0 {
                *value = 1.0;
            }
        }
    }
    Some((ir, width, height))
}

fn extra_ir_page(path: &Path, width: usize, height: usize) -> Option<IrPlane> {
    let mut decoder = Decoder::new(File::open(path).ok()?).ok()?;
    while decoder.more_images() {
        decoder.next_image().ok()?;
        if decoder.dimensions().ok()? != (width as u32, height as u32) {
            continue;
        }
        let color = decoder.colortype().ok()?;
        let subfile = decoder
            .find_tag_unsigned::<u32>(Tag::NewSubfileType)
            .ok()
            .flatten()
            .unwrap_or(0);
        let gray = matches!(color, ColorType::Gray(8) | ColorType::Gray(16));
        if !gray && subfile & NEW_SUBFILE_MASK == 0 {
            continue;
        }
        let values = match decoder.read_image().ok()? {
            DecodingResult::U8(values) => values.into_iter().map(|v| v as f32 / 255.0).collect(),
            DecodingResult::U16(values) => values.into_iter().map(|v| v as f32 / 65535.0).collect(),
            _ => continue,
        };
        return Some((values, width, height));
    }
    None
}

/// Load a TIFF scan. A non-empty `positive_decode` decodes it as a Positive source.
#[must_use]
pub fn load(
    path: &Path,
    linear_raw: bool,
    positive_decode: Option<SourceEncoding>,
    load_ir: bool,
) -> Option<LoadedImage> {
    let (mut image, icc, mut ir) = decode_page(path)?;
    if ir.is_none() && load_ir {
        if let Some((values, _, _)) = ir_sidecar(path) {
            ir = Some(values);
        }
    }
    if ir.is_none() && load_ir {
        if let Some((values, _, _)) = extra_ir_page(path, image.width, image.height) {
            ir = Some(values);
        }
    }
    let mut metadata = LoadedImage {
        orientation: read_orientation(path),
        icc_profile: icc.clone(),
        ir,
        ir_width: image.width,
        ir_height: image.height,
        ..LoadedImage::default()
    };
    if let Some(encoding) = positive_decode {
        let depth = if image.source_u16.is_some() {
            SampleDepth::Sixteen
        } else {
            SampleDepth::Eight
        };
        let resolved = resolve_decode(encoding, icc.as_deref(), depth);
        metadata.source_encoding = Some(resolved.ran.clone());
        metadata.color_space = if resolved.ran == PROFILE {
            icc.as_deref()
                .and_then(identify_color_space_from_icc)
                .map(str::to_string)
        } else {
            None
        };
        if resolved.decode.identity && image.source_u16.is_some() {
            metadata.image = Some(image);
            return Some(metadata);
        }
        let (linear, _) = decode_positive(&image, &resolved.decode);
        metadata.image = Some(linear);
        return Some(metadata);
    }
    if !linear_raw {
        metadata.color_space = icc
            .as_deref()
            .and_then(identify_color_space_from_icc)
            .map(str::to_string);
        if metadata.color_space.is_none() && image.source_u16.is_none() {
            metadata.color_space = Some("sRGB".to_string());
        }
    }
    let decodes = matches!(metadata.color_space.as_deref(), Some("sRGB" | "Adobe RGB"));
    if image.source_u16.is_some() && !decodes {
        metadata.image = Some(image);
        return Some(metadata);
    }
    if let Some(source) = image.source_u16.take() {
        image.pixels = source.iter().map(|v| *v as f32 / 65535.0).collect();
    }
    if metadata.color_space.as_deref() == Some("sRGB") {
        negpy_core::color::srgb_to_linear(&image.pixels.clone(), &mut image.pixels);
    } else if metadata.color_space.as_deref() == Some("Adobe RGB") {
        negpy_core::color::oetf_decode(&image.pixels.clone(), &mut image.pixels, 563.0 / 256.0);
    }
    metadata.image = Some(image);
    Some(metadata)
}
