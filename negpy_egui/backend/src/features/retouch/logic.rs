//! Ports of the detection and IR helpers in `negpy/features/retouch/logic.py`.
//!
//! Repair and clone land in the next commit; this module owns the measured inputs they read.

use negpy_core::retouch::{self, CleanBase, DustStats, GammaFit, LumaDetect, RatioNorm};

use crate::domain::models::RetouchConfig;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const HEAL_SIZE_REF: f32 = 1600.0;
pub const HEAL_SIZE_MIN: f32 = 2.0;
pub const HEAL_SIZE_MAX: f32 = 64.0;

pub const PROXY_MIN_SPREAD: f64 = 0.8;
pub const DETECT_PAD_PX: f64 = 2.5;
pub const DETECT_AVG_PX: usize = 3;
pub const DETECT_MAD_GAIN: f32 = 4.0;
pub const DETECT_SIGMA_MIN: f32 = 0.003;
pub const DETECT_Z_LOOSE: f64 = 3.0;
pub const DETECT_Z_DEFAULT: f64 = 9.0;
pub const DETECT_Z_TIGHT: f64 = 48.0;
pub const DETECT_DEFAULT_POS: f64 = 0.66;
pub const DETECT_Z_GROW: f64 = 2.5;
pub const DETECT_Z_GROW_FRAC: f64 = 0.3;
pub const DETECT_GROW_REACH: f64 = 2.0;
pub const DETECT_TEXTURE_KNEE: f32 = 0.02;
pub const DETECT_TEXTURE_GAIN: f32 = 40.0;
pub const DETECT_HAIR_TEXTURE_KNEE: f64 = 0.075;
pub const DETECT_HAIR_TEXTURE_GAIN: f64 = 150.0;
pub const DETECT_HAIR_FILL_GAIN: f32 = 2.0;
pub const DETECT_PROXY_MIN: f32 = 0.15;

pub const SCRATCH_FINE_PX: f32 = 1.2;
pub const SCRATCH_BROAD_PX: f32 = 9.0;
pub const SCRATCH_NOISE_WIN: usize = 151;
pub const SCRATCH_SLOPE_MAX: f64 = 0.02;
pub const SCRATCH_SLOPE_STEP: f64 = 0.00025;
pub const SCRATCH_SEARCH_ROWS: i64 = 30;
pub const SCRATCH_CLICK_PULL: f64 = 12.0;
pub const SCRATCH_Z_LOOSE: f64 = 0.4;
pub const SCRATCH_Z_TIGHT: f64 = 1.6;
pub const SCRATCH_RUN_WIN: usize = 151;
pub const SCRATCH_RUN_FRAC: f32 = 0.35;
pub const SCRATCH_MIN_EVIDENCE: f64 = 0.25;
pub const SCRATCH_WIDTH_MAX: f64 = 14.0;
pub const SCRATCH_WIDTH_MIN: f64 = 3.0;
pub const SCRATCH_RIDGE_REACH: i64 = 128;

