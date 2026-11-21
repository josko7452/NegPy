//! The geometry shader and `geometry::logic` agree on a smooth frame.

use negpy_backend::domain::models::GeometryConfig;
use negpy_backend::features::geometry::processor::{GeometryContext, process};
use negpy_backend::features::geometry::shader::{GeometryUniforms, rotated_dims, run};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn smooth_frame(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            pixels.push(0.1 + 0.8 * fx);
            pixels.push(0.15 + 0.7 * fy);
            pixels.push(0.2 + 0.5 * (fx + fy) * 0.5);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn compare(config: GeometryConfig, width: usize, height: usize) {
    let Some(gpu) = device() else {
        return;
    };
    let image = smooth_frame(width, height);
    let cpu = process(&image, &config, GeometryContext { scale_factor: 1.0 });
    let (out_w, out_h) = rotated_dims(width, height, config.rotation);

    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, out_w as u32, out_h as u32, "output");
    let uniforms = GeometryUniforms::from_config(&config, out_w, out_h);
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);

    assert_eq!(cpu.image.width, out_w);
    assert_eq!(cpu.image.height, out_h);
    let mut violations = 0usize;
    for (index, want) in cpu.image.pixels.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        if (got - want).abs() > 2e-2 {
            violations += 1;
        }
    }
    let fraction = violations as f64 / cpu.image.pixels.len() as f64;
    assert!(
        fraction < 0.01,
        "{violations} of {} samples differ",
        cpu.image.pixels.len()
    );
}

#[test]
fn identity_matches_the_cpu() {
    compare(GeometryConfig::default(), 40, 24);
}

#[test]
fn a_quarter_turn_swaps_the_output() {
    compare(
        GeometryConfig {
            rotation: 1,
            ..GeometryConfig::default()
        },
        40,
        24,
    );
}

#[test]
fn a_mirror_matches_the_cpu() {
    compare(
        GeometryConfig {
            flip_horizontal: true,
            ..GeometryConfig::default()
        },
        40,
        24,
    );
}
