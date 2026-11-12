//! Process-mode detection from a scan's channel statistics.
//!
//! Mirrors `negpy/features/process/logic.py`: an orange or purple base means a
//! color negative; decorrelated channels mean a slide; a neutral axis means B&W.

use negpy_core::process::{chroma_spread, process_mode_stats};

use crate::domain::models::ProcessMode;

const ANALYSIS_BUFFER: f64 = 0.12;
const MAX_ANALYSIS_DIM: usize = 256;
const BW_CORR_THRESHOLD: f64 = 0.99;
const BW_CHROMA_THRESHOLD: f64 = 0.022;
const CHROMA_BLOCK: usize = 4;
const CHROMA_LO: f32 = 0.003;
const CHROMA_HI: f32 = 0.97;
const CHROMA_MIN_SHARE: f64 = 0.1;
const C41_ORANGE_THRESHOLD: f64 = 1.5;
const PURPLE_G_DEFICIT: f64 = 0.05;
const PURPLE_RB_BALANCE: f64 = 1.05;

/// The process mode and whether the source is color film, from one pass of the stats.
#[must_use]
pub fn detect_process_and_source(
    pixels: &[f32],
    width: usize,
    height: usize,
) -> (ProcessMode, bool) {
    if width == 0 || height == 0 || pixels.len() < width * height * 3 {
        return (ProcessMode::C41, true);
    }
    let (crop, crop_width, crop_height) = center_crop(pixels, width, height, ANALYSIS_BUFFER);
    let (sample, sample_width, sample_height) =
        downsample(&crop, crop_width, crop_height, MAX_ANALYSIS_DIM);
    if sample_width == 0 || sample_height == 0 {
        return (ProcessMode::C41, true);
    }
    let sample = clean(&sample);
    let (min_corr, stats) = process_mode_stats(&sample);
    let [r_mean, b_mean, r_p25, b_p25, r_p98, g_p98, b_p98] = stats;
    let orange = ((r_mean + 1e-6) / (b_mean + 1e-6))
        .max((r_p25 + 1e-6) / (b_p25 + 1e-6))
        .max((r_p98 + 1e-6) / (b_p98 + 1e-6));
    let masked = orange > C41_ORANGE_THRESHOLD || has_purple_mask(r_p98, g_p98, b_p98);
    let spread = chroma_spread(
        &sample,
        sample_width,
        CHROMA_BLOCK,
        CHROMA_LO,
        CHROMA_HI,
        CHROMA_MIN_SHARE,
    );
    let monochrome = spread.map_or(min_corr > BW_CORR_THRESHOLD, |value| {
        value < BW_CHROMA_THRESHOLD
    });
    if monochrome {
        return (ProcessMode::BW, masked);
    }
    (
        if masked {
            ProcessMode::C41
        } else {
            ProcessMode::E6
        },
        true,
    )
}

/// The process mode alone; invalid input reads as a color negative.
#[must_use]
pub fn detect_process_mode(pixels: &[f32], width: usize, height: usize) -> ProcessMode {
    detect_process_and_source(pixels, width, height).0
}

/// True when the scan is of color film.
#[must_use]
pub fn detect_source_color(pixels: &[f32], width: usize, height: usize) -> bool {
    detect_process_and_source(pixels, width, height).1
}

fn has_purple_mask(r: f64, g: f64, b: f64) -> bool {
    let deficit = (r + b) / 2.0 - g;
    let balance = r.min(b) / (g + 1e-6);
    deficit > PURPLE_G_DEFICIT && balance > PURPLE_RB_BALANCE
}

fn center_crop(
    pixels: &[f32],
    width: usize,
    height: usize,
    buffer: f64,
) -> (Vec<f32>, usize, usize) {
    if buffer <= 0.0 {
        return (pixels.to_vec(), width, height);
    }
    let buffer = buffer.clamp(0.0, 0.3);
    let cut_y = (height as f64 * buffer) as usize;
    let cut_x = (width as f64 * buffer) as usize;
    let (w, h) = (width - 2 * cut_x, height - 2 * cut_y);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in cut_y..height - cut_y {
        let start = (y * width + cut_x) * 3;
        out.extend_from_slice(&pixels[start..start + w * 3]);
    }
    (out, w, h)
}

fn downsample(
    pixels: &[f32],
    width: usize,
    height: usize,
    max_dim: usize,
) -> (Vec<f32>, usize, usize) {
    let longest = width.max(height);
    if longest <= max_dim {
        return (pixels.to_vec(), width, height);
    }
    let step = longest.div_ceil(max_dim);
    let (w, h) = (width.div_ceil(step), height.div_ceil(step));
    let mut out = Vec::with_capacity(w * h * 3);
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let at = (y * width + x) * 3;
            out.extend_from_slice(&pixels[at..at + 3]);
        }
    }
    (out, w, h)
}

fn clean(pixels: &[f32]) -> Vec<f32> {
    pixels
        .iter()
        .map(|value| {
            if value.is_nan() {
                0.0
            } else if *value == f32::INFINITY {
                1.0
            } else if *value == f32::NEG_INFINITY {
                0.0
            } else {
                value.clamp(0.0, 1.0)
            }
        })
        .collect()
}
