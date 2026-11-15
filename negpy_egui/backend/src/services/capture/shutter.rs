//! Shutter-label parsing, true exposure, display and ladder helpers.
//!
//! Mirrors the pure helpers in `negpy/services/capture/calibration.py`.

/// The built-in ladder, fastest first (third stops).
pub const SHUTTER_CANDIDATES: [&str; 45] = [
    "1/8000", "1/6400", "1/5000", "1/4000", "1/3200", "1/2500", "1/2000", "1/1600", "1/1250",
    "1/1000", "1/800", "1/640", "1/500", "1/400", "1/320", "1/250", "1/200", "1/160", "1/125",
    "1/100", "1/80", "1/60", "1/50", "1/40", "1/30", "1/25", "1/20", "1/15", "1/13", "1/10", "1/8",
    "1/6", "1/5", "1/4", "1/3", "0.4", "1/2", "0.6", "0.8", "1", "1.3", "1.6", "2", "2.5", "3.2",
];

const THIRD_STOP_SPEEDS: [f64; 49] = [
    1.0 / 8000.0,
    1.0 / 6400.0,
    1.0 / 5000.0,
    1.0 / 4000.0,
    1.0 / 3200.0,
    1.0 / 2500.0,
    1.0 / 2000.0,
    1.0 / 1600.0,
    1.0 / 1250.0,
    1.0 / 1000.0,
    1.0 / 800.0,
    1.0 / 640.0,
    1.0 / 500.0,
    1.0 / 400.0,
    1.0 / 320.0,
    1.0 / 250.0,
    1.0 / 200.0,
    1.0 / 160.0,
    1.0 / 125.0,
    1.0 / 100.0,
    1.0 / 80.0,
    1.0 / 60.0,
    1.0 / 50.0,
    1.0 / 40.0,
    1.0 / 30.0,
    1.0 / 25.0,
    1.0 / 20.0,
    1.0 / 15.0,
    1.0 / 13.0,
    1.0 / 10.0,
    1.0 / 8.0,
    1.0 / 6.0,
    1.0 / 5.0,
    1.0 / 4.0,
    0.3,
    0.4,
    0.5,
    0.6,
    0.8,
    1.0,
    1.3,
    1.6,
    2.0,
    2.5,
    3.2,
    4.0,
    5.0,
    6.0,
    8.0,
];

const HALF_STOP_SPEEDS: [f64; 37] = [
    1.0 / 8000.0,
    1.0 / 6000.0,
    1.0 / 4000.0,
    1.0 / 3000.0,
    1.0 / 2000.0,
    1.0 / 1500.0,
    1.0 / 1000.0,
    1.0 / 750.0,
    1.0 / 500.0,
    1.0 / 350.0,
    1.0 / 250.0,
    1.0 / 180.0,
    1.0 / 125.0,
    1.0 / 90.0,
    1.0 / 60.0,
    1.0 / 45.0,
    1.0 / 30.0,
    1.0 / 20.0,
    1.0 / 15.0,
    1.0 / 10.0,
    1.0 / 8.0,
    1.0 / 6.0,
    1.0 / 4.0,
    0.3,
    0.5,
    0.7,
    1.0,
    1.5,
    2.0,
    3.0,
    4.0,
    6.0,
    8.0,
    10.0,
    15.0,
    20.0,
    30.0,
];

const DECIMAL_SLACK_S: f64 = 0.00005;

/// Parse a shutter label into nominal seconds.
pub fn shutter_seconds(label: &str) -> Result<f64, String> {
    let mut label = label.trim();
    for suffix in ["seconds", "second", "secs", "sec", "s"] {
        if label.to_lowercase().ends_with(suffix) {
            label = label[..label.len() - suffix.len()].trim();
            break;
        }
    }
    if let Some((num, den)) = label.split_once('/') {
        let denominator: f64 = den
            .trim()
            .parse()
            .map_err(|_| format!("invalid shutter label {label:?}"))?;
        if denominator == 0.0 {
            return Err(format!("invalid shutter label {label:?}: zero denominator"));
        }
        let numerator: f64 = num
            .trim()
            .parse()
            .map_err(|_| format!("invalid shutter label {label:?}"))?;
        return Ok(numerator / denominator);
    }
    label
        .parse()
        .map_err(|_| format!("invalid shutter label {label:?}"))
}

