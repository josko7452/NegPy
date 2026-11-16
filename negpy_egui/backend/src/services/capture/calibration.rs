//! Shutter labels and the capture start point for the ETTR calibration.
//!
//! Mirrors the pure label half of `negpy/services/capture/calibration.py`. The
//! camera, light and demosaic loops stay with the capture service.

pub const CLIP_CEILING: u32 = 65535;
pub const SATURATION_VALUE: u32 = 65404;
pub const PWM_MIN: u32 = 40;
pub const PWM_MAX: u32 = 255;
pub const PWM_MIN_SINGLE: u32 = 10;
pub const PWM_MAX_SAFE: u32 = 250;
pub const TARGET_FRACTION: f64 = 0.9;
pub const MIN_SIGNAL: f64 = 10.0;
pub const MAX_CLIP_FRACTION: f64 = 0.002;
pub const MAX_LINEARITY_FRACTION: f64 = 0.02;
pub const MAX_TARGET_UNDER_FRACTION: f64 = 0.2;
pub const REFERENCE_ISO: f64 = 100.0;
pub const REFERENCE_APERTURE: f64 = 8.0;
pub const REFERENCE_SHUTTER: &str = "0.4";
pub const REFERENCE_LEVELS: [u32; 3] = [210, 95, 80];

pub const SHUTTER_CANDIDATES: [&str; 28] = [
    "1/250", "1/200", "1/160", "1/125", "1/100", "1/80", "1/60", "1/50", "1/40", "1/30", "1/25",
    "1/20", "1/15", "1/13", "1/10", "1/8", "1/6", "1/5", "1/4", "1/3", "0.4", "1/2", "0.6", "0.8",
    "1", "1.3", "1.6", "2",
];

const THIRD_DIVISORS: [u32; 33] = [
    8000, 6400, 5000, 4000, 3200, 2500, 2000, 1600, 1250, 1000, 800, 640, 500, 400, 320, 250, 200,
    160, 125, 100, 80, 60, 50, 40, 30, 25, 20, 15, 13, 10, 8, 6, 5,
];
const THIRD_DIVISORS_2: [u32; 4] = [4, 3, 2, 1];
const THIRD_DECIMALS: [f64; 21] = [
    0.3, 0.4, 0.5, 0.6, 0.8, 1.0, 1.3, 1.6, 2.0, 2.5, 3.2, 4.0, 5.0, 6.0, 8.0, 10.0, 13.0, 15.0,
    20.0, 25.0, 30.0,
];
const HALF_DIVISORS: [u32; 22] = [
    8000, 6000, 4000, 3000, 2000, 1500, 1000, 750, 500, 350, 250, 180, 125, 90, 60, 45, 30, 20, 15,
    10, 8, 6,
];
const HALF_DIVISORS_2: [u32; 2] = [4, 1];
const HALF_DECIMALS: [f64; 12] = [0.3, 0.5, 0.7, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 15.0];
const DECIMAL_SLACK_S: f64 = 0.00005;

/// Parse a shutter label into its nominal seconds.
///
/// The unit suffix a body may attach is ignored. A zero denominator is an error, so
/// a bulb-like `1/0` gets dropped by the ladder cleaner.
pub fn shutter_seconds(label: &str) -> Result<f64, String> {
    let label = label.trim();
    let label = strip_unit(label);
    if let Some((numerator, denominator)) = label.split_once('/') {
        let denominator: f64 = denominator
            .trim()
            .parse()
            .map_err(|_| format!("invalid shutter label {label:?}"))?;
        if denominator == 0.0 {
            return Err(format!("invalid shutter label {label:?}: zero denominator"));
        }
        let numerator: f64 = numerator
            .trim()
            .parse()
            .map_err(|_| format!("invalid shutter label {label:?}"))?;
        return Ok(numerator / denominator);
    }
    label
        .parse()
        .map_err(|_| format!("invalid shutter label {label:?}"))
}

fn strip_unit(label: &str) -> &str {
    let lower = label.to_ascii_lowercase();
    for suffix in ["seconds", "second", "secs", "sec", "s"] {
        if lower.ends_with(suffix) {
            let candidate = label[..label.len() - suffix.len()].trim_end();
            if candidate
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '/' || c == ' ')
            {
                return candidate.trim_end();
            }
        }
    }
    label
}

