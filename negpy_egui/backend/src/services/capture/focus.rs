//! Focus meter for the camera live view.

pub const MIN_MEAN_LEVEL: f64 = 4.0;
pub const SMOOTHING: f64 = 0.3;

/// Mean squared Laplacian over mean level squared; 0 for a frame too dark or small.
#[must_use]
pub fn sharpness(gray: &[f32], width: usize, height: usize) -> f64 {
    if width < 3 || height < 3 || gray.len() != width * height {
        return 0.0;
    }
    negpy_core::capture::sharpness(gray, height, width, MIN_MEAN_LEVEL)
}

/// Smoothed sharpness as a fraction of its peak since the last reset.
#[derive(Clone, Debug, Default)]
pub struct FocusMeter {
    value: Option<f64>,
    peak: f64,
}

impl FocusMeter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.value = None;
        self.peak = 0.0;
    }

    /// Feed one frame; `None` for a dark or degenerate frame.
    pub fn update(&mut self, gray: &[f32], width: usize, height: usize) -> Option<f64> {
        let score = sharpness(gray, width, height);
        if score <= 0.0 {
            return None;
        }
        self.value = Some(match self.value {
            Some(value) => value + SMOOTHING * (score - value),
            None => score,
        });
        self.peak = self.peak.max(self.value.unwrap_or_default());
        Some(self.value.unwrap_or_default() / self.peak)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dark_frame_has_no_sharpness() {
        let dark = vec![0.0f32; 16 * 16];
        assert_eq!(sharpness(&dark, 16, 16), 0.0);
        assert_eq!(sharpness(&[128.0; 4], 2, 2), 0.0);
    }

    #[test]
    fn the_meter_reads_one_until_a_sharper_frame_arrives() {
        let flat = vec![100.0f32; 32 * 32];
        let edge = (0..32 * 32)
            .map(|index| if (index / 32) % 2 == 0 { 40.0 } else { 200.0 })
            .collect::<Vec<_>>();
        let soft = (0..32 * 32)
            .map(|index| if (index / 32) % 2 == 0 { 80.0 } else { 160.0 })
            .collect::<Vec<_>>();
        let mut meter = FocusMeter::new();
        assert!(meter.update(&flat, 32, 32).is_none());
        assert!((meter.update(&edge, 32, 32).unwrap() - 1.0).abs() < 1e-9);
        assert!(meter.update(&soft, 32, 32).unwrap() < 1.0);
        meter.reset();
        assert!((meter.update(&edge, 32, 32).unwrap() - 1.0).abs() < 1e-9);
    }
}
