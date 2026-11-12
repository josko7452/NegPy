//! Ports of `negpy/features/lab/logic.py` onto the `negpy_core` Lab kernels.

use negpy_core::lab::{self, Rl, Spread};

use crate::domain::color::{WORKING_GAMMA, WORKING_INV_GAMMA, clip01, working_lab};
use crate::domain::types::{LUM_B, LUM_G, LUM_R};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const SHARPEN_SHADOW_FLOOR: f32 = 1.0 / 3.0;
pub const SHARPEN_SHADOW_L_HI: f32 = 35.0;
pub const RL_EPS: f32 = 1e-6;
pub const AUTO_MASKING_GAIN: f32 = 0.27;
pub const NEUTRAL_MASKING: f32 = 0.1;
pub const CHROMA_DENOISE_MIN_SIGMA: f64 = 0.25;
pub const HALATION_SIGMA: f64 = 0.0027;
pub const HALATION_BOUNCES: [(f64, f64); 3] = [
    (4.0 / 7.0, 1.0),
    (2.0 / 7.0, std::f64::consts::SQRT_2),
    (1.0 / 7.0, 1.732_050_807_568_877_2),
];
pub const HALATION_AMOUNT_MAX: f64 = 0.2;
pub const HALATION_TINT: [f64; 3] = [1.0, 0.3, 0.05];
pub const GLOW_PSF: [(f64, f64); 4] = [
    (0.1104, 0.001582),
    (0.3559, 0.004122),
    (0.4014, 0.010181),
    (0.1323, 0.024798),
];
pub const GLOW_AMOUNT_MAX: f64 = 0.25;
pub const GLOW_THRESHOLD: f64 = 0.5;

/// `gaussian_kernel_1d(sigma)`: the blur taps both engines share.
#[must_use]
pub fn gaussian_kernel_1d(sigma: f64) -> Vec<f32> {
    let radius = (2.5 * sigma).ceil().clamp(1.0, 255.0) as i32;
    let values: Vec<f32> = (-radius..=radius)
        .map(|x| {
            let x = x as f32;
            negpy_core::mathf::np_expf(-(x * x) / (2.0 * sigma * sigma) as f32)
        })
        .collect();
    let sum: f32 = values.iter().sum();
    values.into_iter().map(|value| value / sum).collect()
}

/// `rl_iterations(radius)`: a deterministic count from the user radius.
#[must_use]
pub fn rl_iterations(radius: f64) -> usize {
    (10.0 * radius).round_ties_even().clamp(5.0, 20.0) as usize
}

