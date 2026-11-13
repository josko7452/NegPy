//! Ports the CPU readouts in tests/test_waveform.py and tests/test_vectorscope.py.

use negpy_backend::features::exposure::analysis::{
    SCOPE_CHROMA_BINS, SCOPE_HUE_BINS, WAVEFORM_BINS, WAVEFORM_COLS, scope_chroma, scope_point,
    scope_radius, vectorscope_density, vectorscope_histogram, waveform, waveform_color,
};

fn ramp(width: usize, height: usize) -> Vec<f32> {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for _ in 0..height {
        for x in 0..width {
            let value = x as f32 / width as f32;
            pixels.extend_from_slice(&[value; 3]);
        }
    }
    pixels
}

#[test]
fn a_ramp_waveform_climbs_by_column() {
    let pixels = ramp(512, 8);
    let counts = waveform(&pixels, 512, 8).expect("counts");
    assert_eq!(counts.len(), WAVEFORM_BINS * WAVEFORM_COLS);
    let total: f64 = counts.iter().sum();
    assert_eq!(total, (8 * 512) as f64);
    let first = (0..WAVEFORM_COLS)
        .max_by(|a, b| counts[*a].total_cmp(&counts[*b]))
        .unwrap();
    assert!(first < WAVEFORM_COLS / 20);
}

#[test]
fn a_narrow_frame_gives_each_column_its_share() {
    let pixels = vec![0.5; 64 * 10 * 3];
    let counts = waveform(&pixels, 64, 10).expect("counts");
    let columns: Vec<f64> = (0..WAVEFORM_COLS)
        .map(|col| {
            (0..WAVEFORM_BINS)
                .map(|bin| counts[bin * WAVEFORM_COLS + col])
                .sum()
        })
        .collect();
    assert!(columns.iter().any(|value| *value > 0.0));
    assert_eq!(columns.iter().sum::<f64>(), 640.0);
}

#[test]
fn waveform_color_sums_each_channels_encoded_value() {
    let mut pixels = vec![0.0; 10 * 512 * 3];
    for y in 0..10 {
        for x in 0..256 {
            let at = (y * 512 + x) * 3;
            pixels[at..at + 3].copy_from_slice(&[0.8, 0.3, 0.2]);
        }
        for x in 256..512 {
            let at = (y * 512 + x) * 3;
            pixels[at..at + 3].copy_from_slice(&[0.2, 0.3, 0.8]);
        }
    }
    let counts = waveform(&pixels, 512, 10).expect("counts");
    let sums = waveform_color(&pixels, 512, 10).expect("sums");
    assert_eq!(sums.len(), 3 * WAVEFORM_BINS * WAVEFORM_COLS);
    let red_sum: f64 = (0..WAVEFORM_BINS)
        .map(|bin| sums[bin * WAVEFORM_COLS])
        .sum();
    let red_count: f64 = (0..WAVEFORM_BINS)
        .map(|bin| counts[bin * WAVEFORM_COLS])
        .sum();
    assert!(red_count > 0.0);
    assert!((red_sum / red_count / 1023.0 - 0.8).abs() < 1e-2);
}

#[test]
fn the_chroma_scale_round_trips() {
    for log in [false, true] {
        for chroma in [0.0, 2.0, 5.0, 20.0, 60.0] {
            assert!((scope_chroma(scope_radius(chroma, log), log) - chroma).abs() < 1e-9);
        }
    }
}

#[test]
fn a_gray_frame_lands_in_the_first_chroma_bin() {
    let pixels = vec![0.5; 20 * 30 * 3];
    let counts = vectorscope_histogram(&pixels, 30, 20).expect("counts");
    assert_eq!(counts.len(), SCOPE_HUE_BINS * SCOPE_CHROMA_BINS);
    assert_eq!(counts.iter().sum::<f64>(), 600.0);
    let first_bins: f64 = (0..SCOPE_HUE_BINS)
        .map(|hue| counts[hue * SCOPE_CHROMA_BINS])
        .sum();
    assert_eq!(first_bins, 600.0);
}

#[test]
fn a_red_patch_sits_far_out_on_the_red_hue() {
    let pixels = [0.9, 0.1, 0.1].repeat(4 * 4);
    let counts = vectorscope_histogram(&pixels, 4, 4).expect("counts");
    let (best, _) = counts
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    let (hue, chroma) = (best / SCOPE_CHROMA_BINS, best % SCOPE_CHROMA_BINS);
    let (_, a, b) = scope_point([0.9, 0.1, 0.1]);
    let expected = (((b.atan2(a) + std::f64::consts::PI) / (2.0 * std::f64::consts::PI))
        * SCOPE_HUE_BINS as f64) as i64;
    assert!((hue as i64 - expected).abs() <= 1);
    assert!(chroma > SCOPE_CHROMA_BINS / 2);
}

#[test]
fn the_scope_display_keeps_every_pixel() {
    let counts = vec![1.0; SCOPE_HUE_BINS * SCOPE_CHROMA_BINS];
    for log in [false, true] {
        let density = vectorscope_density(&counts, 64, log);
        assert_eq!(density.len(), 64 * 64);
        assert!(density.iter().all(|value| value.is_finite()));
    }
}

#[test]
fn a_gray_pixel_is_neutral_on_the_scope() {
    let (luma, a, b) = scope_point([0.5, 0.5, 0.5]);
    assert!((luma - 0.5).abs() < 1e-6);
    assert!(a.abs() < 1e-4 && b.abs() < 1e-4);
}
