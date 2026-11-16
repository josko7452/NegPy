//! Mirrors the date and development-time helpers in `negpy/features/metadata/capture.py`.

/// A capture date known only to some precision, as compacted ISO text.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureDate {
    pub text: String,
    pub precision: &'static str,
    pub tz_offset: String,
}

impl CaptureDate {
    /// The year the text names.
    #[must_use]
    pub fn year(&self) -> i64 {
        self.text[..4].parse().unwrap_or(0)
    }

    /// `YYYYMMDD`, with unknown parts padded to January 1.
    #[must_use]
    pub fn compact(&self) -> String {
        let (year, month, day, _, _, _) = parts(&self.text);
        format!("{year:04}{month:02}{day:02}")
    }
}

fn number(value: Option<&str>, default: i64) -> i64 {
    value.and_then(|text| text.parse().ok()).unwrap_or(default)
}

fn parts(text: &str) -> (i64, i64, i64, i64, i64, i64) {
    let (date, time) = text.split_once(' ').unwrap_or((text, ""));
    let mut date_parts = date.split('-');
    let year = number(date_parts.next(), 1);
    let month = number(date_parts.next(), 1);
    let day = number(date_parts.next(), 1);
    let mut time_parts = time.split(':');
    let hour = number(time_parts.next(), 0);
    let minute = number(time_parts.next(), 0);
    let second = number(time_parts.next(), 0);
    (year, month, day, hour, minute, second)
}

/// A `CaptureDate` from a truncated ISO-8601 string, or `None` when it is not a real instant.
#[must_use]
pub fn parse_capture_date(text: &str) -> Option<CaptureDate> {
    let cleaned = text.trim().replace('/', "-");
    if cleaned.is_empty() {
        return None;
    }
    let (year, month, day, hour, minute, second) = parts(&cleaned);
    if !(1800..=2999).contains(&year) {
        return None;
    }
    chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)?.and_hms_opt(
        hour as u32,
        minute as u32,
        second as u32,
    )?;
    let mut normalized = format!("{year:04}");
    let mut precision = "year";
    if cleaned.contains('-') {
        normalized.push_str(&format!("-{month:02}"));
        precision = "month";
    }
    if cleaned.matches('-').count() >= 2 {
        normalized.push_str(&format!("-{day:02}"));
        precision = "day";
    }
    if cleaned.contains(' ') {
        normalized.push_str(&format!(" {hour:02}:{minute:02}"));
        precision = "minute";
    }
    if cleaned.matches(':').count() >= 2 {
        normalized.push_str(&format!(":{second:02}"));
        precision = "second";
    }
    Some(CaptureDate {
        text: normalized,
        precision,
        tz_offset: String::new(),
    })
}

/// Seconds as `mm:ss`; empty when unset.
#[must_use]
pub fn format_dev_time(seconds: Option<i64>) -> String {
    seconds.map_or_else(String::new, |seconds| {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    })
}

/// A temperature as Python's `:g` prints it; empty when unset.
#[must_use]
pub fn format_temperature(celsius: Option<f64>) -> String {
    match celsius {
        None => String::new(),
        Some(value) => format!("{value}"),
    }
}

/// Latitude and longitude from a typed pair or a pasted OpenStreetMap or Google Maps link.
#[must_use]
pub fn parse_coords(text: &str) -> Option<(f64, f64)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let valid = |pair: (f64, f64)| pair.0.abs() <= 90.0 && pair.1.abs() <= 180.0;
    if let Some(after) = text.split_once("map=").map(|(_, after)| after)
        && let Some(numbers) = parse_numbers(after, 3)
    {
        let pair = (numbers[1], numbers[2]);
        if valid(pair) {
            return Some(pair);
        }
    }
    if let Some(after) = text.split_once("mlat=").map(|(_, after)| after)
        && let Some(pair) = parse_number_pair(after)
        && valid(pair)
    {
        return Some(pair);
    }
    for marker in ['@', '='] {
        if let Some(after) = text.split_once(marker).map(|(_, after)| after)
            && let Some(pair) = parse_number_pair(after)
            && valid(pair)
        {
            return Some(pair);
        }
    }
    let numbers = parse_number_pair(text)?;
    (numbers.0.abs() <= 90.0 && numbers.1.abs() <= 180.0).then_some(numbers)
}

