//! Pinned GPU stage output: a driver or shader change that moves the bits fails here.

use blake2::digest::consts::U8;
use blake2::{Blake2b, Digest};
use negpy_backend::domain::models::{FinishConfig, ProcessMode, WorkspaceConfig};
use negpy_backend::features::exposure::exposure_shader::{
    ExposureRun, ExposureUniforms, run as run_exposure,
};
use negpy_backend::features::finish::shader::{FinishUniforms, run as run_finish};
use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::GpuTexture;
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;

fn hash(values: &[f32]) -> String {
    let mut hasher = Blake2b::<U8>::new();
    for value in values {
        hasher.update(value.to_bits().to_ne_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn frame(width: usize, height: usize) -> Vec<f32> {
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

fn read(
    output: &GpuTexture,
    gpu: &negpy_backend::infrastructure::gpu::device::GpuDevice,
) -> Vec<f32> {
    let rgba = output.readback(gpu);
    rgba.chunks_exact(4)
        .flat_map(|texel| [texel[0], texel[1], texel[2]])
        .collect()
}

#[test]
fn the_print_shader_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let pixels = frame(width, height);
    let settings = WorkspaceConfig::default();
    let run = ExposureRun::plain(
        &settings.exposure,
        &settings.process,
        settings.process.process_mode,
    );
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "print");
    run_exposure(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &ExposureUniforms::pack(&run),
    );
    assert_eq!(hash(&read(&output, &gpu)), "ab82ed1ef0b4c31b");
}

#[test]
fn the_vignette_shader_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let pixels = frame(width, height);
    let config = FinishConfig {
        vignette_stops: 1.2,
        vignette_size: 0.35,
        vignette_roundness: 0.7,
        ..FinishConfig::default()
    };
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "source");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "finish");
    run_finish(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &FinishUniforms::from_config(&config, width, height, [1.0; 3]),
    );
    assert_eq!(hash(&read(&output, &gpu)), "2bcadf66e6b28d6c");
}

#[test]
fn a_per_channel_print_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (24, 18);
    let pixels = frame(width, height);
    let mut settings = WorkspaceConfig::default();
    settings.exposure.grade_trim_red = 12.0;
    settings.exposure.grade_trim_blue = -8.0;
    settings.exposure.toe_trim_green = 0.2;
    settings.exposure.shoulder_trim_red = -0.15;
    let run = ExposureRun::plain(&settings.exposure, &settings.process, ProcessMode::C41);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "normalized");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "print");
    run_exposure(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &ExposureUniforms::pack(&run),
    );
    assert_eq!(hash(&read(&output, &gpu)), "b16ee14d8e4e34b2");
}

#[test]
fn the_clahe_shader_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let pixels = frame(width, height);
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "clahe");
    negpy_backend::features::lab::clahe_shader::run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &negpy_backend::features::lab::clahe_shader::ClaheUniforms::for_frame(1.0, width, height),
    );
    assert_eq!(hash(&read(&output, &gpu)), "f0b3fbe07153e072");
}

#[test]
fn the_toning_shader_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let pixels = frame(width, height);
    let config = negpy_backend::domain::models::ToningConfig {
        shadow_tint_hue: 30.0,
        shadow_tint_strength: 0.4,
        highlight_tint_hue: 210.0,
        highlight_tint_strength: 0.3,
        ..negpy_backend::domain::models::ToningConfig::default()
    };
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "linear");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "toning");
    negpy_backend::features::toning::shader::run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &negpy_backend::features::toning::shader::ToningUniforms::new(
            &config,
            1.0,
            false,
            negpy_backend::domain::models::AltProcess::None,
            [0, 0],
        ),
    );
    assert_eq!(hash(&read(&output, &gpu)), "cfd86f63ff1ce507");
}

#[test]
fn the_lab_shader_keeps_its_bits() {
    let Some(gpu) = device() else {
        return;
    };
    let (width, height) = (32, 24);
    let pixels = frame(width, height);
    let config = negpy_backend::domain::models::LabConfig {
        saturation: 1.2,
        sharpen: 0.6,
        sharpen_masking: 0.0,
        chroma_denoise: 0.0,
        ..negpy_backend::domain::models::LabConfig::default()
    };
    let source = GpuTexture::new(&gpu.device, width as u32, height as u32, "encoded");
    source.upload(&gpu.queue, &pixels, false);
    let output = GpuTexture::new(&gpu.device, width as u32, height as u32, "lab");
    let _ = negpy_backend::features::lab::lab_shader::run(
        &gpu,
        &ShaderLoader::default(),
        &source,
        &output,
        &config,
        negpy_backend::features::lab::lab_shader::LabRun {
            mono: false,
            scale_factor: 1.0,
            auto_masking: None,
        },
        &negpy_backend::infrastructure::gpu::resources::TexturePool::default(),
    );
    assert_eq!(hash(&read(&output, &gpu)), "0f912719594648be");
}
