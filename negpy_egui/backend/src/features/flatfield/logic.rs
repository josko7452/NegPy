//! Ports `negpy/features/flatfield/logic.py` onto the negpy_core flat-field kernels.

use blake2::digest::consts::U8;
use blake2::{Blake2b, Digest};

use crate::domain::models::FlatFieldConfig;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

const GAIN_MIN: f32 = 0.25;
const GAIN_MAX: f32 = 4.0;
const GAIN_WORK_SIZE: usize = 256;
const BLUR_DIVISOR: f64 = 64.0;
const CHECK_COPY_SIZE: usize = 512;
const LIT_PERCENTILE: f64 = 95.0;
const LIT_FRACTION: f64 = 0.35;
const MIN_LIT_FRACTION: f64 = 0.02;
const MIN_LIT_WEIGHT: f32 = 0.05;
const CLIP_LEVEL: f32 = 0.98;
const CLIP_FRACTION: f64 = 0.001;

pub const EVENNESS_RANGE: f64 = 0.10;
pub const GAIN_VIEW_RANGE: f64 = 0.15;
pub const UNEVEN_LIMIT: f64 = 0.06;

const LIT: negpy_core::flatfield::Lit = negpy_core::flatfield::Lit {
    percentile: LIT_PERCENTILE,
    fraction: LIT_FRACTION,
    min_fraction: MIN_LIT_FRACTION,
};

/// One baked profile's gain, its content token and whether it is in sensor layout.
#[derive(Clone, Debug, PartialEq)]
pub struct GainEntry {
    pub gain: ImageBuffer,
    pub token: String,
    pub sensor_layout: bool,
}

/// The spread of a reference about its own lit-area median.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Evenness {
    pub low: f64,
    pub high: f64,
    pub color: f64,
    pub clipped: bool,
}

impl Evenness {
    #[must_use]
    pub fn spread(self) -> f64 {
        (-self.low).max(self.high)
    }
}

/// A gain map resolved for one render.
pub trait GainProvider {
    fn gain(&self, profile_id: &str) -> Option<GainEntry>;
}

fn copy_at(reference: &ImageBuffer, long_edge: usize) -> ImageBuffer {
    let h = reference.height;
    let w = reference.width;
    let scale = (long_edge as f64 / h.max(w) as f64).min(1.0);
    if scale >= 1.0 {
        return reference.clone();
    }
    let dh = ((h as f64 * scale).round() as usize).max(1);
    let dw = ((w as f64 * scale).round() as usize).max(1);
    let pixels = negpy_core::resize::resize_area(&reference.pixels, h, w, 3, dh, dw)
        .expect("positive reference dimensions");
    ImageBuffer::from_rgb(dw, dh, pixels)
}

/// `work_copy`: the reference at the gain's working size.
#[must_use]
pub fn work_copy(reference: &ImageBuffer) -> ImageBuffer {
    copy_at(reference, GAIN_WORK_SIZE)
}

/// `check_copy`: the finer reference copy a profile stores for Check Flat Field.
#[must_use]
pub fn check_copy(reference: &ImageBuffer) -> ImageBuffer {
    copy_at(reference, CHECK_COPY_SIZE)
}

/// `compute_gain`: the lit-area mean of a normalized blur over the blur.
#[must_use]
pub fn compute_gain(reference: &ImageBuffer) -> ImageBuffer {
    let reference = work_copy(reference);
    let params = negpy_core::flatfield::Gain {
        lit: negpy_core::flatfield::Lit {
            percentile: LIT.percentile,
            fraction: LIT.fraction,
            min_fraction: LIT.min_fraction,
        },
        sigma: reference.height.max(reference.width) as f64 / BLUR_DIVISOR,
        eps: 1e-4,
        min_lit_weight: MIN_LIT_WEIGHT,
        range: (GAIN_MIN, GAIN_MAX),
    };
    let mut gain = vec![1.0; reference.pixels.len()];
    negpy_core::flatfield::compute_gain(
        &reference.pixels,
        (reference.height, reference.width),
        &params,
        &mut gain,
    );
    ImageBuffer::from_rgb(reference.width, reference.height, gain)
}

