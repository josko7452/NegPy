//! The Blend exposure plan and the validated single capture.
//!
//! Mirrors the pure part of `negpy/services/capture/service.py`: each channel keeps the
//! light of its Trichrome exposure, so one factor can dim every level without breaking
//! the balance.

use std::path::{Path, PathBuf};

use super::calibration::{PWM_MAX, PWM_MIN};

/// The shortest channel the host can time; a shorter one takes a few percent of error.
pub const BLEND_MIN_CHANNEL_S: f64 = 0.2;

/// Capture order: red first, blue last, matching TriRGB and NegPy RGB-scan.
pub const CAPTURE_ORDER: [Channel; 3] = [Channel::Red, Channel::Green, Channel::Blue];

/// One narrowband exposure of a trichromatic capture, or the infrared exposure beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Ir,
}

impl Channel {
    /// The file suffix a channel's exposure carries.
    #[must_use]
    pub fn letter(self) -> &'static str {
        match self {
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Ir => "IR",
        }
    }

    /// The `(r, g, b)` Scanlight tuple that lights only this channel. R, G and B only.
    #[must_use]
    pub fn rgb(self, level: u32) -> Option<[u32; 3]> {
        match self {
            Self::Red => Some([level, 0, 0]),
            Self::Green => Some([0, level, 0]),
            Self::Blue => Some([0, 0, level]),
            Self::Ir => None,
        }
    }
}

/// One channel of a Blend exposure: its LED level and how long it stays lit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendStep {
    pub channel: Channel,
    pub level: u32,
    pub seconds: f64,
}

/// The error a plan cannot be built.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureError(pub String);

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CaptureError {}

/// Each channel's LED level and time for one Blend exposure.
///
/// `levels` are the R, G and B primaries of a valid mono scan. The factor stays 1 unless
/// a channel would be too short to time, or the channels together outlast the release
/// cable's limit.
pub fn blend_plan(
    levels: [u32; 3],
    seconds: f64,
    max_light_s: Option<f64>,
) -> Result<Vec<BlendStep>, CaptureError> {
    let lit: Vec<(Channel, u32)> = CAPTURE_ORDER
        .into_iter()
        .zip(levels)
        .filter(|(_, level)| *level > 0)
        .collect();
    if lit.is_empty() || seconds <= 0.0 {
        return Err(CaptureError(
            "a Blend exposure needs a shutter time and at least one lit channel".to_string(),
        ));
    }
    let low = lit.iter().map(|(_, level)| *level).min().unwrap_or(1);
    let high = lit.iter().map(|(_, level)| *level).max().unwrap_or(1);
    let mut floor = f64::from(PWM_MIN.min(low)) / f64::from(low);
    if let Some(limit) = max_light_s {
        floor = floor.max(lit.len() as f64 * seconds / limit);
    }
    let ceiling = f64::from(PWM_MAX) / f64::from(high);
    if floor > ceiling {
        let needed = lit.len() as f64 * seconds * f64::from(high) / f64::from(PWM_MAX);
        return Err(CaptureError(match max_light_s {
            Some(limit) => format!(
                "the Blend exposure needs {needed:.1} s of light, more than the release cable holds ({limit:.1} s); open the aperture or raise the ISO, then calibrate again"
            ),
            None => format!(
                "the Blend exposure needs {needed:.1} s of light, more than the cable holds"
            ),
        }));
    }
    let factor = (seconds / BLEND_MIN_CHANNEL_S)
        .min(1.0)
        .max(floor)
        .min(ceiling);
    let mut steps = Vec::with_capacity(lit.len());
    for (channel, level) in lit {
        let scaled = if max_light_s.is_some() {
            (f64::from(level) * factor - 1e-9).ceil()
        } else {
            (f64::from(level) * factor).round()
        };
        let scaled = scaled.max(1.0).min(f64::from(PWM_MAX)) as u32;
        steps.push(BlendStep {
            channel,
            level: scaled,
            seconds: seconds * f64::from(level) / f64::from(scaled),
        });
    }
    if let Some(limit) = max_light_s {
        let total: f64 = steps.iter().map(|step| step.seconds).sum();
        if total > limit + 1e-9 {
            return Err(CaptureError(format!(
                "the Blend exposure is longer than the release cable holds ({limit:.1} s)"
            )));
        }
    }
    Ok(steps)
}

