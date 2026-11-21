//! The GPU CLAHE passes match `negpy_core::lab::clahe` on an encoded frame.

use negpy_backend::features::lab::clahe_shader::{ClaheUniforms, run};
use negpy_backend::features::lab::logic::apply_clahe;
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

fn encoded_expectation(width: usize, height: usize) -> (ImageBuffer, ImageBuffer) {
    let linear = frame(width, height);
    let want = ImageBuffer::from_rgb(
        width,
        height,
        negpy_backend::features::exposure::analysis::working_oetf_encode(
            &apply_clahe(&linear, 1.0).pixels,
        ),
    );
    let encoded = ImageBuffer::from_rgb(
        width,
        height,
        negpy_backend::features::exposure::analysis::working_oetf_encode(&linear.pixels),
    );
    (encoded, want)
}

fn render(encoded: &ImageBuffer, width: usize, height: usize) -> Vec<f32> {
    let Some(gpu) = device() else {
        return Vec::new();
    };
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    source.upload(&gpu.queue, &encoded.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "clahe");
    run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &ClaheUniforms::for_frame(1.0, width, height),
    );
    output.readback(&gpu)
}

fn compare(want: &ImageBuffer, read: &[f32], tolerance: f32) -> (usize, f32) {
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.pixels.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > tolerance {
            failures += 1;
        }
    }
    (failures, worst)
}

#[test]
fn the_gpu_clahe_matches_the_core_kernel() {
    if device().is_none() {
        return;
    }
    let (width, height) = (64, 64);
    let (encoded, want) = encoded_expectation(width, height);
    let read = render(&encoded, width, height);
    let (failures, worst) = compare(&want, &read, 2e-2);
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ, worst {worst}");
}

/// A dimension that is not a multiple of 8 leaves the last tile row empty, and
/// `clahe_cdf.wgsl` divides that tile's zero count by its zero total.
#[test]
fn an_empty_tile_row_matches_the_core_after_the_divide_fix() {
    if device().is_none() {
        return;
    }
    let (width, height) = (49, 49);
    let (encoded, want) = encoded_expectation(width, height);
    let read = render(&encoded, width, height);
    let (failures, worst) = compare(&want, &read, 2e-2);
    let fraction = failures as f64 / want.pixels.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ, worst {worst}");
}
