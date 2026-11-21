//! The display encode matches `working_oetf_encode` within one float step.

use negpy_backend::features::exposure::analysis::working_oetf_encode;
use negpy_backend::features::exposure::shader::run_encode;
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

#[test]
fn the_encode_matches_the_cpu() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 24);
    let already: Vec<f32> = (0..width * height)
        .flat_map(|index| {
            let value = (index % (width * height)) as f32 / (width * height) as f32;
            [value, (value * 0.8).min(1.0), (value * 0.6).min(1.0)]
        })
        .collect();
    let want = working_oetf_encode(&already);

    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "linear");
    source.upload(&gpu.queue, &already, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    run_encode(&gpu, &ShaderLoader::default(), &source, &output);
    let read = output.readback(&gpu);

    let mut worst = 0.0f32;
    for (index, want) in want.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        worst = worst.max((got - want).abs());
    }
    assert!(worst < 2e-6, "worst difference {worst}");
}
