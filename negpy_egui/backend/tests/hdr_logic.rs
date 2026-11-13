//! Ports the HDR ordering, anchor and shadow-seed cases of tests/test_hdr.py.

use negpy_backend::domain::models::HdrConfig;
use negpy_backend::features::hdr::logic::{
    ExposureStats, Frame, SEED_SHADOW_LIMIT, SEED_SHADOW_PER_STOP, anchor_choices, anchor_ratio,
    choose_reference, exposure_order_key, measure_step, output_scale, pair_ratio, resolve_anchor,
    seed_shadow_density, shadow_reach_stops, solve_ratios,
};

fn paths() -> Vec<String> {
    ["ref.nef", "long.nef", "short.nef"]
        .map(str::to_string)
        .to_vec()
}

#[test]
fn the_longest_unclipped_exposure_is_the_reference() {
    let stats = [
        ExposureStats {
            path: "short".into(),
            level: 0.25,
            clipped: 0.0,
        },
        ExposureStats {
            path: "ref".into(),
            level: 0.9,
            clipped: 0.0,
        },
        ExposureStats {
            path: "long".into(),
            level: 1.0,
            clipped: 0.2,
        },
    ];
    assert_eq!(choose_reference(&stats), Some(1));
    assert!(choose_reference(&[]).is_none());
}

#[test]
fn an_all_clipped_bracket_picks_the_least_clipped_frame() {
    let stats = [
        ExposureStats {
            path: "a".into(),
            level: 1.0,
            clipped: 0.4,
        },
        ExposureStats {
            path: "b".into(),
            level: 1.0,
            clipped: 0.1,
        },
    ];
    assert_eq!(choose_reference(&stats), Some(1));
}

#[test]
fn a_longer_exposure_orders_after_a_shorter_one_at_equal_level() {
    assert!(exposure_order_key(1.0, 0.0) < exposure_order_key(1.0, 0.2));
}

#[test]
fn the_measure_stride_keeps_the_pixel_budget() {
    assert_eq!(measure_step(100, 100), 1);
    assert_eq!(measure_step(3000, 4000), 2);
}

#[test]
fn an_anchor_wins_and_never_passes_the_reference() {
    let upward = [1.0, 2.0, 4.0, 8.0, 16.0];
    assert_eq!(output_scale(&upward, None), 1.0);
    assert_eq!(output_scale(&upward, Some(0.5)), 0.5);
    assert_eq!(output_scale(&upward, Some(8.0)), 1.0);
    assert_eq!(output_scale(&[0.5, 1.0, 2.0], Some(0.0)), 1.0);
}

#[test]
fn the_default_scale_is_the_geometric_middle_exposure() {
    let ratios = [0.5, 1.0, 2.0];
    assert_eq!(output_scale(&ratios, None), 1.0);
    let even = [0.5, 0.5, 2.0, 8.0];
    let expected = (0.5_f64 * 2.0).sqrt();
    assert!((output_scale(&even, None) - expected).abs() < 1e-12);
}

#[test]
fn an_anchor_path_resolves_and_a_stale_one_falls_back() {
    let ratios = [1.0, 4.0, 0.5];
    assert_eq!(anchor_ratio(&paths(), &ratios, "long.nef"), Some(4.0));
    assert_eq!(anchor_ratio(&paths(), &ratios, "missing.nef"), None);
    assert_eq!(anchor_ratio(&paths(), &ratios, ""), None);
}

#[test]
fn a_stop_anchor_takes_precedence_over_a_path() {
    let config = HdrConfig {
        hdr_anchor: "long.nef".into(),
        hdr_anchor_ev: -1.0,
        ..HdrConfig::default()
    };
    assert_eq!(
        resolve_anchor(&paths(), &[1.0, 4.0, 0.5], &config),
        Some(0.5)
    );
}

