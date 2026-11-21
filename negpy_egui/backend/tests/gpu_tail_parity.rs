//! The GPU print chain through CLAHE matches the CPU engine on a cropped frame.

use negpy_backend::domain::models::{AltProcess, GeometryConfig, ProcessMode, WorkspaceConfig};
use negpy_backend::features::exposure::processor::{NormalizationSettings, measure_negative};
use negpy_backend::features::geometry::processor::{GeometryContext, process};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::render_engine::PipelineContext;
use negpy_backend::services::rendering::gpu_engine::GpuEngine;
use negpy_backend::services::rendering::gpu_render::{GpuBase, gpu_eligible, render};

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

fn config() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.geometry = GeometryConfig {
        crop_rect: Some(vec![0.10, 0.10, 0.80, 0.80]),
        ..GeometryConfig::default()
    };
    config.lab.clahe_strength = 1.0;
    config.lab.sharpen = 0.0;
    config.lab.chroma_denoise = 0.0;
    config.toning.shadow_tint_strength = 0.3;
    config.toning.shadow_tint_hue = 35.0;
    config
}

#[test]
fn an_alt_process_and_auto_masking_stay_on_the_cpu() {
    let mut alt = WorkspaceConfig::default();
    alt.process.process_mode = ProcessMode::BW;
    alt.altproc.alt_process = AltProcess::Cyanotype;
    assert!(!gpu_eligible(&alt, &PipelineContext::new()));

    let mut auto = WorkspaceConfig::default();
    auto.lab.sharpen_auto_masking = true;
    assert!(!gpu_eligible(&auto, &PipelineContext::new()));
}

#[test]
fn a_cropped_print_matches_the_cpu_engine() {
    let Some(mut gpu) = GpuEngine::new() else {
        return;
    };
    let config = config();
    let image = frame(96, 72);
    let geometry = process(
        &image,
        &config.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let roi = geometry.active_roi;
    assert!(roi.is_some(), "the crop rect must resolve");

    assert!(gpu_eligible(&config, &PipelineContext::new()));

    // The worker measures through the crop window, as the CPU engine does.
    let measured = measure_negative(
        &image.pixels,
        image.width,
        image.height,
        NormalizationSettings {
            config: &config.process,
            mode: config.process.process_mode,
            active_roi: roi.map(|(y1, y2, x1, x2)| {
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
    let base = GpuBase {
        bounds: measured.final_bounds,
        norm_density_range: measured.norm_density_range,
        textural_range: measured.textural_range,
        shadow_point: measured.shadow_point,
        highlight_point: measured.highlight_point,
        anchor: Some(measured.metered_anchor),
        active_roi: roi.map(|(y1, y2, x1, x2)| {
            (
                y1.max(0) as usize,
                y2.max(0) as usize,
                x1.max(0) as usize,
                x2.max(0) as usize,
            )
        }),
    };
    let mut gpu_context = PipelineContext::new();
    let gpu_image = render(&mut gpu, &image, &config, "test", &base, &mut gpu_context);

    let mut cpu_context = PipelineContext::new();
    let cpu_image = negpy_backend::render_engine::DarkroomEngine::new().process(
        &image,
        &config,
        "test",
        &mut cpu_context,
    );

    assert_eq!(
        (gpu_image.width, gpu_image.height),
        (cpu_image.width, cpu_image.height),
        "the GPU tail must crop once, to the same window as the CPU engine"
    );
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in cpu_image.pixels.iter().enumerate() {
        let got = gpu_image.pixels[index];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > 4e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / cpu_image.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}
