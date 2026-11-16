//! Mirrors the EXIF half of `negpy/features/metadata/writer.py`: the tags a file carries.

use crate::features::metadata::capture::dms;
use crate::features::metadata::exif_read::{ExifTags, ExifValue, SourceExif};
use crate::features::metadata::payload::{MetadataPayload, NEGPY_SOFTWARE};
use crate::features::metadata::resolution::{Resolution, to_rational};
use crate::infrastructure::loaders::jxl_boxes::{
    JXL_CODESTREAM, JXL_EXIF_BOX, JXL_SIGNATURE, JXL_XMP_BOX, jxl_boxes,
};

pub const TAG_IMAGE_DESCRIPTION: u16 = 270;
pub const TAG_MAKE: u16 = 271;
pub const TAG_MODEL: u16 = 272;
pub const TAG_ORIENTATION: u16 = 274;
pub const TAG_SOFTWARE: u16 = 305;
pub const TAG_X_RESOLUTION: u16 = 282;
pub const TAG_Y_RESOLUTION: u16 = 283;
pub const TAG_RESOLUTION_UNIT: u16 = 296;
pub const TAG_EXIF_IFD: u16 = 34665;
pub const TAG_GPS_IFD: u16 = 34853;
pub const TAG_TIFF_EP_STANDARD: u16 = 37398;
pub const TAG_XMP_PACKET: u16 = 700;

pub const TAG_EXPOSURE_TIME: u16 = 33434;
pub const TAG_F_NUMBER: u16 = 33437;
pub const TAG_ISO: u16 = 34855;
pub const TAG_EXPOSURE_BIAS: u16 = 37380;
pub const TAG_MAX_APERTURE: u16 = 37381;
pub const TAG_FOCAL_LENGTH: u16 = 37386;
pub const TAG_USER_COMMENT: u16 = 37510;
pub const TAG_MAKER_NOTE: u16 = 37500;
pub const TAG_DATE_TIME_ORIGINAL: u16 = 36867;
pub const TAG_DATE_TIME_DIGITIZED: u16 = 36868;
pub const TAG_OFFSET_TIME_ORIGINAL: u16 = 36881;
pub const TAG_LENS_MAKE: u16 = 42035;
pub const TAG_LENS_MODEL: u16 = 42036;
pub const TAG_COLOR_SPACE: u16 = 40961;
pub const TAG_INTEROP_IFD: u16 = 40965;
pub const TAG_CFA_PATTERN: u16 = 41730;
pub const TAG_SENSING_METHOD: u16 = 41495;
pub const TAG_FILE_SOURCE: u16 = 41728;
pub const TAG_SCENE_TYPE: u16 = 41729;

pub const TAG_GPS_VERSION: u16 = 0;
pub const TAG_GPS_LATITUDE_REF: u16 = 1;
pub const TAG_GPS_LATITUDE: u16 = 2;
pub const TAG_GPS_LONGITUDE_REF: u16 = 3;
pub const TAG_GPS_LONGITUDE: u16 = 4;
pub const TAG_GPS_MAP_DATUM: u16 = 18;
pub const APP1_EXIF_LIMIT: usize = 65533;
const XMP_APP1_HEADER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

pub const TAG_FOCAL_LENGTH_35MM: u16 = 41989;
pub const TAG_LENS_SERIAL: u16 = 42037;
pub const TAG_BODY_SERIAL: u16 = 42033;
pub const TAG_LENS_SPECIFICATION: u16 = 42034;
pub const TAG_SUBJECT_DISTANCE: u16 = 37382;
pub const TAG_DIGITAL_ZOOM: u16 = 41988;

/// The five IFDs piexif-shaped EXIF carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExifData {
    pub zeroth: ExifTags,
    pub exif: ExifTags,
    pub gps: ExifTags,
    pub interop: ExifTags,
    pub first: ExifTags,
}

impl ExifData {
    /// A tag map by IFD name.
    #[must_use]
    pub fn ifd(&self, name: &str) -> Option<&ExifTags> {
        match name {
            "0th" => Some(&self.zeroth),
            "Exif" => Some(&self.exif),
            "GPS" => Some(&self.gps),
            "Interop" => Some(&self.interop),
            "1st" => Some(&self.first),
            _ => None,
        }
    }

    /// A mutable tag map by IFD name.
    pub fn ifd_mut(&mut self, name: &str) -> Option<&mut ExifTags> {
        match name {
            "0th" => Some(&mut self.zeroth),
            "Exif" => Some(&mut self.exif),
            "GPS" => Some(&mut self.gps),
            "Interop" => Some(&mut self.interop),
            "1st" => Some(&mut self.first),
            _ => None,
        }
    }
}

/// The TIFF field type piexif records for a tag, or None for a tag it does not know.
#[must_use]
pub fn tiff_type(ifd: &str, tag: u16) -> Option<u16> {
    let table: &[(u16, u16)] = match ifd {
        "0th" => &ZEROTH_TYPES,
        "Exif" => &EXIF_TYPES,
        "GPS" => &GPS_TYPES,
        "Interop" => &INTEROP_TYPES,
        _ => return None,
    };
    table
        .iter()
        .find(|(known, _)| *known == tag)
        .map(|(_, kind)| *kind)
}

const TIFF_BYTE: u16 = 1;
const TIFF_ASCII: u16 = 2;
const TIFF_SHORT: u16 = 3;
const TIFF_LONG: u16 = 4;
const TIFF_RATIONAL: u16 = 5;
const TIFF_UNDEFINED: u16 = 7;
const TIFF_SRATIONAL: u16 = 10;

const ZEROTH_TYPES: [(u16, u16); 23] = [
    (254, TIFF_LONG),
    (256, TIFF_LONG),
    (257, TIFF_LONG),
    (258, TIFF_SHORT),
    (259, TIFF_SHORT),
    (262, TIFF_SHORT),
    (270, TIFF_ASCII),
    (271, TIFF_ASCII),
    (272, TIFF_ASCII),
    (273, TIFF_LONG),
    (274, TIFF_SHORT),
    (277, TIFF_SHORT),
    (278, TIFF_LONG),
    (279, TIFF_LONG),
    (282, TIFF_RATIONAL),
    (283, TIFF_RATIONAL),
    (284, TIFF_SHORT),
    (296, TIFF_SHORT),
    (305, TIFF_ASCII),
    (330, TIFF_LONG),
    (513, TIFF_LONG),
    (514, TIFF_LONG),
    (37398, TIFF_BYTE),
];

