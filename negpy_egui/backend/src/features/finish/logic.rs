//! Ports of `negpy/features/finish/logic.py` onto the `negpy_core` finish kernels.

use negpy_core::finish::{self, Carrier};

use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const CARRIER_SAMPLES: usize = 2048;
pub const CARRIER_TONE_SAMPLES: usize = 64;
pub const CARRIER_TONE_POWER: f32 = 3.0;

const CARRIER_PROFILES: &[u8] = include_bytes!("../../../assets/carrier_profiles.f32");

/// `carrier_profiles()`: the fixed 8 x 2048 filed-edge profiles.
#[must_use]
pub fn carrier_profiles() -> Vec<f32> {
    CARRIER_PROFILES
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect()
}

/// `carrier_tone_exposures()`: the table's exposure fraction, dense in the toe.
#[must_use]
pub fn carrier_tone_exposures() -> [f32; CARRIER_TONE_SAMPLES] {
    let mut out = [0.0; CARRIER_TONE_SAMPLES];
    for (index, value) in out.iter_mut().enumerate() {
        let t = index as f32 / (CARRIER_TONE_SAMPLES - 1) as f32;
        *value = negpy_core::mathf::pow_f64(t as f64, CARRIER_TONE_POWER as f64) as f32;
    }
    out
}

/// `linear_carrier_tone()`: plain light, paper to black.
#[must_use]
pub fn linear_carrier_tone() -> [[f32; 3]; CARRIER_TONE_SAMPLES] {
    carrier_tone_exposures().map(|exposure| [1.0 - exposure; 3])
}

/// `apply_vignette`: an exposure change in scene-linear.
#[must_use]
pub fn apply_vignette(img: &ImageBuffer, stops: f64, size: f64, roundness: f64) -> ImageBuffer {
    if stops == 0.0 {
        return img.clone();
    }
    let mut out = vec![0.0; img.pixels.len()];
    finish::apply_vignette(
        &img.pixels,
        img.height,
        img.width,
        3,
        stops,
        size,
        roundness,
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `apply_carrier`: no-op at zero width.
#[must_use]
pub fn apply_carrier(
    img: &ImageBuffer,
    width_px: f64,
    rough: f64,
    flare: f64,
    corner: f64,
    paper: [f32; 3],
    tone: Option<&[[f32; 3]]>,
) -> ImageBuffer {
    if width_px <= 0.0 {
        return img.clone();
    }
    let default_tone = linear_carrier_tone();
    let tone = tone.unwrap_or(&default_tone);
    let profiles = carrier_profiles();
    let carrier = Carrier {
        width_px,
        rough,
        flare,
        corner,
        paper,
        tone,
        profiles: &profiles,
    };
    let mut out = vec![0.0; img.pixels.len()];
    finish::apply_carrier(&img.pixels, (img.height, img.width), &carrier, &mut out);
    ImageBuffer::from_rgb(img.width, img.height, out)
}
