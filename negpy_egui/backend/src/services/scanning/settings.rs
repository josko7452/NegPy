//! Persisted scanner preferences.
//!
//! Mirrors `negpy/infrastructure/scanners/settings.py`: a JSON blob with legacy key
//! migrations that never fail the whole load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services::scanning::params::{MAX_N_PASSES, MIN_N_PASSES, MultiExposureMode, ScanArea};

pub const MONO_TIFF: &str = "TIFF (mono)";
pub const OUTPUT_FORMATS: [&str; 2] = ["TIFF", MONO_TIFF];

/// The platform's default scanner backend.
#[must_use]
pub fn default_backend_id() -> &'static str {
    if std::env::var("NEGPY_SIMULATE_HARDWARE").is_ok_and(|value| value == "1") {
        "sim"
    } else if cfg!(target_os = "windows") {
        "plustek"
    } else {
        "sane"
    }
}

fn default_dpi() -> i64 {
    3600
}

fn default_depth() -> i64 {
    16
}

fn default_true() -> bool {
    true
}

fn default_n_passes() -> i64 {
    MIN_N_PASSES
}

fn default_output_format() -> String {
    "TIFF".to_string()
}

fn default_filename_pattern() -> String {
    "{{ date }}_{{ \"%03d\" % seq }}".to_string()
}

fn default_nkscan_log_level() -> String {
    "off".to_string()
}

fn default_strip_tile_height() -> i64 {
    140
}

/// One saved scanner preference set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScannerSettings {
    #[serde(default)]
    pub last_device_id: String,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_dpi")]
    pub dpi: i64,
    #[serde(default = "default_depth")]
    pub depth: i64,
    #[serde(default)]
    pub capture_ir: bool,
    #[serde(default)]
    pub multi_exposure_mode: MultiExposureMode,
    #[serde(default = "default_n_passes")]
    pub n_passes: i64,
    #[serde(default = "default_true")]
    pub autofocus: bool,
    #[serde(default)]
    pub auto_exposure: bool,
    #[serde(default)]
    pub exposure_time_us: Option<i64>,
    #[serde(default)]
    pub clean: bool,
    #[serde(default = "default_one")]
    pub samples: i64,
    #[serde(default)]
    pub superfine: bool,
    #[serde(default)]
    pub film_format: Option<String>,
    #[serde(default = "default_film_type")]
    pub film_type: String,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    #[serde(default)]
    pub scan_window: Option<ScanArea>,
    #[serde(default)]
    pub frame_offset_mm: f64,
    #[serde(default)]
    pub frame_offset_modifier_mm: f64,
    #[serde(default)]
    pub frame_windows: BTreeMap<i64, ScanArea>,
    #[serde(default)]
    pub selected_frames: Vec<i64>,
    #[serde(default)]
    pub frame_offsets: BTreeMap<i64, f64>,
    #[serde(default = "default_true")]
    pub eject_after_batch: bool,
    #[serde(default = "default_nkscan_log_level")]
    pub nkscan_log_level: String,
    #[serde(default = "default_strip_tile_height")]
    pub strip_tile_height: i64,
    #[serde(default)]
    pub exposure_lock: Option<BTreeMap<String, i64>>,
    #[serde(default)]
    pub exposure_lock_device: String,
    #[serde(default)]
    pub exposure_lock_frame: i64,
    #[serde(default)]
    pub exposure_lock_at: String,
    #[serde(default)]
    pub analog_gain_ev: f64,
    #[serde(default)]
    pub channel_gain_ev: [f64; 3],
}

fn default_backend() -> String {
    default_backend_id().to_string()
}

fn default_film_type() -> String {
    "negative".to_string()
}

const fn default_one() -> i64 {
    1
}

impl Default for ScannerSettings {
    fn default() -> Self {
        serde_json::from_value(Value::Object(serde_json::Map::new()))
            .expect("the scanner defaults deserialize")
    }
}