pub const IR_MAX_UPSAMPLE: f64 = 1.5;
pub const IR_DETECT_MAX: usize = 3600;
pub const IR_DETECT_REF: f64 = 1600.0;
pub const IR_BASE_WIN: usize = 25;
pub const IR_GAIN_IDENTITY: f64 = 0.97;
pub const IR_GAIN_CLAMP: f64 = 2.0;
pub const IR_GAMMA_LO: f64 = 1.0;
pub const IR_GAMMA_HI: f64 = 2.2;
pub const IR_GAMMA_FALLBACK: f64 = 1.5;
pub const IR_DEAD_FLOOR: f32 = 0.05;
pub const IR_DEAD_MIN_AREA: f64 = 0.002;
pub const IR_NOISE_SIGMA: f64 = 3.0;
pub const IR_PIVOT_LO: f64 = 0.60;
pub const IR_REF_SIGMA: f64 = 0.02;
pub const IR_SCALE_MAX: f64 = 6.0;
pub const IR_GLARE_SIGMA: f64 = 4.0;
pub const IR_XTALK_MAX: f64 = 0.8;
pub const IR_XTALK_MIN: f64 = 0.02;
pub const IR_DEGENERATE_GHOST: f64 = 0.5;
pub const IR_XTALK_TRIM: f64 = 5.0;
pub const IR_FIT_FLAT_PCT: f64 = 40.0;
pub const IR_FIT_MIN_PX: usize = 200;
pub const IR_FIT_MAX_PX: usize = 200_000;
pub const IR_CAP_WIN: usize = 9;
pub const IR_CAP_SIGMA: f32 = 1.0;
pub const IR_CAP_MIN_SUPPORT: f32 = 0.1;
pub const IR_SCORE_FLOOR: f64 = 0.02;
pub const IR_ROUTE_RADIUS: usize = 5;
pub const IR_ROUTE_DILATE: usize = 2;
pub const IR_ROUTE_BUDGET: f64 = 0.02;
pub const REPAIR_MAX_COMPONENTS: usize = 256;
pub const HAIR_MIN_AREA: usize = 20;
pub const HAIR_MIN_ELONG: f64 = 8.0;
pub const HAIR_DILATE_PX: i64 = 1;
pub const HAIR_INPAINT_RADIUS: i64 = 3;
pub const HAIR_INPAINT_GAMMA: f64 = 2.2;
pub const HAIR_INPAINT_PAD: usize = 16;
pub const HAIR_TILE_PX: usize = 1024;
pub const HAIR_TILE_HALO: usize = 32;
pub const HAIR_RANGE_SAMPLE_PX: usize = 1 << 20;

/// `_density`: log density of luminance, exposure-invariant.
#[must_use]
pub fn density(img: &ImageBuffer) -> Vec<f32> {
    let mut lum = vec![0.0; img.width * img.height];
    negpy_core::analysis::luminance(&img.pixels, &mut lum);
    let mut out = vec![0.0; lum.len()];
    negpy_core::analysis::log_density_open(&lum, &mut out);
    for value in &mut out {
        *value = -*value;
    }
    out
}

/// `film_scale`: pixels per film-footprint unit.
#[must_use]
pub fn film_scale(height: usize, width: usize) -> f64 {
    (height.max(width) as f64 / IR_DETECT_REF).max(1.0)
}

/// `detect_bar`: the slider's seed bar in local sigma.
#[must_use]
pub fn detect_bar(slider: f64) -> f64 {
    let s = slider.clamp(0.0, 1.0);
    if s >= 1.0 {
        return f64::INFINITY;
    }
    if s <= DETECT_DEFAULT_POS {
        return DETECT_Z_LOOSE + (DETECT_Z_DEFAULT - DETECT_Z_LOOSE) * s / DETECT_DEFAULT_POS;
    }
    let t = (s - DETECT_DEFAULT_POS) / (1.0 - DETECT_DEFAULT_POS);
    DETECT_Z_DEFAULT * (DETECT_Z_TIGHT / DETECT_Z_DEFAULT).powf(t)
}

/// `scratch_detect_bar`.
#[must_use]
pub fn scratch_detect_bar(slider: f64) -> f64 {
    let s = slider.clamp(0.0, 1.0);
    SCRATCH_Z_LOOSE + (SCRATCH_Z_TIGHT - SCRATCH_Z_LOOSE) * s
}

/// `ir_detect_cutoff`.
#[must_use]
pub fn ir_detect_cutoff(slider: f64, attenuation: bool) -> f64 {
    let s = slider.clamp(0.0, 1.0);
    if attenuation {
        0.85 - 0.40 * s
    } else {
        0.95 - 0.20 * s
    }
}

/// `ir_detect_target`.
#[must_use]
pub fn ir_detect_target(buffer_long_edge: usize, preview_long_edge: usize) -> usize {
    let want = (buffer_long_edge as f64 / IR_MAX_UPSAMPLE).ceil() as usize;
    preview_long_edge
        .max(want)
        .min(IR_DETECT_MAX)
        .min(buffer_long_edge)
}

/// `_ir_detect_scale`.
#[must_use]
pub fn ir_detect_scale(height: usize, width: usize) -> f64 {
    (height.max(width) as f64 / IR_DETECT_REF).max(1.0)
}

/// `_ir_win`: a pinned footprint as an odd window on this plane.
#[must_use]
pub fn ir_win(px: usize, scale: f64) -> usize {
    (px as f64 * scale).round() as usize | 1
}

