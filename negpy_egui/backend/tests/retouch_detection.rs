//! Ports of the detection and IR rules in `tests/test_retouch_logic.py`.

use negpy_backend::domain::models::RetouchConfig;
use negpy_backend::features::retouch::logic::{
    compute_dust_stats, detect_bar, detect_luma_score, downsample_ir, film_scale, ir_defect_score,
    ir_detect_cutoff, ir_detect_target, ir_ratio_and_gain, normalize_ir, scratch_detect_bar,
    trace_scratch,
};
use negpy_backend::features::retouch::processor::{detect_dust, source_tokens};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn dusty_source() -> ImageBuffer {
    let mut pixels = vec![0.18f32; 160 * 160 * 3];
    for y in 80..83 {
        for x in 80..83 {
            let i = (y * 160 + x) * 3;
            pixels[i..i + 3].fill(0.005);
        }
    }
    ImageBuffer::from_rgb(160, 160, pixels)
}

fn grainy(height: usize, width: usize) -> ImageBuffer {
    let mut state = 21u32;
    let mut pixels = Vec::with_capacity(height * width * 3);
    for _ in 0..height * width * 3 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let noise = (state >> 8) as f32 / 16_777_216.0 - 0.5;
        pixels.push(0.18 + noise * 0.02);
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_detect_bar_is_monotonic() {
    let bars: Vec<f64> = [0.0, 0.25, 0.66, 0.9, 1.0]
        .iter()
        .map(|v| detect_bar(*v))
        .collect();
    assert!(bars.windows(2).all(|pair| pair[1] >= pair[0]));
    assert!(bars[0] < bars[2] && bars[2] < bars[3]);
    assert!(bars[4].is_infinite());
}

#[test]
fn the_scratch_bar_and_ir_cutoff_move_with_the_slider() {
    assert!(scratch_detect_bar(0.1) < scratch_detect_bar(0.9));
    assert!(ir_detect_cutoff(0.9, true) < ir_detect_cutoff(0.1, true));
    assert!(ir_detect_cutoff(0.9, false) < ir_detect_cutoff(0.1, false));
}

#[test]
fn the_ir_target_stays_between_preview_and_buffer() {
    assert_eq!(ir_detect_target(6000, 1600), 3600);
    assert_eq!(ir_detect_target(1200, 1600), 1200);
    assert_eq!(ir_detect_target(3000, 3000), 3000);
}

#[test]
fn dust_stats_find_a_dark_speck() {
    let img = dusty_source();
    let stats = compute_dust_stats(&img, 4).expect("stats");
    assert_eq!(stats.len(), 4);
    assert_eq!(stats[0].len(), 160 * 160);
    let (score, hairs) =
        detect_luma_score(&img, 0.66, 4, Some(&stats), Some(0.66)).expect("detection");
    assert!(hairs.is_none());
    let score = score.expect("a speck score");
    assert!(score[80 * 160 + 80] < 1.0);
    assert!(score.iter().any(|v| *v < 1.0));
}

#[test]
fn a_clean_grainy_frame_has_no_speck_score() {
    let img = grainy(160, 160);
    let (score, hairs) = detect_luma_score(&img, 0.66, 4, None, Some(0.66)).expect("detection");
    assert!(score.is_none() && hairs.is_none());
}

#[test]
fn dust_detection_is_exposure_invariant() {
    let img = dusty_source();
    let scaled = ImageBuffer::from_rgb(160, 160, img.pixels.iter().map(|v| v * 4.0).collect());
    let first = detect_luma_score(&img, 0.66, 4, None, Some(0.66))
        .expect("detection")
        .0
        .unwrap();
    let second = detect_luma_score(&scaled, 0.66, 4, None, Some(0.66))
        .expect("detection")
        .0
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_dust_toggle_off_skips_detection() {
    let config = RetouchConfig::default();
    assert!(detect_dust(&dusty_source(), &config).is_none());
    let enabled = RetouchConfig {
        dust_remove: true,
        ..RetouchConfig::default()
    };
    assert!(detect_dust(&dusty_source(), &enabled).is_some());
}

#[test]
fn the_source_tokens_follow_the_toggles() {
    let base = RetouchConfig::default();
    assert_eq!(source_tokens(&base, true), "");
    let dust = RetouchConfig {
        dust_remove: true,
        ..RetouchConfig::default()
    };
    assert!(source_tokens(&dust, false).contains("dust"));
    let ir = RetouchConfig {
        ir_dust_remove: true,
        ..RetouchConfig::default()
    };
    assert!(source_tokens(&ir, true).contains("ir"));
    assert_eq!(source_tokens(&ir, false), "");
}

#[test]
fn a_min_preserving_downsample_keeps_a_defect_dip() {
    let mut ir = vec![0.9f32; 200 * 200];
    ir[100 * 200 + 100] = 0.1;
    ir[100 * 200 + 101] = 0.1;
    let small = downsample_ir(&ir, 200, 200, 100, None);
    assert_eq!(small.len(), 100 * 100);
    assert!(small.iter().any(|v| *v < 0.5), "the dip must survive");
}

#[test]
fn normalize_ir_keeps_clean_film_near_one() {
    let ir = vec![0.9f32; 64 * 64];
    let out = normalize_ir(&ir, 64, 64, None);
    assert!(out.iter().all(|v| (*v - 1.0).abs() < 1e-3));
}

#[test]
fn a_deep_ir_dip_scores_as_a_defect() {
    let mut ir = vec![0.9f32; 200 * 200];
    for y in 90..110 {
        for x in 90..110 {
            ir[y * 200 + x] = 0.2;
        }
    }
    let ratio = normalize_ir(&ir, 200, 200, None);
    let score = ir_defect_score(&ratio, 200, 200, 0.5);
    assert!(score[100 * 200 + 100] < 1.0);
    assert!(score.iter().any(|v| *v >= 0.9));
}

#[test]
fn ir_ratio_and_gain_leave_clean_film_alone() {
    let ir = vec![0.9f32; 160 * 160];
    let img = ImageBuffer::from_rgb(160, 160, vec![0.4; 160 * 160 * 3]);
    let (_ratio, gain, degenerate, gammas) = ir_ratio_and_gain(&ir, &img, 160, 160);
    assert!(!degenerate);
    assert!(gammas.iter().all(|g| *g >= 1.0 && *g <= 2.2));
    assert!(gain.iter().all(|v| *v >= 1.0 && *v <= 2.0));
}

#[test]
fn film_scale_is_the_footprint_ratio() {
    assert_eq!(film_scale(1600, 1600), 1.0);
    assert_eq!(film_scale(3200, 3200), 2.0);
}

fn transport_scratch(h: usize, w: usize, depth: f32, slope: f64) -> (ImageBuffer, f64) {
    let mut state = 3u32;
    let mut pixels = Vec::with_capacity(h * w * 3);
    for _ in 0..h * w * 3 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let noise = (state >> 8) as f32 / 16_777_216.0 - 0.5;
        pixels.push(0.45 + noise * 0.012);
    }
    let row0 = h as f64 / 2.0;
    for x in 0..w {
        let y = (row0 + slope * (x as f64 - w as f64 / 2.0)).round() as usize;
        for dy in 0..2 {
            if y + dy < h {
                let i = ((y + dy) * w + x) * 3;
                for c in 0..3 {
                    pixels[i + c] *= 1.0 + depth;
                }
            }
        }
    }
    (ImageBuffer::from_rgb(w, h, pixels), row0)
}

#[test]
fn a_transport_scratch_is_traced_from_one_click() {
    let (img, row0) = transport_scratch(400, 1200, 0.12, 0.006);
    let line = trace_scratch(&img, 0.5, (row0 + 0.5) / 400.0, 0.5).expect("a traced scratch");
    assert!(line.2 - line.0 > 0.8);
    let fitted = (line.3 - line.1) * 400.0 / ((line.2 - line.0) * 1200.0);
    assert!((fitted - 0.006).abs() < 0.002, "fitted slope {fitted}");
}

#[test]
fn clean_film_does_not_trace_a_scratch() {
    let (img, _) = transport_scratch(300, 900, 0.0, 0.0);
    assert!(trace_scratch(&img, 0.5, 0.5, 0.5).is_none());
}
