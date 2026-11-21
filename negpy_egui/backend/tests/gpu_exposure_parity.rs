//! The GPU print curve matches the CPU print and its working-space encode.

use negpy_backend::domain::models::{ProcessMode, WorkspaceConfig};
use negpy_backend::features::exposure::analysis::working_oetf_encode;
use negpy_backend::features::exposure::exposure_shader::{self, ExposureRun, ExposureUniforms};
use negpy_backend::features::exposure::print::{PrintRenderSettings, render_negative};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

fn normalized(width: usize, height: usize) -> Vec<f32> {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            pixels.extend_from_slice(&[
                (0.05 + 0.9 * fx).min(1.0),
                (0.1 + 0.8 * fy).min(1.0),
                (0.2 + 0.6 * (fx + fy) * 0.5).min(1.0),
            ]);
        }
    }
    pixels
}

#[test]
fn the_gpu_print_matches_the_cpu_curve() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (40, 28);
    let pixels = normalized(width, height);
    let settings = WorkspaceConfig::default();
    let bounds = negpy_backend::features::exposure::normalization::LogNegativeBounds::new(
        [-1.2, -1.1, -1.0],
        [-0.1, -0.12, -0.08],
    );
    let run = ExposureRun {
        bounds: Some(bounds),
        density_range: Some(1.0),
        textural_range: Some(0.8),
        ..ExposureRun::plain(
            &settings.exposure,
            &settings.process,
            settings.process.process_mode,
        )
    };
    let print = PrintRenderSettings {
        config: &settings.exposure,
        process: &settings.process,
        density_range: run.density_range,
        textural_range: run.textural_range,
        shadow_point: None,
        highlight_point: None,
        anchor: None,
        bounds: run.bounds,
        neutral_axis: None,
        mode: settings.process.process_mode,
        base_black_hold: 0.0,
        mask: None,
        diffusion: None,
        local_exposure: None,
        local_grade: None,
        local_flash: None,
        key_params: None,
        key_alpha: None,
        grade_deltas: None,
    };
    let want = working_oetf_encode(&render_negative(&pixels, width, &print));
    let uniforms = ExposureUniforms::pack(&run);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "print");
    exposure_shader::run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);
    let mut failures = 0usize;
    let mut worst = 0.0f32;
    for (index, expected) in want.iter().enumerate() {
        let got = read[index / 3 * 4 + index % 3];
        worst = worst.max((got - expected).abs());
        if (got - expected).abs() > 2e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ, worst {worst}");
}

#[test]
fn a_bw_print_matches_the_cpu_curve() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (24, 18);
    let pixels = normalized(width, height);
    let mut settings = WorkspaceConfig::default();
    settings.process.process_mode = ProcessMode::BW;
    let run = ExposureRun::plain(&settings.exposure, &settings.process, ProcessMode::BW);
    let print = PrintRenderSettings {
        config: &settings.exposure,
        process: &settings.process,
        density_range: None,
        textural_range: None,
        shadow_point: None,
        highlight_point: None,
        anchor: None,
        bounds: None,
        neutral_axis: None,
        mode: ProcessMode::BW,
        base_black_hold: 0.0,
        mask: None,
        diffusion: None,
        local_exposure: None,
        local_grade: None,
        local_flash: None,
        key_params: None,
        key_alpha: None,
        grade_deltas: None,
    };
    let want = working_oetf_encode(&render_negative(&pixels, width, &print));
    let uniforms = ExposureUniforms::pack(&run);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "print");
    exposure_shader::run(&gpu, &ShaderLoader::default(), &source, &output, &uniforms);
    let read = output.readback(&gpu);
    let mut failures = 0usize;
    for (index, expected) in want.iter().enumerate() {
        if (read[index / 3 * 4 + index % 3] - expected).abs() > 2e-2 {
            failures += 1;
        }
    }
    let fraction = failures as f64 / want.len() as f64;
    assert!(fraction < 0.01, "{failures} samples differ");
}
