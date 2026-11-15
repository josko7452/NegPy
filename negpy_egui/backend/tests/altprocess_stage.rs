//! Ports the tonal rules in tests/test_lith_logic.py, test_cyanotype_logic.py and
//! test_sabattier_logic.py through the Rust stage entry points.

use negpy_backend::domain::models::{AltProcess, AltProcessConfig, ProcessMode, Sensitizer};
use negpy_backend::features::altprocess::{
    SABATTIER_EDGE_WIDTH, apply_cyanotype, apply_lith, apply_sabattier, line_sigma_px,
};
use negpy_backend::features::exposure::papers::{PAPER_PROFILES, resolve_paper};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

const D_MAX: f64 = 2.0;

/// A grey image whose columns sit at the given print densities.
fn ramp(densities: &[f32]) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(densities.len() * 3);
    for density in densities {
        let linear = 10f32.powf(-density);
        pixels.extend_from_slice(&[linear; 3]);
    }
    ImageBuffer::from_rgb(densities.len(), 1, pixels)
}

fn density(img: &ImageBuffer) -> Vec<f32> {
    img.pixels
        .chunks_exact(3)
        .map(|pixel| {
            let mean = (pixel[0] + pixel[1] + pixel[2]) / 3.0;
            -(mean.clamp(1e-6, 1.0).log10())
        })
        .collect()
}

/// A grey image whose columns sit at the given linear values.
fn linear(tones: &[f32]) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(tones.len() * 3);
    for tone in tones {
        pixels.extend_from_slice(&[*tone; 3]);
    }
    ImageBuffer::from_rgb(tones.len(), 1, pixels)
}

fn fomatone() -> &'static negpy_backend::features::exposure::papers::PaperProfile {
    resolve_paper("foma_fomatone")
}

fn lith(img: &ImageBuffer, exposure: f64, snatch: f64, abruptness: f64) -> ImageBuffer {
    apply_lith(
        img,
        fomatone().lith_path,
        D_MAX,
        true,
        exposure,
        snatch,
        abruptness,
    )
}

#[test]
fn a_disabled_alt_process_returns_the_input() {
    let img = ramp(&[0.2, 0.5, 0.9]);
    assert_eq!(
        apply_lith(&img, fomatone().lith_path, D_MAX, false, 0.0, 0.55, 0.6),
        img
    );
    assert_eq!(
        apply_cyanotype(&img, Sensitizer::Classic, false, 0.0, 1.4, 0.0, 0.0),
        img
    );
    assert_eq!(apply_sabattier(&img, D_MAX, false, 1.3, 0.45, 0.0), img);
}

#[test]
fn the_lith_curve_is_monotone_in_input_density() {
    let densities: Vec<f32> = (0..256).map(|index| index as f32 * 2.5 / 255.0).collect();
    let out = density(&lith(&ramp(&densities), 2.0, 0.55, 0.6));
    assert!(out.windows(2).all(|pair| pair[1] >= pair[0] - 1e-5));
}

#[test]
fn the_lith_curve_has_a_foot_a_knee_and_a_flat_band() {
    let densities: Vec<f32> = (0..512).map(|index| index as f32 * 2.5 / 511.0).collect();
    let out = density(&lith(&ramp(&densities), 2.0, 0.55, 1.0));
    let step = 2.5 / 511.0;
    let grad: Vec<f32> = out
        .windows(2)
        .map(|pair| (pair[1] - pair[0]) / step)
        .collect();
    let knee = grad
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| index)
        .unwrap();
    let foot = &grad[..knee / 2];
    assert!(foot.iter().cloned().fold(0.0, f32::max) < 0.6);
    assert!(grad[knee] > 10.0, "the knee separates from the foot");
    let band = &grad[knee + (grad.len() - knee) / 4..];
    assert!(band.iter().cloned().fold(0.0, f32::max) < 0.05);
}

#[test]
fn the_lith_over_exposure_veils_the_white() {
    let white = ramp(&[0.0, 0.0, 0.0, 0.0]);
    let clean = density(&lith(&white, 0.0, 0.55, 0.6));
    assert!(clean.iter().cloned().fold(0.0, f32::max) < 0.1);
    let veil = density(&lith(&white, 2.0, 0.55, 0.6));
    let peak = veil.iter().cloned().fold(0.0, f32::max);
    assert!((0.15..0.35).contains(&peak), "paper white sits at {peak}");
}

