//! The full-resolution export render: source decode to a laid-out float buffer.
//!
//! Mirrors `ImageProcessor._render_export_buffer` and `_prepare_export_source`: the
//! source is decoded once, the source bakes run in linear space, and the pipeline runs
//! without stage caching because nothing else reads its checkpoints.

use std::path::Path;

use crate::domain::models::{ExportConfig, FinishConfig, WorkspaceConfig};
use crate::features::lab::logic::apply_resize_sharpening;
use crate::features::retouch::processor::detect_dust;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::render_engine::{DarkroomEngine, PipelineContext};
use crate::services::export::print::{apply_layout, effective_border_color};
use crate::services::rendering::gpu_engine::GpuEngine;
use crate::services::rendering::gpu_render::{self, GpuBase};
use crate::services::rendering::import::{import_source, precorrect_source};
use crate::services::rendering::lens::flatfield_source;
use crate::services::rendering::retouch::IrPlane;

/// One export render before color management.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportRender {
    pub image: ImageBuffer,
    /// The color space the rendered buffer is in.
    pub color_space: String,
    /// The picture's `(x, y, width, height)` inside the paper.
    pub content_rect: (usize, usize, usize, usize),
    /// The engine that rendered the picture.
    pub engine: ExportEngine,
}

/// Which engine painted an export, or why the GPU did not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportEngine {
    Gpu,
    Cpu(String),
}

impl ExportEngine {
    /// The label a report or a log writes.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Gpu => "GPU".to_string(),
            Self::Cpu(reason) => format!("CPU ({reason})"),
        }
    }
}

/// The source corrections that change before the pipeline reads the frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExportBakes {
    pub retouch: bool,
}

/// Decode, bake and render one file at full resolution.
pub fn render_export_file(
    path: &Path,
    config: &WorkspaceConfig,
    color_space: Option<&str>,
    engine: &mut DarkroomEngine,
) -> Result<ExportRender, String> {
    render_export_file_with(path, config, color_space, engine)
}

fn render_export_file_with(
    path: &Path,
    config: &WorkspaceConfig,
    color_space: Option<&str>,
    engine: &mut DarkroomEngine,
) -> Result<ExportRender, String> {
    let entry = load_flatfield(config);
    let imported = import_source(path, config, entry.as_ref(), &[])?;
    let (baked, retouch_tokens) = crate::services::rendering::retouch::bake_with_tokens(
        &imported.image,
        config,
        imported.ir.as_deref().map(|samples| IrPlane {
            samples,
            width: imported.ir_width,
            height: imported.ir_height,
        }),
        1600.0,
    );
    let mut context = PipelineContext {
        cache_stages: false,
        readback_metrics: false,
        wants_uv_grid: false,
        crop_preview_full: false,
        film_base: imported.base_log,
        source_base_log: imported.base_log,
        ..PipelineContext::default()
    };
    let source_hash = format!(
        "{}{}",
        crate::services::rendering::source_identity::source_token(config, None),
        retouch_tokens
    );
    let buffer = engine.process(&baked, config, &source_hash, &mut context);
    let border_color = effective_border_color(&config.finish, &config.toning);
    let layout = apply_layout(
        &buffer,
        &config.export,
        config.finish.border_size,
        &border_color,
        &config.finish,
    );
    let content_rect = layout.content_rect;
    let image = if sharpens_after_resize(config) {
        sharpen_rect(&layout.image, content_rect, &config.lab)
    } else {
        layout.image
    };
    Ok(ExportRender {
        image,
        color_space: color_space
            .map(str::to_string)
            .or(imported.color_space)
            .unwrap_or_else(|| {
                crate::infrastructure::display::color_mgmt::WORKING_COLOR_SPACE.to_string()
            }),
        content_rect,
        engine: ExportEngine::Cpu("full-resolution export".to_string()),
    })
}

