//! The injected camera and light contracts the capture service drives.
//!
//! Mirrors `negpy/infrastructure/capture/base.py`: a capture owns neither object, so a
//! simulated camera and light exercise the orchestration without hardware.

use std::path::Path;

/// What holds a Blend exposure's shutter open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BulbRelease {
    /// The camera's own bulb control over USB.
    #[default]
    Usb,
    /// A release cable on the Scanlight's 3.5 mm jack.
    Cable,
}

impl BulbRelease {
    /// The saved spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Usb => "usb",
            Self::Cable => "cable",
        }
    }

    /// The release for a saved value; anything else reads as USB, as the Python
    /// `_missing_` does.
    #[must_use]
    pub fn coerce(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "cable" => Self::Cable,
            _ => Self::Usb,
        }
    }
}

/// Everything needed to capture one R/G/B triplet.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureSettings {
    pub roll_name: String,
    pub frame_number: u32,
    pub output_folder: std::path::PathBuf,
    /// R, G and B LED levels, 0-255.
    pub levels: [u32; 3],
    pub settle_s: f64,
    pub inter_exposure_delay_s: f64,
    pub min_raw_bytes: u64,
    pub max_raw_bytes: u64,
    /// Per-channel exposure; `None` uses the operator's fixed camera shutter.
    pub shutters: Option<[Option<String>; 3]>,
    /// ISO and aperture the preset was calibrated at.
    pub iso: Option<String>,
    pub aperture: Option<String>,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            roll_name: String::new(),
            frame_number: 1,
            output_folder: std::path::PathBuf::new(),
            levels: [255, 255, 255],
            settle_s: 0.4,
            inter_exposure_delay_s: 0.0,
            min_raw_bytes: 8 * 1024 * 1024,
            max_raw_bytes: 200 * 1024 * 1024,
            shutters: None,
            iso: None,
            aperture: None,
        }
    }
}

impl CaptureSettings {
    /// The `{roll}_Frame{nnn}` stem of this frame.
    #[must_use]
    pub fn frame_name(&self) -> String {
        format!("{}_Frame{:03}", self.roll_name, self.frame_number)
    }
}

/// The three raw files written for one frame, red being the primary asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureResult {
    pub frame_number: u32,
    pub red_path: std::path::PathBuf,
    pub green_path: std::path::PathBuf,
    pub blue_path: std::path::PathBuf,
}

impl CaptureResult {
    /// The three paths, red first.
    #[must_use]
    pub fn paths(&self) -> [&std::path::Path; 3] {
        [&self.red_path, &self.green_path, &self.blue_path]
    }
}

/// The minimal Scanlight interface the capture service depends on.
pub trait LightSource: Send {
    fn set_color(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32);
    fn off(&mut self);
    fn close(&mut self);

    /// `set_color` that returns once the light switched, for a timed Blend channel.
    fn set_color_now(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32) {
        self.set_color(r, g, b, w, ir);
    }
}

/// The minimal camera interface: download one RAW to `out_path`, return its path.
pub trait Camera: Send {
    fn capture(
        &mut self,
        out_path: &Path,
        shutter: Option<&str>,
        iso: Option<&str>,
        aperture: Option<&str>,
    ) -> Result<std::path::PathBuf, String>;
    fn close(&mut self);
}

/// A light with a shutter-release output: holds the camera's shutter line for `seconds`.
pub trait ShutterRelease: Send {
    fn shutter_pulse(&mut self, seconds: f64);
}

/// A camera that takes one Bulb exposure.
pub trait BulbCamera: Camera {
    /// The shutter opens, `expose` runs, the shutter closes, and the RAW lands beside
    /// `out_path`. With `usb_release` false the camera only sets Bulb and waits for the
    /// file, because `expose` triggers the shutter by cable.
    #[allow(clippy::too_many_arguments)]
    fn capture_bulb(
        &mut self,
        out_path: &Path,
        expose: &mut dyn FnMut(),
        usb_release: bool,
        iso: Option<&str>,
        aperture: Option<&str>,
        keep_bulb: bool,
    ) -> Result<std::path::PathBuf, String>;
}
