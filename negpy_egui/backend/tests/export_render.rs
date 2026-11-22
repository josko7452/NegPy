//! Full-resolution export render checks over a decoded fixture.

use std::path::Path;

use negpy_backend::domain::models::{ExportResolutionMode, WorkspaceConfig};
use negpy_backend::render_engine::DarkroomEngine;
use negpy_backend::services::export::render::{
    ExportEngine, render_export_file, sharpens_after_resize,
};

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
