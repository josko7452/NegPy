//! Mirrors the pure half of `negpy/features/metadata/exif_read.py`: EXIF values as the
//! camera or scanner marks them.
//!
//! Callers pass tag values they read from a file; this module holds no decoder.

/// The XMP packet tag a TIFF-based RAW keeps in IFD0.
pub const XMP_PACKET_TAG: u16 = 700;
const XMP_NS: &str = "http://ns.adobe.com/xap/1.0/";

/// A tag value as the reader hands it over.
#[derive(Clone, Debug, PartialEq)]
pub enum ExifValue {
    Text(String),
    Bytes(Vec<u8>),
    /// A rational numerator and denominator.
    Rational(i64, i64),
    /// The unsigned and signed integers the tag forms carry.
    UInt(u64),
    SInt(i64),
    Float(f64),
    /// A GPS triplet of rationals.
    Rationals(Vec<(i64, i64)>),
}

impl ExifValue {
    fn as_text(&self) -> String {
        match self {
            ExifValue::Text(text) => text.trim_end_matches('\0').trim().to_string(),
            ExifValue::Bytes(bytes) => String::from_utf8_lossy(bytes)
                .trim_end_matches('\0')
                .trim()
                .to_string(),
            ExifValue::Rational(num, den) => format!("{num}/{den}"),
            ExifValue::UInt(value) => value.to_string(),
            ExifValue::SInt(value) => value.to_string(),
            ExifValue::Float(value) => value.to_string(),
            ExifValue::Rationals(values) => values
                .iter()
                .map(|(num, den)| format!("{num}/{den}"))
                .collect::<Vec<_>>()
                .join(","),
        }
    }
}

/// A tag map, keyed by the EXIF tag number.
pub type ExifTags = std::collections::BTreeMap<u16, ExifValue>;

/// `safe_str`: a value as text without padding NULs.
#[must_use]
pub fn safe_str(value: Option<&ExifValue>) -> String {
    value.map(ExifValue::as_text).unwrap_or_default()
}

/// `rational_to_float`: a rational as a number, None for a zero denominator.
#[must_use]
pub fn rational_to_float(value: Option<&ExifValue>) -> Option<f64> {
    match value? {
        ExifValue::Rational(num, den) => {
            if *den == 0 {
                None
            } else {
                Some(*num as f64 / *den as f64)
            }
        }
        ExifValue::UInt(value) => Some(*value as f64),
        ExifValue::SInt(value) => Some(*value as f64),
        ExifValue::Float(value) => Some(*value),
        _ => None,
    }
}

