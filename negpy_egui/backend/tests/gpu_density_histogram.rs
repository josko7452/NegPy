//! The GPU density histogram matches the core bins on a uniform frame.

use negpy_backend::features::exposure::analysis::density_histogram;
use negpy_backend::features::exposure::histogram_shader::{DensityHistogramUniforms, run};
use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

#[test]
fn the_bins_match_the_core_histogram() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let mut normalized = Vec::with_capacity(width * height * 3);
    for index in 0..width * height {
        let value = (index as f32 / (width * height) as f32).clamp(0.0, 1.0);
        normalized.extend_from_slice(&[value, 1.0 - value, 0.5 * value + 0.25]);
    }
    let want = density_histogram(&normalized, width, height, None).expect("a histogram");
    let bounds = LogNegativeBounds::new([-1.5, -1.5, -1.5], [-0.1, -0.1, -0.1]);
    let uniforms = DensityHistogramUniforms::new((0, height, 0, width), bounds);
    let normalized_tex = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    normalized_tex.upload(&gpu.queue, &normalized, false);
    let geometry = GpuTexture::new(&gpu.device, width as u32, height as u32, "geometry");
    geometry.upload(&gpu.queue, &vec![0.0f32; width * height * 3], false);
    let got = run(
        &gpu,
        &ShaderLoader::default(),
        &normalized_tex,
        &geometry,
        &uniforms,
    );
    assert_eq!(got.len(), want.len());
    let mut worst = 0.0f64;
    for (a, b) in got.iter().zip(&want) {
        worst = worst.max((a - b).abs());
    }
    // The shader bins an edge value in float32 and the core snaps with an edge
    // comparison, so one value can land in the next bin.
    let budget = (width * height) as f64 * 0.01;
    assert!(worst <= budget, "bins differ by {worst}");
}
