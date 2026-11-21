//! The normalization pass applies a camera matrix before the log, as the core does.

use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::features::exposure::shader::{
    CROSSOVER_TABLE_BYTES, NormalizationSetup, NormalizationUniforms, run,
};
use negpy_backend::features::transparency::logic::DensityStage;
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

#[test]
fn a_camera_matrix_matches_the_core() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let mut image = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            image.extend_from_slice(&[0.1 + 0.6 * fx, 0.2 + 0.5 * fy, 0.3]);
        }
    }
    let camera = [
        [0.95f64, 0.03, 0.02],
        [0.01, 1.01, -0.02],
        [0.0, 0.04, 0.96],
    ];
    let stage = DensityStage {
        pre: Some(camera),
        unmix: None,
        post: None,
        anchor: None,
    };
    let bounds = LogNegativeBounds::new([-1.2; 3], [-0.15; 3]);
    let uniforms = NormalizationUniforms::pack(
        NormalizationSetup {
            stage: &stage,
            bounds,
            mono: false,
            transfer: true,
        },
        &vec![0u8; CROSSOVER_TABLE_BYTES],
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &image, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);

    let linear =
        negpy_backend::features::process::capture_color::apply_camera_matrix(&image, Some(camera));
    let mut log = vec![0.0; image.len()];
    negpy_core::analysis::log_density_open(&linear, &mut log);
    let mut want = vec![0.0; image.len()];
    negpy_core::analysis::normalize_log_image(
        &log,
        bounds.floors.map(|value| value as f32),
        bounds.ceils.map(|value| value as f32),
        &mut want,
    );
    let mut failures = 0usize;
    for (index, expected) in want.iter().enumerate() {
        if (read[index / 3 * 4 + index % 3] - expected).abs() > 2e-5 {
            failures += 1;
        }
    }
    assert_eq!(failures, 0);
}