const EXIF_TYPES: [(u16, u16); 22] = [
    (33434, TIFF_RATIONAL),
    (33437, TIFF_RATIONAL),
    (34855, TIFF_SHORT),
    (36867, TIFF_ASCII),
    (36868, TIFF_ASCII),
    (36881, TIFF_ASCII),
    (37380, TIFF_SRATIONAL),
    (37381, TIFF_RATIONAL),
    (37386, TIFF_RATIONAL),
    (37500, TIFF_UNDEFINED),
    (37510, TIFF_UNDEFINED),
    (40961, TIFF_SHORT),
    (40965, TIFF_LONG),
    (41495, TIFF_SHORT),
    (41728, TIFF_UNDEFINED),
    (41729, TIFF_UNDEFINED),
    (41730, TIFF_UNDEFINED),
    (42035, TIFF_ASCII),
    (42036, TIFF_ASCII),
    (42033, TIFF_ASCII),
    (42034, TIFF_RATIONAL),
    (42037, TIFF_ASCII),
];

const GPS_TYPES: [(u16, u16); 8] = [
    (0, TIFF_BYTE),
    (1, TIFF_ASCII),
    (2, TIFF_RATIONAL),
    (3, TIFF_ASCII),
    (4, TIFF_RATIONAL),
    (5, TIFF_BYTE),
    (6, TIFF_RATIONAL),
    (18, TIFF_ASCII),
];

const INTEROP_TYPES: [(u16, u16); 1] = [(1, TIFF_ASCII)];

const RAW_PREVIEW_0TH_TAGS: [u16; 17] = [
    254, 256, 257, 258, 259, 262, 273, 277, 278, 279, 284, 330, 513, 514, 37398, 0xC4A5, 700,
];
const RAW_CAPTURE_EXIF_TAGS: [u16; 4] = [41730, 41495, 41728, 41729];
const MAX_SAFE_0TH_TAG: u16 = TAG_GPS_IFD;

/// `_sanitize_exif`: drop values the writer cannot encode and tags tied to the source.
#[must_use]
pub fn sanitize_exif(data: &ExifData) -> ExifData {
    let mut result = ExifData::default();
    for (name, tags) in [
        ("0th", &data.zeroth),
        ("Exif", &data.exif),
        ("GPS", &data.gps),
        ("Interop", &data.interop),
        ("1st", &data.first),
    ] {
        let clean = sanitize_ifd(name, tags);
        match name {
            "0th" => result.zeroth = clean,
            "Exif" => result.exif = clean,
            "GPS" => result.gps = clean,
            "Interop" => result.interop = clean,
            "1st" => result.first = clean,
            _ => {}
        }
    }
    result
}

fn sanitize_ifd(name: &str, tags: &ExifTags) -> ExifTags {
    let mut clean = ExifTags::new();
    if name == "GPS"
        && !(tags.contains_key(&TAG_GPS_LATITUDE) || tags.contains_key(&TAG_GPS_LONGITUDE))
    {
        return clean;
    }
    for (tag, value) in tags {
        if name == "0th" && RAW_PREVIEW_0TH_TAGS.contains(tag) {
            continue;
        }
        if name == "0th" && *tag > MAX_SAFE_0TH_TAG {
            continue;
        }
        if name == "Exif" && RAW_CAPTURE_EXIF_TAGS.contains(tag) {
            continue;
        }
        if name == "Exif" && matches!(*tag, TAG_COLOR_SPACE | TAG_MAKER_NOTE) {
            continue;
        }
        let kind = tiff_type(name, *tag);
        if matches!(*value, ExifValue::Bytes(_))
            && matches!(kind, Some(TIFF_RATIONAL | TIFF_SRATIONAL))
        {
            continue;
        }
        if kind == Some(TIFF_SHORT) && short_overflows(value) {
            continue;
        }
        clean.insert(*tag, value.clone());
    }
    clean
}

fn short_overflows(value: &ExifValue) -> bool {
    match value {
        ExifValue::UInt(value) => *value > u64::from(u16::MAX),
        ExifValue::SInt(value) => !(0..=i64::from(u16::MAX)).contains(value),
        ExifValue::Rationals(values) => values.iter().any(|(num, _)| !(0..=65535).contains(num)),
        _ => false,
    }
}

/// `_prepare_jpeg_exif`: sanitize and drop the thumbnail directory.
#[must_use]
pub fn prepare_jpeg_exif(data: &ExifData) -> ExifData {
    let mut prepared = sanitize_exif(data);
    prepared.first.clear();
    prepared
}

/// `_demote_scan_datetime`: keep the source timestamp as the digitized time.
pub fn demote_scan_datetime(data: &mut ExifData) {
    if let Some(source) = data.exif.get(&TAG_DATE_TIME_ORIGINAL).cloned()
        && !data.exif.contains_key(&TAG_DATE_TIME_DIGITIZED)
    {
        data.exif.insert(TAG_DATE_TIME_DIGITIZED, source);
    }
}

impl From<&SourceExif> for ExifData {
    fn from(source: &SourceExif) -> Self {
        Self {
            zeroth: source.zeroth.clone(),
            exif: source.exif.clone(),
            gps: source.gps.clone(),
            interop: ExifTags::new(),
            first: ExifTags::new(),
        }
    }
}

/// `_with_resolution`: replace the EXIF resolution record, when one was chosen.
#[must_use]
pub fn with_resolution(mut data: ExifData, resolution: Option<&Resolution>) -> ExifData {
    let Some(resolution) = resolution else {
        return data;
    };
    data.zeroth.insert(
        TAG_X_RESOLUTION,
        ExifValue::Rational(resolution.x.0, resolution.x.1),
    );
    data.zeroth.insert(
        TAG_Y_RESOLUTION,
        ExifValue::Rational(resolution.y.0, resolution.y.1),
    );
    data.zeroth.insert(
        TAG_RESOLUTION_UNIT,
        ExifValue::UInt(u64::from(resolution.unit)),
    );
    data
}

