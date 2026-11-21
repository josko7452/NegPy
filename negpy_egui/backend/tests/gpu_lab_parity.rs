//! The GPU Lab pass matches the CPU Lab stage.

use negpy_backend::domain::models::LabConfig;
use negpy_backend::features::lab::lab_shader::{LabRun, run};
use negpy_backend::features::lab::processor::{LabContext, process};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::{GpuTexture, TexturePool};
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

fn compare(want: &ImageBuffer, read: &[f32], tolerance: f32) -> (usize, f32) {
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.pixels.iter().enumerate() {
        let got = read[index];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > tolerance {
            failures += 1;
        }
    }
    (failures, worst)
}

fn render(
    config: &LabConfig,
    linear: &ImageBuffer,
    width: usize,
    height: usize,
) -> (ImageBuffer, Vec<f32>) {
    let want = process(linear, config, false, LabContext { scale_factor: 1.0 });
    let Some(gpu) = device() else {
        return (want.image, Vec::new());
    };
    let encoded = ImageBuffer::from_rgb(
        width,
        height,
        negpy_backend::features::exposure::analysis::working_oetf_encode(&linear.pixels),
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    source.upload(&gpu.queue, &encoded.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "lab");
    let textures = TexturePool::default();
    let _ = run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        config,
        LabRun {
            mono: false,
            scale_factor: 1.0,
            auto_masking: None,
        },
        &textures,
    );
    // `lab.wgsl` decodes its encoded input and writes scene-linear pixels.
    let read = output.readback(&gpu);
    let linear_out: Vec<f32> = read
        .chunks_exact(4)
        .flat_map(|texel| [texel[0], texel[1], texel[2]])
        .collect();
    (want.image, linear_out)
}

#[test]
fn the_gpu_lab_matches_the_cpu_stage() {
    if device().is_none() {
        return;
    }
    let (width, height) = (64, 64);
    let linear = frame(width, height);
    let config = LabConfig {
        sharpen: 0.8,
        sharpen_masking: 0.0,
        chroma_denoise: 0.0,
        ..LabConfig::default()
    };
    let (want, read) = render(&config, &linear, width, height);
    let (failures, worst) = compare(&want, &read, 3e-2);
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn the_gpu_saturation_matches_the_cpu_stage() {
    if device().is_none() {
        return;
    }
    let (width, height) = (48, 48);
    let linear = frame(width, height);
    let config = LabConfig {
        saturation: 1.3,
        skin_protection: 0.0,
        sharpen: 0.0,
        chroma_denoise: 0.0,
        ..LabConfig::default()
    };
    let (want, read) = render(&config, &linear, width, height);
    let (failures, worst) = compare(&want, &read, 3e-2);
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn the_gpu_chroma_denoise_matches_the_cpu_stage() {
    if device().is_none() {
        return;
    }
    let (width, height) = (64, 64);
    let linear = frame(width, height);
    let config = LabConfig {
        chroma_denoise: 6.0,
        sharpen: 0.0,
        saturation: 1.0,
        ..LabConfig::default()
    };
    let (want, read) = render(&config, &linear, width, height);
    let (failures, worst) = compare(&want, &read, 3e-2);
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ, worst {worst}");
}

#[test]
fn the_gpu_auto_masking_matches_the_core_meter() {
    let (width, height) = (64, 64);
    // A flat field with slight grain, so the meter has midtones to read.
    let mut linear = frame(width, height);
    for (index, value) in linear.pixels.iter_mut().enumerate() {
        *value += 0.004 * ((index * 7 % 11) as f32 / 10.0 - 0.5) + 0.3;
    }
    let config = LabConfig {
        sharpen: 0.8,
        sharpen_radius: 1.0,
        sharpen_masking: 0.1,
        sharpen_auto_masking: true,
        chroma_denoise: 0.0,
        ..LabConfig::default()
    };
    let want = negpy_backend::features::lab::logic::auto_masking(&linear, &config)
        .expect("auto masking is on");
    let Some(gpu) = device() else {
        return;
    };
    let encoded = ImageBuffer::from_rgb(
        width,
        height,
        negpy_backend::features::exposure::analysis::working_oetf_encode(&linear.pixels),
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    source.upload(&gpu.queue, &encoded.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "lab");
    let textures = TexturePool::default();
    let out = run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &config,
        LabRun {
            mono: false,
            scale_factor: 1.0,
            auto_masking: None,
        },
        &textures,
    );
    let got = out.auto_masking.expect("the pass measured a share");
    assert!((got - want).abs() < 2e-2, "masking {got} vs {want}");
}
