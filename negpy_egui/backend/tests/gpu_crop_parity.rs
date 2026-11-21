//! The GPU crop matches the CPU crop window.

use negpy_backend::features::geometry::crop_shader::run;
use negpy_backend::features::geometry::processor::crop_image;
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
                y as f32 / height as f32,
                (x + y) as f32 / (width + height) as f32,
            ]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_gpu_crop_matches_the_cpu_window() {
    let Some(gpu) = device() else {
        return;
    };
    let image = frame(24, 18);
    let roi = Some((3i64, 15i64, 5i64, 21i64));
    let want = crop_image(&image, roi);
    let (output, uniforms) = negpy_backend::features::geometry::crop_shader::target(
        &gpu,
        roi.expect("a crop"),
        image.width,
        image.height,
        "crop",
    );
    let source = GpuTexture::new(
        &gpu.device,
        image.width as u32,
        image.height as u32,
        "source",
    );
    source.upload(&gpu.queue, &image.pixels, false);
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);
    assert_eq!(
        (output.width as usize, output.height as usize),
        (want.width, want.height)
    );
    let mut failures = 0usize;
    for (index, expected) in want.pixels.iter().enumerate() {
        if (read[index / 3 * 4 + index % 3] - expected).abs() > 1e-6 {
            failures += 1;
        }
    }
    assert_eq!(failures, 0);
}
