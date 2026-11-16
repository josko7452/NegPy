//! Persisted Scanlight-capture panel settings.
//!
//! Mirrors `negpy/infrastructure/capture/settings.py`. The settings live under one global
//! setting, so a preset measured on one film stock is recalled by name.

use serde::{Deserialize, Serialize};

use crate::services::capture::hardware::BulbRelease;

/// What a white-light capture is: a slide, a B&W or color negative, or left to autodetect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhiteCaptureMode {
    #[default]
    Auto,
    Bw,
    C41,
    E6,
}

impl WhiteCaptureMode {
    /// The value a saved dict carries.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Bw => "bw",
            Self::C41 => "c41",
            Self::E6 => "e6",
        }
    }

    /// The mode for a saved value, with the pre-rename names and anything stale reading
    /// as Auto.
    #[must_use]
    pub fn coerce(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "e-6" | "e6" => Self::E6,
            "b&w" | "bw" => Self::Bw,
            "c41" => Self::C41,
            _ => Self::Auto,
        }
    }
}

/// Sticky settings for the Scanlight capture sidebar.
///
/// An empty `port` auto-discovers the serial port; the camera needs no address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScanlightSettings {
    #[serde(default = "full_level")]
    pub r_level: u32,
    #[serde(default = "full_level")]
    pub g_level: u32,
    #[serde(default = "full_level")]
    pub b_level: u32,
    #[serde(default)]
    pub shutter_r: String,
    #[serde(default)]
    pub shutter_g: String,
    #[serde(default)]
    pub shutter_b: String,
    #[serde(default)]
    pub white_mode: bool,
    #[serde(default)]
    pub w_level: u32,
    #[serde(default)]
    pub shutter_w: String,
    /// The infrared exposure's own shutter, set by hand; empty uses the body's current.
    #[serde(default)]
    pub shutter_ir: String,
    /// End every frame with an infrared exposure at `shutter_ir`.
    #[serde(default)]
    pub infrared: bool,
    /// The RGB preset's calibrated ISO and aperture, forced on the body at scan time.
    #[serde(default)]
    pub iso: String,
    /// Empty for a manual-aperture lens, set by hand on the ring.
    #[serde(default)]
    pub aperture: String,
    /// The RGB preset lights R, G and B together for one exposure.
    #[serde(default)]
    pub single_capture: bool,
    /// The preset lights R, G and B in turn inside one Bulb exposure.
    #[serde(default)]
    pub blend_capture: bool,
    #[serde(default)]
    pub bulb_release: BulbRelease,
    /// The sensor crosstalk profile a Blend frame's roll takes; Blend needs one.
    #[serde(default)]
    pub blend_sensor_profile: String,
    #[serde(default)]
    pub inter_exposure_delay_ms: u32,
    #[serde(default)]
    pub white_process_mode: WhiteCaptureMode,
    /// The Scanlight serial port; empty auto-discovers it.
    #[serde(default)]
    pub port: String,
    /// Re-light the framing light after each scan without Live View.
    #[serde(default)]
    pub keep_light_on: bool,
    /// A light that moves film: `off`, `next` (one frame per Scan) or `strip` (the whole strip).
    #[serde(default = "off_transport")]
    pub film_transport: String,
    /// After a whole strip ejects, shoot the empty dock for the roll's Flat Field profile.
    #[serde(default)]
    pub strip_flat: bool,
    /// Seconds Scan and Retake hold the first exposure.
    #[serde(default)]
    pub capture_delay_s: f64,
    /// Built-in presets added to the dropdown.
    #[serde(default)]
    pub adopted_presets: Vec<String>,
}

fn full_level() -> u32 {
    255
}

fn off_transport() -> String {
    "off".to_string()
}

impl Default for ScanlightSettings {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the Scanlight defaults deserialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_match_the_panel() {
        let settings = ScanlightSettings::default();
        assert_eq!(
            (settings.r_level, settings.g_level, settings.b_level),
            (255, 255, 255)
        );
        assert_eq!(settings.w_level, 0);
        assert!(!settings.white_mode && !settings.infrared);
        assert_eq!(settings.film_transport, "off");
        assert_eq!(settings.bulb_release, BulbRelease::Usb);
        assert!(settings.adopted_presets.is_empty());
    }

    #[test]
    fn the_pre_rename_white_modes_read_as_their_mode() {
        assert_eq!(WhiteCaptureMode::coerce("e-6"), WhiteCaptureMode::E6);
        assert_eq!(WhiteCaptureMode::coerce("B&W"), WhiteCaptureMode::Bw);
        assert_eq!(WhiteCaptureMode::coerce("c41"), WhiteCaptureMode::C41);
        assert_eq!(WhiteCaptureMode::coerce("stale"), WhiteCaptureMode::Auto);
    }

    #[test]
    fn a_saved_release_coerces_to_usb() {
        assert_eq!(BulbRelease::coerce("cable"), BulbRelease::Cable);
        assert_eq!(BulbRelease::coerce("CABLE"), BulbRelease::Cable);
        assert_eq!(BulbRelease::coerce("stale"), BulbRelease::Usb);
    }

    #[test]
    fn a_saved_dict_round_trips() {
        let mut settings = ScanlightSettings::default();
        settings.blend_capture = true;
        settings.bulb_release = BulbRelease::Cable;
        settings.white_process_mode = WhiteCaptureMode::E6;
        settings.adopted_presets = vec!["Portra 400".to_string()];
        let value = serde_json::to_value(&settings).unwrap();
        let back: ScanlightSettings = serde_json::from_value(value).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn a_missing_key_takes_its_default() {
        let settings: ScanlightSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, ScanlightSettings::default());
    }
}