/// The parseable labels, slowest last, duplicates removed.
#[must_use]
pub fn usable_ladder(candidates: &[String]) -> Vec<String> {
    let mut parsed: Vec<(String, f64)> = candidates
        .iter()
        .filter_map(|label| {
            shutter_seconds(label)
                .ok()
                .map(|seconds| (label.clone(), seconds))
        })
        .filter(|(_, seconds)| *seconds > 0.0)
        .collect();
    parsed.sort_by(|left, right| left.1.total_cmp(&right.1));
    parsed
        .into_iter()
        .map(|(label, _)| label)
        .fold(Vec::new(), |mut out, label| {
            if !out.contains(&label) {
                out.push(label);
            }
            out
        })
}

fn ladder_stops(candidates: &[String]) -> f64 {
    let seconds: Vec<f64> = usable_ladder(candidates)
        .iter()
        .filter_map(|label| shutter_seconds(label).ok())
        .collect();
    let mut ratios: Vec<f64> = seconds
        .windows(2)
        .map(|pair| pair[1] / pair[0])
        .filter(|ratio| *ratio > 0.0)
        .collect();
    if ratios.len() < 3 {
        return 1.0 / 3.0;
    }
    ratios.sort_by(f64::total_cmp);
    let step = ratios[ratios.len() / 2].log2();
    if (step - 1.0 / 3.0).abs() < (step - 1.0 / 2.0).abs() {
        1.0 / 3.0
    } else {
        1.0 / 2.0
    }
}

/// The true exposure time of a label, undoing display rounding within 8%.
#[must_use]
pub fn true_seconds(label: &str, candidates: &[String]) -> f64 {
    let Ok(nominal) = shutter_seconds(label) else {
        return 0.0;
    };
    if nominal <= 0.0 {
        return nominal;
    }
    let stops = ladder_stops(candidates);
    let exact = 2.0f64.powf((nominal.log2() / stops).round() * stops);
    if (exact / nominal - 1.0).abs() <= 0.08 {
        exact
    } else {
        nominal
    }
}

/// A shutter label as a camera shows it.
#[must_use]
pub fn shutter_display(label: &str, ladder: &[String]) -> String {
    if label
        .trim()
        .strip_prefix("1/")
        .is_some_and(|den| den.chars().all(|ch| ch.is_ascii_digit()))
    {
        return label.trim().to_string();
    }
    let Ok(seconds) = shutter_seconds(label) else {
        return label.to_string();
    };
    if seconds <= 0.0 {
        return label.to_string();
    }
    let speed_sets: [&[f64]; 2] = [&THIRD_STOP_SPEEDS, &HALF_STOP_SPEEDS];
    let sets: &[&[f64]] = if ladder_is_half_stop(ladder) {
        &speed_sets[1..]
    } else if ladder_is_third_stop(ladder) {
        &speed_sets[..1]
    } else {
        &speed_sets
    };
    let mut seconds = seconds;
    for speeds in sets {
        let marked = speeds
            .iter()
            .copied()
            .min_by(|left, right| (left - seconds).abs().total_cmp(&(right - seconds).abs()))
            .unwrap_or(seconds);
        if (marked - seconds).abs() <= DECIMAL_SLACK_S + 0.02 * marked {
            seconds = marked;
            break;
        }
    }
    if seconds < 0.29 {
        format!("1/{}", (1.0 / seconds).round())
    } else {
        format!("{}″", (seconds * 10.0).round() / 10.0)
    }
}

fn ladder_is_third_stop(ladder: &[String]) -> bool {
    (ladder_stops(ladder) - 1.0 / 3.0).abs() < 0.01
}

fn ladder_is_half_stop(ladder: &[String]) -> bool {
    (ladder_stops(ladder) - 1.0 / 2.0).abs() < 0.01
}

/// A body's ladder limited to the built-in span.
#[must_use]
pub fn scan_ladder(labels: &[String]) -> Vec<String> {
    let floor = shutter_seconds(SHUTTER_CANDIDATES[0]).unwrap_or(0.0);
    let ceiling = shutter_seconds(SHUTTER_CANDIDATES[SHUTTER_CANDIDATES.len() - 1]).unwrap_or(0.0);
    usable_ladder(labels)
        .into_iter()
        .filter(|label| {
            shutter_seconds(label).is_ok_and(|seconds| (floor..=ceiling).contains(&seconds))
        })
        .collect()
}

