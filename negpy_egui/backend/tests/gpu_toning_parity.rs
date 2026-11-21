//! The GPU toning pass matches the CPU toning stage and crops in place.

use negpy_backend::domain::models::{AltProcess, ToningConfig, WorkspaceConfig};
use negpy_backend::features::toning::processor::process;
use negpy_backend::features::toning::shader::{ToningUniforms, run};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

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

fn settings() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.toning = ToningConfig {
        shadow_tint_hue: 30.0,
        shadow_tint_strength: 0.4,
        highlight_tint_hue: 210.0,
        highlight_tint_strength: 0.3,
        ..ToningConfig::default()
    };
    config
}

#[test]
fn the_gpu_split_toning_matches_the_cpu_stage() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (48, 40);
    let image = frame(width, height);
    let config = settings();
    let want = process(&image, &config.toning, AltProcess::None, false);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "linear");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "toning");
    run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &ToningUniforms::new(
            &config.toning,
            config.lab.saturation,
            false,
            AltProcess::None,
            [0, 0],
        ),
    );
    let read = output.readback(&gpu);
    let got: Vec<f32> = read
        .chunks_exact(4)
        .flat_map(|texel| [texel[0], texel[1], texel[2]])
        .collect();
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.pixels.iter().enumerate() {
        let value = got[index];
        worst = worst.max((value - expected).abs());
        if (value - expected).abs() > 2e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn the_toning_pass_crops_to_the_offset() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 32);
    let image = frame(width, height);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "linear");
    source.upload(&gpu.queue, &image.pixels, false);
    let (out_w, out_h) = (20, 16);
    let output = GpuTexture::new(&gpu.device, out_w, out_h, "toning");
    let config = ToningConfig::default();
    run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &ToningUniforms::new(&config, 1.0, false, AltProcess::None, [4, 3]),
    );
    let read = output.readback(&gpu);
    for y in 0..out_h as usize {
        for x in 0..out_w as usize {
            let want = image.pixels[((y + 3) * width + x + 4) * 3];
            let got = read[(y * out_w as usize + x) * 4];
            assert!(
                (got - want).abs() < 1e-4,
                "crop offset drifted at ({x}, {y})"
            );
        }
    }
}
