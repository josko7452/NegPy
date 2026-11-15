//! The mutually exclusive lith, cyanotype and Sabattier print stages.
//!
//! Each stage reads the same `AltProcessConfig`; a processor gates itself on the film
//! mode and on the one process it owns. Mirrors `negpy/features/{lith,cyanotype,sabattier}`.

use negpy_core::altprocess::{Cyanotype, Interp, Lith, Sabattier};

use crate::domain::color::working_lab;
use crate::domain::models::{AltProcess, AltProcessConfig, ProcessMode, Sensitizer};
use crate::features::exposure::papers::{PaperProfile, effective_constants};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// `LITH_CONSTANTS` from `features/lith/logic.py`.
const LITH_FOOT_MAX: f32 = 0.70;
const LITH_FOOT_RATE: f32 = 0.60;
const LITH_FOOT_VEIL: f32 = 0.60;
const LITH_KNEE_LO: f32 = 1.25;
const LITH_KNEE_SPAN: f32 = 0.70;
const LITH_ABRUPT_LO: f32 = 0.30;
const LITH_ABRUPT_SPAN: f32 = 0.27;
const LITH_PATH_U: [f32; 4] = [0.10, 0.35, 0.65, 1.00];

/// `CYANOTYPE_CONSTANTS` from `features/cyanotype/logic.py`.
const CYANO_MID_COMPRESS: f32 = 0.45;
const CYANO_BLEACH_FLOOR: f32 = 0.15;
const CYANO_TANNIN_RESTORE: f32 = 1.05;
const CYANO_TANNIN_DMAX_GAIN: f32 = 0.15;
const CYANO_BROWN_DIR: [f32; 2] = [9.0, 20.0];
const CYANO_PATH_U: [f32; 4] = [0.00, 0.15, 0.55, 1.00];

/// Per-sensitizer Dmax and hue path.
const CLASSIC_D_MAX: f32 = 0.95;
const NEW_D_MAX: f32 = 1.40;
const CLASSIC_PATH: [[f32; 2]; 4] = [[0.5, 3.0], [-4.0, 6.0], [-7.0, -17.0], [-6.0, -25.0]];
const NEW_PATH: [[f32; 2]; 4] = [[0.3, 2.0], [-2.0, 2.0], [-9.0, -24.0], [-8.0, -34.0]];

/// `SABATTIER_CONSTANTS` from `features/sabattier/logic.py`.
pub const SABATTIER_FOLD_WIDTH: f32 = 0.08;
pub const SABATTIER_EDGE_WIDTH: f32 = 0.10;
pub const SABATTIER_STILL_REACH: f64 = 1.0;

fn path_interp(u: [f32; 4], anchors: [[f32; 2]; 4]) -> (Interp, Interp) {
    let a = std::array::from_fn(|index| anchors[index][0]);
    let b = std::array::from_fn(|index| anchors[index][1]);
    (Interp::new(u, a), Interp::new(u, b))
}

/// The lith stage, over a linear-reflectance print.
#[must_use]
pub fn apply_lith(
    img: &ImageBuffer,
    path: [[f64; 2]; 4],
    d_max: f64,
    enabled: bool,
    exposure: f64,
    snatch: f64,
    abruptness: f64,
) -> ImageBuffer {
    if !enabled {
        return img.clone();
    }
    let d_max = d_max as f32;
    let (a_path, b_path) = path_interp(LITH_PATH_U, path.map(|row| row.map(|value| value as f32)));
    let (to_lab, to_rgb) = working_lab();
    let _ = to_lab;
    let c = Lith {
        d_max,
        over: (0.301 * exposure) as f32,
        foot_max: LITH_FOOT_MAX,
        foot_veil: LITH_FOOT_VEIL,
        foot_rate: LITH_FOOT_RATE,
        knee: d_max * (LITH_KNEE_LO - LITH_KNEE_SPAN * snatch as f32),
        width: (LITH_ABRUPT_LO - LITH_ABRUPT_SPAN * abruptness as f32).max(0.01),
        a_path,
        b_path,
        lab_eps: 0.008856,
        lab_kappa: 7.787,
    };
    let mut out = vec![0.0; img.pixels.len()];
    negpy_core::altprocess::lith(&img.pixels, &c, to_rgb, &mut out);
    ImageBuffer::from_rgb(img.width, img.height, out)
}