/// The fastest candidate whose true exposure is at least `seconds`.
#[must_use]
pub fn shutter_at_least(seconds: f64, candidates: &[String]) -> String {
    let ladder: Vec<String> = if candidates.is_empty() {
        SHUTTER_CANDIDATES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        candidates.to_vec()
    };
    for candidate in &ladder {
        if true_seconds(candidate, &ladder) >= seconds {
            return candidate.clone();
        }
    }
    ladder.last().cloned().unwrap_or_default()
}

/// The candidate closest in seconds to `label`.
#[must_use]
pub fn nearest_shutter(label: &str, candidates: &[String]) -> String {
    if candidates.is_empty() || candidates.iter().any(|candidate| candidate == label) {
        return label.to_string();
    }
    let Ok(target) = shutter_seconds(label) else {
        return label.to_string();
    };
    candidates
        .iter()
        .min_by(|left, right| {
            let left = shutter_seconds(left).unwrap_or(f64::INFINITY);
            let right = shutter_seconds(right).unwrap_or(f64::INFINITY);
            (left - target).abs().total_cmp(&(right - target).abs())
        })
        .cloned()
        .unwrap_or_else(|| label.to_string())
}

/// Parse an aperture label into an f-number.
#[must_use]
pub fn aperture_fnumber(label: &str) -> Option<f64> {
    let digits: String = label
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    digits.parse().ok().filter(|value| *value > 0.0)
}

/// One proportional LED trim toward target.
#[must_use]
pub fn correct_led_level(current_level: i64, signal: f64, target: i64) -> i64 {
    const PWM_MIN: i64 = 40;
    const PWM_MAX: i64 = 255;
    const MIN_SIGNAL: f64 = 10.0;
    if signal < MIN_SIGNAL {
        return PWM_MAX;
    }
    let corrected = (current_level as f64 * (target as f64 / signal)).round() as i64;
    corrected.clamp(PWM_MIN, PWM_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin() -> Vec<String> {
        SHUTTER_CANDIDATES
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    #[test]
    fn a_label_with_its_unit_parses() {
        assert!((shutter_seconds("0.4000s").unwrap() - 0.4).abs() < 1e-12);
        assert!((shutter_seconds("2 sec").unwrap() - 2.0).abs() < 1e-12);
        assert!((shutter_seconds("1/100").unwrap() - 0.01).abs() < 1e-12);
        assert!(shutter_seconds("1/0").is_err());
    }

    #[test]
    fn unusable_labels_leave_the_ladder() {
        let raw = ["1/250", "1/0", "Bulb", "", "1/60", "1/4", "2", "30"].map(str::to_string);
        assert_eq!(usable_ladder(&raw), ["1/250", "1/60", "1/4", "2", "30"]);
    }

    #[test]
    fn true_seconds_undoes_third_stop_rounding() {
        let thirds = [
            "1", "8/10", "6/10", "5/10", "4/10", "1/3", "1/4", "1/5", "1/6", "1/8",
        ]
        .map(str::to_string);
        assert!((true_seconds("1/3", &thirds) - 2.0f64.powf(-5.0 / 3.0)).abs() < 0.01);
        assert!((true_seconds("1/6", &thirds) - 2.0f64.powf(-8.0 / 3.0)).abs() < 0.01);
        assert!((shutter_seconds("1/3").unwrap() - 1.0 / 3.0).abs() < 1e-12);
        assert!((true_seconds("0.7", &thirds) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn display_keeps_published_fractions_and_marks_long_times() {
        assert_eq!(shutter_display("1/30", &builtin()), "1/30");
        assert_eq!(shutter_display("0.4", &builtin()), "0.4″");
        assert_eq!(shutter_display("2", &builtin()), "2″");
    }

    #[test]
    fn the_shared_shutter_is_as_fast_as_the_dim_channel_allows() {
        let ladder = builtin();
        assert_eq!(shutter_at_least(0.01, &ladder), "1/80");
        assert_eq!(shutter_at_least(100.0, &ladder), "3.2");
    }

    #[test]
    fn leds_trim_proportionally_and_clamp() {
        assert_eq!(correct_led_level(100, 500.0, 1000), 200);
        assert_eq!(correct_led_level(100, 0.0, 1000), 255);
        assert_eq!(correct_led_level(1, 500.0, 1000), 40);
    }
}
