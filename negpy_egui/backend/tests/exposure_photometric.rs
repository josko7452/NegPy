//! Ports of the print-render assertions in `tests/test_exposure_render.py` and
//! `tests/test_exposure_bw_filter.py`.

use negpy_backend::domain::models::{ExposureConfig, ProcessConfig, ProcessMode};
use negpy_backend::features::exposure::print::{PrintRenderSettings, render_negative, solve_print};
use negpy_backend::features::exposure::processor::NeutralAxisRefs;

fn render(
    config: &ExposureConfig,
    process: &ProcessConfig,
    image: &[f32],
    mode: ProcessMode,
) -> Vec<f32> {
    let bounds = negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
        [-1.6; 3], [-0.05; 3],
    );
    render_negative(
        image,
        4,
        &PrintRenderSettings {
            config,
            process,
            density_range: Some(1.5),
            textural_range: Some(1.0),
            shadow_point: Some(0.8),
            highlight_point: Some(0.2),
            anchor: Some(0.46),
            bounds: Some(bounds),
            neutral_axis: None,
            mode,
            base_black_hold: 0.0,
        },
    )
}

#[test]
fn a_brighter_density_prints_lighter() {
    let image = vec![0.3; 4 * 4 * 3];
    let process = ProcessConfig::default();
    let mut light = ExposureConfig::default();
    light.density = 0.5;
    let mut dark = ExposureConfig::default();
    dark.density = 1.5;
    let light_sum: f32 = render(&light, &process, &image, ProcessMode::C41)
        .iter()
        .sum();
    let dark_sum: f32 = render(&dark, &process, &image, ProcessMode::C41)
        .iter()
        .sum();
    assert!(light_sum > dark_sum);
}

#[test]
fn the_solved_curve_has_three_channels() {
    let process = ProcessConfig::default();
    let config = ExposureConfig::default();
    let solved = solve_print(&PrintRenderSettings {
        config: &config,
        process: &process,
        density_range: Some(1.5),
        textural_range: Some(1.0),
        shadow_point: Some(0.8),
        highlight_point: Some(0.2),
        anchor: Some(0.46),
        bounds: Some(
            negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
                [-1.6; 3], [-0.05; 3],
            ),
        ),
        neutral_axis: None,
        mode: ProcessMode::C41,
        base_black_hold: 0.0,
    });
    assert!(solved.slopes.iter().all(|value| *value >= 2.0));
    assert_eq!(solved.curvatures, [0.0; 3]);
}

#[test]
fn a_cast_free_neutral_axis_does_not_curve() {
    let process = ProcessConfig::default();
    let config = ExposureConfig::default();
    let bounds = negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
        [-1.6; 3], [-0.05; 3],
    );
    let solved = solve_print(&PrintRenderSettings {
        config: &config,
        process: &process,
        density_range: Some(1.5),
        textural_range: Some(1.0),
        shadow_point: Some(0.8),
        highlight_point: Some(0.2),
        anchor: Some(0.46),
        bounds: Some(bounds),
        neutral_axis: Some(NeutralAxisRefs {
            mid: [-1.0; 3],
            shadow: [-0.5; 3],
            highlight: None,
            confidence: 1.0,
        }),
        mode: ProcessMode::C41,
        base_black_hold: 0.0,
    });
    for ch in 1..3 {
        assert!((solved.slopes[ch] - solved.slopes[1]).abs() < 1e-6);
    }
}

#[test]
fn a_bw_print_outputs_equal_channels() {
    let image = vec![0.3; 4 * 4 * 3];
    let mut process = ProcessConfig::default();
    process.process_mode = ProcessMode::BW;
    let config = ExposureConfig::default();
    let out = render(&config, &process, &image, ProcessMode::BW);
    for pixel in out.chunks_exact(3) {
        assert!((pixel[0] - pixel[1]).abs() < 1e-6);
        assert!((pixel[1] - pixel[2]).abs() < 1e-6);
    }
}

#[test]
fn a_preflash_lifts_the_print() {
    let image = vec![0.25; 4 * 4 * 3];
    let process = ProcessConfig::default();
    let config = ExposureConfig::default();
    let mut flashed = config.clone();
    flashed.preflash = 1.0;
    let light = |density: f32| 10f32.powf(-density);
    let plain: f32 = render(&config, &process, &image, ProcessMode::C41)
        .iter()
        .map(|value| light(*value))
        .sum();
    let flash: f32 = render(&flashed, &process, &image, ProcessMode::C41)
        .iter()
        .map(|value| light(*value))
        .sum();
    assert!(flash > plain);
}