/// Decode and render one file at full resolution on the GPU.
///
/// Returns the CPU reason when the chain cannot paint the frame or no device opens.
pub fn render_export_file_gpu(
    path: &Path,
    config: &WorkspaceConfig,
    color_space: Option<&str>,
    gpu: &mut GpuEngine,
) -> Result<ExportRender, String> {
    let entry = load_flatfield(config);
    let imported = import_source(path, config, entry.as_ref(), &[])?;
    let (baked, retouch_tokens) = crate::services::rendering::retouch::bake_with_tokens(
        &imported.image,
        config,
        imported.ir.as_deref().map(|samples| IrPlane {
            samples,
            width: imported.ir_width,
            height: imported.ir_height,
        }),
        1600.0,
    );
    let mut context = PipelineContext {
        cache_stages: false,
        readback_metrics: false,
        wants_uv_grid: false,
        crop_preview_full: false,
        film_base: imported.base_log,
        source_base_log: imported.base_log,
        ..PipelineContext::default()
    };
    if let Some(reason) = gpu_render::gpu_rejection(config, &context) {
        return Err(reason.to_string());
    }
    let roi = crate::features::geometry::processor::process(
        &baked,
        &config.geometry,
        crate::features::geometry::processor::GeometryContext {
            scale_factor: render_scale_factor(baked.height, baked.width),
        },
    )
    .active_roi;
    let measured = crate::features::exposure::processor::measure_negative(
        &baked.pixels,
        baked.width,
        baked.height,
        crate::features::exposure::processor::NormalizationSettings {
            config: &config.process,
            mode: config.process.process_mode,
            active_roi: roi.map(|(y1, y2, x1, x2)| {
                (
                    y1.max(0) as usize,
                    y2.max(0) as usize,
                    x1.max(0) as usize,
                    x2.max(0) as usize,
                )
            }),
            base_log: imported.base_log,
        },
    );
    let base = GpuBase {
        bounds: measured.final_bounds,
        norm_density_range: measured.norm_density_range,
        textural_range: measured.textural_range,
        shadow_point: measured.shadow_point,
        highlight_point: measured.highlight_point,
        anchor: Some(measured.metered_anchor),
        active_roi: roi.map(|(y1, y2, x1, x2)| {
            (
                y1.max(0) as usize,
                y2.max(0) as usize,
                x1.max(0) as usize,
                x2.max(0) as usize,
            )
        }),
    };
    base.metrics(&mut context);
    let source_hash = format!(
        "{}{}",
        crate::services::rendering::source_identity::source_token(config, None),
        retouch_tokens
    );
    let buffer = gpu_render::render(gpu, &baked, config, &source_hash, &base, &mut context);
    let border_color = effective_border_color(&config.finish, &config.toning);
    let layout = apply_layout(
        &buffer,
        &config.export,
        config.finish.border_size,
        &border_color,
        &config.finish,
    );
    let content_rect = layout.content_rect;
    let image = if sharpens_after_resize(config) {
        sharpen_rect(&layout.image, content_rect, &config.lab)
    } else {
        layout.image
    };
    Ok(ExportRender {
        image,
        color_space: color_space
            .map(str::to_string)
            .or(imported.color_space)
            .unwrap_or_else(|| {
                crate::infrastructure::display::color_mgmt::WORKING_COLOR_SPACE.to_string()
            }),
        content_rect,
        engine: ExportEngine::Gpu,
    })
}

fn render_scale_factor(height: usize, width: usize) -> f64 {
    height.max(width) as f64 / 4000.0
}