/// `apply_clahe`: no-op at zero strength.
#[must_use]
pub fn apply_clahe(img: &ImageBuffer, strength: f64) -> ImageBuffer {
    if strength <= 0.0 {
        return img.clone();
    }
    let mut out = vec![0.0; img.pixels.len()];
    let spaces = working_lab();
    lab::clahe(
        &img.pixels,
        (img.height, img.width),
        strength,
        spaces,
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `chroma_denoise_radius(radius, mono)`.
#[must_use]
pub fn chroma_denoise_radius(radius: f64, mono: bool) -> f64 {
    if mono { 0.0 } else { radius }
}

/// `chroma_denoise_sigma(radius, scale_factor)`.
#[must_use]
pub fn chroma_denoise_sigma(radius: f64, scale_factor: f64) -> f64 {
    let sigma = radius * scale_factor;
    if sigma >= CHROMA_DENOISE_MIN_SIGMA {
        sigma
    } else {
        0.0
    }
}

/// `apply_chroma_denoise`.
#[must_use]
pub fn apply_chroma_denoise(img: &ImageBuffer, radius: f64, scale_factor: f64) -> ImageBuffer {
    let sigma = chroma_denoise_sigma(radius, scale_factor);
    if sigma == 0.0 {
        return img.clone();
    }
    let mut out = vec![0.0; img.pixels.len()];
    let spaces = working_lab();
    lab::chroma_denoise(
        &img.pixels,
        (img.height, img.width),
        sigma,
        &gaussian_kernel_1d(sigma),
        spaces,
        &mut out,
    )
    .expect("the kernel covers the frame");
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `apply_saturation`.
#[must_use]
pub fn apply_saturation(img: &ImageBuffer, saturation: f64, skin_protection: f64) -> ImageBuffer {
    if saturation == 1.0 && skin_protection <= 0.0 {
        return img.clone();
    }
    let mut out = vec![0.0; img.pixels.len()];
    let (to_lab, to_rgb) = working_lab();
    negpy_core::color::saturation(
        &img.pixels,
        &mut out,
        (saturation as f32, skin_protection as f32),
        to_lab,
        to_rgb,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `auto_masking`: grain-driven masking, or `None` when the toggle is off.
#[must_use]
pub fn auto_masking(img: &ImageBuffer, config: &crate::domain::models::LabConfig) -> Option<f32> {
    if !config.sharpen_auto_masking {
        return None;
    }
    let grain = lab::sharpen_grain(&img.pixels, (img.height, img.width), [LUM_R, LUM_G, LUM_B]);
    Some(match grain {
        None => NEUTRAL_MASKING,
        Some(value) => (AUTO_MASKING_GAIN * value).min(1.0),
    })
}

/// `effective_masking(config, auto)`.
#[must_use]
pub fn effective_masking(config: &crate::domain::models::LabConfig, auto: Option<f32>) -> f64 {
    match auto {
        None => config.sharpen_masking,
        Some(value) => {
            (value + config.sharpen_masking as f32 - NEUTRAL_MASKING).clamp(0.0, 1.0) as f64
        }
    }
}

/// `apply_usm_sharpening`, row-blocked at the Python block size.
#[must_use]
pub fn apply_usm_sharpening(
    img: &ImageBuffer,
    amount: f64,
    radius: f64,
    masking: f64,
) -> ImageBuffer {
    if amount <= 0.0 {
        return img.clone();
    }
    let halo = (gaussian_kernel_1d(radius).len() / 2).max(2);
    map_row_blocks(img, halo, |block| usm_block(block, amount, radius, masking))
}

/// `apply_rl_sharpening`.
#[must_use]
pub fn apply_rl_sharpening(
    img: &ImageBuffer,
    amount: f64,
    radius: f64,
    masking: f64,
) -> ImageBuffer {
    if amount <= 0.0 {
        return img.clone();
    }
    let params = Rl {
        amount: amount as f32,
        masking,
        iterations: rl_iterations(radius),
        lum: [LUM_R, LUM_G, LUM_B],
        eps: RL_EPS,
    };
    let mut out = vec![0.0; img.pixels.len()];
    lab::rl_sharpen(
        &img.pixels,
        (img.height, img.width),
        &params,
        &gaussian_kernel_1d(radius),
        &mut out,
    )
    .expect("the kernel covers the frame");
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `apply_lab_sharpening`.
#[must_use]
pub fn apply_lab_sharpening(
    img: &ImageBuffer,
    config: &crate::domain::models::LabConfig,
    auto: Option<f32>,
) -> ImageBuffer {
    let masking = effective_masking(config, auto);
    match config.sharpen_method {
        crate::domain::models::SharpenMethod::Rl => {
            apply_rl_sharpening(img, config.sharpen, config.sharpen_radius, masking)
        }
        crate::domain::models::SharpenMethod::Usm => {
            apply_usm_sharpening(img, config.sharpen, config.sharpen_radius, masking)
        }
    }
}

/// `halation_amounts(strength, bw)`.
#[must_use]
pub fn halation_amounts(strength: f64, bw: bool) -> [f32; 3] {
    let mut amounts = HALATION_TINT.map(|tint| (HALATION_AMOUNT_MAX * strength * tint) as f32);
    if bw {
        let y = amounts[0] * LUM_R + amounts[1] * LUM_G + amounts[2] * LUM_B;
        amounts = [y; 3];
    }
    amounts
}

/// `halation_sigma_px(shape)`.
#[must_use]
pub fn halation_sigma_px(height: usize, width: usize) -> f64 {
    HALATION_SIGMA * height.min(width) as f64
}

/// `halation_reach_px(shape)`.
#[must_use]
pub fn halation_reach_px(height: usize, width: usize) -> usize {
    (3.0 * HALATION_BOUNCES[2].1 * halation_sigma_px(height, width)).ceil() as usize
}

/// `glow_reach_px(shape)`.
#[must_use]
pub fn glow_reach_px(height: usize, width: usize) -> usize {
    (3.0 * GLOW_PSF[3].1 * height.min(width) as f64).ceil() as usize
}

/// `apply_glow_and_halation`.
#[must_use]
pub fn apply_glow_and_halation(
    img: &ImageBuffer,
    glow_amount: f64,
    halation_strength: f64,
    bw: bool,
) -> ImageBuffer {
    if glow_amount == 0.0 && halation_strength == 0.0 {
        return img.clone();
    }
    let short_side = img.height.min(img.width) as f64;
    let glow = (glow_amount > 0.0).then(|| {
        let a = (GLOW_AMOUNT_MAX * glow_amount) as f32;
        (
            Spread {
                amounts: [a; 3],
                psf: GLOW_PSF
                    .iter()
                    .map(|&(weight, sigma)| (weight as f32, sigma * short_side))
                    .collect(),
            },
            GLOW_THRESHOLD,
        )
    });
    let halation = (halation_strength > 0.0).then(|| {
        let sigma = halation_sigma_px(img.height, img.width);
        Spread {
            amounts: halation_amounts(halation_strength, bw),
            psf: HALATION_BOUNCES
                .iter()
                .map(|&(weight, k)| (weight as f32, k * sigma))
                .collect(),
        }
    });
    let mut out = vec![0.0; img.pixels.len()];
    lab::glow_and_halation(
        &img.pixels,
        (img.height, img.width),
        (glow, halation),
        ([LUM_R, LUM_G, LUM_B], WORKING_INV_GAMMA),
        &mut out,
    )
    .expect("the kernel covers the frame");
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `clip01` over a whole frame.
#[must_use]
pub fn clip_frame(img: ImageBuffer) -> ImageBuffer {
    let mut pixels = img.pixels;
    for value in &mut pixels {
        *value = clip01(*value);
    }
    ImageBuffer::from_rgb(img.width, img.height, pixels)
}

fn usm_block(img: &ImageBuffer, amount: f64, radius: f64, masking: f64) -> ImageBuffer {
    let mut out = vec![0.0; img.pixels.len()];
    let spaces = working_lab();
    lab::usm_sharpen(
        &img.pixels,
        (img.height, img.width),
        ((amount * 2.5) as f32, masking),
        &gaussian_kernel_1d(radius),
        spaces,
        &mut out,
    )
    .expect("the kernel covers the frame");
    ImageBuffer::from_rgb(img.width, img.height, out)
}

fn map_row_blocks(
    img: &ImageBuffer,
    halo: usize,
    apply: impl Fn(&ImageBuffer) -> ImageBuffer,
) -> ImageBuffer {
    let rows = (1_048_576 / img.width).max(1);
    if img.height <= rows {
        return apply(img);
    }
    let mut out = vec![0.0; img.pixels.len()];
    for start in (0..img.height).step_by(rows) {
        let end = (start + rows).min(img.height);
        let low = start.saturating_sub(halo);
        let high = (end + halo).min(img.height);
        let block = slice_rows(img, low, high);
        let block = apply(&block);
        let from = start - low;
        let to = from + (end - start);
        out[start * img.width * 3..end * img.width * 3]
            .copy_from_slice(&block.pixels[from * img.width * 3..to * img.width * 3]);
    }
    ImageBuffer::from_rgb(img.width, img.height, out)
}

fn slice_rows(img: &ImageBuffer, low: usize, high: usize) -> ImageBuffer {
    let begin = low * img.width * 3;
    let end = high * img.width * 3;
    ImageBuffer::from_rgb(img.width, high - low, img.pixels[begin..end].to_vec())
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `sharpen_shadow_gain`: per-pixel sharpen gain from L*, lighter shadows sharpen fully.
#[must_use]
pub fn sharpen_shadow_gain(luminance: &[f32]) -> Vec<f32> {
    luminance
        .iter()
        .map(|value| {
            SHARPEN_SHADOW_FLOOR
                + (1.0 - SHARPEN_SHADOW_FLOOR) * smoothstep(0.0, SHARPEN_SHADOW_L_HI, *value)
        })
        .collect()
}

/// `apply_resize_sharpening`: Lab Sharpening on working-OETF code values.
#[must_use]
pub fn apply_resize_sharpening(
    encoded: &ImageBuffer,
    config: &crate::domain::models::LabConfig,
) -> ImageBuffer {
    if config.sharpen <= 0.0 {
        return encoded.clone();
    }
    let mut linear = vec![0.0; encoded.pixels.len()];
    negpy_core::color::oetf_decode(&encoded.pixels, &mut linear, WORKING_GAMMA as f32);
    let linear = ImageBuffer::from_rgb(encoded.width, encoded.height, linear);
    let sharpened = apply_lab_sharpening(&linear, config, auto_masking(&linear, config));
    let mut out = vec![0.0; encoded.pixels.len()];
    negpy_core::color::oetf_encode(&sharpened.pixels, &mut out, WORKING_INV_GAMMA);
    ImageBuffer::from_rgb(encoded.width, encoded.height, out)
}
