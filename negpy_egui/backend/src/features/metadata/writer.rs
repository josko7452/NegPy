//! Mirrors the EXIF half of `negpy/features/metadata/writer.py`: the tags a file carries.

use crate::features::metadata::capture::dms;
use crate::features::metadata::exif_read::{ExifTags, ExifValue, SourceExif};
use crate::features::metadata::payload::{MetadataPayload, NEGPY_SOFTWARE};
use crate::features::metadata::resolution::{Resolution, to_rational};

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
            let Some((sum, at)) = parse_terms(&text[start..end]) else {
                continue;
            };
            if at != end - start {
                continue;
            }
            if start >= 2
                && matches!(bytes[start - 2], b'+' | b'-' | 0xe2)
                && bytes[start - 1] == b' '
            {
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
        if start == at {
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
        assert_eq!(exposure_bias("1+2/3ev"), Some((5, 3)));
        assert_eq!(exposure_bias("-1-2/3ev"), Some((-5, 3)));
        assert_eq!(exposure_bias(".25ev"), Some((1, 4)));
        assert_eq!(exposure_bias("1 + 2/3 ev"), None);
    }
}
