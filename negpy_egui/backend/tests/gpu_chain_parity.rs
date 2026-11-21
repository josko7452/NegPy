//! The GPU prefix matches the CPU base stage for a frame with no crop.

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::features::exposure::processor::{NormalizationSettings, normalize_negative};
use negpy_backend::features::geometry::processor::{GeometryContext, process};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::rendering::gpu_engine::{BaseRun, GpuEngine};

fn frame(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            pixels.extend_from_slice(&[
                0.05 + 0.9 * fx,
                0.08 + 0.85 * fy,
                0.06 + 0.8 * (fx + fy) * 0.5,
            ]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_gpu_prefix_matches_the_cpu_base() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let settings = WorkspaceConfig::default();
    let image = frame(48, 32);
    let geometry = process(
        &image,
        &settings.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let cpu = normalize_negative(
        &geometry.image.pixels,
        geometry.image.width,
        geometry.image.height,
        NormalizationSettings {
            config: &settings.process,
            mode: settings.process.process_mode,
            active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
                (
                    y1.max(0) as usize,
                    y2.max(0) as usize,
                    x1.max(0) as usize,
                    x2.max(0) as usize,
                )
            }),
            base_log: None,
        },
    );
    let gpu = engine.run_normalized(&image, &settings, &BaseRun::plain(cpu.metrics.final_bounds));
    assert_eq!(gpu.width, geometry.image.width);
    assert_eq!(gpu.height, geometry.image.height);
    let mut worst = 0.0f32;
    let mut violations = 0usize;
    for (index, want) in cpu.image.iter().enumerate() {
        let got = gpu.pixels[index];
        worst = worst.max((got - want).abs());
        if (got - want).abs() > 3e-2 {
            violations += 1;
        }
    }
    let fraction = violations as f64 / cpu.image.len() as f64;
    assert!(
        fraction < 0.01,
        "{violations} samples differ, worst {worst}"
    );
}

#[test]
fn a_rotated_prefix_swaps_dimensions() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let mut settings = WorkspaceConfig::default();
    settings.geometry.rotation = 1;
    let image = frame(48, 32);
    let geometry = process(
        &image,
        &settings.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let cpu = normalize_negative(
        &geometry.image.pixels,
        geometry.image.width,
        geometry.image.height,
        NormalizationSettings {
            config: &settings.process,
            mode: settings.process.process_mode,
            active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
                (
                    y1.max(0) as usize,
                    y2.max(0) as usize,
                    x1.max(0) as usize,
                    x2.max(0) as usize,
                )
            }),
            base_log: None,
        },
    );
    let gpu = engine.run_normalized(&image, &settings, &BaseRun::plain(cpu.metrics.final_bounds));
    assert_eq!((gpu.width, gpu.height), (32, 48));
    let mut violations = 0usize;
    for (index, want) in cpu.image.iter().enumerate() {
        if (gpu.pixels[index] - want).abs() > 3e-2 {
            violations += 1;
        }
    }
    let fraction = violations as f64 / cpu.image.len() as f64;
    assert!(fraction < 0.01, "{violations} samples differ");
}

#[test]
fn the_encoded_prefix_matches_the_cpu_encode() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let settings = WorkspaceConfig::default();
    let image = frame(40, 28);
    let geometry = process(
        &image,
        &settings.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let cpu = normalize_negative(
        &geometry.image.pixels,
        geometry.image.width,
        geometry.image.height,
        NormalizationSettings {
            config: &settings.process,
            mode: settings.process.process_mode,
            active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
                (
                    y1.max(0) as usize,
                    y2.max(0) as usize,
                    x1.max(0) as usize,
                    x2.max(0) as usize,
                )
            }),
            base_log: None,
        },
    );
    let encoded = negpy_backend::features::exposure::analysis::working_oetf_encode(&cpu.image);
    let gpu = engine.run_base(&image, &settings, &BaseRun::plain(cpu.metrics.final_bounds));
    let mut violations = 0usize;
    for (index, want) in encoded.iter().enumerate() {
        if (gpu.pixels[index] - want).abs() > 3e-3 {
            violations += 1;
        }
    }
    let fraction = violations as f64 / encoded.len() as f64;
    assert!(fraction < 0.01, "{violations} samples differ");
}