/// The parseable, ascending subset of a body's labels.
pub fn usable_ladder(candidates: &[&str]) -> Vec<String> {
    let mut parsed: Vec<(String, f64)> = Vec::new();
    for label in candidates {
        let Ok(seconds) = shutter_seconds(label) else {
            continue;
        };
        if seconds <= 0.0 || parsed.iter().any(|(known, _)| known == label) {
            continue;
        }
        parsed.push(((*label).to_string(), seconds));
    }
    parsed.sort_by(|a, b| a.1.total_cmp(&b.1));
    parsed.into_iter().map(|(label, _)| label).collect()
}

fn marked_speeds(ladder: &[&str]) -> Vec<Vec<f64>> {
    let rungs: Vec<f64> = usable_ladder(ladder)
        .iter()
        .filter_map(|label| shutter_seconds(label).ok())
        .collect();
    if rungs.len() >= 3 {
        let mut ratios: Vec<f64> = rungs
            .windows(2)
            .map(|pair| (pair[1] / pair[0]).log2())
            .collect();
        ratios.sort_by(f64::total_cmp);
        let step = ratios[ratios.len() / 2];
        if (step - 1.0 / 3.0).abs() < 0.08 {
            return vec![third_stop_speeds()];
        }
        if (step - 0.5).abs() < 0.08 {
            return vec![half_stop_speeds()];
        }
    }
    vec![third_stop_speeds(), half_stop_speeds()]
}

fn third_stop_speeds() -> Vec<f64> {
    THIRD_DIVISORS
        .iter()
        .chain(THIRD_DIVISORS_2.iter())
        .map(|&d| 1.0 / d as f64)
        .chain(THIRD_DECIMALS)
        .collect()
}

fn half_stop_speeds() -> Vec<f64> {
    HALF_DIVISORS
        .iter()
        .chain(HALF_DIVISORS_2.iter())
        .map(|&d| 1.0 / d as f64)
        .chain(HALF_DECIMALS)
        .collect()
}

/// A label as the camera displays it.
#[must_use]
pub fn shutter_display(label: &str, ladder: &[&str]) -> String {
    display(label, &marked_speeds(ladder))
}

/// The display of every label in a body's ladder.
#[must_use]
pub fn shutter_displays(ladder: &[&str]) -> Vec<String> {
    let speeds = marked_speeds(ladder);
    ladder.iter().map(|label| display(label, &speeds)).collect()
}

fn display(label: &str, marked: &[Vec<f64>]) -> String {
    let trimmed = label.trim();
    if is_numeric_fraction(trimmed) {
        return trimmed.to_string();
    }
    let Ok(mut seconds) = shutter_seconds(label) else {
        return label.to_string();
    };
    if seconds <= 0.0 {
        return label.to_string();
    }
    for speeds in marked {
        let Some(best) = speeds
            .iter()
            .copied()
            .min_by(|a, b| (a - seconds).abs().total_cmp(&(b - seconds).abs()))
        else {
            continue;
        };
        if (best - seconds).abs() <= DECIMAL_SLACK_S + 0.02 * best {
            seconds = best;
            break;
        }
    }
    if seconds < 0.29 {
        format!("1/{}", (1.0 / seconds).round() as i64)
    } else {
        format!("{}\"", format_g(seconds.round_to_one()))
    }
}

fn is_numeric_fraction(label: &str) -> bool {
    let Some((num, den)) = label.split_once('/') else {
        return false;
    };
    num == "1" && !den.is_empty() && den.chars().all(|c| c.is_ascii_digit())
}