/// `_merged_exif_and_payload`: source EXIF plus the capture fields the config sets.
#[must_use]
pub fn merge_exif(source: Option<&ExifData>, payload: &MetadataPayload) -> ExifData {
    let mut merged = source.cloned().unwrap_or_default();
    let custom = build_custom_exif(payload);
    if payload.exif_flags.strip_scan_residuals() {
        strip_scan_exif_for_capture(&mut merged);
    }
    if payload.capture_date.is_some() {
        demote_scan_datetime(&mut merged);
    }
    if payload.gps_latitude.is_some() && payload.gps_longitude.is_some() {
        merged.gps.clear();
    }
    for (name, tags) in [
        ("0th", custom.zeroth),
        ("Exif", custom.exif),
        ("GPS", custom.gps),
        ("Interop", custom.interop),
        ("1st", custom.first),
    ] {
        if tags.is_empty() {
            continue;
        }
        if let Some(target) = merged.ifd_mut(name) {
            target.extend(tags);
        }
    }
    merged.zeroth.insert(TAG_ORIENTATION, ExifValue::UInt(1));
    merged.first.remove(&TAG_ORIENTATION);
    merged
}

/// `strip_scan_exif_for_capture`: remove the rig's lens and body fields.
pub fn strip_scan_exif_for_capture(data: &mut ExifData) {
    for tag in [
        TAG_FOCAL_LENGTH_35MM,
        TAG_LENS_SERIAL,
        TAG_BODY_SERIAL,
        TAG_LENS_SPECIFICATION,
        TAG_SUBJECT_DISTANCE,
        TAG_DIGITAL_ZOOM,
        TAG_EXPOSURE_TIME,
        TAG_F_NUMBER,
        TAG_ISO,
    ] {
        data.exif.remove(&tag);
    }
}

struct EncodedValue {
    tag: u16,
    kind: u16,
    count: u32,
    bytes: Vec<u8>,
    inline: bool,
}

fn encoded_values(tags: &ExifTags, name: &str) -> Vec<EncodedValue> {
    let mut values = Vec::new();
    for (tag, value) in tags {
        let Some(kind) = tiff_type(name, *tag) else {
            continue;
        };
        let Some((count, bytes)) = encode_value(kind, value) else {
            continue;
        };
        values.push(EncodedValue {
            tag: *tag,
            kind,
            count,
            bytes,
            inline: false,
        });
    }
    values.sort_by_key(|entry| entry.tag);
    values
}

fn encode_value(kind: u16, value: &ExifValue) -> Option<(u32, Vec<u8>)> {
    match (kind, value) {
        (TIFF_ASCII, ExifValue::Text(text)) => {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            Some((bytes.len() as u32, bytes))
        }
        (TIFF_ASCII, ExifValue::Bytes(bytes)) => {
            let mut bytes = bytes.clone();
            if !bytes.ends_with(&[0]) {
                bytes.push(0);
            }
            Some((bytes.len() as u32, bytes))
        }
        (TIFF_BYTE | TIFF_UNDEFINED, ExifValue::Bytes(bytes)) => {
            Some((bytes.len() as u32, bytes.clone()))
        }
        (TIFF_SHORT, ExifValue::UInt(value)) => {
            let value = u16::try_from(*value).ok()?;
            Some((1, value.to_le_bytes().to_vec()))
        }
        (TIFF_LONG, ExifValue::UInt(value)) => {
            let value = u32::try_from(*value).ok()?;
            Some((1, value.to_le_bytes().to_vec()))
        }
        (TIFF_SRATIONAL, ExifValue::Rational(num, den)) => {
            let mut bytes = Vec::with_capacity(8);
            bytes.extend_from_slice(&(*num as i32).to_le_bytes());
            bytes.extend_from_slice(&(*den as i32).to_le_bytes());
            Some((1, bytes))
        }
        (TIFF_RATIONAL, ExifValue::Rational(num, den)) => {
            let mut bytes = Vec::with_capacity(8);
            bytes.extend_from_slice(&u32::try_from(*num).ok()?.to_le_bytes());
            bytes.extend_from_slice(&u32::try_from(*den).ok()?.to_le_bytes());
            Some((1, bytes))
        }
        (TIFF_RATIONAL, ExifValue::Rationals(values)) => {
            let mut bytes = Vec::with_capacity(values.len() * 8);
            for (num, den) in values {
                bytes.extend_from_slice(&u32::try_from(*num).ok()?.to_le_bytes());
                bytes.extend_from_slice(&u32::try_from(*den).ok()?.to_le_bytes());
            }
            Some((values.len() as u32, bytes))
        }
        _ => None,
    }
}

fn encode_directory(values: &mut [EncodedValue], extra_at: usize, next: u32) -> (Vec<u8>, Vec<u8>) {
    let mut directory = Vec::with_capacity(2 + values.len() * 12 + 4);
    directory.extend_from_slice(&(values.len() as u16).to_le_bytes());
    let mut extra = Vec::new();
    for entry in values {
        directory.extend_from_slice(&entry.tag.to_le_bytes());
        directory.extend_from_slice(&entry.kind.to_le_bytes());
        directory.extend_from_slice(&entry.count.to_le_bytes());
        if entry.bytes.len() <= 4 || entry.inline {
            directory.extend_from_slice(&entry.bytes);
            directory.resize(directory.len() + 4 - entry.bytes.len(), 0);
        } else {
            let place = extra_at + extra.len();
            directory.extend_from_slice(&(place as u32).to_le_bytes());
            extra.extend_from_slice(&entry.bytes);
        }
    }
    directory.extend_from_slice(&next.to_le_bytes());
    (directory, extra)
}

/// `piexif.dump`: a little-endian TIFF block with its IFD0, Exif and GPS directories.
#[must_use]
pub fn dump_exif(data: &ExifData) -> Vec<u8> {
    dump_exif_with_pixels(data, 0)
}