/// `rational_to_int`: the nearest whole number.
#[must_use]
pub fn rational_to_int(value: Option<&ExifValue>) -> Option<i64> {
    rational_to_float(value).map(|value| value.round() as i64)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `_format_exposure_time`: an EXIF exposure rational as a camera marks it.
#[must_use]
pub fn format_exposure_time(num: i64, den: i64) -> String {
    if num == 0 || den == 0 {
        return String::new();
    }
    let negative = (num < 0) != (den < 0);
    let (mut n, mut d) = ((num.unsigned_abs()), den.unsigned_abs());
    let divisor = gcd(n, d).max(1);
    n /= divisor;
    d /= divisor;
    let sign = if negative { "-" } else { "" };
    if n == 1 {
        return format!("{sign}1/{d}s");
    }
    if n * 100 < d * 29 {
        let value = d as f64 / n as f64;
        return format!("{sign}1/{}s", round_g(value, 1));
    }
    format!("{sign}{}s", round_g(n as f64 / d as f64, 1))
}

/// Python's `:g` for a value rounded to one decimal.
fn round_g(value: f64, decimals: usize) -> String {
    let factor = 10f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        let text = format!("{rounded:.decimals$}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// `format_exposure`: scan exposure time, f-number and ISO from an Exif IFD.
#[must_use]
pub fn format_exposure(tags: &ExifTags) -> String {
    let mut parts = Vec::new();
    match tags.get(&33434) {
        Some(ExifValue::Rational(num, den)) => {
            let text = format_exposure_time(*num, *den);
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Some(ExifValue::UInt(value)) => parts.push(format!("{value}s")),
        Some(ExifValue::SInt(value)) => parts.push(format!("{value}s")),
        Some(ExifValue::Float(value)) => parts.push(format!("{value}s")),
        _ => {}
    }
    if let Some(value) = rational_to_float(tags.get(&33437)) {
        parts.push(format!("f/{value:.1}"));
    }
    if let Some(iso) = rational_to_int(tags.get(&34855)) {
        parts.push(format!("ISO {iso}"));
    }
    parts.join("  ")
}

/// `format_exif_datetime`: `YYYY:MM:DD HH:MM:SS` shown with ISO date separators.
#[must_use]
pub fn format_exif_datetime(value: Option<&ExifValue>) -> String {
    let text = safe_str(value);
    let bytes = text.as_bytes();
    if bytes.len() >= 10 && bytes[4] == b':' && bytes[7] == b':' {
        return format!("{}-{}-{}", &text[..4], &text[5..7], &text[8..]);
    }
    text
}

/// `gps_decimal`: signed decimal degrees from a DMS triplet and its hemisphere reference.
#[must_use]
pub fn gps_decimal(dms: Option<&ExifValue>, reference: Option<&ExifValue>) -> Option<f64> {
    let ExifValue::Rationals(parts) = dms? else {
        return None;
    };
    if parts.len() != 3 {
        return None;
    }
    let values: Option<Vec<f64>> = parts
        .iter()
        .map(|(num, den)| {
            if *den == 0 {
                None
            } else {
                Some(*num as f64 / *den as f64)
            }
        })
        .collect();
    let values = values?;
    let value = values[0] + values[1] / 60.0 + values[2] / 3600.0;
    let reference = safe_str(reference).to_uppercase();
    Some(if reference == "S" || reference == "W" {
        -value
    } else {
        value
    })
}

/// DSLR or scanner rig metadata read from the source before an export overwrite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanExif {
    pub camera_make: String,
    pub camera_model: String,
    pub lens_make: String,
    pub lens_model: String,
    pub focal_length_mm: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<i64>,
    pub exposure: String,
    pub datetime_original: String,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
}

impl ScanExif {
    /// Whether any rig field was read.
    #[must_use]
    pub fn has_any(&self) -> bool {
        !self.camera_make.is_empty()
            || !self.camera_model.is_empty()
            || !self.lens_make.is_empty()
            || !self.lens_model.is_empty()
            || self.focal_length_mm.is_some()
            || self.aperture.is_some()
            || self.iso.is_some()
            || !self.exposure.is_empty()
            || !self.datetime_original.is_empty()
            || self.gps_latitude.is_some()
    }
}

/// The EXIF blocks one file carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceExif {
    pub zeroth: ExifTags,
    pub exif: ExifTags,
    pub gps: ExifTags,
}

/// `extract_scan_from_exif`.
#[must_use]
pub fn extract_scan_from_exif(source: Option<&SourceExif>) -> ScanExif {
    let Some(source) = source else {
        return ScanExif::default();
    };
    let zeroth = &source.zeroth;
    let exif = &source.exif;
    let gps = &source.gps;
    let iso = match exif.get(&34855) {
        Some(ExifValue::UInt(value)) => i64::try_from(*value).ok(),
        Some(ExifValue::SInt(value)) => Some(*value),
        value => rational_to_int(value),
    };
    ScanExif {
        camera_make: safe_str(zeroth.get(&271)),
        camera_model: safe_str(zeroth.get(&272)),
        lens_make: safe_str(exif.get(&42035)),
        lens_model: safe_str(exif.get(&42036)),
        focal_length_mm: rational_to_float(exif.get(&37386)),
        aperture: rational_to_float(exif.get(&33437)),
        iso,
        exposure: format_exposure(exif),
        datetime_original: format_exif_datetime(exif.get(&36867).or_else(|| zeroth.get(&306))),
        gps_latitude: gps_decimal(gps.get(&2), gps.get(&1)),
        gps_longitude: gps_decimal(gps.get(&4), gps.get(&3)),
    }
}

/// `xmp_packet_from_exif`: the XMP packet in IFD0 tag 700, stripped of padding.
#[must_use]
pub fn xmp_packet_from_exif(source: Option<&SourceExif>) -> Option<Vec<u8>> {
    let value = source?.zeroth.get(&XMP_PACKET_TAG)?;
    let bytes = match value {
        ExifValue::Text(text) => text.clone().into_bytes(),
        ExifValue::Bytes(bytes) => bytes.clone(),
        _ => return None,
    };
    let end = bytes
        .iter()
        .rposition(|byte| !matches!(byte, b'\0' | b' ' | b'\t' | b'\r' | b'\n'))
        .map_or(0, |index| index + 1);
    (end > 0).then(|| bytes[..end].to_vec())
}

fn namespace_prefix(namespace: &str) -> Option<&'static str> {
    match namespace {
        XMP_NS => Some("xmp"),
        _ => None,
    }
}

/// The opening attribute or element of the first `name` in `namespace`.
fn rdf_value(packet: &str, namespace: &str, name: &str) -> Option<String> {
    let prefix = namespace_prefix(namespace)?;
    let attribute = format!(" {prefix}:{name}=\"");
    if let Some(start) = packet.find(&attribute) {
        let rest = &packet[start + attribute.len()..];
        return rest.find('"').map(|end| rest[..end].to_string());
    }
    let element = format!("<{prefix}:{name}>");
    if let Some(start) = packet.find(&element) {
        let rest = &packet[start + element.len()..];
        let close = format!("</{prefix}:{name}>");
        return rest.find(&close).map(|end| rest[..end].to_string());
    }
    None
}

/// `source_xmp_rating`: `xmp:Rating` as written; empty when absent, malformed or 0.
#[must_use]
pub fn source_xmp_rating(source: Option<&SourceExif>) -> String {
    let Some(packet) = xmp_packet_from_exif(source) else {
        return String::new();
    };
    let Ok(text) = String::from_utf8(packet) else {
        return String::new();
    };
    let Some(value) = rdf_value(&text, XMP_NS, "Rating") else {
        return String::new();
    };
    match value.trim().parse::<f64>() {
        Ok(rating) if rating != 0.0 && (-1.0..=5.0).contains(&rating) => value.trim().to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exposure_time_reads_as_the_camera_marks_it() {
        for (rational, shown) in [
            ((10, 300), "1/30s"),
            ((1, 30), "1/30s"),
            ((10, 25), "0.4s"),
            ((13, 10), "1.3s"),
            ((10, 75), "1/7.5s"),
            ((5, 0), ""),
        ] {
            assert_eq!(format_exposure_time(rational.0, rational.1), shown);
        }
    }

    #[test]
    fn the_exposure_line_joins_its_parts() {
        let mut tags = ExifTags::new();
        tags.insert(33434, ExifValue::Rational(1, 30));
        tags.insert(33437, ExifValue::Rational(28, 10));
        tags.insert(34855, ExifValue::UInt(400));
        assert_eq!(format_exposure(&tags), "1/30s  f/2.8  ISO 400");
    }

    #[test]
    fn the_exif_datetime_takes_iso_separators() {
        assert_eq!(
            format_exif_datetime(Some(&ExifValue::Text("2024:01:02 03:04:05".into()))),
            "2024-01-02 03:04:05"
        );
        assert_eq!(
            format_exif_datetime(Some(&ExifValue::Text("2024-01-02".into()))),
            "2024-01-02"
        );
        assert_eq!(format_exif_datetime(None), "");
    }

    #[test]
    fn a_gps_triplet_becomes_signed_degrees() {
        let dms = ExifValue::Rationals(vec![(51, 1), (30, 1), (0, 1)]);
        assert_eq!(
            gps_decimal(Some(&dms), Some(&ExifValue::Text("N".into()))),
            Some(51.5)
        );
        assert_eq!(
            gps_decimal(Some(&dms), Some(&ExifValue::Text("W".into()))),
            Some(-51.5)
        );
    }

    #[test]
    fn a_scan_record_joins_zeroth_and_exif_tags() {
        let mut zeroth = ExifTags::new();
        zeroth.insert(271, ExifValue::Text("Plustek".into()));
        zeroth.insert(272, ExifValue::Text("OpticFilm 8100".into()));
        zeroth.insert(306, ExifValue::Text("2025:01:15 12:00:00".into()));
        let mut exif = ExifTags::new();
        exif.insert(33437, ExifValue::Rational(28, 10));
        exif.insert(34855, ExifValue::UInt(100));
        let source = SourceExif {
            zeroth,
            exif,
            gps: ExifTags::new(),
        };
        let scan = extract_scan_from_exif(Some(&source));
        assert_eq!(scan.camera_make, "Plustek");
        assert_eq!(scan.camera_model, "OpticFilm 8100");
        assert_eq!(scan.aperture, Some(2.8));
        assert_eq!(scan.iso, Some(100));
        assert_eq!(scan.datetime_original, "2025-01-15 12:00:00");
        assert!(scan.has_any());
    }
}