/// `_texture_win`.
#[must_use]
pub fn texture_win(height: usize, width: usize, dust_size: i64) -> usize {
    (7.0f64.max(dust_size.max(1) as f64 * film_scale(height, width) * 4.0) as usize) * 2 + 1
}

/// `compute_dust_stats`: `(proxy, background, z, texture)`.
#[must_use]
pub fn compute_dust_stats(img: &ImageBuffer, dust_size: i64) -> Option<[Vec<f32>; 4]> {
    let shape = (img.height, img.width);
    let base_size = dust_size.max(1) as f64 * film_scale(shape.0, shape.1);
    let v_win = (3u64.max((base_size * 3.0) as u64) as usize) * 2 + 1;
    let side = (2 * (base_size.round() as usize) + 1).max(1);
    let disk = negpy_core::morph::ellipse_kernel(side, side);
    let c = DustStats {
        v_win,
        w_win: texture_win(shape.0, shape.1, dust_size),
        disk: negpy_core::morph::Kernel {
            mask: &disk,
            w: side,
            h: side,
        },
        min_spread: PROXY_MIN_SPREAD,
        avg_px: DETECT_AVG_PX,
        mad_gain: DETECT_MAD_GAIN,
        sigma_min: DETECT_SIGMA_MIN,
    };
    retouch::dust_stats(&density(img), shape, &c)
}

/// `detect_luma_score`: a fill score for specks and an inpaint mask for hairs.
#[must_use]
pub fn detect_luma_score(
    img: &ImageBuffer,
    dust_threshold: f64,
    dust_size: i64,
    stats: Option<&[Vec<f32>; 4]>,
    hair_threshold: Option<f64>,
) -> Option<(Option<Vec<f32>>, Option<Vec<u8>>)> {
    let owned;
    let planes = match stats {
        Some(s) => s,
        None => {
            owned = compute_dust_stats(img, dust_size)?;
            &owned
        }
    };
    let proxy = planes[0].as_slice();
    let hi = detect_bar(dust_threshold);
    let hi_hair = hair_threshold.map_or(hi, detect_bar);
    let scale = film_scale(img.height, img.width);
    let lo = DETECT_Z_GROW.max(hi.min(hi_hair) * DETECT_Z_GROW_FRAC);
    let reach = (DETECT_GROW_REACH * dust_size.max(1) as f64 * scale).round() as usize;
    let side = (2 * reach + 1).max(1);
    let disk = negpy_core::morph::ellipse_kernel(side, side);
    let c = LumaDetect {
        hi,
        hi_hair,
        lo,
        proxy_min: DETECT_PROXY_MIN,
        reach_disk: negpy_core::morph::Kernel {
            mask: &disk,
            w: side,
            h: side,
        },
        texture_knee: DETECT_TEXTURE_KNEE,
        texture_gain: DETECT_TEXTURE_GAIN,
        hair: (HAIR_MIN_AREA, HAIR_MIN_ELONG),
        hair_fill_gain: DETECT_HAIR_FILL_GAIN,
        hair_texture: (DETECT_HAIR_TEXTURE_KNEE, DETECT_HAIR_TEXTURE_GAIN),
        texture_win: texture_win(img.height, img.width, dust_size),
        pad_px: DETECT_PAD_PX * scale,
        score_floor: IR_SCORE_FLOOR,
    };
    let _ = proxy;
    retouch::detect_luma_score(
        [&planes[0], &planes[1], &planes[2], &planes[3]],
        (img.height, img.width),
        &c,
    )
}

/// `luma_bake_token` and `ir_bake_token`: the cache identity of the source bakes.
#[must_use]
pub fn luma_bake_token(config: &RetouchConfig) -> String {
    if !config.dust_remove {
        return String::new();
    }
    let exclusion = format!("ex{}", config.dust_exclusion_strokes.len());
    format!(
        "|dust{}_{}_{}{}",
        round3(config.dust_threshold),
        round3(config.dust_hair_threshold),
        config.dust_size,
        exclusion
    )
}

