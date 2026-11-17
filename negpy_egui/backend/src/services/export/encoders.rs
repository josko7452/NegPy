//! Container encoding for the export paths in `negpy/services/export/encoders.py`.
//!
//! Integer pixels in, file bytes out. No color management, no sizing, no naming and no
//! disk access: callers own those.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

/// The PNG density unit, metres.
const PNG_METRE: u8 = 1;
const INCH_PER_METRE: f64 = 39.3701;

/// A resolution tag: pixels per inch on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub x_dpi: f64,
    pub y_dpi: f64,
    /// `2` for inch, `3` for centimetre, as TIFF's `ResolutionUnit`.
    pub unit: u16,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            x_dpi: 300.0,
            y_dpi: 300.0,
            unit: 2,
        }
    }
}

fn png_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(tag);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32fast::hash(&crc_input).to_be_bytes());
    out
}

/// `zlib.compress(data, level)`.
#[must_use]
pub fn zlib_compress(data: &[u8], level: u32) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level.min(9)));
    let _ = encoder.write_all(data);
    encoder.finish().unwrap_or_default()
}

/// A PNG of uint8 or uint16 RGB/gray pixels with the ancillary chunks spliced in.
///
/// The ICC profile, EXIF block, XMP text and resolution all land ahead of the image data,
/// where a reader expects them.
#[must_use]
pub fn encode_png(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    depth: u8,
    level: u8,
    icc: Option<&[u8]>,
    resolution: Option<Resolution>,
    exif: Option<&[u8]>,
    xmp: Option<&[u8]>,
) -> Option<Vec<u8>> {
    let bytes_per_sample = depth as usize / 8;
    let row_bytes = width * channels * bytes_per_sample;
    if width == 0
        || height == 0
        || !(channels == 1 || channels == 3)
        || !(depth == 8 || depth == 16)
    {
        return None;
    }
    if pixels.len() != row_bytes * height {
        return None;
    }
    let png = negpy_core::png::encode(height, width, channels, depth, level.min(9), |y, row| {
        let start = y * row_bytes;
        row.extend_from_slice(&pixels[start..start + row_bytes]);
    });
    let mut chunks = Vec::new();
    if let Some(icc) = icc.filter(|value| !value.is_empty()) {
        let mut data = b"ICC Profile\x00\x00".to_vec();
        data.extend_from_slice(&zlib_compress(icc, 9));
        chunks.extend(png_chunk(b"iCCP", &data));
    }
    if let Some(resolution) = resolution {
        let ppm_x = (resolution.x_dpi * INCH_PER_METRE).round().max(1.0) as u32;
        let ppm_y = (resolution.y_dpi * INCH_PER_METRE).round().max(1.0) as u32;
        let mut data = Vec::with_capacity(9);
        data.extend_from_slice(&ppm_x.to_be_bytes());
        data.extend_from_slice(&ppm_y.to_be_bytes());
        data.push(PNG_METRE);
        chunks.extend(png_chunk(b"pHYs", &data));
    }
    if let Some(exif) = exif.filter(|value| !value.is_empty()) {
        // eXIf carries the bare TIFF structure; the APP1 marker is JPEG framing.
        let body = exif.strip_prefix(b"Exif\x00\x00").unwrap_or(exif);
        chunks.extend(png_chunk(b"eXIf", body));
    }
    if let Some(xmp) = xmp.filter(|value| !value.is_empty()) {
        let mut data = b"XML:com.adobe.xmp\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(xmp);
        chunks.extend(png_chunk(b"iTXt", &data));
    }
    if chunks.is_empty() {
        return Some(png);
    }
    // Ancillary chunks must precede the image data.
    let cut = find_idat(&png)?;
    let mut out = Vec::with_capacity(png.len() + chunks.len());
    out.extend_from_slice(&png[..cut]);
    out.extend_from_slice(&chunks);
    out.extend_from_slice(&png[cut..]);
    Some(out)
}

/// The offset of the `IDAT` length field.
fn find_idat(png: &[u8]) -> Option<usize> {
    png.windows(4)
        .position(|window| window == b"IDAT")
        .map(|at| at.saturating_sub(4))
}

