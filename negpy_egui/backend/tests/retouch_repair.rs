//! Ports of the repair, exclusion and clone rules in `tests/test_retouch_logic.py`.

use negpy_backend::features::retouch::CloneStrokeConfig;
use negpy_backend::features::retouch::logic::{
    apply_clone_strokes, apply_hair_inpaint, apply_score_repair, drop_exclusions, exclusion_cover,
    ir_defect_score, lines_to_score, normalize_ir, repair_components, route_wide_defects,
    strokes_to_score, trace_scratch,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn grainy(width: usize, height: usize, level: f32) -> ImageBuffer {
    let mut state = 31u32;
    let mut pixels = Vec::with_capacity(width * height * 3);
    for _ in 0..width * height * 3 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let noise = (state >> 8) as f32 / 16_777_216.0 - 0.5;
        pixels.push(level + noise * 0.01);
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn an_exclusion_stroke_covers_its_path() {
    let cover = exclusion_cover(&[(vec![[0.2, 0.5], [0.8, 0.5]], 20.0)], 100, 100);
    assert_eq!(cover[50 * 100 + 50], 1);
    assert_eq!(cover[5 * 100 + 50], 0);
}

#[test]
fn an_exclusion_releases_the_score_under_it_and_keeps_the_rest() {
    let mut score = vec![0.2f32; 100 * 100];
    score[10 * 100 + 10] = 0.9;
    let (out, _) = drop_exclusions(
        Some(score.clone()),
        None,
        &[(vec![[0.5, 0.5]], 30.0)],
        100,
        100,
    );
    let out = out.expect("the far score remains");
    assert!(out[50 * 100 + 50] > score[50 * 100 + 50]);
    assert_eq!(out[10 * 100 + 10], 0.9);
}

#[test]
fn an_exclusion_that_releases_everything_drops_the_score() {
    let score = vec![0.2f32; 100 * 100];
    let (out, _) = drop_exclusions(
        Some(score),
        None,
        &[(vec![[0.5, 0.5], [0.5, 0.5]], 3200.0)],
        100,
        100,
    );
    assert!(out.is_none());
}

#[test]
fn a_painted_stroke_finds_a_defect_and_leaves_clean_film_alone() {
    let mut img = grainy(100, 100, 0.5);
    for y in 49..52 {
        for x in 49..52 {
            let i = (y * 100 + x) * 3;
            img.pixels[i..i + 3].fill(0.9);
        }
    }
    let score = strokes_to_score(&img, &[(vec![[0.5, 0.5]], 15.0)], &[]).expect("a speck");
    let out = repair_components(&img, &score, 100, 100, false);
    let center = (50 * 100 + 50) * 3;
    assert!((out.pixels[center] - 0.5).abs() < (img.pixels[center] - 0.5).abs());
    let clean = (10 * 100 + 10) * 3;
    assert_eq!(out.pixels[clean], img.pixels[clean]);
}

#[test]
fn a_painted_stroke_on_clean_film_is_a_noop() {
    let img = grainy(100, 100, 0.5);
    assert!(strokes_to_score(&img, &[(vec![[0.5, 0.5]], 15.0)], &[]).is_none());
}

#[test]
fn a_scratch_line_scores_only_its_traced_band() {
    let mut img = grainy(400, 1200, 0.45);
    for x in 0..1200 {
        let y = 200;
        for dy in 0..2 {
            let i = ((y + dy) * 1200 + x) * 3;
            for c in 0..3 {
                img.pixels[i + c] *= 1.12;
            }
        }
    }
    let line = trace_scratch(&img, 0.5, 200.5 / 400.0, 0.5).expect("a traced line");
    let score =
        lines_to_score(&img, &[[line.0, line.1, line.2, line.3, line.4]], 0.5).expect("a score");
    assert!(score.iter().any(|v| *v < 1.0));
    assert!(score[10 * 1200 + 10].is_nan() || score[10 * 1200 + 10] >= 0.9);
}

#[test]
fn a_wide_ir_defect_routes_to_the_inpaint() {
    let mut ir = vec![0.9f32; 200 * 200];
    for y in 80..120 {
        for x in 80..120 {
            ir[y * 200 + x] = 0.1;
        }
    }
    let ratio = normalize_ir(&ir, 200, 200, None);
    let score = ir_defect_score(&ratio, 200, 200, 0.5);
    let routed = route_wide_defects(&score, 200, 200, None).expect("a routed defect");
    assert!(routed.iter().any(|v| *v != 0));
}

#[test]
fn the_hair_inpaint_is_a_noop_without_a_mask() {
    let img = grainy(64, 64, 0.4);
    assert_eq!(apply_hair_inpaint(&img, &[], 64, 64, 3).pixels, img.pixels);
}

#[test]
fn a_clone_stroke_copies_the_source_offset() {
    let mut img = grainy(64, 64, 0.4);
    for y in 20..28 {
        for x in 20..28 {
            let i = (y * 64 + x) * 3;
            img.pixels[i..i + 3].fill(0.8);
        }
    }
    let stroke = CloneStrokeConfig {
        points: vec![[0.5, 0.5]],
        size: 12.0,
        dx: 0.25,
        dy: 0.0,
        strength: 1.0,
        feather: 0.0,
        match_tone: false,
    };
    let out = apply_clone_strokes(&img, &[stroke]);
    assert_ne!(out.pixels, img.pixels);
    assert_eq!(out.pixels.len(), img.pixels.len());
}

#[test]
fn the_score_repair_keeps_an_unscored_frame_byte_identical() {
    let img = grainy(64, 64, 0.4);
    let score = vec![1.0f32; 64 * 64];
    let out = apply_score_repair(&img, &score, 64, 64, true, None);
    assert_eq!(out.pixels, img.pixels);
}