/// The flat-field profile the source decode bakes, when the config names one.
#[must_use]
pub fn load_flatfield(
    config: &WorkspaceConfig,
) -> Option<crate::features::flatfield::logic::GainEntry> {
    if !config.flatfield.apply || config.flatfield.profile_id.is_empty() {
        return None;
    }
    let dir = std::env::var_os("NEGPY_FLATFIELD_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::kernel::system::paths::default_user_dir().join("flatfield"));
    crate::services::assets::flatfield::FlatFieldProfiles::new(dir)
        .load_gain(&config.flatfield.profile_id)
}

/// A Print or Pixels export with Lab Sharpening on, or any with Auto Masking, sharpens
/// where the resize happened so the radius is in exported pixels.
#[must_use]
pub fn sharpens_after_resize(config: &WorkspaceConfig) -> bool {
    config.lab.sharpen > 0.0
        && (config.export.export_resolution_mode
            != crate::domain::models::ExportResolutionMode::Original
            || config.lab.sharpen_auto_masking)
        && config.exposure.render_intent != crate::domain::models::RenderIntent::Flat
}

fn sharpen_rect(
    buffer: &ImageBuffer,
    (x, y, width, height): (usize, usize, usize, usize),
    lab: &crate::domain::models::LabConfig,
) -> ImageBuffer {
    if width == 0 || height == 0 {
        return buffer.clone();
    }
    let mut pixels = buffer.pixels.clone();
    let mut content = vec![0.0; width * height * 3];
    for row in 0..height {
        let source = ((y + row) * buffer.width + x) * 3;
        let target = row * width * 3;
        content[target..target + width * 3]
            .copy_from_slice(&buffer.pixels[source..source + width * 3]);
    }
    let sharpened = apply_resize_sharpening(&ImageBuffer::from_rgb(width, height, content), lab);
    for row in 0..height {
        let target = ((y + row) * buffer.width + x) * 3;
        let source = row * width * 3;
        pixels[target..target + width * 3]
            .copy_from_slice(&sharpened.pixels[source..source + width * 3]);
    }
    ImageBuffer::from_rgb(buffer.width, buffer.height, pixels)
}

/// Bake the source repairs the config asks for, returning the buffer the pipeline reads.
///
/// Dust detection needs the IR plane, which `import_source` releases; this helper keeps
/// the luma path only and leaves the IR tiers to the caller that owns the plane.
#[must_use]
pub fn bake_luma_repairs(image: &ImageBuffer, config: &WorkspaceConfig) -> ImageBuffer {
    let Some(detection) = detect_dust(image, &config.retouch) else {
        return image.clone();
    };
    let Some(score) = detection.score else {
        return image.clone();
    };
    crate::features::retouch::logic::apply_score_repair(
        image,
        &score,
        image.height,
        image.width,
        true,
        None,
    )
}

/// The source a render reads after its flat field, kept separate so a caller can reuse
/// the decode while the bakes change.
#[must_use]
pub fn source_after_flatfield(
    path: &Path,
    config: &WorkspaceConfig,
) -> Result<ImageBuffer, String> {
    let entry = load_flatfield(config);
    let loaded = crate::infrastructure::loaders::factory::load(
        path,
        crate::infrastructure::loaders::factory::LoadOptions {
            linear_raw: config.process.linear_raw,
            positive_decode: crate::features::process::path::positive_decode(&config.process),
            load_ir: false,
        },
    )?;
    let image = loaded
        .image
        .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
    let image = crate::services::rendering::import::oriented(&image, loaded.orientation);
    let image = flatfield_source(
        &image,
        config,
        entry.as_ref(),
        &[],
        Some(loaded.orientation),
    );
    Ok(precorrect_source(
        &image,
        config,
        None,
        &[],
        loaded.orientation,
    ))
}

/// The border color an export lays down, exposed for callers that build their own layout.
#[must_use]
pub fn export_border_color(
    finish: &FinishConfig,
    toning: &crate::domain::models::ToningConfig,
) -> String {
    effective_border_color(finish, toning)
}

/// The export's effective DPI, for a caller that writes its own resolution tag.
#[must_use]
pub fn export_dpi(config: &ExportConfig) -> i64 {
    crate::services::export::print::effective_dpi(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_original_size_export_does_not_sharpen_after_the_resize() {
        let mut config = WorkspaceConfig::default();
        config.lab.sharpen = 2.0;
        config.export.export_resolution_mode =
            crate::domain::models::ExportResolutionMode::Original;
        assert!(!sharpens_after_resize(&config));
        config.lab.sharpen_auto_masking = true;
        assert!(sharpens_after_resize(&config));
    }

    #[test]
    fn a_flat_master_never_sharpens_after_the_resize() {
        let mut config = WorkspaceConfig::default();
        config.lab.sharpen = 2.0;
        config.exposure.render_intent = crate::domain::models::RenderIntent::Flat;
        assert!(!sharpens_after_resize(&config));
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn a_decoded_frame_reaches_a_laid_out_export() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
        if !fixture.exists() {
            return;
        }
        let mut config = WorkspaceConfig::default();
        config.export.export_resolution_mode =
            crate::domain::models::ExportResolutionMode::Original;
        config.export.paper_aspect_ratio = "original".into();
        let mut engine = DarkroomEngine::new();
        let render =
            render_export_file(&fixture, &config, Some("sRGB"), &mut engine).expect("a render");
        assert!(render.image.width > 0);
        assert!(render.image.height > 0);
        assert_eq!(render.color_space, "sRGB");
        assert!(render.content_rect.2 <= render.image.width);
    }

    #[test]
    fn a_flatfield_profile_is_loaded_from_the_user_directory() {
        let mut config = WorkspaceConfig::default();
        config.flatfield.apply = true;
        config.flatfield.profile_id = "missing".into();
        assert!(load_flatfield(&config).is_none());
    }
}
