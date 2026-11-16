//! Mirrors the EXIF half of `negpy/features/metadata/writer.py`: the tags a file carries.

use crate::features::metadata::capture::dms;
use crate::features::metadata::exif_read::{ExifTags, ExifValue};
use crate::features::metadata::payload::{MetadataPayload, NEGPY_SOFTWARE};
use crate::features::metadata::resolution::to_rational;

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