/// The same block with the image data after it, so a reader can resolve strip offsets.
#[must_use]
pub fn dump_exif_with_pixels(data: &ExifData, pixels: usize) -> Vec<u8> {
    let mut zeroth = encoded_values(&data.zeroth, "0th");
    let mut exif = encoded_values(&data.exif, "Exif");
    let mut gps = encoded_values(&data.gps, "GPS");
    if !exif.is_empty() {
        zeroth.push(EncodedValue {
            tag: TAG_EXIF_IFD,
            kind: TIFF_LONG,
            count: 1,
            bytes: vec![0; 4],
            inline: true,
        });
    }
    if !gps.is_empty() {
        zeroth.push(EncodedValue {
            tag: TAG_GPS_IFD,
            kind: TIFF_LONG,
            count: 1,
            bytes: vec![0; 4],
            inline: true,
        });
    }
    zeroth.sort_by_key(|entry| entry.tag);
    let zeroth_size = 2 + zeroth.len() * 12 + 4;
    let exif_size = 2 + exif.len() * 12 + 4;
    let gps_size = 2 + gps.len() * 12 + 4;
    let exif_at = 8 + zeroth_size;
    let gps_at = exif_at + if exif.is_empty() { 0 } else { exif_size };
    let mut extra_at = gps_at + if gps.is_empty() { 0 } else { gps_size };
    for entry in &mut zeroth {
        if entry.tag == TAG_EXIF_IFD {
            entry.bytes = (exif_at as u32).to_le_bytes().to_vec();
        } else if entry.tag == TAG_GPS_IFD {
            entry.bytes = (gps_at as u32).to_le_bytes().to_vec();
        }
    }
    let (zeroth_dir, mut zeroth_extra) = encode_directory(&mut zeroth, extra_at, 0);
    extra_at += zeroth_extra.len();
    let (exif_dir, mut exif_extra) = encode_directory(&mut exif, extra_at, 0);
    extra_at += exif_extra.len();
    let (gps_dir, gps_extra) = encode_directory(&mut gps, extra_at, 0);
    zeroth_extra.extend_from_slice(&exif_extra);
    exif_extra.clear();
    let mut out = Vec::new();
    out.extend_from_slice(b"II*\0");
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&zeroth_dir);
    out.extend_from_slice(&exif_dir);
    out.extend_from_slice(&gps_dir);
    out.extend_from_slice(&zeroth_extra);
    out.extend_from_slice(&gps_extra);
    out.resize(out.len() + pixels, 0);
    out
}

/// `_strip_jpeg_xmp_segments`: remove Adobe XMP APP1 segments.
#[must_use]
pub fn strip_jpeg_xmp_segments(data: &[u8]) -> Vec<u8> {
    let mut out = data.get(..2).unwrap_or(data).to_vec();
    let mut at = 2usize;
    while at < data.len() {
        if data[at] != 0xFF {
            out.extend_from_slice(&data[at..]);
            break;
        }
        let marker = data[at + 1];
        if marker == 0xD9 {
            out.extend_from_slice(&data[at..]);
            break;
        }
        if (0xD0..0xD8).contains(&marker) {
            out.extend_from_slice(&data[at..at + 2]);
            at += 2;
            continue;
        }
        if at + 4 > data.len() {
            out.extend_from_slice(&data[at..]);
            break;
        }
        let length = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
        let end = at + 2 + length;
        if marker == 0xE1 && end <= data.len() {
            let payload = at + 4;
            if data.get(payload..payload + XMP_APP1_HEADER.len()) == Some(XMP_APP1_HEADER) {
                at = end;
                continue;
            }
        }
        if end > data.len() {
            out.extend_from_slice(&data[at..]);
            break;
        }
        out.extend_from_slice(&data[at..end]);
        at = end;
    }
    out
}

/// `_inject_jpeg_xmp`: put the packet after the leading application segments.
#[must_use]
pub fn inject_jpeg_xmp(jpeg: &[u8], xmp: &[u8]) -> Vec<u8> {
    if xmp.is_empty() {
        return jpeg.to_vec();
    }
    let cleaned = strip_jpeg_xmp_segments(jpeg);
    let payload_len = XMP_APP1_HEADER.len() + xmp.len();
    if payload_len + 2 > u16::MAX as usize {
        return jpeg.to_vec();
    }
    let mut segment = Vec::with_capacity(payload_len + 4);
    segment.extend_from_slice(&[0xFF, 0xE1]);
    segment.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes());
    segment.extend_from_slice(XMP_APP1_HEADER);
    segment.extend_from_slice(xmp);
    let mut insert_at = 2usize;
    let mut at = 2usize;
    while at < cleaned.len() && cleaned[at] == 0xFF {
        let marker = cleaned[at + 1];
        if (0xD0..0xD8).contains(&marker) {
            at += 2;
            continue;
        }
        if at + 4 > cleaned.len() {
            break;
        }
        let length = u16::from_be_bytes([cleaned[at + 2], cleaned[at + 3]]) as usize;
        let end = at + 2 + length;
        if matches!(marker, 0xE0 | 0xE1 | 0xED | 0xFE) {
            insert_at = end;
            at = end;
            continue;
        }
        break;
    }
    let mut out = Vec::with_capacity(cleaned.len() + segment.len());
    out.extend_from_slice(&cleaned[..insert_at]);
    out.extend_from_slice(&segment);
    out.extend_from_slice(&cleaned[insert_at..]);
    out
}

/// A PNG with fresh eXIf and iTXt XMP chunks ahead of its image data.
#[must_use]
pub fn rewrite_png_metadata(png: &[u8], exif: Option<&[u8]>, xmp: Option<&[u8]>) -> Vec<u8> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png.to_vec();
    }
    let Some(first_idat) = png.windows(4).position(|window| window == b"IDAT") else {
        return png.to_vec();
    };
    let cut = first_idat.saturating_sub(4);
    let mut kept = png[..8].to_vec();
    let mut at = 8usize;
    while at + 12 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let end = at + 12 + length;
        if end > png.len() {
            return png.to_vec();
        }
        let kind = &png[at + 4..at + 8];
        if kind != b"eXIf" && kind != b"iTXt" {
            kept.extend_from_slice(&png[at..end]);
        }
        at = end;
    }
    if at < png.len() {
        kept.extend_from_slice(&png[at..]);
    }
    let mut chunks = Vec::new();
    if let Some(exif) = exif.filter(|value| !value.is_empty()) {
        let body = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
        chunks.extend_from_slice(&png_chunk(b"eXIf", body));
    }
    if let Some(xmp) = xmp.filter(|value| !value.is_empty()) {
        let mut body = b"XML:com.adobe.xmp\0\0\0\0\0".to_vec();
        body.extend_from_slice(xmp);
        chunks.extend_from_slice(&png_chunk(b"iTXt", &body));
    }
    let cut_in_kept = kept
        .windows(4)
        .position(|window| window == b"IDAT")
        .map_or(cut.min(kept.len()), |at| at.saturating_sub(4));
    let mut out = Vec::with_capacity(kept.len() + chunks.len());
    out.extend_from_slice(&kept[..cut_in_kept]);
    out.extend_from_slice(&chunks);
    out.extend_from_slice(&kept[cut_in_kept..]);
    out
}

