//! Mirrors `negpy/services/capture/meter.py`: one linear probe sets a shutter.

use crate::features::geometry::processor::{
    film_and_picture, normalize_detection_input, rebate_mask, scale_roi,
};
use crate::features::geometry::skew::AUTOCROP_DETECT_RES;
use crate::features::process::detect::detect_process_mode;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::services::capture::calibration::{
    CLIP_CEILING, SATURATION_VALUE, SHUTTER_CANDIDATES, TARGET_FRACTION, true_seconds,
};

pub const BLOWN_SHARE: f64 = 0.02;
pub const CLIPPED_STEP_STOPS: f64 = -2.0;
pub const PEAK_MAP_EDGE: usize = 960;
pub const ASPECT_TOLERANCE: f64 = 0.02;
pub const SAME_FRAMING: f64 = 0.8;
const THUMB: (usize, usize) = (43, 64);
const SATURATION: f32 = SATURATION_VALUE as f32 / CLIP_CEILING as f32;

/// One probe's verdict: what was metered, where it sits, and its shutter.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterReading {
    pub medium: String,
    pub region: String,
    pub measured: f64,
    pub clipped: bool,
    pub clipped_fraction: f64,
    pub shutter: String,
    pub recommended: Option<String>,
    pub stops: f64,
    pub peaks: Option<Vec<f32>>,
    pub thumb: Option<Vec<f32>>,
    pub shape: Option<(usize, usize)>,
    pub ring_saturated: bool,
}

/// `gray_of`: the float32 mean of the three channels.
#[must_use]
pub fn gray_of(rgb: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; rgb.len() / 3];
    negpy_core::convert::mean_channels(rgb, &mut out);
    out
}

/// `ranked_thumb`: a picture at thumbnail size as zero-mean, unit-variance ranks.
#[must_use]
pub fn ranked_thumb(gray: &[f32], width: usize, height: usize) -> Vec<f32> {
    let Some(small) = negpy_core::resize::resize_area(gray, height, width, 1, THUMB.0, THUMB.1)
    else {
        return Vec::new();
    };
    let mut order: Vec<usize> = (0..small.len()).collect();
    order.sort_by(|a, b| small[*a].total_cmp(&small[*b]));
    let mut ranks = vec![0.0f32; small.len()];
    for (rank, index) in order.into_iter().enumerate() {
        ranks[index] = rank as f32;
    }
    let mean = ranks.iter().sum::<f32>() / ranks.len().max(1) as f32;
    for value in &mut ranks {
        *value -= mean;
    }
    let variance = ranks.iter().map(|value| value * value).sum::<f32>() / ranks.len().max(1) as f32;
    let std = variance.sqrt().max(1e-6);
    ranks.into_iter().map(|value| value / std).collect()
}

/// `framing_mismatch`: why a probe's map cannot lie over a preview, or None.
#[must_use]
pub fn framing_mismatch(
    reading: &MeterReading,
    preview: &[f32],
    width: usize,
    height: usize,
) -> Option<String> {
    let thumb = reading.thumb.as_ref()?;
    let (probe_h, probe_w) = reading.shape?;
    if probe_h == 0 || probe_w == 0 || width == 0 || height == 0 {
        return Some("the probe kept no picture".to_string());
    }
    let aspect = (probe_w as f64 / probe_h as f64) / (width as f64 / height as f64);
    if (aspect - 1.0).abs() > ASPECT_TOLERANCE {
        return Some(format!(
            "the preview is {width}:{height} and the RAW {probe_w}:{probe_h}"
        ));
    }
    let preview_thumb = ranked_thumb(&gray_of(preview), width, height);
    if preview_thumb.len() != thumb.len() {
        return Some("the preview does not frame the RAW the same way".to_string());
    }
    let product = preview_thumb
        .iter()
        .zip(thumb)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        / thumb.len() as f32;
    (product < SAME_FRAMING as f32)
        .then(|| "the preview does not frame the RAW the same way".to_string())
}

/// `shutter_at_most`: the slowest rung at or under `seconds`.
#[must_use]
pub fn shutter_at_most(seconds: f64, candidates: &[&str]) -> Option<String> {
    for label in candidates.iter().rev() {
        if true_seconds(label, candidates) <= seconds {
            return Some((*label).to_string());
        }
    }
    None
}

