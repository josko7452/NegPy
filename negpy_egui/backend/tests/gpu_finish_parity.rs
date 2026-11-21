//! The GPU vignette matches the core finish kernel.

use negpy_backend::domain::models::FinishConfig;
use negpy_backend::features::finish::logic::apply_vignette;
use negpy_backend::features::finish::shader::{FinishUniforms, run};
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