fn png_chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc = Vec::with_capacity(4 + body.len());
    crc.extend_from_slice(kind);
    crc.extend_from_slice(body);
    out.extend_from_slice(&crc32fast::hash(&crc).to_be_bytes());
    out
}

/// A WebP rebuilt in extended form with EXIF and XMP chunks in spec order.
#[must_use]
pub fn rewrite_webp_metadata(webp: &[u8], exif: Option<&[u8]>, xmp: Option<&[u8]>) -> Vec<u8> {
    let Some(chunks) = webp_chunks(webp) else {
        return webp.to_vec();
    };
    let mut header = None;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut kept = Vec::new();
    for (kind, start, end) in &chunks {
        let payload = &webp[*start..*end];
        if *kind == *b"VP8X" {
            header = Some(payload.to_vec());
            if payload.len() >= 10 {
                width = 1 + u32::from_le_bytes([payload[4], payload[5], payload[6], 0]);
                height = 1 + u32::from_le_bytes([payload[7], payload[8], payload[9], 0]);
            }
        } else if *kind != *b"EXIF" && *kind != *b"XMP " {
            kept.push((*kind, payload.to_vec()));
        }
    }
    let mut vp8x = header.unwrap_or_else(|| vec![0; 10]);
    if vp8x.len() < 10 {
        vp8x.resize(10, 0);
    }
    if width == 0 || height == 0 {
        if let Some((_, size)) = kept.iter().find(|(kind, _)| *kind == *b"VP8 ") {
            if size.len() >= 10 {
                width = u32::from_le_bytes([size[6], size[7], 0, 0]) & 0x3fff;
                height = u32::from_le_bytes([size[8], size[9], 0, 0]) & 0x3fff;
            }
        }
    }
    if width == 0 || height == 0 {
        return webp.to_vec();
    }
    let exif_payload = exif
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_prefix(b"Exif\0\0").unwrap_or(value).to_vec());
    let mut flags = vp8x[0] & !0x0c;
    if kept.iter().any(|(kind, _)| *kind == *b"ICCP") {
        flags |= 0x20;
    }
    if exif_payload.is_some() {
        flags |= 0x08;
    }
    if xmp.is_some_and(|value| !value.is_empty()) {
        flags |= 0x04;
    }
    vp8x[0] = flags;
    vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    let mut parts = Vec::new();
    parts.extend_from_slice(&webp_chunk(b"VP8X", &vp8x));
    for (kind, payload) in kept.iter().filter(|(kind, _)| *kind == *b"ICCP") {
        parts.extend_from_slice(&webp_chunk(kind, payload));
    }
    for (kind, payload) in kept.iter().filter(|(kind, _)| *kind != *b"ICCP") {
        parts.extend_from_slice(&webp_chunk(kind, payload));
    }
    if let Some(exif) = exif_payload {
        parts.extend_from_slice(&webp_chunk(b"EXIF", &exif));
    }
    if let Some(xmp) = xmp.filter(|value| !value.is_empty()) {
        parts.extend_from_slice(&webp_chunk(b"XMP ", xmp));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((parts.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&parts);
    out
}

fn webp_chunks(data: &[u8]) -> Option<Vec<([u8; 4], usize, usize)>> {
    if data.get(..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WEBP") {
        return None;
    }
    let riff_end =
        (8 + u32::from_le_bytes(data.get(4..8)?.try_into().ok()?) as usize).min(data.len());
    let mut chunks = Vec::new();
    let mut at = 12usize;
    while at + 8 <= riff_end {
        let size = u32::from_le_bytes(data[at + 4..at + 8].try_into().ok()?) as usize;
        let start = at + 8;
        let end = start.checked_add(size)?;
        if end > riff_end {
            return None;
        }
        chunks.push((data[at..at + 4].try_into().ok()?, start, end));
        at = end + (size & 1);
    }
    (!chunks.is_empty()).then_some(chunks)
}

fn webp_chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len() + 1);
    out.extend_from_slice(kind);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() & 1 == 1 {
        out.push(0);
    }
    out
}