/// `gain_token`: the first 8 bytes of BLAKE2b over the float32 gain.
#[must_use]
pub fn gain_token(gain: &ImageBuffer) -> String {
    let mut hasher = Blake2b::<U8>::new();
    for value in &gain.pixels {
        hasher.update(value.to_ne_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn measure_evenness(image: &ImageBuffer) -> (Vec<f32>, Evenness) {
    let mut ratio = vec![0.0; image.pixels.len()];
    let result = negpy_core::flatfield::evenness(
        &image.pixels,
        (image.height, image.width),
        &LIT,
        CLIP_LEVEL,
        &mut ratio,
    );
    (
        ratio,
        result.map_or(Evenness::default(), |value| Evenness {
            low: value.low,
            high: value.high,
            color: value.color,
            clipped: value.clipped > CLIP_FRACTION,
        }),
    )
}

/// `evenness`: how even the image is about its own lit-area median.
#[must_use]
pub fn evenness(image: &ImageBuffer) -> Evenness {
    measure_evenness(image).1
}

/// `evenness_view`: each channel over the lit median, with carrier black.
#[must_use]
pub fn evenness_view(image: &ImageBuffer, span: f64) -> (Vec<f32>, Evenness) {
    let (mut ratio, result) = measure_evenness(image);
    negpy_core::flatfield::evenness_view(&mut ratio, (2.0 * span) as f32);
    (ratio, result)
}

/// `self_corrected`: the reference times the gain, resized when their sizes differ.
#[must_use]
pub fn self_corrected(reference: &ImageBuffer, gain: &ImageBuffer) -> ImageBuffer {
    let gain_pixels = if gain.width != reference.width || gain.height != reference.height {
        let mut resized = vec![0.0; reference.pixels.len()];
        let _ = negpy_core::resize::resize_linear(
            &gain.pixels,
            (gain.height, gain.width, 3),
            (reference.height, reference.width),
            false,
            &mut resized,
        );
        resized
    } else {
        gain.pixels.clone()
    };
    let mut out = vec![0.0; reference.pixels.len()];
    negpy_core::convert::multiply(&reference.pixels, &gain_pixels, &mut out);
    ImageBuffer::from_rgb(reference.width, reference.height, out)
}

/// `_is_clipped`: whether more than the clip fraction of lit pixels reaches the clip level.
#[must_use]
pub fn is_clipped(image: &ImageBuffer) -> bool {
    let (fraction, _) = negpy_core::flatfield::clipped(
        &image.pixels,
        (image.height, image.width),
        &LIT,
        CLIP_LEVEL,
    );
    fraction > CLIP_FRACTION
}

/// `check_reference`: self-correct the reference's check copy and measure what is left.
#[must_use]
pub fn check_reference(reference: &ImageBuffer, gain: &ImageBuffer) -> Evenness {
    let correctable = self_corrected(&check_copy(reference), gain);
    let mut result = evenness(&correctable);
    result.clipped = is_clipped(reference);
    result
}

/// `to_sensor_layout`: undo an EXIF orientation baked into the reference.
#[must_use]
pub fn to_sensor_layout(image: &ImageBuffer, orientation: Option<u8>) -> ImageBuffer {
    let code = match orientation.unwrap_or(1) {
        6 => 8,
        8 => 6,
        other => other,
    };
    if !(2..=8).contains(&code) {
        return image.clone();
    }
    let src: Vec<u8> = image.pixels.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let (oh, ow) = negpy_core::orient::oriented_dims(image.height, image.width, code);
    let mut dst = vec![0u8; oh * ow * 12];
    negpy_core::orient::orient(&src, image.height, image.width, 12, code, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    ImageBuffer::from_rgb(ow, oh, pixels)
}

/// `apply_flatfield`: no-op when inactive or unresolved.
#[must_use]
pub fn apply_flatfield(
    image: &ImageBuffer,
    config: &FlatFieldConfig,
    entry: Option<&GainEntry>,
    orientation: Option<u8>,
    gain_crop: Option<&dyn Fn(&ImageBuffer) -> ImageBuffer>,
) -> ImageBuffer {
    if !config.apply || config.profile_id.is_empty() {
        return image.clone();
    }
    let Some(entry) = entry else {
        return image.clone();
    };
    let mut gain = if entry.sensor_layout {
        to_sensor_layout(&entry.gain, orientation)
    } else {
        entry.gain.clone()
    };
    if let Some(crop) = gain_crop {
        gain = crop(&gain);
    }
    if gain.width != image.width || gain.height != image.height {
        let mut resized = vec![0.0; image.pixels.len()];
        let _ = negpy_core::resize::resize_linear(
            &gain.pixels,
            (gain.height, gain.width, 3),
            (image.height, image.width),
            false,
            &mut resized,
        );
        gain.pixels = resized;
    }
    let mut out = vec![0.0; image.pixels.len()];
    negpy_core::convert::multiply(&image.pixels, &gain.pixels, &mut out);
    ImageBuffer::from_rgb(image.width, image.height, out)
}

/// `flatfield_token`: identity of the active correction, empty when inactive.
#[must_use]
pub fn flatfield_token(config: &FlatFieldConfig, entry: Option<&GainEntry>) -> String {
    if !config.apply || config.profile_id.is_empty() {
        return String::new();
    }
    let Some(entry) = entry else {
        return String::new();
    };
    format!(
        "|ff:{}:{}{}",
        config.profile_id,
        entry.token,
        if entry.sensor_layout { "" } else { ":ref" }
    )
}
