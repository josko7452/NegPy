//! Full-resolution export render checks over a decoded fixture.

use std::path::Path;

use negpy_backend::domain::models::{ExportResolutionMode, WorkspaceConfig};
use negpy_backend::render_engine::DarkroomEngine;
use negpy_backend::services::export::render::{
    ExportEngine, render_export_file, render_export_file_gpu, sharpens_after_resize,
};
use negpy_backend::services::rendering::gpu_engine::GpuEngine;

fn fixture() -> Option<std::path::PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
    path.exists().then_some(path)
}

#[test]
fn a_full_resolution_render_returns_working_space_pixels() {
    let Some(path) = fixture() else {
        return;
    };
    let mut config = WorkspaceConfig::default();
    config.export.export_resolution_mode = ExportResolutionMode::Original;
    config.export.paper_aspect_ratio = "original".into();
    config.finish.border_size = 0.0;
    config.finish.vignette_stops = 0.0;
    let mut engine = DarkroomEngine::new();
    let render = render_export_file(&path, &config, None, &mut engine).expect("a render");
    assert!(render.image.width > 0);
    assert_eq!(
        render.color_space,
        negpy_backend::infrastructure::display::color_mgmt::WORKING_COLOR_SPACE
    );
}

#[test]
fn a_second_render_reuses_the_pipeline_uncached() {
    let Some(path) = fixture() else {
        return;
    };
    let mut config = WorkspaceConfig::default();
    config.export.export_resolution_mode = ExportResolutionMode::Original;
    config.export.paper_aspect_ratio = "original".into();
    let mut engine = DarkroomEngine::new();
    let first = render_export_file(&path, &config, Some("sRGB"), &mut engine).expect("first");
    let second = render_export_file(&path, &config, Some("sRGB"), &mut engine).expect("second");
    assert_eq!(first.image.width, second.image.width);
    assert_eq!(first.image.pixels, second.image.pixels);
}

#[test]
fn an_original_export_keeps_its_pixel_size() {
    let Some(path) = fixture() else {
        return;
    };
    let mut config = WorkspaceConfig::default();
    config.export.export_resolution_mode = ExportResolutionMode::Original;
    config.export.paper_aspect_ratio = "original".into();
    config.finish.border_size = 0.0;
    let mut engine = DarkroomEngine::new();
    let render = render_export_file(&path, &config, None, &mut engine).expect("a render");
    assert_eq!(
        (render.image.width, render.image.height),
        (render.content_rect.2, render.content_rect.3)
    );
    assert!(!sharpens_after_resize(&config));
}

#[test]
fn an_export_without_a_gpu_reports_the_cpu_fallback() {
    let Some(path) = fixture() else {
        return;
    };
    let mut config = WorkspaceConfig::default();
    config.export.export_resolution_mode = ExportResolutionMode::Original;
    config.export.paper_aspect_ratio = "original".into();
    let mut engine = DarkroomEngine::new();
    let render = render_export_file(&path, &config, None, &mut engine).expect("a render");
    match render.engine {
        ExportEngine::Cpu(reason) => assert_eq!(reason, "full-resolution export"),
        ExportEngine::Gpu => panic!("the export runs on the CPU"),
    }
}

#[test]
#[ignore = "G1-4: full-resolution GPU export still differs from the CPU"]
fn a_full_resolution_gpu_export_matches_the_cpu() {
    let Some(path) = fixture() else {
        return;
    };
    let Some(mut gpu) = GpuEngine::new() else {
        return;
    };
    let mut config = WorkspaceConfig::default();
    config.export.export_resolution_mode = ExportResolutionMode::Original;
    config.export.paper_aspect_ratio = "original".into();
    config.finish.border_size = 0.0;
    config.finish.vignette_stops = 0.0;
    let mut engine = DarkroomEngine::new();
    let cpu = render_export_file(&path, &config, None, &mut engine).expect("a CPU render");
    let gpu = render_export_file_gpu(&path, &config, None, &mut gpu).expect("a GPU render");
    assert_eq!(gpu.engine, ExportEngine::Gpu);
    assert_eq!(
        (gpu.image.width, gpu.image.height),
        (cpu.image.width, cpu.image.height)
    );
    let failures = gpu
        .image
        .pixels
        .iter()
        .zip(&cpu.image.pixels)
        .filter(|(got, want)| (*got - *want).abs() > 4e-2)
        .count();
    let fraction = failures as f64 / cpu.image.pixels.len() as f64;
    assert!(fraction < 0.02, "{failures} samples differ");
}