/// A JPEG XL container with fresh Exif and XMP boxes before its codestream.
#[must_use]
pub fn rewrite_jxl_metadata(jxl: &[u8], exif: Option<&[u8]>, xmp: Option<&[u8]>) -> Vec<u8> {
    let container = if jxl.starts_with(JXL_CODESTREAM) {
        let mut data = JXL_SIGNATURE.to_vec();
        data.extend_from_slice(&jxl_box(b"ftyp", b"jxl \0\0\0\0jxl "));
        data.extend_from_slice(&jxl_box(b"jxlc", jxl));
        data
    } else {
        jxl.to_vec()
    };
    let Ok(boxes) = jxl_boxes(&container) else {
        return jxl.to_vec();
    };
    let mut metadata = Vec::new();
    if let Some(exif) = exif.filter(|value| !value.is_empty()) {
        let tiff = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(tiff);
        metadata.extend_from_slice(&jxl_box(JXL_EXIF_BOX, &payload));
    }
    if let Some(xmp) = xmp.filter(|value| !value.is_empty()) {
        metadata.extend_from_slice(&jxl_box(JXL_XMP_BOX, xmp));
    }
    let mut out = Vec::new();
    let mut inserted = false;
    for entry in boxes {
        let kind = &container[entry.start - 4..entry.start];
        let is_metadata = entry.kind == *JXL_EXIF_BOX
            || entry.kind == *JXL_XMP_BOX
            || (entry.kind == *b"brob"
                && &container[entry.start..entry.start + 4] == JXL_EXIF_BOX.as_slice())
            || (entry.kind == *b"brob"
                && &container[entry.start..entry.start + 4] == JXL_XMP_BOX.as_slice());
        if is_metadata {
            continue;
        }
        if !inserted && (entry.kind == *b"jxlc" || entry.kind == *b"jxlp") {
            out.extend_from_slice(&metadata);
            inserted = true;
        }
        let size = (entry.end - (entry.start - 8)) as u32;
        out.extend_from_slice(&size.to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(&container[entry.start..entry.end]);
    }
    if !inserted {
        return jxl.to_vec();
    }
    out
}

/// `embed_metadata`: attach EXIF and XMP to an encoded export in its own container.
#[must_use]
pub fn embed_metadata(
    image: &[u8],
    payload: &MetadataPayload,
    source: Option<&SourceExif>,
    resolution: Option<&Resolution>,
) -> Vec<u8> {
    let merged = with_resolution(
        merge_exif(source.map(ExifData::from).as_ref(), payload),
        resolution,
    );
    let exif = sanitize_exif(&merged);
    let xmp = payload
        .has_any_data()
        .then(|| crate::features::metadata::xmp::build_xmp_bytes(payload, true));
    if image.starts_with(&[0xFF, 0xD8]) {
        let exif = prepare_jpeg_exif(&with_resolution(exif, None));
        let bytes = dump_exif(&exif);
        let with_exif = inject_jpeg_exif(image, &bytes);
        return match xmp {
            Some(xmp) => inject_jpeg_xmp(&with_exif, &xmp),
            None => with_exif,
        };
    }
    if image.starts_with(b"\x89PNG\r\n\x1a\n") {
        let exif = dump_exif(&exif);
        return rewrite_png_metadata(image, Some(&exif), xmp.as_deref());
    }
    if image.starts_with(b"RIFF") {
        let exif = dump_exif(&exif);
        return rewrite_webp_metadata(image, Some(&exif), xmp.as_deref());
    }
    if image.starts_with(JXL_SIGNATURE) || image.starts_with(JXL_CODESTREAM) {
        let exif = dump_exif(&exif);
        return rewrite_jxl_metadata(image, Some(&exif), xmp.as_deref());
    }
    image.to_vec()
}

fn jxl_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

/// Remove an existing Exif APP1 segment and insert a fresh one after APP0.
#[must_use]
pub fn inject_jpeg_exif(jpeg: &[u8], exif: &[u8]) -> Vec<u8> {
    let cleaned = strip_jpeg_exif_segment(jpeg);
    if exif.is_empty() || exif.len() + 8 > u16::MAX as usize {
        return cleaned;
    }
    let mut segment = Vec::with_capacity(exif.len() + 10);
    segment.extend_from_slice(&[0xFF, 0xE1]);
    segment.extend_from_slice(&((exif.len() + 8) as u16).to_be_bytes());
    segment.extend_from_slice(b"Exif\0\0");
    segment.extend_from_slice(exif);
    let insert_at = after_app0(&cleaned);
    let mut out = Vec::with_capacity(cleaned.len() + segment.len());
    out.extend_from_slice(&cleaned[..insert_at]);
    out.extend_from_slice(&segment);
    out.extend_from_slice(&cleaned[insert_at..]);
    out
}

fn strip_jpeg_exif_segment(data: &[u8]) -> Vec<u8> {
    let mut out = data.get(..2).unwrap_or(data).to_vec();
    let mut at = 2usize;
    while at + 4 <= data.len() && data[at] == 0xFF {
        let marker = data[at + 1];
        if marker == 0xD9 || marker == 0xDA {
            out.extend_from_slice(&data[at..]);
            return out;
        }
        if (0xD0..0xD8).contains(&marker) {
            out.extend_from_slice(&data[at..at + 2]);
            at += 2;
            continue;
        }
        let length = u16::from_be_bytes([data[at + 2], data[at + 3]]) as usize;
        let end = at + 2 + length;
        if end > data.len() {
            out.extend_from_slice(&data[at..]);
            return out;
        }
        if marker == 0xE1 && data.get(at + 4..at + 10) == Some(b"Exif\0\0") {
            at = end;
            continue;
        }
        out.extend_from_slice(&data[at..end]);
        at = end;
    }
    if at < data.len() {
        out.extend_from_slice(&data[at..]);
    }
    out
}

fn after_app0(data: &[u8]) -> usize {
    if data.len() >= 6 && data[2] == 0xFF && data[3] == 0xE0 {
        let length = u16::from_be_bytes([data[4], data[5]]) as usize;
        (4 + length).min(data.len())
    } else {
        2
    }
}

/// `_exif_ascii`: 7-bit EXIF text with the punctuation NegPy writes transliterated.
#[must_use]
pub fn exif_ascii(text: &str) -> Vec<u8> {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{2022}' => out.push('-'),
            '\u{00d7}' => out.push('x'),
            '\u{00b0}' => {}
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            '\u{2026}' => out.push_str("..."),
            '\u{00a0}' => out.push(' '),
            other if other.is_ascii() => out.push(other),
            _ => out.push('?'),
        }
    }
    out.into_bytes()
}

/// `_rational_tuple`: the nearest fraction with a denominator at most 1000.
#[must_use]
pub fn rational_tuple(value: f64) -> (i64, i64) {
    to_rational(value)
}