#[test]
fn the_chain_reports_the_density_histogram() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let settings = WorkspaceConfig::default();
    let image = frame(32, 24);
    let bounds = negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
        [-1.5, -1.5, -1.5],
        [-0.1, -0.1, -0.1],
    );
    let (rendered, histogram) =
        engine.run_base_with_histogram(&image, &settings, &BaseRun::plain(bounds));
    assert_eq!(rendered.width, 32);
    assert_eq!(histogram.len(), 4 * 120);
    assert!(
        histogram.iter().sum::<f64>() > 0.0,
        "the frame must have samples"
    );
}

#[test]
fn the_cpu_override_stops_the_engine() {
    // The default app config leaves the GPU on; a machine without an adapter
    // still returns `None`. Only the disabled case is asserted here, because
    // toggling the process config would race the other tests.
    if !negpy_backend::kernel::system::config::app_config().use_gpu {
        assert!(GpuEngine::new().is_none());
    }
}

#[test]
fn a_cropped_chain_matches_the_cpu_crop() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let settings = WorkspaceConfig::default();
    let image = frame(40, 28);
    let geometry = process(
        &image,
        &settings.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let cpu = normalize_negative(
        &geometry.image.pixels,
        geometry.image.width,
        geometry.image.height,
        NormalizationSettings {
            config: &settings.process,
            mode: settings.process.process_mode,
            active_roi: None,
            base_log: None,
        },
    );
    let roi = (
        2i64,
        geometry.image.height as i64 - 3,
        4i64,
        geometry.image.width as i64 - 1,
    );
    let want = negpy_backend::features::geometry::processor::crop_image(
        &ImageBuffer::from_rgb(
            geometry.image.width,
            geometry.image.height,
            negpy_backend::features::exposure::analysis::working_oetf_encode(&cpu.image),
        ),
        Some(roi),
    );
    let gpu = engine.run_base_cropped(
        &image,
        &settings,
        &BaseRun::plain(cpu.metrics.final_bounds),
        Some(roi),
    );
    assert_eq!((gpu.width, gpu.height), (want.width, want.height));
    let mut failures = 0usize;
    for (index, expected) in want.pixels.iter().enumerate() {
        if (gpu.pixels[index] - expected).abs() > 3e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ");
}

#[test]
fn a_print_chain_matches_the_cpu_print() {
    let Some(mut engine) = GpuEngine::new() else {
        return;
    };
    let settings = WorkspaceConfig::default();
    let image = frame(40, 28);
    let geometry = process(
        &image,
        &settings.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let cpu = normalize_negative(
        &geometry.image.pixels,
        geometry.image.width,
        geometry.image.height,
        NormalizationSettings {
            config: &settings.process,
            mode: settings.process.process_mode,
            active_roi: None,
            base_log: None,
        },
    );
    let print = negpy_backend::features::exposure::print::PrintRenderSettings {
        config: &settings.exposure,
        process: &settings.process,
        density_range: Some(cpu.metrics.norm_density_range),
        textural_range: Some(cpu.metrics.textural_range),
        shadow_point: Some(cpu.metrics.shadow_point),
        highlight_point: Some(cpu.metrics.highlight_point),
        anchor: Some(cpu.metrics.metered_anchor),
        bounds: Some(cpu.metrics.final_bounds),
        neutral_axis: None,
        mode: settings.process.process_mode,
        base_black_hold: 0.0,
        mask: None,
        diffusion: None,
        local_exposure: None,
        local_grade: None,
        local_flash: None,
        key_params: None,
        key_alpha: None,
        grade_deltas: None,
    };
    let want = negpy_backend::features::exposure::analysis::working_oetf_encode(
        &negpy_backend::features::exposure::print::render_negative(
            &cpu.image,
            geometry.image.width,
            &print,
        ),
    );
    let run = negpy_backend::features::exposure::exposure_shader::ExposureRun {
        density_range: Some(cpu.metrics.norm_density_range),
        textural_range: Some(cpu.metrics.textural_range),
        shadow_point: Some(cpu.metrics.shadow_point),
        highlight_point: Some(cpu.metrics.highlight_point),
        anchor: Some(cpu.metrics.metered_anchor),
        bounds: Some(cpu.metrics.final_bounds),
        ..negpy_backend::features::exposure::exposure_shader::ExposureRun::plain(
            &settings.exposure,
            &settings.process,
            settings.process.process_mode,
        )
    };
    let gpu = engine.run_print(
        &image,
        &settings,
        &BaseRun::plain(cpu.metrics.final_bounds),
        &run,
        None,
    );
    let mut failures = 0usize;
    for (index, expected) in want.iter().enumerate() {
        if (gpu.pixels[index] - expected).abs() > 3e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ");
}
