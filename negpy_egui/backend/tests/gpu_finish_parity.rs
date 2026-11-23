//! The GPU vignette matches the core finish kernel.

use negpy_backend::domain::models::FinishConfig;
use negpy_backend::features::finish::logic::{apply_carrier, apply_vignette, carrier_profiles};
use negpy_backend::features::finish::processor::{RebateMetrics, rebate_tone};
use negpy_backend::features::finish::shader::{FinishUniforms, run, run_with_tables};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn frame(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[
                x as f32 / width as f32,
                1.0 - y as f32 / height as f32,
                (x + y) as f32 / (width + height) as f32,
            ]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_gpu_vignette_matches_the_core_kernel() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 28);
    let image = frame(width, height);
    let config = FinishConfig {
        vignette_stops: 1.2,
        vignette_size: 0.35,
        vignette_roundness: 0.7,
        ..FinishConfig::default()
    };
    let want = apply_vignette(
        &image,
        config.vignette_stops,
        config.vignette_size,
        config.vignette_roundness,
    );
    let uniforms = FinishUniforms::from_config(&config, width, height, [1.0; 3]);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "finish");
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.pixels.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > 2e-4 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ, worst {worst}");
}

#[test]
fn the_gpu_carrier_matches_the_core_kernel() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 28);
    let image = frame(width, height);
    let config = FinishConfig {
        carrier_width: 2.0,
        carrier_rough: 0.25,
        carrier_flare: 0.5,
        carrier_corner: 0.25,
        ..FinishConfig::default()
    };
    let width_px = negpy_backend::features::finish::processor::carrier_width_px(
        config.carrier_width,
        30.0,
        width.max(height) as f64,
    );
    let paper = [1.0; 3];
    let want = apply_carrier(
        &image,
        width_px,
        config.carrier_rough,
        config.carrier_flare,
        config.carrier_corner,
        paper,
        None,
    );
    let uniforms =
        FinishUniforms::from_config(&config, width, height, paper).with_carrier(width_px as f32);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "finish");
    run_with_tables(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &uniforms,
        &carrier_profiles(),
        &negpy_backend::features::finish::logic::linear_carrier_tone(),
    );
    let read = output.readback(&gpu);
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.pixels.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > 3e-3 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn a_rebate_tone_table_matches_the_core_carrier() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 28);
    let image = frame(width, height);
    let mut settings = negpy_backend::domain::models::WorkspaceConfig::default();
    settings.finish.carrier_width = 2.0;
    settings.finish.carrier_rough = 0.25;
    settings.finish.carrier_flare = 0.5;
    settings.finish.carrier_corner = 0.25;
    let bounds = negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
        [-1.8, -1.7, -1.6],
        [-0.2, -0.1, -0.05],
    );
    let tone = rebate_tone(
        &settings,
        RebateMetrics {
            final_bounds: Some(bounds),
        },
    );
    let paper = negpy_backend::services::export::print::effective_paper_linear(
        &settings.finish,
        &settings.toning,
    );
    let width_px = negpy_backend::features::finish::processor::carrier_width_px(
        settings.finish.carrier_width,
        30.0,
        width.max(height) as f64,
    );
    let want = apply_carrier(
        &image,
        width_px,
        settings.finish.carrier_rough,
        settings.finish.carrier_flare,
        settings.finish.carrier_corner,
        paper,
        Some(&tone),
    );
    let uniforms = FinishUniforms::from_config(&settings.finish, width, height, paper)
        .with_carrier(width_px as f32);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "finish");
    run_with_tables(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &uniforms,
        &carrier_profiles(),
        &tone,
    );
    let read = output.readback(&gpu);
    let failures = (0..want.pixels.len())
        .filter(|index| (read[index / 3 * 4 + index % 3] - want.pixels[*index]).abs() > 1e-2)
        .count();
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ");
}