impl ScannerSettings {
    /// The defaults of a fresh install.
    #[must_use]
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Build from a persisted blob, migrating retired keys and ignoring unknown ones.
    #[must_use]
    pub fn from_dict(value: Value) -> Self {
        let Value::Object(mut data) = value else {
            return Self::default();
        };
        if !data.contains_key("multi_exposure_mode")
            && let Some(flag) = data.remove("multi_exposure")
        {
            let mode = if flag.as_bool().unwrap_or(false) {
                MultiExposureMode::Adaptive
            } else {
                MultiExposureMode::Off
            };
            data.insert(
                "multi_exposure_mode".to_string(),
                Value::String(mode.as_str().to_string()),
            );
        }
        let mode = data
            .get("multi_exposure_mode")
            .and_then(Value::as_str)
            .map(MultiExposureMode::coerce)
            .unwrap_or_default();
        data.insert(
            "multi_exposure_mode".to_string(),
            Value::String(mode.as_str().to_string()),
        );
        if let Some(passes) = data.get("n_passes").and_then(Value::as_i64)
            && !(MIN_N_PASSES..=MAX_N_PASSES).contains(&passes)
        {
            data.insert(
                "n_passes".to_string(),
                Value::from(passes.clamp(MIN_N_PASSES, MAX_N_PASSES)),
            );
        }
        if data
            .get("output_format")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case("DNG"))
        {
            data.insert("output_format".to_string(), Value::from("TIFF"));
        }
        let first = data.remove("frame_from").and_then(|value| value.as_i64());
        let last = data.remove("frame_to").and_then(|value| value.as_i64());
        let has_selection = data
            .get("selected_frames")
            .and_then(Value::as_array)
            .is_some_and(|frames| !frames.is_empty());
        if !has_selection
            && let (Some(first), Some(last)) = (first, last)
            && (first, last) != (1, 1)
        {
            let frames: Vec<Value> = (first..=last).map(Value::from).collect();
            data.insert("selected_frames".to_string(), Value::Array(frames));
        }
        serde_json::from_value(Value::Object(data)).unwrap_or_default()
    }

    /// The frame list the operator's text names, or an error for an unreadable part.
    pub fn parse_frame_spec(text: &str) -> Result<Vec<i64>, String> {
        let mut frames = std::collections::BTreeSet::new();
        for part in text.replace(' ', "").split(',') {
            if part.is_empty() {
                continue;
            }
            let (first, last) = match part.split_once('-') {
                Some((first, last)) => (first, last),
                None => (part, part),
            };
            let lo: i64 = first
                .parse()
                .map_err(|_| format!("Cannot read {part:?} as a frame number"))?;
            let hi: i64 = last
                .parse()
                .map_err(|_| format!("Cannot read {part:?} as a frame number"))?;
            if lo < 1 || hi < lo {
                return Err(format!("{part:?} is not a frame range"));
            }
            frames.extend(lo..=hi);
        }
        Ok(frames.into_iter().collect())
    }

    /// The compact form of a frame selection, contiguous runs collapsed.
    #[must_use]
    pub fn format_frame_spec(frames: &[i64]) -> String {
        let mut ordered: Vec<i64> = frames.to_vec();
        ordered.sort_unstable();
        ordered.dedup();
        let Some(first) = ordered.first().copied() else {
            return String::new();
        };
        let mut runs: Vec<(i64, i64)> = vec![(first, first)];
        for frame in &ordered[1..] {
            let run = runs.last_mut().expect("a run exists");
            if *frame == run.1 + 1 {
                run.1 = *frame;
            } else {
                runs.push((*frame, *frame));
            }
        }
        runs.into_iter()
            .map(|(lo, hi)| {
                if lo == hi {
                    lo.to_string()
                } else {
                    format!("{lo}-{hi}")
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The frames, per-frame windows and base window a batch request reads.
    #[must_use]
    pub fn resolve_batch_selection(
        &self,
        capacity: Option<usize>,
        whole_strip: bool,
    ) -> (Vec<i64>, BTreeMap<i64, ScanArea>, Option<ScanArea>) {
        if !self.selected_frames.is_empty() {
            let mut frames = self.selected_frames.clone();
            frames.sort_unstable();
            let windows = frames
                .iter()
                .filter_map(|frame| {
                    self.frame_windows
                        .get(frame)
                        .map(|window| (*frame, *window))
                })
                .collect();
            return (frames, windows, None);
        }
        if whole_strip {
            return (Vec::new(), BTreeMap::new(), None);
        }
        match capacity {
            Some(capacity) => ((1..=capacity as i64).collect(), BTreeMap::new(), None),
            None => (Vec::new(), BTreeMap::new(), self.scan_window),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_spec_reads_ranges_and_lists() {
        assert_eq!(
            ScannerSettings::parse_frame_spec("1-6").unwrap(),
            vec![1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            ScannerSettings::parse_frame_spec(" 5, 1-3 ").unwrap(),
            vec![1, 2, 3, 5]
        );
        assert!(ScannerSettings::parse_frame_spec("").unwrap().is_empty());
    }

    #[test]
    fn an_unreadable_frame_spec_is_refused() {
        for text in ["x", "2-", "-3", "0", "5-2", "1,,x"] {
            assert!(ScannerSettings::parse_frame_spec(text).is_err(), "{text}");
        }
    }

    #[test]
    fn a_frame_spec_round_trips() {
        for text in ["1-6", "1,3,5", "1-3,7-9"] {
            let frames = ScannerSettings::parse_frame_spec(text).unwrap();
            assert_eq!(ScannerSettings::format_frame_spec(&frames), text);
        }
        assert_eq!(ScannerSettings::format_frame_spec(&[]), "");
        assert_eq!(ScannerSettings::format_frame_spec(&[4, 1, 2]), "1-2,4");
    }

    #[test]
    fn a_legacy_range_becomes_a_selection() {
        let restored = ScannerSettings::from_dict(serde_json::json!({
            "frame_from": 2,
            "frame_to": 4,
            "dpi": 4000
        }));
        assert_eq!(restored.selected_frames, vec![2, 3, 4]);
        assert_eq!(restored.dpi, 4000);
        let unchanged = ScannerSettings::from_dict(serde_json::json!({
            "frame_from": 1,
            "frame_to": 1
        }));
        assert!(unchanged.selected_frames.is_empty());
    }

    #[test]
    fn a_retired_key_keeps_the_rest_of_the_blob() {
        let restored = ScannerSettings::from_dict(serde_json::json!({
            "gone_in_this_version": true,
            "output_folder": "/scans",
            "output_format": MONO_TIFF
        }));
        assert_eq!(restored.output_format, MONO_TIFF);
    }

    #[test]
    fn legacy_multi_exposure_migrates_to_a_mode() {
        let on = ScannerSettings::from_dict(serde_json::json!({"multi_exposure": true}));
        assert_eq!(on.multi_exposure_mode, MultiExposureMode::Adaptive);
        let off = ScannerSettings::from_dict(serde_json::json!({"multi_exposure": false}));
        assert_eq!(off.multi_exposure_mode, MultiExposureMode::Off);
    }

    #[test]
    fn a_batch_selection_wins_and_a_strip_needs_nothing() {
        let settings = ScannerSettings {
            selected_frames: vec![4, 1, 2],
            scan_window: Some([0.2, 0.2, 0.8, 0.8]),
            frame_windows: BTreeMap::from([(4, [0.1, 0.1, 0.5, 0.5])]),
            ..ScannerSettings::default()
        };
        let (frames, windows, base) = settings.resolve_batch_selection(Some(6), false);
        assert_eq!(frames, vec![1, 2, 4]);
        assert_eq!(windows, BTreeMap::from([(4, [0.1, 0.1, 0.5, 0.5])]));
        assert_eq!(base, None);
        let (frames, _, base) = ScannerSettings::default().resolve_batch_selection(Some(4), false);
        assert_eq!(frames, vec![1, 2, 3, 4]);
        assert_eq!(base, None);
        let (frames, _, base) = ScannerSettings::default().resolve_batch_selection(None, true);
        assert!(frames.is_empty());
        assert_eq!(base, None);
    }
}
