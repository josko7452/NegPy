//! Ports of the normalization and bounds assertions in
//! `tests/test_batch_norm.py` and `tests/test_exposure_bounds.py`.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::features::exposure::processor::{
    NormalizationSettings, luma_source_bounds, normalize_negative, resolve_bounds,
};

fn gradient(width: usize, height: usize) -> Vec<f32> {
    let count = width * height;
    (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.98 * index as f32 / (count - 1) as f32;
            [value, value * 0.92, value * 0.84]
        })
        .collect()
}

fn bounds() -> LogNegativeBounds {
    LogNegativeBounds::new([-1.8, -1.9, -2.0], [-0.1, -0.2, -0.3])
}

#[test]
fn a_locked_luma_and_color_baseline_skips_the_analyzer() {
    let mut process = ProcessConfig::default();
    process.local_floors = [-1.0, -1.0, -1.0];
    process.locked_floors = [-2.0, -2.0, -2.0];
    process.locked_ceils = [-0.2, -0.2, -0.2];
    process.use_luma_average = true;
    process.use_color_average = true;
    let (final_bounds, base) = resolve_bounds(&process, || panic!("analyzer must not run"));
    assert_eq!(final_bounds, LogNegativeBounds::new([-2.0; 3], [-0.2; 3]));
    assert_eq!(base, final_bounds);
}

#[test]
fn a_manual_pick_pair_replaces_the_analyzed_ends() {
    let mut process = ProcessConfig::default();
    process.manual_dmin = Some(vec![-0.05, -0.06, -0.07]);
    process.manual_dmax = Some(vec![-1.5, -1.6, -1.7]);
    let (_, base) = resolve_bounds(&process, || bounds());
    assert_eq!(
        base,
        LogNegativeBounds::new([-1.5, -1.6, -1.7], [-0.05, -0.06, -0.07])
    );
}

#[test]
fn luma_average_meters_the_locked_baseline() {
    let mut process = ProcessConfig::default();
    process.local_floors = [-1.0, -1.0, -1.0];
    process.locked_floors = [-2.0, -2.0, -2.0];
    process.locked_ceils = [-0.2, -0.2, -0.2];
    process.use_luma_average = true;
    assert_eq!(
        luma_source_bounds(&process, bounds()),
        LogNegativeBounds::new([-2.0; 3], [-0.2; 3])
    );
    process.use_luma_average = false;
    assert_eq!(luma_source_bounds(&process, bounds()), bounds());
}

#[test]
fn normalization_publishes_the_metered_metrics() {
    let (w, h) = (128, 128);
    let image = gradient(w, h);
    let process = ProcessConfig::default();
    let result = normalize_negative(
        &image,
        w,
        h,
        NormalizationSettings {
            config: &process,
            mode: ProcessMode::C41,
            active_roi: None,
            base_log: None,
        },
    );
    assert_eq!(result.image.len(), image.len());
    assert!(result.metrics.norm_density_range > 0.0);
    assert!(result.metrics.textural_range > 0.0);
    assert!(result.metrics.shadow_point > result.metrics.metered_anchor);
    assert!(result.metrics.metered_anchor > result.metrics.highlight_point);
    assert!(result.metrics.neutral_axis.is_some());
}

#[test]
fn a_bw_frame_publishes_no_neutral_axis() {
    let (w, h) = (64, 64);
    let image = gradient(w, h);
    let process = ProcessConfig::default();
    let result = normalize_negative(
        &image,
        w,
        h,
        NormalizationSettings {
            config: &process,
            mode: ProcessMode::BW,
            active_roi: None,
            base_log: None,
        },
    );
    assert_eq!(result.metrics.neutral_axis, None);
}

#[test]
fn the_anchor_stays_inside_its_band() {
    let (w, h) = (128, 128);
    let mut image = vec![0.4; w * h * 3];
    for pixel in image.chunks_exact_mut(3) {
        pixel[0] = 0.6;
    }
    let process = ProcessConfig::default();
    let result = normalize_negative(
        &image,
        w,
        h,
        NormalizationSettings {
            config: &process,
            mode: ProcessMode::C41,
            active_roi: None,
            base_log: None,
        },
    );
    assert!(result.metrics.metered_anchor >= c::ASSUMED_ANCHOR - c::ANCHOR_METER_BAND);
    assert!(result.metrics.metered_anchor <= c::ASSUMED_ANCHOR + c::ANCHOR_METER_BAND);
}

#[test]
fn white_and_black_point_trims_move_the_window() {
    let (w, h) = (96, 96);
    let image = gradient(w, h);
    let mut process = ProcessConfig::default();
    process.white_point_offset = 0.05;
    process.black_point_trim_red = -0.03;
    let result = normalize_negative(
        &image,
        w,
        h,
        NormalizationSettings {
            config: &process,
            mode: ProcessMode::C41,
            active_roi: None,
            base_log: None,
        },
    );
    assert!(result.metrics.final_bounds.floors[0] > result.metrics.pre_trim_bounds.floors[0]);
    assert!(result.metrics.final_bounds.ceils[0] < result.metrics.pre_trim_bounds.ceils[0]);
}