fn median(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

/// Meter one decoded linear frame. `medium` is `negative`, `positive`, or None to detect it.
#[must_use]
pub fn meter_frame(
    image: &ImageBuffer,
    shutter: &str,
    candidates: &[&str],
    medium: Option<&str>,
    white_balance: Option<[f32; 3]>,
) -> MeterReading {
    let candidates = if candidates.is_empty() {
        &SHUTTER_CANDIDATES[..]
    } else {
        candidates
    };
    let gain = match white_balance {
        Some(wb) if wb.iter().copied().fold(f32::INFINITY, f32::min) > 0.0 => {
            let low = wb.iter().copied().fold(f32::INFINITY, f32::min);
            wb.map(|value| value / low)
        }
        _ => [1.0; 3],
    };
    let mut linear = vec![0.0f32; image.pixels.len()];
    negpy_core::capture::normalized(&image.pixels, CLIP_CEILING as f32, &mut linear);
    let medium = match medium {
        Some(value) => value.to_string(),
        None => {
            if detect_process_mode(&linear, image.width, image.height)
                == crate::domain::models::ProcessMode::E6
            {
                "positive".to_string()
            } else {
                "negative".to_string()
            }
        }
    };
    let (det, scale) = normalize_detection_input(
        &ImageBuffer::from_rgb(image.width, image.height, linear.clone()),
        AUTOCROP_DETECT_RES as u32,
    );
    let Some((film_roi, gate)) = film_and_picture(&det) else {
        return MeterReading {
            medium,
            region: "highlights".to_string(),
            measured: 0.0,
            clipped: true,
            clipped_fraction: 1.0,
            shutter: shutter.to_string(),
            recommended: None,
            stops: CLIPPED_STEP_STOPS,
            peaks: None,
            thumb: None,
            shape: Some((image.height, image.width)),
            ring_saturated: false,
        };
    };
    let roi = scale_roi(gate, scale, image.height, image.width);
    let (clipped_fraction, blown, thinnest) = negpy_core::capture::meter_picture(
        &linear,
        image.width,
        (
            roi.0.max(0) as usize,
            roi.1.max(0) as usize,
            roi.2.max(0) as usize,
            roi.3.max(0) as usize,
        ),
        SATURATION,
        BLOWN_SHARE,
    );
    let mask = rebate_mask((det.height, det.width), film_roi, gate);
    let mut ring: Vec<[f32; 3]> = Vec::new();
    for (index, inside) in mask.iter().enumerate() {
        if !inside {
            continue;
        }
        let at = index * 3;
        if let Some(pixel) = det.pixels.get(at..at + 3) {
            ring.push([pixel[0], pixel[1], pixel[2]]);
        }
    }
    let unclipped: Vec<[f32; 3]> = ring
        .iter()
        .copied()
        .filter(|pixel| pixel.iter().copied().fold(0.0f32, f32::max) < SATURATION)
        .collect();
    let ring_saturated =
        medium == "negative" && !ring.is_empty() && unclipped.len() < ring.len() / 2;
    let mut picture: Vec<[f32; 3]> = Vec::new();
    for y in gate.0.max(0) as usize..gate.1.max(0) as usize {
        for x in gate.2.max(0) as usize..gate.3.max(0) as usize {
            let at = (y * det.width + x) * 3;
            if let Some(pixel) = det.pixels.get(at..at + 3) {
                picture.push([pixel[0], pixel[1], pixel[2]]);
            }
        }
    }
    let mut film_means: Vec<f32> = unclipped
        .iter()
        .map(|pixel| (pixel[0] + pixel[1] + pixel[2]) / 3.0)
        .collect();
    let mut picture_means: Vec<f32> = picture
        .iter()
        .map(|pixel| (pixel[0] + pixel[1] + pixel[2]) / 3.0)
        .collect();
    let base_visible = medium == "negative"
        && !ring_saturated
        && !film_means.is_empty()
        && median(&mut film_means) > median(&mut picture_means);
    let (region, levels) = if base_visible {
        let mut levels = [0.0f32; 3];
        for channel in 0..3 {
            let mut values: Vec<f32> = unclipped.iter().map(|pixel| pixel[channel]).collect();
            levels[channel] = median(&mut values).max(thinnest[channel]);
        }
        ("base", levels)
    } else {
        ("highlights", thinnest)
    };
    let clipped = blown || levels.iter().copied().fold(0.0f32, f32::max) >= SATURATION;
    let measured =
        f64::from(levels[0].max(levels[1]).max(levels[2]) * gain[0].max(gain[1]).max(gain[2]))
            * CLIP_CEILING as f64;
    let probe_seconds = true_seconds(shutter, candidates);
    let needed = if clipped {
        CLIPPED_STEP_STOPS
    } else {
        (TARGET_FRACTION * CLIP_CEILING as f64 / measured.max(1.0)).log2()
    };
    let mut recommended = shutter_at_most(probe_seconds * 2f64.powf(needed), candidates);
    if recommended.is_none() && clipped && true_seconds(candidates[0], candidates) < probe_seconds {
        recommended = Some(candidates[0].to_string());
    }
    let stops = recommended.as_ref().map_or(needed, |label| {
        (true_seconds(label, candidates) / probe_seconds).log2()
    });
    let factor = (image.width.max(image.height) as f64 / PEAK_MAP_EDGE as f64).ceil() as usize;
    let (peaks, _, _) =
        negpy_core::capture::peak_map(&linear, image.height, image.width, factor.max(1));
    MeterReading {
        medium,
        region: region.to_string(),
        measured,
        clipped,
        clipped_fraction,
        shutter: shutter.to_string(),
        recommended,
        stops,
        peaks: Some(peaks),
        thumb: Some(ranked_thumb(&gray_of(&linear), image.width, image.height)),
        shape: Some((image.height, image.width)),
        ring_saturated,
    }
}