/// Raise when a captured RAW is missing or implausibly sized.
pub fn verify_raw_size(path: &Path, min_bytes: u64, max_bytes: u64) -> Result<u64, CaptureError> {
    let size = std::fs::metadata(path)
        .map_err(|_| CaptureError(format!("captured file is missing: {}", path.display())))?
        .len();
    if !(min_bytes..=max_bytes).contains(&size) {
        let low = min_bytes / 1024 / 1024;
        let high = max_bytes / 1024 / 1024;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(CaptureError(format!(
            "{name} is {:.1} MB, outside the plausible RAW range ({low}-{high} MB) — did the film move or the camera misfire?",
            size as f64 / 1024.0 / 1024.0
        )));
    }
    Ok(size)
}

/// A captured file moved into place only after its size passes.
///
/// `capture` writes beside the destination and returns the file's path; the destination
/// replaces the staged file only once it validates.
pub fn capture_validated_single<F>(
    destination: &Path,
    capture: F,
    min_bytes: u64,
    max_bytes: u64,
) -> Result<PathBuf, CaptureError>
where
    F: FnOnce(&Path) -> Result<PathBuf, CaptureError>,
{
    let folder = destination
        .parent()
        .ok_or_else(|| CaptureError("the capture destination has no folder".to_string()))?;
    std::fs::create_dir_all(folder)
        .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
    let stem = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".to_string());
    let staging = folder.join(format!(".{stem}.capture"));
    std::fs::create_dir_all(&staging)
        .map_err(|error| CaptureError(format!("cannot create the staging folder: {error}")))?;
    let staged = staging.join(&stem);
    let result = capture(&staged).and_then(|captured| {
        verify_raw_size(&captured, min_bytes, max_bytes)?;
        let final_path = folder.join(
            captured
                .file_name()
                .ok_or_else(|| CaptureError("the capture returned no file name".to_string()))?,
        );
        std::fs::rename(&captured, &final_path).map_err(|error| {
            CaptureError(format!("cannot move the capture into place: {error}"))
        })?;
        Ok(final_path)
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_balanced_plan_keeps_every_channel_at_full_level() {
        let steps = blend_plan([255, 128, 64], 0.5, None).expect("plan");
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].level, 255);
        assert_eq!(steps[1].level, 128);
        assert_eq!(steps[2].level, 64);
        assert!((steps[0].seconds - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_short_exposure_dims_a_channel_to_keep_the_balance() {
        let steps = blend_plan([255, 255, 255], 0.1, Some(2.5)).expect("plan");
        for step in &steps {
            assert!(step.level <= 255);
            assert!(step.seconds <= 2.5);
        }
        let total: f64 = steps.iter().map(|step| step.seconds).sum();
        assert!(total <= 2.5 + 1e-9);
        for step in &steps {
            assert!(step.level <= 128, "{} stayed bright", step.level);
            assert!((step.seconds - 0.1 * 255.0 / f64::from(step.level)).abs() < 1e-9);
        }
    }

    #[test]
    fn an_unlit_plan_is_rejected() {
        assert!(blend_plan([0, 0, 0], 0.5, None).is_err());
        assert!(blend_plan([255, 0, 0], 0.0, None).is_err());
    }

    #[test]
    fn a_cable_that_is_too_short_is_rejected() {
        let error = blend_plan([255, 255, 255], 3.0, Some(0.5)).expect_err("too long");
        assert!(error.0.contains("release cable"));
    }

    #[test]
    fn a_capture_that_is_too_small_never_reaches_its_name() {
        let folder = std::env::temp_dir().join(format!("negpy-capture-{}", std::process::id()));
        let destination = folder.join("Frame001.raw");
        let result = capture_validated_single(
            &destination,
            |staged| {
                std::fs::write(staged, b"short").expect("staged file");
                Ok(staged.to_path_buf())
            },
            1024,
            2048,
        );
        assert!(result.is_err());
        assert!(!destination.exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_valid_capture_lands_under_its_own_name() {
        let folder = std::env::temp_dir().join(format!("negpy-capture-ok-{}", std::process::id()));
        let destination = folder.join("Frame001.raw");
        let final_path = capture_validated_single(
            &destination,
            |staged| {
                let landed = staged.with_extension("NEF");
                std::fs::write(&landed, vec![0u8; 1500]).expect("staged file");
                Ok(landed)
            },
            1024,
            2048,
        )
        .expect("capture");
        assert_eq!(final_path.file_name().unwrap(), "Frame001.NEF");
        assert!(final_path.is_file());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