/// `ir_bake_token`.
#[must_use]
pub fn ir_bake_token(config: &RetouchConfig, has_ir: bool) -> String {
    if !(config.ir_dust_remove && has_ir) {
        return String::new();
    }
    format!(
        "|ir{}r{}",
        i32::from(config.ir_attenuation),
        round3(config.ir_threshold)
    )
}

fn round3(value: f64) -> String {
    format!("{value:.3}")
}

/// `_ir_live`: everything but the below-floor holder regions.
#[must_use]
pub fn ir_live(plane: &[f32], height: usize, width: usize) -> Vec<bool> {
    retouch::ir_live(plane, (height, width), IR_DEAD_FLOOR, IR_DEAD_MIN_AREA)
}

/// `_ir_glare`: pixels far over the live median.
#[must_use]
pub fn ir_glare(plane: &[f32], live: &[bool]) -> Option<Vec<bool>> {
    retouch::ir_glare(plane, live, IR_FIT_MAX_PX, IR_GLARE_SIGMA)
}

/// `downsample_ir`: a min-preserving area downsample.
#[must_use]
pub fn downsample_ir(
    plane: &[f32],
    height: usize,
    width: usize,
    target_long_edge: usize,
    dims: Option<(usize, usize)>,
) -> Vec<f32> {
    let long_edge = height.max(width);
    let (dw, dh) = match dims {
        Some(d) => d,
        None => {
            if long_edge <= target_long_edge {
                return plane.to_vec();
            }
            let s = target_long_edge as f64 / long_edge as f64;
            (
                ((width as f64 * s).round() as usize).max(1),
                ((height as f64 * s).round() as usize).max(1),
            )
        }
    };
    if (dw, dh) == (width, height) {
        return plane.to_vec();
    }
    let k = (long_edge as f64 / dw.max(dh) as f64).round() as usize | 1;
    let source = if k > 1 {
        let mut eroded = vec![0.0; plane.len()];
        negpy_core::morph::morphology(
            plane,
            &mut eroded,
            (height, width, 1),
            negpy_core::morph::Op::Erode,
            (k, k),
            1,
            negpy_core::filter::Border::Constant,
        )
        .expect("the kernel covers the plane");
        eroded
    } else {
        plane.to_vec()
    };
    negpy_core::resize::resize_area(&source, height, width, 1, dh, dw).expect("positive dimensions")
}

/// `normalize_ir`.
#[must_use]
pub fn normalize_ir(
    plane: &[f32],
    height: usize,
    width: usize,
    glare: Option<&[bool]>,
) -> Vec<f32> {
    let win = ir_win(IR_BASE_WIN, ir_detect_scale(height, width));
    let mut out = vec![0.0; height * width];
    retouch::normalize_ir(plane, glare, (height, width), win, &mut out)
        .expect("the plane matches its size");
    out
}

/// `ir_defect_score`.
#[must_use]
pub fn ir_defect_score(ratio: &[f32], height: usize, width: usize, cutoff: f64) -> Vec<f32> {
    let span = (IR_GAIN_IDENTITY - cutoff).max(1e-4) as f32;
    let mut out = vec![0.0; height * width];
    retouch::ir_defect_score(
        ratio,
        (height, width),
        (
            cutoff as f32,
            span,
            (1.0 - IR_SCORE_FLOOR) as f32,
            IR_SCORE_FLOOR as f32,
        ),
        &mut out,
    )
    .expect("the ratio matches its size");
    out
}

fn ir_decontaminate(
    ratio: &[f32],
    vis_log: &[f32],
    _height: usize,
    _width: usize,
) -> (Vec<f32>, f64) {
    if ratio.len() < 500 {
        return (ratio.to_vec(), 0.0);
    }
    let cut = negpy_core::stats::percentiles(ratio, &[IR_XTALK_TRIM])
        .and_then(|p| p.first().copied())
        .unwrap_or(0.0);
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (i, &r) in ratio.iter().enumerate() {
        if r >= cut {
            let v = &vis_log[i * 3..i * 3 + 3];
            xs.push([v[0] as f64, v[1] as f64, v[2] as f64, 1.0]);
            ys.push(r.clamp(1e-4, 1.0).ln() as f64);
        }
    }
    if ys.len() < 500 {
        return (ratio.to_vec(), 0.0);
    }
    let b = least_squares4(&xs, &ys)
        .map(|v| v.map(|x| x.clamp(0.0, IR_XTALK_MAX)))
        .unwrap_or([0.0; 4]);
    let ghost = b[0].abs() + b[1].abs() + b[2].abs();
    if ghost < IR_XTALK_MIN {
        return (ratio.to_vec(), ghost);
    }
    let mut out = vec![0.0; ratio.len()];
    retouch::ir_ghost_divide(
        ratio,
        vis_log,
        [b[0] as f32, b[1] as f32, b[2] as f32],
        &mut out,
    );
    (out, ghost)
}