/// `_build_custom_exif`: the capture tags resolved from a payload.
#[must_use]
pub fn build_custom_exif(payload: &MetadataPayload) -> ExifData {
    let mut data = ExifData::default();
    let flags = payload.exif_flags;

    let description = if !payload.image_description.is_empty() {
        &payload.image_description
    } else {
        &payload.film_stock
    };
    if !description.is_empty() {
        data.zeroth.insert(
            TAG_IMAGE_DESCRIPTION,
            ExifValue::Bytes(exif_ascii(description)),
        );
    }
    data.zeroth
        .insert(TAG_SOFTWARE, ExifValue::Bytes(exif_ascii(NEGPY_SOFTWARE)));

    if flags.camera {
        if !payload.camera_make.is_empty() {
            data.zeroth
                .insert(TAG_MAKE, ExifValue::Bytes(exif_ascii(&payload.camera_make)));
        }
        if !payload.camera_model.is_empty() {
            data.zeroth.insert(
                TAG_MODEL,
                ExifValue::Bytes(exif_ascii(&payload.camera_model)),
            );
        }
    }
    if flags.lens {
        if !payload.lens_make.is_empty() {
            data.exif.insert(
                TAG_LENS_MAKE,
                ExifValue::Bytes(exif_ascii(&payload.lens_make)),
            );
        }
        if !payload.lens_model.is_empty() {
            data.exif.insert(
                TAG_LENS_MODEL,
                ExifValue::Bytes(exif_ascii(&payload.lens_model)),
            );
        }
        if let Some(value) = payload.focal_length_mm {
            let (num, den) = rational_tuple(value);
            data.exif
                .insert(TAG_FOCAL_LENGTH, ExifValue::Rational(num, den));
        }
        if let Some(value) = payload.max_aperture {
            let (num, den) = rational_tuple(value);
            data.exif
                .insert(TAG_F_NUMBER, ExifValue::Rational(num, den));
            if value >= 1.0 {
                let (num, den) = rational_tuple(2.0 * value.log2());
                data.exif
                    .insert(TAG_MAX_APERTURE, ExifValue::Rational(num, den));
            }
        }
    }
    if flags.film_iso
        && let Some(iso) = payload.iso
    {
        data.exif
            .insert(TAG_ISO, ExifValue::UInt(iso.max(0) as u64));
    }

    let mut comment = Vec::new();
    for (key, value) in [
        ("film", &payload.film_stock),
        ("format", &payload.film_format),
        ("developer", &payload.developer),
        ("dilution", &payload.dilution),
        ("push_pull", &payload.push_pull),
        ("development_time", &payload.development_time),
        ("development_temperature", &payload.development_temperature),
    ] {
        if value.is_empty() || (key == "push_pull" && value == "Normal") {
            continue;
        }
        if !comment.is_empty() {
            comment.push(b'\n');
        }
        comment.extend_from_slice(key.replace('_', " ").to_title_case().as_bytes());
        comment.extend_from_slice(b": ");
        comment.extend_from_slice(&exif_ascii(value));
    }
    if !comment.is_empty() {
        let mut bytes = b"ASCII\0\0\0".to_vec();
        bytes.extend_from_slice(&comment);
        data.exif.insert(TAG_USER_COMMENT, ExifValue::Bytes(bytes));
    }

    if flags.exposure && !payload.capture_exposure.is_empty() {
        let parsed = parse_exposure(&payload.capture_exposure);
        for (tag, value) in parsed {
            data.exif.insert(tag, value);
        }
    }
    if let Some(date) = &payload.capture_date {
        data.exif.insert(
            TAG_DATE_TIME_ORIGINAL,
            ExifValue::Bytes(exif_ascii(&date.exif_text())),
        );
        if !date.tz_offset.is_empty() {
            data.exif.insert(
                TAG_OFFSET_TIME_ORIGINAL,
                ExifValue::Bytes(exif_ascii(&date.tz_offset)),
            );
        }
    }
    if let (Some(latitude), Some(longitude)) = (payload.gps_latitude, payload.gps_longitude) {
        data.gps.insert(
            TAG_GPS_VERSION,
            ExifValue::UInt(u64::from(u32::from_be_bytes([2, 3, 0, 0]))),
        );
        data.gps.insert(
            TAG_GPS_LATITUDE_REF,
            ExifValue::Text(if latitude >= 0.0 { "N" } else { "S" }.to_string()),
        );
        data.gps.insert(
            TAG_GPS_LATITUDE,
            ExifValue::Rationals(dms(latitude).to_vec()),
        );
        data.gps.insert(
            TAG_GPS_LONGITUDE_REF,
            ExifValue::Text(if longitude >= 0.0 { "E" } else { "W" }.to_string()),
        );
        data.gps.insert(
            TAG_GPS_LONGITUDE,
            ExifValue::Rationals(dms(longitude).to_vec()),
        );
        data.gps
            .insert(TAG_GPS_MAP_DATUM, ExifValue::Text("WGS-84".to_string()));
    }
    data
}

/// `_parse_exposure_str`: shutter, aperture, ISO and bias from free exposure text.
#[must_use]
pub fn parse_exposure(text: &str) -> Vec<(u16, ExifValue)> {
    let mut tags = Vec::new();
    if let Some(value) = shutter_fraction(text) {
        tags.push((TAG_EXPOSURE_TIME, ExifValue::Rational(value.0, value.1)));
    }
    if let Some(value) = aperture_fraction(text) {
        tags.push((TAG_F_NUMBER, ExifValue::Rational(value.0, value.1)));
    }
    if let Some(iso) = unsigned_after(text, "ISO", false) {
        tags.push((TAG_ISO, ExifValue::UInt(iso)));
    }
    if let Some(value) = exposure_bias(text) {
        tags.push((TAG_EXPOSURE_BIAS, ExifValue::Rational(value.0, value.1)));
    }
    tags
}

fn shutter_fraction(text: &str) -> Option<(i64, i64)> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() {
            continue;
        }
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let numerator_end = end;
        let mut denominator = None;
        if end < bytes.len() && bytes[end] == b'/' {
            let den_start = end + 1;
            let mut den_end = den_start;
            while den_end < bytes.len()
                && (bytes[den_end].is_ascii_digit() || bytes[den_end] == b'.')
            {
                den_end += 1;
            }
            let Ok(den) = decimal_fraction(&text[den_start..den_end]).ok_or(()) else {
                continue;
            };
            if den.0 == 0 {
                return None;
            }
            denominator = Some(den);
            end = den_end;
        }
        let mut marker = end;
        while marker < bytes.len() && bytes[marker].is_ascii_whitespace() {
            marker += 1;
        }
        if marker < bytes.len() && bytes[marker] == b's' {
            let Ok(value) = decimal_fraction(&text[start..numerator_end]).ok_or(()) else {
                continue;
            };
            let (num, den) = match denominator {
                Some((den_num, den_den)) => (value.0 * den_den, value.1 * den_num),
                None => value,
            };
            if den == 0 {
                return None;
            }
            let divisor = gcd_i64(num.unsigned_abs(), den.unsigned_abs()).max(1) as i64;
            return Some((num / divisor, den / divisor));
        }
    }
    None
}

