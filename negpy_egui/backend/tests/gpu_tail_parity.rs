//! The GPU print chain through CLAHE matches the CPU engine on a cropped frame.

use negpy_backend::domain::models::{
    AltProcess, GeometryConfig, LocalMask, MaskShape, ProcessMode, WorkspaceConfig,
};
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

fn masked_config() -> WorkspaceConfig {
    let mut config = config();
    config.exposure.contrast_mask_strength = 0.5;
    config.exposure.diffusion = 0.4;
    config.local.masks.push(LocalMask {
        vertices: vec![[0.2, 0.2], [0.7, 0.2], [0.7, 0.7], [0.2, 0.7]],
        stops: 0.6,
        grade: 4.0,
        flash: 0.0,
        shape: MaskShape::Polygon,
        ..LocalMask::default()
    });
    config
}

#[test]
fn an_alt_process_stays_on_the_cpu() {
    let mut alt = WorkspaceConfig::default();
    alt.process.process_mode = ProcessMode::BW;
    alt.altproc.alt_process = AltProcess::Cyanotype;
    assert!(!gpu_eligible(&alt, &PipelineContext::new()));

    let mut auto = WorkspaceConfig::default();
    auto.lab.sharpen_auto_masking = true;
    assert!(gpu_eligible(&auto, &PipelineContext::new()));
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

#[test]
fn a_masked_print_matches_the_cpu_engine() {
    let Some(mut gpu) = GpuEngine::new() else {
        return;
    };
    let config = masked_config();
    let image = frame(96, 72);
    let geometry = process(
        &image,
        &config.geometry,
        GeometryContext { scale_factor: 1.0 },
    );
    let roi = geometry.active_roi;
    assert!(gpu_eligible(&config, &PipelineContext::new()));
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
    let gpu_image = render(
        &mut gpu,
        &image,
        &config,
        "masked",
        &base,
        &mut PipelineContext::new(),
    );
    let cpu_image = negpy_backend::render_engine::DarkroomEngine::new().process(
        &image,
        &config,
        "masked",
        &mut PipelineContext::new(),
    );
    assert_eq!(
        (gpu_image.width, gpu_image.height),
        (cpu_image.width, cpu_image.height)
    );
    let failures = gpu_image
        .pixels
        .iter()
        .zip(&cpu_image.pixels)
        .filter(|(got, expected)| (*got - *expected).abs() > 4e-2)
        .count();
    let fraction = failures as f64 / cpu_image.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ");
}

fn run_both(config: &WorkspaceConfig) -> Option<(ImageBuffer, ImageBuffer)> {
    let mut gpu = GpuEngine::new()?;
    let image = frame(80, 64);
    let roi = process(
        &image,
        &config.geometry,
        GeometryContext { scale_factor: 1.0 },
    )
    .active_roi;
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
    let gpu_image = render(
        &mut gpu,
        &image,
        config,
        "hue",
        &base,
        &mut PipelineContext::new(),
    );
    let cpu_image = negpy_backend::render_engine::DarkroomEngine::new().process(
        &image,
        config,
        "hue",
        &mut PipelineContext::new(),
    );
    Some((gpu_image, cpu_image))
}

#[test]
fn a_hue_trim_matches_the_cpu_engine() {
    let mut config = config();
    config.process.hue_trim = 12.0;
    let Some((gpu_image, cpu_image)) = run_both(&config) else {
        return;
    };
    assert_eq!(
        (gpu_image.width, gpu_image.height),
        (cpu_image.width, cpu_image.height)
    );
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (got, expected) in gpu_image.pixels.iter().zip(&cpu_image.pixels) {
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > 4e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / cpu_image.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn a_bw_print_matches_the_cpu_engine() {
    let Some(mut gpu) = GpuEngine::new() else {
        return;
    };
    let mut config = config();
    config.process.process_mode = ProcessMode::BW;
    config.toning.sepia_strength = 0.4;
    let image = frame(80, 64);
    let roi = negpy_backend::features::geometry::processor::process(
        &image,
        &config.geometry,
        GeometryContext { scale_factor: 1.0 },
    )
    .active_roi;
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
    let gpu_image = render(
        &mut gpu,
        &image,
        &config,
        "bw",
        &base,
        &mut PipelineContext::new(),
    );
    let cpu_image = negpy_backend::render_engine::DarkroomEngine::new().process(
        &image,
        &config,
        "bw",
        &mut PipelineContext::new(),
    );
    assert_eq!(
        (gpu_image.width, gpu_image.height),
        (cpu_image.width, cpu_image.height)
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