#[test]
fn anchor_choices_are_longest_first_and_exclude_longer_frames() {
    let choices = anchor_choices(&paths(), &[1.0, 4.0, 0.5]);
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].0, "ref.nef");
    assert_eq!(choices[0].1, 0.0);
    assert_eq!(choices[1].0, "short.nef");
}

#[test]
fn the_shadow_seed_scales_with_recovered_reach() {
    let one = [1.0, 2.0];
    let two = [1.0, 2.0, 4.0];
    assert!((shadow_reach_stops(&one, None) - 1.0).abs() < 1e-12);
    assert!((shadow_reach_stops(&two, None) - 2.0).abs() < 1e-12);
    assert!((seed_shadow_density(&one, None) + SEED_SHADOW_PER_STOP).abs() < 1e-12);
    assert!((seed_shadow_density(&two, None) + 2.0 * SEED_SHADOW_PER_STOP).abs() < 1e-12);
}

#[test]
fn a_bracket_that_recovered_nothing_gets_no_lift() {
    assert_eq!(seed_shadow_density(&[1.0], None), 0.0);
    assert_eq!(seed_shadow_density(&[], None), 0.0);
    assert_eq!(shadow_reach_stops(&[1.0], None), 0.0);
}

#[test]
fn the_seed_is_capped() {
    let wide = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
    assert!((seed_shadow_density(&wide, None) + SEED_SHADOW_LIMIT).abs() < 1e-12);
}

fn scene(height: usize, width: usize, seed: u64) -> Vec<f32> {
    let mut state = seed | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state as f64 / u64::MAX as f64) * 2.0 - 1.0
    };
    let mut pixels = Vec::with_capacity(height * width * 3);
    for _ in 0..height {
        for x in 0..width {
            let t = x as f64 / width as f64;
            let value = 0.95 * 10.0_f64.powf(-4.0 * t);
            let noise = 1.0 + 0.02 * next();
            pixels.extend(
                [value * noise, value * 0.9 * noise, value * 0.8 * noise].map(|v| v as f32),
            );
        }
    }
    pixels
}

fn expose(pixels: &[f32], gain: f64) -> Frame {
    let samples: Vec<u16> = pixels
        .iter()
        .map(|value| ((f64::from(*value) * gain).clamp(0.0, 1.0) * 65535.0).round() as u16)
        .collect();
    let side = (pixels.len() / 3).isqrt();
    Frame::new(
        side,
        side,
        samples.iter().map(|v| f32::from(*v) / 65535.0).collect(),
    )
}

#[test]
fn the_merge_recovers_a_stop_from_the_images() {
    let pixels = scene(64, 64, 0);
    let short = expose(&pixels, 0.25);
    let long = expose(&pixels, 0.5);
    let ratio = pair_ratio(&short, &long).unwrap();
    assert!((ratio - 2.0).abs() < 0.05, "{ratio}");
}

#[test]
fn a_black_pair_has_no_ratio() {
    let black = Frame::new(40, 40, vec![0.0; 40 * 40 * 3]);
    assert!(pair_ratio(&black, &black).is_none());
}

#[test]
fn the_merge_stays_inside_the_unit_range_and_keeps_the_canvas() {
    use negpy_backend::features::hdr::logic::{apply_render_exposure, merge_frames};
    let pixels = scene(64, 64, 2);
    let frames = [
        expose(&pixels, 0.25),
        expose(&pixels, 0.5),
        expose(&pixels, 1.0),
    ];
    let ratios = solve_ratios(&frames, 2);
    let merged = merge_frames(&frames, &ratios, 2, false).unwrap();
    assert_eq!((merged.width, merged.height), (64, 64));
    let rendered = apply_render_exposure(&merged, &ratios, None);
    assert!(
        rendered
            .pixels
            .iter()
            .all(|value| (0.0..=1.0).contains(value))
    );
}

#[test]
fn an_empty_merge_is_rejected() {
    use negpy_backend::features::hdr::logic::merge_frames;
    assert!(merge_frames(&[], &[], 0, false).is_none());
}