fn aperture_fraction(text: &str) -> Option<(i64, i64)> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len().saturating_sub(1) {
        if bytes[start] != b'f' || bytes[start + 1] != b'/' {
            continue;
        }
        let mut at = start + 2;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
            at += 1;
        }
        let value = decimal_fraction(&text[start..at])?;
        return Some(value);
    }
    None
}

fn unsigned_after(text: &str, marker: &str, require_space: bool) -> Option<u64> {
    let at = find_ignore_case(text, marker, 0)? + marker.len();
    let mut at = at;
    if require_space || text.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
        while text.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
    }
    let start = at;
    while text.as_bytes().get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    (at > start).then(|| text[start..at].parse().ok()).flatten()
}

fn decimal_fraction(text: &str) -> Option<(i64, i64)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let whole = if whole.is_empty() { "0" } else { whole };
    let digits: String = whole.chars().chain(fraction.chars()).collect();
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let numerator: i64 = digits.parse().ok()?;
    let denominator = 10i64.checked_pow(fraction.len() as u32)?;
    let numerator = if negative { -numerator } else { numerator };
    let divisor = gcd_i64(numerator.unsigned_abs(), denominator.unsigned_abs()).max(1) as i64;
    Some((numerator / divisor, denominator / divisor))
}

fn gcd_i64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

fn find_ignore_case(text: &str, needle: &str, from: usize) -> Option<usize> {
    text.get(from..)?
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
        .map(|at| at + from)
}

fn exposure_bias(text: &str) -> Option<(i64, i64)> {
    let mut from = 0;
    while let Some(ev) = find_ignore_case(text, "ev", from) {
        from = ev + 2;
        let bytes = text.as_bytes();
        let mut end = ev;
        if end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        for start in 0..end {
            let candidate = &text[start..end];
            if candidate.starts_with(' ') {
                continue;
            }
            if candidate.contains(" + ")
                || candidate.contains(" - ")
                || candidate.contains(" \u{2212} ")
            {
                return None;
            }
            if start >= 2
                && bytes[start - 1] == b' '
                && matches!(bytes[start - 2], b'+' | b'-' | 0xe2)
            {
                continue;
            }
            let Some((sum, at)) = parse_terms(candidate) else {
                continue;
            };
            if at != candidate.len() {
                continue;
            }
            return Some(reduce(sum));
        }
    }
    None
}

fn parse_terms(text: &str) -> Option<((i64, i64), usize)> {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    let mut total_num: i128 = 0;
    let mut total_den: i128 = 1;
    while at < bytes.len() {
        let mut sign = 1i128;
        if matches!(bytes[at], b'+' | b'-') {
            sign = if bytes[at] == b'-' { -1 } else { 1 };
            at += 1;
        } else if text
            .get(at..)
            .is_some_and(|rest| rest.starts_with('\u{2212}'))
        {
            sign = -1;
            at += '\u{2212}'.len_utf8();
        }
        let start = at;
        while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
            at += 1;
        }
        if start == at || (start + 1 == at && bytes[start] == b'.') {
            return None;
        }
        let (mut num, mut den) = decimal_fraction(&text[start..at])?;
        if at < bytes.len() && bytes[at] == b'/' {
            at += 1;
            let den_start = at;
            while at < bytes.len() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            if den_start == at {
                return None;
            }
            let (divisor, divisor_den) = decimal_fraction(&text[den_start..at])?;
            if divisor == 0 {
                return None;
            }
            num *= divisor_den;
            den *= divisor;
        }
        num *= sign as i64;
        total_num = total_num * den as i128 + num as i128 * total_den;
        total_den *= den as i128;
    }
    let divisor = gcd_i128(total_num.unsigned_abs(), total_den.unsigned_abs()).max(1) as i128;
    Some((
        ((total_num / divisor) as i64, (total_den / divisor) as i64),
        at,
    ))
}

fn gcd_i128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

trait TitleCase {
    fn to_title_case(&self) -> String;
}

impl TitleCase for str {
    fn to_title_case(&self) -> String {
        self.split(' ')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            })
            .collect::<Vec<String>>()
            .join(" ")
    }
}

fn reduce((mut num, mut den): (i64, i64)) -> (i64, i64) {
    if den < 0 {
        num = -num;
        den = -den;
    }
    let divisor = gcd_i64(num.unsigned_abs(), den.unsigned_abs()).max(1) as i64;
    (num / divisor, den / divisor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_text_folds_punctuation_to_ascii() {
        assert_eq!(
            exif_ascii("Development Temperature: 20 °C"),
            b"Development Temperature: 20 C"
        );
        assert_eq!(exif_ascii("4×5 • 1…2"), b"4x5 - 1...2");
    }

    #[test]
    fn the_exposure_fields_parse_from_free_text() {
        let tags = parse_exposure("ISO400 f/2.8 -2/3ev 1/60s");
        assert!(tags.contains(&(TAG_EXPOSURE_TIME, ExifValue::Rational(1, 60))));
        assert!(tags.contains(&(TAG_ISO, ExifValue::UInt(400))));
        assert!(tags.contains(&(TAG_EXPOSURE_BIAS, ExifValue::Rational(-2, 3))));
        let tags = parse_exposure("1/2.5s f/8");
        assert!(tags.contains(&(TAG_EXPOSURE_TIME, ExifValue::Rational(2, 5))));
        assert!(
            parse_exposure("1/0s")
                .iter()
                .all(|(tag, _)| *tag != TAG_EXPOSURE_TIME)
        );
    }

    #[test]
    fn an_ev_sum_keeps_its_exact_fraction() {
        assert_eq!(exposure_bias("1+2/3ev"), Some((5, 3)), "1+2/3");
        assert_eq!(exposure_bias("-1-2/3ev"), Some((-5, 3)), "-1-2/3");
        assert_eq!(exposure_bias(".25ev"), Some((1, 4)), ".25");
        assert_eq!(exposure_bias("1 + 2/3 ev"), None, "1 + 2/3");
    }
}
