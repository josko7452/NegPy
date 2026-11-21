//! The normalization shader matches `negpy_core`'s stretch for the same bounds.

use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::features::exposure::shader::{
    CROSSOVER_TABLE_BYTES, NormalizationSetup, NormalizationUniforms, run,
};
use negpy_backend::features::transparency::logic::DensityStage;
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

fn compare(bounds: LogNegativeBounds) {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (48, 32);
    let image = frame(width, height);
    let mut log = vec![0.0; image.pixels.len()];
    negpy_core::analysis::log_density_open(&image.pixels, &mut log);
    let mut want = vec![0.0; image.pixels.len()];
    negpy_core::analysis::normalize_log_image(
        &log,
        bounds.floors.map(|value| value as f32),
        bounds.ceils.map(|value| value as f32),
        &mut want,
    );

    let stage = DensityStage {
        pre: None,
        unmix: None,
        post: None,
        anchor: None,
    };
    let uniforms = NormalizationUniforms::pack(
        NormalizationSetup {
            stage: &stage,
            bounds,
            mono: false,
            transfer: false,
        },
        &vec![0u8; CROSSOVER_TABLE_BYTES],
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);

    let mut violations = 0usize;
    for (index, want) in want.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        if (got - want).abs() > 1e-5 {
            violations += 1;
        }
    }
    assert!(
        violations == 0,
        "{violations} of {} samples differ",
        want.len()
    );
}

#[test]
fn the_stretch_matches_the_core_kernel() {
    let bounds = LogNegativeBounds::new([-1.6, -1.5, -1.4], [-0.1, -0.12, -0.08]);
    compare(bounds);
}

#[test]
fn a_degenerate_span_uses_the_epsilon() {
    let bounds = LogNegativeBounds::new([-1.4, -1.4, -1.4], [-0.2, -0.2, -0.2]);
    compare(bounds);
}

#[test]
fn a_density_stage_and_crossover_match_the_core_kernel() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let image = frame(width, height);
    let bounds = LogNegativeBounds::new([-1.5, -1.4, -1.3], [-0.15, -0.1, -0.2]);
    let matrix = [[1.05f64, -0.05, 0.0], [0.02, 0.97, 0.01], [0.0, 0.03, 0.95]];
    let stage = DensityStage {
        pre: None,
        unmix: Some(std::array::from_fn(|row| {
            [matrix[row][0], matrix[row][1], matrix[row][2], 0.0]
        })),
        post: None,
        anchor: None,
    };
    let mut crossover = vec![0u8; CROSSOVER_TABLE_BYTES];
    crossover[0..4].copy_from_slice(&(-1.2f32).to_ne_bytes());
    crossover[4..8].copy_from_slice(&(-0.4f32).to_ne_bytes());
    crossover[8..12].copy_from_slice(&1.0f32.to_ne_bytes());
    let bins = [[0.002f32, -0.001, 0.0]; 64];
    for (index, row) in bins.iter().enumerate() {
        let base = 16 * (index + 1);
        for (lane, value) in row.iter().enumerate() {
            crossover[base + lane * 4..base + lane * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
    }

    let mut log = vec![0.0; image.pixels.len()];
    negpy_core::analysis::log_density_open(&image.pixels, &mut log);
    let mut unmixed = vec![0.0; log.len()];
    negpy_core::analysis::unmix_log(
        &log,
        matrix.map(|row| row.map(|value| value as f32)),
        &mut unmixed,
    );
    let mut crossed = vec![0.0; log.len()];
    negpy_core::analysis::apply_crossover(&unmixed, &bins, -1.2, 0.8, &mut crossed);
    let mut want = vec![0.0; log.len()];
    negpy_core::analysis::normalize_log_image(
        &crossed,
        bounds.floors.map(|value| value as f32),
        bounds.ceils.map(|value| value as f32),
        &mut want,
    );

    let uniforms = NormalizationUniforms::pack(
        NormalizationSetup {
            stage: &stage,
            bounds,
            mono: false,
            transfer: false,
        },
        &crossover,
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image.pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);
    let mut violations = 0usize;
    for (index, want) in want.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        if (got - want).abs() > 2e-5 {
            violations += 1;
        }
    }
    assert!(
        violations == 0,
        "{violations} of {} samples differ",
        want.len()
    );
}
