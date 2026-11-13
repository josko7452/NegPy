//! Ports tests/test_fade_auto_red.py.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::process::fade::{FadeDensity, fade_density};
use negpy_backend::features::process::fade_auto_red::{
    AUTO_RED_LIGHT, AUTO_RED_MIN, AUTO_RED_SPAN, RANGE_PCT, auto_red, range_points,
};

fn points(span: f64, light: f64) -> Vec<f64> {
    (0..RANGE_PCT.len())
        .map(|index| light + span * index as f64 / (RANGE_PCT.len() - 1) as f64)
        .collect()
}

fn scaled(points: Vec<f64>, old: f64, pivot: f64) -> impl Fn(f64) -> Option<Vec<f64>> {
    move |new| {
        Some(
            points
                .iter()
                .map(|point| pivot + (point - pivot) * old / new)
                .collect(),
        )
    }
}

fn grid(value: f32) -> Vec<[f32; 3]> {
    (0..64 * 64)
        .map(|index| {
            let x = 0.2 + 1.2 * index as f32 / (64.0 * 64.0);
            let _ = value;
            [x, x, x]
        })
        .collect()
}

#[test]
fn the_points_are_mean_density_quantiles_light_end_first() {
    let points = range_points(&grid(0.0)).unwrap();
    assert_eq!(points.len(), RANGE_PCT.len());
    assert!(points.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!((points[0] - 0.2).abs() < 0.03);
    assert!((points[12] - 1.4).abs() < 0.03);
    assert!(range_points(&vec![[0.5; 3]; 16 * 16]).is_none());
}

#[test]
fn auto_red_scales_the_span_onto_the_adaptive_span_and_holds_its_bounds() {
    let got = auto_red(scaled(points(1.2, 0.3), 0.9, 0.0), 0.9, 0.0).unwrap();
    assert!((got.red - 0.9 * 1.2 / AUTO_RED_SPAN).abs() < 0.006);
    assert_eq!(
        auto_red(scaled(points(0.3, 0.3), 1.0, 0.0), 1.0, 0.0)
            .unwrap()
            .red,
        AUTO_RED_MIN
    );
    assert_eq!(
        auto_red(scaled(points(2.0, 0.3), 0.6, 0.0), 0.6, 0.0)
            .unwrap()
            .red,
        0.75
    );
    assert_eq!(
        auto_red(scaled(points(4.0, 0.3), 1.0, 0.0), 1.0, 0.0)
            .unwrap()
            .red,
        1.0
    );
    assert!(auto_red(scaled(points(0.01, 0.3), 1.0, 0.0), 1.0, 0.0).is_none());
    assert!(auto_red(|_| None, 1.0, 0.0).is_none());
}

#[test]
fn auto_red_shifts_density_by_what_the_median_gains() {
    let points = points(1.2, 1.0);
    let gain = 1.6 / 0.75 - 1.6;
    let got = auto_red(scaled(points.clone(), 1.0, 0.0), 1.0, 0.0).unwrap();
    assert_eq!(got.red, 0.75);
    assert!((got.shift - (-gain)).abs() < 1e-4);
    let shifted = auto_red(scaled(points, 1.0, 0.0), 1.0, 0.1).unwrap();
    assert!((shifted.shift - (0.1 - gain)).abs() < 1e-4);
}

#[test]
fn auto_red_keeps_the_light_end_off_white() {
    let got = auto_red(scaled(points(1.2, 0.3), 1.0, 0.0), 1.0, 0.0).unwrap();
    assert!((got.shift - (AUTO_RED_LIGHT - 0.3 / 0.75)).abs() < 1e-4);
    let light = auto_red(scaled(points(1.2, 0.1), 1.0, 0.0), 1.0, 0.0).unwrap();
    assert!((light.shift - (0.1 - 0.1 / 0.75)).abs() < 1e-4);
    let centred = auto_red(scaled(points(1.2, -0.6), 1.0, 0.0), 1.0, 0.0).unwrap();
    assert!((centred.shift - (0.6 / 0.75 - 0.6)).abs() < 1e-4);
}

#[test]
fn the_density_shift_adds_density_to_every_channel_after_the_fade() {
    let base = ProcessConfig {
        process_mode: ProcessMode::E6,
        fade_ratio_r: 0.5,
        ..ProcessConfig::default()
    };
    let plain = fade_density(&base, "");
    let mut shifted_base = base.clone();
    shifted_base.fade_density_shift = 0.3;
    let shifted = fade_density(&shifted_base, "");
    assert_eq!(shifted.matrix, plain.matrix);
    assert_eq!(shifted.offset, [-0.3, -0.3, -0.3]);
    let negative = ProcessConfig {
        process_mode: ProcessMode::C41,
        fade_density_shift: 0.3,
        ..ProcessConfig::default()
    };
    let density = fade_density(&negative, "");
    assert_eq!(density.matrix, None);
    assert_eq!(density.offset, [0.0; 3]);
}
