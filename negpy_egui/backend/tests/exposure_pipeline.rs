//! Ports of the exposure-stage order assertions in `tests/test_engine_stages.py`.

use negpy_backend::domain::models::{ExposureConfig, ProcessConfig, ProcessMode, WorkspaceConfig};
use negpy_backend::exposure_pipeline::{
    ExposureContext, Frame, analysis_roi, black_hold_taper, metrics_for_event, preview_frame,
    render_exposure,
};

fn gradient(width: usize, height: usize) -> Frame {
    let count = width * height;
    let pixels = (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.98 * index as f32 / (count - 1) as f32;
            [value, value * 0.92, value * 0.84]
        })
        .collect();
    Frame {
        pixels,
        width,
        height,
    }
}

#[test]
fn the_pipeline_keeps_the_frame_size() {
    let frame = gradient(64, 48);
    let config = WorkspaceConfig::default();
    let result = render_exposure(&frame, &config, ExposureContext::default());
    assert_eq!((result.image.width, result.image.height), (64, 48));
    assert_eq!(result.image.pixels.len(), 64 * 48 * 3);
    assert!(result.image.pixels.iter().all(|value| value.is_finite()));
}

#[test]
fn the_metrics_carry_the_metered_values() {
    let frame = gradient(96, 96);
    let config = WorkspaceConfig::default();
    let result = render_exposure(&frame, &config, ExposureContext::default());
    let metrics = metrics_for_event(&result.metrics);
    assert_eq!(metrics.0.len(), 5);
    assert_eq!(metrics.0[0].0, "metered_anchor");
    assert!(result.metrics.shadow_point > result.metrics.highlight_point);
}

#[test]
fn the_preview_preserves_the_long_edge_budget() {
    let frame = gradient(400, 200);
    let preview = preview_frame(&frame, 100);
    assert_eq!((preview.width, preview.height), (100, 50));
    assert_eq!(preview_frame(&frame, 800), frame);
}

#[test]
fn the_analysis_rect_wins_over_the_active_roi() {
    let frame = gradient(100, 80);
    let mut process = ProcessConfig::default();
    process.analysis_rect = Some(vec![0.25, 0.25, 0.75, 0.75]);
    assert_eq!(
        analysis_roi(&frame, &process, Some((10, 20, 10, 20))),
        Some((20, 60, 25, 75))
    );
}

#[test]
fn black_hold_is_off_until_the_toggle() {
    let mut exposure = ExposureConfig::default();
    assert_eq!(black_hold_taper(&exposure), 0.0);
    exposure.hold_black = true;
    assert!(black_hold_taper(&exposure) > 0.0);
}

#[test]
fn a_bw_frame_renders_neutral_output() {
    let frame = gradient(32, 32);
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::BW;
    let result = render_exposure(&frame, &config, ExposureContext::default());
    for pixel in result.image.pixels.chunks_exact(3) {
        assert!((pixel[0] - pixel[1]).abs() < 1e-5);
        assert!((pixel[1] - pixel[2]).abs() < 1e-5);
    }
}