/// A JPEG of uint8 RGB or gray pixels. Chroma subsampling stays 4:4:4.
#[must_use]
pub fn encode_jpeg(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    quality: i32,
    icc: Option<&[u8]>,
    dpi: Option<(u16, u16)>,
) -> Option<Vec<u8>> {
    if !(channels == 1 || channels == 3) {
        return None;
    }
    let options = negpy_core::jpeg::Options {
        quality,
        icc,
        dpi,
        restart_rows: None,
    };
    negpy_core::jpeg::encode(pixels, (height, width, channels), &options)
}

/// The byte order and magic of a TIFF header.
const TIFF_LITTLE: &[u8] = b"II";
const TIFF_MAGIC: u16 = 42;

/// A TIFF of uint8 or uint16 RGB/gray pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TiffCompression {
    None,
    Deflate,
}

/// One TIFF entry: tag, kind, count and its value or offset.
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// Inline when it fits in four bytes, else an offset into the trailing data.
    data: Vec<u8>,
}

const TIFF_SHORT: u16 = 3;
const TIFF_LONG: u16 = 4;
const TIFF_RATIONAL: u16 = 5;
const TIFF_ASCII: u16 = 2;
const TIFF_UNDEFINED: u16 = 7;

/// A baseline TIFF of uint8/uint16 pixels.
#[must_use]
pub fn encode_tiff(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    bits_per_sample: u16,
    compression: TiffCompression,
    resolution: Option<Resolution>,
    icc: Option<&[u8]>,
) -> Option<Vec<u8>> {
    encode_tiff_with_description(
        pixels,
        width,
        height,
        channels,
        bits_per_sample,
        compression,
        resolution,
        icc,
        None,
    )
}