fn sensitizer_table(sensitizer: Sensitizer) -> (f32, [[f32; 2]; 4]) {
    match sensitizer {
        Sensitizer::New => (NEW_D_MAX, NEW_PATH),
        Sensitizer::Classic => (CLASSIC_D_MAX, CLASSIC_PATH),
    }
}

/// The cyanotype stage, over a linear-reflectance print.
#[must_use]
pub fn apply_cyanotype(
    img: &ImageBuffer,
    sensitizer: Sensitizer,
    enabled: bool,
    exposure: f64,
    scale: f64,
    bleach: f64,
    tannin: f64,
) -> ImageBuffer {
    if !enabled {
        return img.clone();
    }
    let (d_max, anchors) = sensitizer_table(sensitizer);
    let (a_path, b_path) = path_interp(CYANO_PATH_U, anchors);
    let (_, to_rgb) = working_lab();
    let c = Cyanotype {
        d_max,
        over: (0.301 * exposure) as f32,
        scale: scale.max(0.1) as f32,
        mid_compress: CYANO_MID_COMPRESS,
        bleach: bleach as f32,
        bleach_floor: CYANO_BLEACH_FLOOR,
        tannin: tannin as f32,
        tannin_restore: CYANO_TANNIN_RESTORE,
        tannin_dmax_gain: CYANO_TANNIN_DMAX_GAIN,
        brown: CYANO_BROWN_DIR,
        a_path,
        b_path,
        lab_eps: 0.008856,
        lab_kappa: 7.787,
    };
    let mut out = vec![0.0; img.pixels.len()];
    negpy_core::altprocess::cyanotype(&img.pixels, &c, to_rgb, &mut out);
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// The Mackie line's blur sigma in render pixels, a fraction of the short side.
#[must_use]
pub fn line_sigma_px(agitation: f64, height: usize, width: usize) -> f64 {
    let reach = SABATTIER_STILL_REACH * (1.0 - agitation).clamp(0.0, 1.0);
    reach * 0.01 * height.min(width) as f64
}

/// The Sabattier stage, over a linear-reflectance print.
#[must_use]
pub fn apply_sabattier(
    img: &ImageBuffer,
    d_max: f64,
    enabled: bool,
    strength: f64,
    reexposure: f64,
    line_sigma_px: f64,
) -> ImageBuffer {
    if !enabled {
        return img.clone();
    }
    let c = Sabattier {
        re: (reexposure * d_max) as f32,
        fold_width: SABATTIER_FOLD_WIDTH,
        edge_width: SABATTIER_EDGE_WIDTH,
        strength: strength as f32,
        d_max: d_max as f32,
    };
    let kernel = if line_sigma_px > 0.0 {
        crate::features::lab::logic::gaussian_kernel_1d(line_sigma_px)
    } else {
        Vec::new()
    };
    let mut out = vec![0.0; img.pixels.len()];
    negpy_core::altprocess::sabattier(&img.pixels, (img.height, img.width), &c, &kernel, &mut out);
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// The stage tuple the engine runs in order: lith, cyanotype, then Sabattier.
#[must_use]
pub fn process(
    image: &ImageBuffer,
    config: &AltProcessConfig,
    mode: ProcessMode,
    paper: &PaperProfile,
) -> ImageBuffer {
    if mode != ProcessMode::BW {
        return image.clone();
    }
    let d_max = effective_constants(Some(paper)).d_max;
    let mut out = apply_lith(
        image,
        paper.lith_path,
        d_max,
        config.alt_process == AltProcess::Lith,
        config.lith_exposure,
        config.lith_snatch,
        config.lith_abruptness,
    );
    out = apply_cyanotype(
        &out,
        config.cyano_sensitizer,
        config.alt_process == AltProcess::Cyanotype,
        config.cyano_exposure,
        config.cyano_scale,
        config.cyano_bleach,
        config.cyano_tannin,
    );
    apply_sabattier(
        &out,
        d_max,
        config.alt_process == AltProcess::Sabattier,
        config.sabattier_strength,
        config.sabattier_reexposure,
        line_sigma_px(config.sabattier_agitation, image.height, image.width),
    )
}