#[test]
fn the_cyanotype_scale_sets_where_the_print_blocks() {
    let densities: Vec<f32> = (0..512).map(|index| index as f32 * 2.6 / 511.0).collect();
    let short = density(&apply_cyanotype(
        &ramp(&densities),
        Sensitizer::Classic,
        true,
        0.0,
        1.0,
        0.0,
        0.0,
    ));
    let long = density(&apply_cyanotype(
        &ramp(&densities),
        Sensitizer::Classic,
        true,
        0.0,
        2.4,
        0.0,
        0.0,
    ));
    let tail = |values: &[f32]| {
        let start = values.len() / 2;
        let slice = &values[start..];
        slice.iter().cloned().fold(f32::MIN, f32::max)
            - slice.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!(tail(&short) < 1e-4);
    assert!(tail(&long) > 0.1);
}

#[test]
fn the_cyanotype_path_moves_from_green_to_blue() {
    let green = apply_cyanotype(&ramp(&[0.0]), Sensitizer::Classic, true, 0.0, 1.4, 0.0, 0.0);
    let blue = apply_cyanotype(&ramp(&[1.7]), Sensitizer::Classic, true, 0.0, 1.4, 0.0, 0.0);
    assert!(
        green.pixels[1] > green.pixels[2],
        "the highlight prints green"
    );
    assert!(blue.pixels[2] > blue.pixels[0], "the shadow prints blue");
}

#[test]
fn a_zero_strength_sabattier_leaves_the_print_alone() {
    let img = ImageBuffer::from_rgb(4, 4, vec![0.5; 48]);
    let out = apply_sabattier(&img, D_MAX, true, 0.0, 0.45, 0.0);
    for (before, after) in img.pixels.iter().zip(&out.pixels) {
        assert!((before - after).abs() < 1e-6);
    }
}

#[test]
fn the_sabattier_fold_reverses_the_light_tones_in_order() {
    let tones = [1.0f32, 0.7, 0.4, 0.2];
    let img = linear(&tones);
    let out = density(&apply_sabattier(&img, D_MAX, true, 1.3, 0.6, 0.0));
    assert!(
        out[0] > out[1] && out[1] > out[2],
        "white prints darkest after the flash"
    );
}

#[test]
fn the_sabattier_reversal_never_passes_paper_black() {
    let img = ImageBuffer::from_rgb(1, 1, vec![1.0, 1.0, 1.0]);
    let out = density(&apply_sabattier(&img, D_MAX, true, 2.0, 0.9, 0.0));
    assert!((out[0] - D_MAX as f32).abs() < 1e-3);
}

#[test]
fn agitation_shortens_the_mackie_line() {
    let still = line_sigma_px(0.0, 480, 640);
    let half = line_sigma_px(0.5, 480, 640);
    let none = line_sigma_px(1.0, 480, 640);
    assert!((still - 4.8).abs() < 1e-9);
    assert!((half - still / 2.0).abs() < 1e-9);
    assert_eq!(none, 0.0);
}

#[test]
fn the_stage_runs_only_on_a_bw_negative() {
    let img = ImageBuffer::from_rgb(4, 4, vec![0.5; 48]);
    let mut config = AltProcessConfig::default();
    config.alt_process = AltProcess::Sabattier;
    config.sabattier_strength = 1.3;
    let paper = fomatone();
    assert_eq!(
        negpy_backend::features::altprocess::process(&img, &config, ProcessMode::C41, paper),
        img
    );
    let out = negpy_backend::features::altprocess::process(&img, &config, ProcessMode::BW, paper);
    assert!(out.pixels != img.pixels);
}

#[test]
fn every_bw_profile_declares_a_lith_path() {
    for (key, profile) in PAPER_PROFILES.iter() {
        assert_eq!(profile.lith_path.len(), 4, "{key}");
    }
}

#[test]
fn the_sabattier_edge_width_is_the_python_constant() {
    assert_eq!(SABATTIER_EDGE_WIDTH, 0.10);
}