fn format_g(value: f64) -> String {
    let text = format!("{value:.1}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

trait RoundOne {
    fn round_to_one(self) -> f64;
}

impl RoundOne for f64 {
    fn round_to_one(self) -> f64 {
        (self * 10.0).round() / 10.0
    }
}

/// The ladder's spacing in stops, read from the body's own labels.
#[must_use]
pub fn ladder_stops(candidates: &[&str]) -> f64 {
    let secs: Vec<f64> = usable_ladder(candidates)
        .iter()
        .filter_map(|label| shutter_seconds(label).ok())
        .collect();
    let mut ratios: Vec<f64> = secs
        .windows(2)
        .filter(|pair| pair[1] > pair[0])
        .map(|pair| pair[1] / pair[0])
        .collect();
    if ratios.len() < 3 {
        return 1.0 / 3.0;
    }
    ratios.sort_by(f64::total_cmp);
    let step = ratios[ratios.len() / 2].log2();
    [1.0 / 3.0, 0.5]
        .into_iter()
        .min_by(|a, b| (a - step).abs().total_cmp(&(b - step).abs()))
        .unwrap_or(1.0 / 3.0)
}

/// The label's true exposure time, undoing display rounding.
#[must_use]
pub fn true_seconds(label: &str, candidates: &[&str]) -> f64 {
    let Ok(nominal) = shutter_seconds(label) else {
        return 0.0;
    };
    if nominal <= 0.0 {
        return nominal;
    }
    let stops = ladder_stops(if candidates.is_empty() {
        &SHUTTER_CANDIDATES
    } else {
        candidates
    });
    let exact = (nominal.log2() / stops).round() * stops;
    let exact = exact.exp2();
    if (exact / nominal - 1.0).abs() <= 0.08 {
        exact
    } else {
        nominal
    }
}

/// An aperture label as an f-number; `None` for a manual lens.
#[must_use]
pub fn aperture_fnumber(label: &str) -> Option<f64> {
    if label.is_empty() {
        return None;
    }
    let start = label.find(|c: char| c.is_ascii_digit())?;
    let number: String = label[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let f = number.parse::<f64>().ok()?;
    (f > 0.0).then_some(f)
}

/// Scale the reference start point to the live ISO and aperture.
#[must_use]
pub fn normalize_start_point(iso: &str, aperture: &str, candidates: &[&str]) -> ([u32; 3], String) {
    normalize_start_point_with(
        iso,
        aperture,
        REFERENCE_LEVELS,
        REFERENCE_SHUTTER,
        candidates,
    )
}

/// `normalize_start_point` with explicit reference values.
#[must_use]
pub fn normalize_start_point_with(
    iso: &str,
    aperture: &str,
    levels: [u32; 3],
    shutter: &str,
    candidates: &[&str],
) -> ([u32; 3], String) {
    let mut t = shutter_seconds(shutter).unwrap_or(0.4);
    let iso_now = iso
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse::<f64>()
        .unwrap_or(REFERENCE_ISO);
    if iso_now > 0.0 {
        t *= REFERENCE_ISO / iso_now;
    }
    if let Some(f_now) = aperture_fnumber(aperture) {
        t *= (f_now / REFERENCE_APERTURE).powi(2);
    }
    let cleaned = usable_ladder(candidates);
    let ladder: Vec<&str> = if cleaned.is_empty() {
        SHUTTER_CANDIDATES.to_vec()
    } else {
        cleaned.iter().map(String::as_str).collect()
    };
    (levels, nearest_by_seconds(t, &ladder))
}

/// The candidate closest in seconds to a raw value.
#[must_use]
pub fn nearest_by_seconds(seconds: f64, candidates: &[&str]) -> String {
    candidates
        .iter()
        .min_by(|a, b| {
            let a = shutter_seconds(a).unwrap_or(f64::INFINITY);
            let b = shutter_seconds(b).unwrap_or(f64::INFINITY);
            (a - seconds).abs().total_cmp(&(b - seconds).abs())
        })
        .map_or_else(
            || REFERENCE_SHUTTER.to_string(),
            |label| (*label).to_string(),
        )
}

/// The candidate closest to `label`, snapped onto the given ladder.
#[must_use]
pub fn nearest_shutter(label: &str, candidates: &[&str]) -> String {
    if candidates.is_empty() || candidates.contains(&label) {
        return label.to_string();
    }
    let Ok(target) = shutter_seconds(label) else {
        return label.to_string();
    };
    nearest_by_seconds(target, candidates)
}

/// A body's labels inside the built-in span, fastest first.
#[must_use]
pub fn scan_ladder(labels: &[String]) -> Vec<String> {
    let raw: Vec<&str> = labels.iter().map(|s| s.trim()).collect();
    let floor = shutter_seconds(SHUTTER_CANDIDATES[0]).unwrap_or(0.0);
    let ceiling = shutter_seconds(SHUTTER_CANDIDATES[SHUTTER_CANDIDATES.len() - 1]).unwrap_or(0.0);
    usable_ladder(&raw)
        .into_iter()
        .filter(|label| {
            shutter_seconds(label)
                .map(|seconds| floor <= seconds && seconds <= ceiling)
                .unwrap_or(false)
        })
        .collect()
}

/// The fastest candidate whose TRUE exposure is at least `seconds`.
#[must_use]
pub fn shutter_at_least(seconds: f64, candidates: &[String]) -> String {
    let borrowed: Vec<&str> = candidates.iter().map(String::as_str).collect();
    for label in &borrowed {
        if true_seconds(label, &borrowed) >= seconds {
            return (*label).to_string();
        }
    }
    candidates
        .last()
        .cloned()
        .unwrap_or_else(|| REFERENCE_SHUTTER.to_string())
}