fn least_squares4(xs: &[[f64; 4]], ys: &[f64]) -> Option<[f64; 4]> {
    let mut a = [[0.0f64; 4]; 4];
    let mut q = [0.0f64; 4];
    for (row, y) in xs.iter().zip(ys) {
        for i in 0..4 {
            q[i] += row[i] * y;
            for j in 0..4 {
                a[i][j] += row[i] * row[j];
            }
        }
    }
    for i in 0..4 {
        let mut pivot = i;
        for j in i + 1..4 {
            if a[j][i].abs() > a[pivot][i].abs() {
                pivot = j;
            }
        }
        if a[pivot][i].abs() < 1e-12 {
            return None;
        }
        a.swap(i, pivot);
        q.swap(i, pivot);
        for j in i + 1..4 {
            let f = a[j][i] / a[i][i];
            for k in i..4 {
                a[j][k] -= f * a[i][k];
            }
            q[j] -= f * q[i];
        }
    }
    let mut b = [0.0f64; 4];
    for i in (0..4).rev() {
        let mut sum = q[i];
        for j in i + 1..4 {
            sum -= a[i][j] * b[j];
        }
        b[i] = sum / a[i][i];
    }
    Some(b)
}

/// `_ir_normalize_ratio`.
#[must_use]
pub fn ir_normalize_ratio(
    ratio: &[f32],
    live: &[bool],
    pivot_log: Option<&mut f64>,
) -> (Vec<f32>, Option<(f64, f64)>) {
    let mut out = ratio.to_vec();
    let c = RatioNorm {
        identity: IR_GAIN_IDENTITY,
        noise_sigma: IR_NOISE_SIGMA,
        pivot_lo: IR_PIVOT_LO,
        ref_sigma: IR_REF_SIGMA,
        scale_max: IR_SCALE_MAX,
    };
    let measured = retouch::ir_normalize_ratio(&mut out, live, &c);
    if let (Some(log), Some((pivot, sigma))) = (pivot_log.as_deref(), measured) {
        let _ = (log, pivot, sigma);
    }
    (out, measured)
}

/// `ir_ratio_and_gain`: detection-scale ratio and per-channel gain.
#[must_use]
pub fn ir_ratio_and_gain(
    ir: &[f32],
    img: &ImageBuffer,
    ir_height: usize,
    ir_width: usize,
) -> (Vec<f32>, Vec<f32>, bool, [f64; 3]) {
    let live = ir_live(ir, ir_height, ir_width);
    let glare = ir_glare(ir, &live);
    let mut ratio = normalize_ir(ir, ir_height, ir_width, glare.as_deref());
    for (i, keep) in live.iter().enumerate() {
        if !keep {
            ratio[i] = 1.0;
        }
    }
    let mut img_det = img.pixels.clone();
    let (mut ih, mut iw) = (img.height, img.width);
    if (ih, iw) != (ir_height, ir_width) {
        img_det = negpy_core::resize::resize_area(&img_det, ih, iw, 3, ir_height, ir_width)
            .expect("positive dimensions");
        ih = ir_height;
        iw = ir_width;
    }
    let win = ir_win(IR_BASE_WIN, ir_detect_scale(ih, iw));
    let mut vis_log = vec![0.0; ih * iw * 3];
    retouch::ir_vis_log(&img_det, (ih, iw), win, &mut vis_log).expect("the image matches its size");
    let (clean_ratio, ghost) = ir_decontaminate(&ratio, &vis_log, ir_height, ir_width);
    let degenerate = ghost > IR_DEGENERATE_GHOST;
    let (clean_ratio, _) = ir_normalize_ratio(&clean_ratio, &live, None);
    let gammas = fit_refraction_gammas(&clean_ratio, &vis_log, &img_det, ih, iw);
    let clean = ir_clean_base(&img_det, &clean_ratio, ih, iw);
    let mut gain = vec![0.0; img_det.len()];
    retouch::ir_gain(
        &clean_ratio,
        &img_det,
        &clean,
        gammas,
        (IR_GAIN_IDENTITY as f32, IR_GAIN_CLAMP as f32),
        &mut gain,
    );
    (clean_ratio, gain, degenerate, gammas)
}

