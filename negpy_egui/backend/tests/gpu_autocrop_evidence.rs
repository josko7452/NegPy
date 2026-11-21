//! The border-evidence pass reports the same row and column energy as the CPU.

use negpy_backend::features::geometry::shader::border_evidence;
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

fn frame(width: usize, height: usize) -> Vec<f32> {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let bright = if (2..width - 2).contains(&x) && (2..height - 2).contains(&y) {
                0.9
            } else {
                0.05
            };
            pixels.extend_from_slice(&[bright, bright, bright]);
        }
    }
    pixels
}

#[test]
fn the_evidence_finds_a_bright_center() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (24, 16);
    let pixels = frame(width, height);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &pixels, false);
    let evidence = border_evidence(&gpu, &ShaderLoader::default(), &source);

    assert_eq!(evidence.rows.len(), height);
    assert_eq!(evidence.cols.len(), width);
    assert!(
        evidence.rows[height / 2] > evidence.rows[0],
        "the center row is brighter"
    );
    assert!(
        evidence.cols[width / 2] > evidence.cols[0],
        "the center column is brighter"
    );

    let nearest = |x: usize, y: usize| {
        let value = pixels[(y * width + x) * 3];
        let luma = value.powf(1.0 / 2.2);
        (luma * 1_000_000.0) as u64
    };
    let wanted: u64 = (0..width).map(|x| nearest(x, height / 2)).sum();
    let got = evidence.rows[height / 2];
    let tolerance = (wanted as f64) * 0.01;
    assert!(
        (got as f64 - wanted as f64).abs() < tolerance,
        "{got} vs {wanted}"
    );
}
