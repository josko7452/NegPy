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