fn fit_refraction_gammas(
    ratio: &[f32],
    vis_log: &[f32],
    img: &[f32],
    height: usize,
    width: usize,
) -> [f64; 3] {
    let c = GammaFit {
        band: (0.70, 0.92),
        flat_pct: IR_FIT_FLAT_PCT,
        min_flat: IR_FIT_MIN_PX,
        cap: IR_FIT_MAX_PX,
        fallback: IR_GAMMA_FALLBACK,
        range: (IR_GAMMA_LO as f32, IR_GAMMA_HI as f32),
    };
    retouch::fit_refraction_gammas(ratio, vis_log, img, (height, width), &c)
        .unwrap_or([IR_GAMMA_FALLBACK; 3])
}

fn ir_clean_base(img: &[f32], ratio: &[f32], height: usize, width: usize) -> Vec<f32> {
    let win = ir_win(IR_CAP_WIN, ir_detect_scale(height, width));
    let c = CleanBase {
        win,
        identity: IR_GAIN_IDENTITY as f32,
        sigma: IR_CAP_SIGMA,
        min_support: IR_CAP_MIN_SUPPORT,
    };
    let mut out = vec![0.0; img.len()];
    retouch::ir_clean_base(img, ratio, (height, width), &c, &mut out)
        .expect("the image matches its size");
    out
}

/// `apply_ir_attenuation`: the visible buffer times the upsampled gain.
#[must_use]
pub fn apply_ir_attenuation(
    img: &ImageBuffer,
    gain: &[f32],
    gain_height: usize,
    gain_width: usize,
) -> ImageBuffer {
    let gain = if (gain_height, gain_width) == (img.height, img.width) {
        gain.to_vec()
    } else {
        let mut out = vec![0.0; img.pixels.len()];
        negpy_core::resize::resize_linear(
            gain,
            (gain_height, gain_width, 3),
            (img.height, img.width),
            false,
            &mut out,
        )
        .expect("the gain matches its size");
        out
    };
    let mut pixels = vec![0.0; img.pixels.len()];
    negpy_core::convert::multiply(&img.pixels, &gain, &mut pixels);
    ImageBuffer::from_rgb(img.width, img.height, pixels)
}

/// `repair_coverage`: the share each route rewrote.
#[must_use]
pub fn repair_coverage(
    ir_mask: Option<&[u8]>,
    dust_mask: Option<&[f32]>,
    hair_masks: &[Option<Vec<u8>>],
) -> (f64, f64, f64) {
    let frac_u8 = |mask: Option<&[u8]>| {
        mask.map_or(0.0, |m| {
            m.iter().filter(|v| **v != 0).count() as f64 / m.len().max(1) as f64
        })
    };
    let frac_f32 = |mask: Option<&[f32]>| {
        mask.map_or(0.0, |m| {
            m.iter().filter(|v| **v < 1.0).count() as f64 / m.len().max(1) as f64
        })
    };
    let mut by_len = std::collections::BTreeMap::new();
    for mask in hair_masks.iter().flatten() {
        let entry = by_len.entry(mask.len()).or_insert(vec![0u8; mask.len()]);
        for (d, &s) in entry.iter_mut().zip(mask) {
            *d |= u8::from(s != 0);
        }
    }
    let painted = by_len
        .values()
        .map(|m| frac_u8(Some(m)))
        .fold(0.0f64, f64::max);
    (frac_u8(ir_mask), frac_f32(dust_mask), painted)
}