/// A TIFF with an image description and the Software tag the Python writer stamps.
#[must_use]
pub fn encode_tiff_with_description(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    bits_per_sample: u16,
    compression: TiffCompression,
    resolution: Option<Resolution>,
    icc: Option<&[u8]>,
    description: Option<&str>,
) -> Option<Vec<u8>> {
    let samples = if channels == 1 { 1 } else { 3 };
    let row_bytes = width * samples * (bits_per_sample as usize / 8);
    if width == 0
        || height == 0
        || !(samples == 1 || samples == 3)
        || pixels.len() != row_bytes * height
    {
        return None;
    }
    let (payload, compression_tag) = match compression {
        TiffCompression::None => (pixels.to_vec(), 1u16),
        TiffCompression::Deflate => (zlib_compress(pixels, 6), 8u16),
    };
    let mut entries = vec![
        Entry {
            tag: 256,
            kind: TIFF_LONG,
            count: 1,
            data: (width as u32).to_le_bytes().to_vec(),
        },
        Entry {
            tag: 257,
            kind: TIFF_LONG,
            count: 1,
            data: (height as u32).to_le_bytes().to_vec(),
        },
        Entry {
            tag: 258,
            kind: TIFF_SHORT,
            count: samples as u32,
            data: vec![bits_per_sample; samples]
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
        },
        Entry {
            tag: 259,
            kind: TIFF_SHORT,
            count: 1,
            data: compression_tag.to_le_bytes().to_vec(),
        },
        Entry {
            tag: 262,
            kind: TIFF_SHORT,
            count: 1,
            data: (if samples == 1 { 1u16 } else { 2u16 })
                .to_le_bytes()
                .to_vec(),
        },
        Entry {
            tag: 273,
            kind: TIFF_LONG,
            count: 1,
            data: vec![0; 4],
        },
        Entry {
            tag: 277,
            kind: TIFF_SHORT,
            count: 1,
            data: (samples as u16).to_le_bytes().to_vec(),
        },
        Entry {
            tag: 278,
            kind: TIFF_LONG,
            count: 1,
            data: (height as u32).to_le_bytes().to_vec(),
        },
        Entry {
            tag: 279,
            kind: TIFF_LONG,
            count: 1,
            data: (payload.len() as u32).to_le_bytes().to_vec(),
        },
    ];
    if let Some(resolution) = resolution {
        entries.push(Entry {
            tag: 282,
            kind: TIFF_RATIONAL,
            count: 1,
            data: rational(resolution.x_dpi),
        });
        entries.push(Entry {
            tag: 283,
            kind: TIFF_RATIONAL,
            count: 1,
            data: rational(resolution.y_dpi),
        });
        entries.push(Entry {
            tag: 296,
            kind: TIFF_SHORT,
            count: 1,
            data: resolution.unit.to_le_bytes().to_vec(),
        });
    }
    if let Some(icc) = icc.filter(|value| !value.is_empty()) {
        entries.push(Entry {
            tag: 34675,
            kind: TIFF_UNDEFINED,
            count: icc.len() as u32,
            data: icc.to_vec(),
        });
        entries.push(Entry {
            tag: 305,
            kind: TIFF_ASCII,
            count: 0,
            data: Vec::new(),
        });
    }
    if let Some(text) = description {
        let mut data = text.as_bytes().to_vec();
        data.push(0);
        entries.push(Entry {
            tag: 270,
            kind: TIFF_ASCII,
            count: data.len() as u32,
            data,
        });
        let mut software = b"NegPy\0".to_vec();
        software.truncate(6);
        entries.push(Entry {
            tag: 305,
            kind: TIFF_ASCII,
            count: software.len() as u32,
            data: software,
        });
    }
    entries.sort_by_key(|entry| entry.tag);

    let ifd_offset = 8u32;
    let ifd_size = 2 + entries.len() * 12 + 4;
    let mut extra_cursor = ifd_offset as usize + ifd_size;
    let mut extra = Vec::new();
    let mut resolved: Vec<(u16, u16, u32, u32)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let inline = match entry.kind {
            TIFF_SHORT => 2,
            TIFF_LONG => 4,
            TIFF_RATIONAL => 8,
            _ => 1,
        } * entry.count as usize;
        if inline <= 4 {
            let mut bytes = [0u8; 4];
            bytes[..entry.data.len().min(4)]
                .copy_from_slice(&entry.data[..entry.data.len().min(4)]);
            resolved.push((
                entry.tag,
                entry.kind,
                entry.count,
                u32::from_le_bytes(bytes),
            ));
        } else {
            resolved.push((entry.tag, entry.kind, entry.count, extra_cursor as u32));
            extra.extend_from_slice(&entry.data);
            extra_cursor += entry.data.len();
        }
    }
    let image_offset = extra_cursor as u32;

    let mut out = Vec::with_capacity(image_offset as usize + payload.len());
    out.extend_from_slice(TIFF_LITTLE);
    out.extend_from_slice(&TIFF_MAGIC.to_le_bytes());
    out.extend_from_slice(&ifd_offset.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, kind, count, value) in resolved {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        let value = if tag == 273 { image_offset } else { value };
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&extra);
    out.extend_from_slice(&payload);
    Some(out)
}

fn rational(value: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(&(value as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out
}

/// A lossless WebP of uint8 RGB or gray pixels, with its ICC profile.
///
/// A lossy export has no encoder in this build, so the caller reports the block reason
/// rather than writing a lossless file under a lossy setting.
#[must_use]
pub fn encode_webp_lossless(
    pixels: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    icc: Option<&[u8]>,
) -> Option<Vec<u8>> {
    if width == 0
        || height == 0
        || !(channels == 1 || channels == 3)
        || width > 16384
        || height > 16384
    {
        return None;
    }
    if pixels.len() != width * height * channels {
        return None;
    }
    let color = if channels == 1 {
        image_webp::ColorType::L8
    } else {
        image_webp::ColorType::Rgb8
    };
    let mut out = Vec::new();
    let mut encoder = image_webp::WebPEncoder::new(&mut out);
    if let Some(icc) = icc.filter(|value| !value.is_empty()) {
        encoder.set_icc_profile(icc.to_vec());
    }
    encoder
        .encode(pixels, width as u32, height as u32, color)
        .ok()?;
    Some(out)
}
