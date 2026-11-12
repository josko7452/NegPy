//! Ports of tests/test_process_detect.py.

use negpy_backend::domain::models::ProcessMode;
use negpy_backend::features::process::detect::{
    detect_process_and_source, detect_process_mode, detect_source_color,
};

const SIZE: usize = 128;

fn gray_ramp(low: f32, high: f32) -> Vec<f32> {
    (0..SIZE * SIZE)
        .map(|index| low + (high - low) * index as f32 / (SIZE * SIZE - 1) as f32)
        .collect()
}

fn bw_scan() -> Vec<f32> {
    gray_ramp(0.1, 0.9)
        .into_iter()
        .flat_map(|value| [value, value, value])
        .collect()
}

fn tinted_bw_scan() -> Vec<f32> {
    gray_ramp(0.1, 0.9)
        .into_iter()
        .flat_map(|value| [value * 0.8, value * 0.95, value])
        .collect()
}

fn patches(seed: u64, low: f32, high: f32) -> Vec<f32> {
    let mut state = seed
        .wrapping_mul(2_862_933_555_777_941_757)
        .wrapping_add(3_037_000_493);
    let mut values = Vec::with_capacity(16 * 16);
    for _ in 0..16 * 16 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        values.push(low + (high - low) * (state >> 40) as f32 / (1 << 24) as f32);
    }
    let mut out = Vec::with_capacity(SIZE * SIZE);
    for row in 0..SIZE {
        for column in 0..SIZE {
            out.push(values[(row / 8) * 16 + column / 8]);
        }
    }
    out
}

fn stack(r: &[f32], g: &[f32], b: &[f32]) -> Vec<f32> {
    r.iter()
        .zip(g)
        .zip(b)
        .flat_map(|((r, g), b)| [*r, *g, *b])
        .collect()
}

fn c41_scan() -> Vec<f32> {
    let r = patches(0, 0.45, 0.75);
    let g = patches(1, 0.25, 0.55);
    let b = patches(2, 0.05, 0.35);
    stack(&r, &g, &b)
}

fn phoenix_scan() -> Vec<f32> {
    let r = patches(2, 0.4, 0.6);
    let g = patches(3, 0.2, 0.4);
    let b = patches(4, 0.38, 0.58);
    stack(&r, &g, &b)
}

fn e6_scan() -> Vec<f32> {
    stack(
        &patches(5, 0.0, 1.0),
        &patches(6, 0.0, 1.0),
        &patches(7, 0.0, 1.0),
    )
}

#[test]
fn a_neutral_ramp_reads_bw() {
    assert_eq!(detect_process_mode(&bw_scan(), SIZE, SIZE), ProcessMode::BW);
    assert_eq!(
        detect_process_mode(&tinted_bw_scan(), SIZE, SIZE),
        ProcessMode::BW
    );
}

#[test]
fn masked_color_scenes_read_c41() {
    assert_eq!(
        detect_process_mode(&c41_scan(), SIZE, SIZE),
        ProcessMode::C41
    );
    assert_eq!(
        detect_process_mode(&phoenix_scan(), SIZE, SIZE),
        ProcessMode::C41
    );
}

#[test]
fn a_balanced_color_scene_reads_e6() {
    assert_eq!(detect_process_mode(&e6_scan(), SIZE, SIZE), ProcessMode::E6);
}

#[test]
fn invalid_input_reads_c41() {
    assert_eq!(detect_process_mode(&[], 0, 0), ProcessMode::C41);
    assert_eq!(detect_process_mode(&[0.0; 4 * 4], 4, 4), ProcessMode::C41);
}

#[test]
fn a_color_scan_is_a_color_source() {
    for scan in [c41_scan(), phoenix_scan(), e6_scan()] {
        assert!(detect_source_color(&scan, SIZE, SIZE));
    }
    assert!(!detect_source_color(&bw_scan(), SIZE, SIZE));
    assert!(!detect_source_color(&tinted_bw_scan(), SIZE, SIZE));
}

#[test]
fn a_gray_scene_on_masked_film_reads_bw_and_stays_color() {
    let gray = gray_ramp(0.1, 0.5);
    let scan = stack(
        &gray,
        &gray.iter().map(|v| v * 0.6).collect::<Vec<_>>(),
        &gray.iter().map(|v| v * 0.4).collect::<Vec<_>>(),
    );
    assert_eq!(
        detect_process_and_source(&scan, SIZE, SIZE),
        (ProcessMode::BW, true)
    );
}