/// `_scratch_ridge`.
#[must_use]
pub fn scratch_ridge(img: &ImageBuffer) -> Vec<f32> {
    let mut out = vec![0.0; img.width * img.height];
    retouch::scratch_ridge(
        &density(img),
        (img.height, img.width),
        (
            SCRATCH_FINE_PX as f64,
            SCRATCH_BROAD_PX as f64,
            SCRATCH_NOISE_WIN,
        ),
        &mut out,
    )
    .expect("the plane covers the frame");
    out
}

/// `_scratch_ridge_rows`: the ridge on a band that includes the requested rows.
#[must_use]
pub fn scratch_ridge_rows(img: &ImageBuffer, r0: i64, r1: i64) -> (Vec<f32>, usize, usize) {
    let top = (r0 - SCRATCH_RIDGE_REACH).max(0) as usize;
    let bottom = (r1 + SCRATCH_RIDGE_REACH).min(img.height as i64) as usize;
    let band = ImageBuffer::from_rgb(
        img.width,
        bottom.saturating_sub(top),
        img.pixels[top * img.width * 3..bottom * img.width * 3].to_vec(),
    );
    (scratch_ridge(&band), top, bottom)
}

/// `trace_scratch`: one click to a normalized line and width, or None.
#[must_use]
pub fn trace_scratch(
    img: &ImageBuffer,
    nx: f64,
    ny: f64,
    threshold: f64,
) -> Option<(f64, f64, f64, f64, f64)> {
    let (h, w) = (img.height, img.width);
    let (cx, cy) = (nx * w as f64, ny * h as f64);
    let bar = scratch_detect_bar(threshold);
    let y0 = (cy as i64 - SCRATCH_SEARCH_ROWS).max(0);
    let y1 = (cy as i64 + SCRATCH_SEARCH_ROWS).min(h as i64);
    if y1 - y0 < 3 {
        return None;
    }
    let scale = film_scale(h, w);
    let max_half = (0.5 * SCRATCH_WIDTH_MAX * scale).round().max(1.0) as i64;
    let reach = max_half + (SCRATCH_SLOPE_MAX * w as f64).ceil() as i64 + 2;
    let (z, z0, _) = scratch_ridge_rows(img, y0 - reach, y1 + reach);
    let band_rows = scratch_band_rows(&z, w);
    let rows = (y0 - z0 as i64) as usize..(y1 - z0 as i64) as usize;
    if rows.end > band_rows {
        return None;
    }
    let pull: Vec<f64> = (y0..y1)
        .map(|y| (-0.5 * ((y as f64 - cy) / SCRATCH_CLICK_PULL).powi(2)).exp())
        .collect();
    let mut slopes = Vec::new();
    let mut slope = -SCRATCH_SLOPE_MAX;
    while slope <= SCRATCH_SLOPE_MAX + 1e-9 {
        slopes.push(slope);
        slope += SCRATCH_SLOPE_STEP;
    }
    let found = retouch::scratch_slope_search(&z, (band_rows, w), cx, rows, &pull, &slopes)?;
    if found.0 < SCRATCH_MIN_EVIDENCE {
        return None;
    }
    let (_, index, k, along) = found;
    let slope = slopes[index];
    let sign = if along.iter().sum::<f32>() >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let on: Vec<f32> = along
        .iter()
        .map(|v| f32::from(v * sign > bar as f32))
        .collect();
    let mut run = vec![0.0; on.len()];
    negpy_core::filter::box_filter(
        &on,
        &mut run,
        (1, on.len(), 1),
        (SCRATCH_RUN_WIN, 1),
        true,
        negpy_core::filter::Border::Reflect101,
    )?;
    let run_on: Vec<usize> = run
        .iter()
        .enumerate()
        .filter(|(_, v)| **v >= SCRATCH_RUN_FRAC)
        .map(|(i, _)| i)
        .collect();
    if run_on.is_empty() {
        return None;
    }
    let x0 = run_on[0];
    let x1 = *run_on.last().unwrap_or(&x0);
    let row = y0 + k as i64;
    Some((
        x0 as f64 / w as f64,
        (row as f64 + slope * (x0 as f64 - cx)) / h as f64,
        x1 as f64 / w as f64,
        (row as f64 + slope * (x1 as f64 - cx)) / h as f64,
        SCRATCH_WIDTH_MIN,
    ))
}

fn scratch_band_rows(z: &[f32], w: usize) -> usize {
    z.len() / w.max(1)
}
