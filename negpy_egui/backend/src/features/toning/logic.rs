//! Ports of `negpy/features/toning/logic.py` onto the `negpy_core` toning kernels.

use negpy_core::lab;
use negpy_core::toning::{Bath, Toning};

use crate::domain::color::working_lab;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// `TONING_CONSTANTS`: the six baths on a normal silver print.
#[must_use]
pub fn toning_constants() -> Toning {
    Toning {
        selenium: Bath {
            strength: 0.0,
            d_ref: 2.0,
            power: 1.5,
            gain: [1.04, 1.10, 1.02],
        },
        sepia: Bath {
            strength: 0.0,
            d_ref: 1.8,
            power: 2.0,
            gain: [0.82, 0.94, 1.12],
        },
        gold: Bath {
            strength: 0.0,
            d_ref: 1.6,
            power: 1.5,
            gain: [1.08, 1.03, 1.00],
        },
        gold_sepia_gain: [0.80, 0.95, 1.20],
        blue: Bath {
            strength: 0.0,
            d_ref: 0.9,
            power: 0.85,
            gain: [1.30, 1.00, 0.80],
        },
        copper: Bath {
            strength: 0.0,
            d_ref: 0.9,
            power: 0.6,
            gain: [0.72, 0.94, 1.18],
        },
        vanadium: Bath {
            strength: 0.0,
            d_ref: 1.8,
            power: 1.2,
            gain: [1.12, 0.85, 1.03],
        },
        gold_flat: false,
    }
}

/// `LITH_TONING_CONSTANTS`: the selenium and gold overrides for lith silver.
pub fn apply_lith_overrides(t: &mut Toning) {
    t.selenium.d_ref = 1.2;
    t.selenium.power = 1.0;
    t.selenium.gain = [1.10, 1.24, 1.12];
    t.gold.gain = [1.16, 1.06, 0.96];
    t.gold_flat = true;
}

/// `apply_chemical_toning`: no-op with every bath at zero.
#[must_use]
pub fn apply_chemical_toning(
    img: &ImageBuffer,
    selenium_strength: f64,
    sepia_strength: f64,
    gold_strength: f64,
    blue_strength: f64,
    copper_strength: f64,
    vanadium_strength: f64,
    lith_active: bool,
) -> ImageBuffer {
    if [
        selenium_strength,
        sepia_strength,
        gold_strength,
        blue_strength,
        copper_strength,
        vanadium_strength,
    ]
    .iter()
    .all(|strength| *strength == 0.0)
    {
        return img.clone();
    }
    let mut t = toning_constants();
    if lith_active {
        apply_lith_overrides(&mut t);
    }
    t.selenium.strength = selenium_strength;
    t.sepia.strength = sepia_strength;
    t.gold.strength = gold_strength;
    t.blue.strength = blue_strength;
    t.copper.strength = copper_strength;
    t.vanadium.strength = vanadium_strength;
    let mut out = vec![0.0; img.pixels.len()];
    negpy_core::toning::apply_chemical_toning(&img.pixels, &mut out, &t);
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `apply_split_toning`: Lab a*/b* tints, weighted to the shadows and highlights.
#[must_use]
pub fn apply_split_toning(
    img: &ImageBuffer,
    shadow_hue: f64,
    shadow_strength: f64,
    highlight_hue: f64,
    highlight_strength: f64,
) -> ImageBuffer {
    if shadow_strength == 0.0 && highlight_strength == 0.0 {
        return img.clone();
    }
    let tint = |hue: f64, strength: f64| {
        (strength > 0.0).then(|| {
            let radians = hue.to_radians();
            [
                radians.cos() * 20.0 * strength,
                radians.sin() * 20.0 * strength,
            ]
        })
    };
    let mut out = vec![0.0; img.pixels.len()];
    lab::split_toning(
        &img.pixels,
        (
            tint(shadow_hue, shadow_strength),
            tint(highlight_hue, highlight_strength),
        ),
        working_lab(),
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}