/// The first two signed decimals in `text`, whatever separator lies between them.
fn parse_number_pair(text: &str) -> Option<(f64, f64)> {
    let values = parse_numbers(text, 2)?;
    Some((values[0], values[1]))
}

fn parse_numbers(text: &str, count: usize) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    let mut rest = text;
    while values.len() < count {
        let start = rest.find(|ch: char| ch.is_ascii_digit() || ch == '-')?;
        rest = &rest[start..];
        let end = rest
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '-' || ch == '.'))
            .unwrap_or(rest.len());
        let token = &rest[..end];
        let value: f64 = token.parse().ok()?;
        values.push(value);
        rest = &rest[end..];
    }
    Some(values)
}

/// A coordinate pair as five decimal places.
#[must_use]
pub fn format_coords(lat: f64, lon: f64) -> String {
    format!("{lat:.5}, {lon:.5}")
}

/// One-line place label: names when known, otherwise the coordinates.
#[must_use]
pub fn place_summary(
    city: &str,
    state: &str,
    country: &str,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> String {
    let names: Vec<&str> = [city, state, country]
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();
    if !names.is_empty() {
        return names.join(", ");
    }
    match (latitude, longitude) {
        (Some(lat), Some(lon)) => format_coords(lat, lon),
        _ => String::new(),
    }
}

/// A WGS-84 position as EXIF's three rationals.
#[must_use]
pub fn dms(value: f64) -> [(i64, i64); 3] {
    let total = value.abs();
    let degrees = total as i64;
    let minutes_full = (total - degrees as f64) * 60.0;
    let minutes = minutes_full as i64;
    let mut seconds = ((minutes_full - minutes as f64) * 60.0 * 100.0).round() as i64;
    let mut minutes = minutes;
    let mut degrees = degrees;
    if seconds >= 6000 {
        seconds -= 6000;
        minutes += 1;
    }
    if minutes >= 60 {
        minutes -= 60;
        degrees += 1;
    }
    [(degrees, 1), (minutes, 1), (seconds, 100)]
}

/// XMP's `DDD,MM.mmK` form of a coordinate.
#[must_use]
pub fn xmp_coord(value: f64, positive: char, negative: char) -> String {
    let total = value.abs();
    let mut degrees = total as i64;
    let mut minutes = ((total - degrees as f64) * 60.0 * 10000.0).round() / 10000.0;
    if minutes >= 60.0 {
        minutes -= 60.0;
        degrees += 1;
    }
    let sign = if value >= 0.0 { positive } else { negative };
    format!("{degrees},{minutes:.4}{sign}")
}

#[cfg(test)]
mod coord_tests {
    use super::*;

    #[test]
    fn a_typed_pair_or_a_link_gives_the_coordinates() {
        assert_eq!(parse_coords("48.1486, 17.1077"), Some((48.1486, 17.1077)));
        assert_eq!(parse_coords("48.1486; 17.1077"), Some((48.1486, 17.1077)));
        assert_eq!(
            parse_coords("https://www.openstreetmap.org/#map=15/48.1486/17.1077"),
            Some((48.1486, 17.1077))
        );
        assert_eq!(
            parse_coords("https://maps.google.com/?q=48.1486,17.1077"),
            Some((48.1486, 17.1077))
        );
        assert_eq!(parse_coords("91.0, 17.0"), None);
        assert_eq!(parse_coords(""), None);
    }

    #[test]
    fn the_place_prefers_names_then_the_coordinates() {
        assert_eq!(
            place_summary("Bratislava", "", "Slovakia", Some(48.1), Some(17.1)),
            "Bratislava, Slovakia"
        );
        assert_eq!(
            place_summary("", "", "", Some(48.1486), Some(17.1077)),
            "48.14860, 17.10770"
        );
        assert_eq!(place_summary("", "", "", None, None), "");
    }

    #[test]
    fn the_dms_seconds_carry_into_the_minutes() {
        assert_eq!(dms(48.5), [(48, 1), (30, 1), (0, 100)]);
        assert_eq!(dms(59.9999), [(59, 1), (59, 1), (5964, 100)]);
    }

    #[test]
    fn the_xmp_coordinate_carries_minutes_over_sixty() {
        assert_eq!(xmp_coord(48.5, 'N', 'S'), "48,30.0000N");
        assert_eq!(xmp_coord(-17.25, 'E', 'W'), "17,15.0000W");
    }
}
