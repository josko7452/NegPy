//! The GPU-first preview: measure on the CPU, render the base on the GPU.
//!
//! A print frame without local maps, a print LUT or a carrier runs here. The
//! CPU measures the base and builds the solved print; the GPU runs geometry,
//! normalization, the print curve, CLAHE, Lab and toning with the crop. The
//! finish stage stays on the CPU.

use crate::domain::models::{RenderIntent, WorkspaceConfig};
use crate::features::exposure::exposure_shader::{ExposureRun, GpuPlanes};
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::papers::{PrintColor, effective_paper_profile, resolve_print_color};
use crate::features::finish::processor::{FinishContext, process as finish_process};
use crate::features::geometry::processor::GeometryContext;
use crate::features::process::path::{RenderPath, is_mono, render_path};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::render_engine::PipelineContext;
use crate::services::rendering::gpu_engine::{BaseRun, GpuEngine};

/// The measured base the GPU exposure pass reads.
#[derive(Clone, Debug)]
pub struct GpuBase {
    pub bounds: LogNegativeBounds,
    pub norm_density_range: f64,
    pub textural_range: f64,
    pub shadow_point: f64,
    pub highlight_point: f64,
    pub anchor: Option<f64>,
    pub active_roi: Option<crate::features::exposure::metering::Roi>,
}

impl GpuBase {
    /// Record the measured values the later stages read from the context.
    pub fn metrics(&self, context: &mut PipelineContext) {
        context.active_roi = self.active_roi;
        context.metrics.insert(
            "norm_density_range".to_string(),
            serde_json::json!(self.norm_density_range),
        );
        context.metrics.insert(
            "metered_anchor".to_string(),
            serde_json::json!(self.anchor.unwrap_or(0.0)),
        );
        context.metrics.insert(
            "textural_range".to_string(),
            serde_json::json!(self.textural_range),
        );
        context.metrics.insert(
            "shadow_point".to_string(),
            serde_json::json!(self.shadow_point),
        );
        context.metrics.insert(
            "highlight_point".to_string(),
            serde_json::json!(self.highlight_point),
        );
        context.metrics.insert(
            "final_bounds".to_string(),
            serde_json::to_value(self.bounds).unwrap_or(serde_json::Value::Null),
        );
        context.metrics.insert(
            "pre_trim_bounds".to_string(),
            serde_json::to_value(self.bounds).unwrap_or(serde_json::Value::Null),
        );
    }
}

/// Whether this config can run the current GPU path without dropping a stage.
#[must_use]
pub fn gpu_eligible(config: &WorkspaceConfig, context: &PipelineContext) -> bool {
    gpu_rejection(config, context).is_none()
}

/// Why the GPU chain cannot paint this frame, or `None` when it can.
#[must_use]
pub fn gpu_rejection(config: &WorkspaceConfig, context: &PipelineContext) -> Option<&'static str> {
    if render_path(&config.process) != RenderPath::Print
        || config.exposure.render_intent != RenderIntent::Print
    {
        return Some("not a print frame");
    }
    if context.crop_preview_full {
        return Some("full-frame crop preview");
    }
    if config.finish.carrier_width > 0.0 {
        return Some("carrier");
    }
    // The alt-process passes and the grain measurement are not on the device yet.
    if config.altproc.alt_process != crate::domain::models::AltProcess::None
        && config.process.process_mode == crate::domain::models::ProcessMode::BW
    {
        return Some("alt process");
    }
    let paper = effective_paper_profile(
        &config.exposure.paper_profile,
        Some(config.process.process_mode),
    );
    if !matches!(resolve_print_color(Some(paper)), (None, None))
        || !matches!(paper.print_color, PrintColor::Matrix(_))
    {
        return Some("print LUT");
    }
    None
}

#[cfg(test)]
mod rejection_tests {
    use super::*;

    #[test]
    fn a_carrier_frame_reports_the_carrier() {
        let mut config = WorkspaceConfig::default();
        config.finish.carrier_width = 1.0;
        assert_eq!(
            gpu_rejection(&config, &PipelineContext::new()),
            Some("carrier")
        );
    }

    #[test]
    fn a_full_frame_preview_reports_the_preview() {
        let mut context = PipelineContext::new();
        context.crop_preview_full = true;
        assert_eq!(
            gpu_rejection(&WorkspaceConfig::default(), &context),
            Some("full-frame crop preview")
        );
    }
}

/// Run the base, exposure, CLAHE, Lab and toning stages on the GPU, then finish.
#[must_use]
pub fn render(
    gpu: &mut GpuEngine,
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    source_key: &str,
    base: &GpuBase,
    context: &mut PipelineContext,
) -> ImageBuffer {
    base.metrics(context);
    gpu.set_source(source_key);
    let roi = crate::features::geometry::processor::process(
        image,
        &config.geometry,
        GeometryContext {
            scale_factor: image.height.max(image.width) as f64 / 4000.0,
        },
    )
    .active_roi;
    let run = BaseRun {
        bounds: base.bounds,
        stage: crate::features::transparency::logic::DensityStage {
            pre: None,
            unmix: None,
            post: None,
            anchor: None,
        },
        crossover: vec![0u8; crate::features::exposure::shader::CROSSOVER_TABLE_BYTES],
        transfer: false,
    };
    let print = ExposureRun {
        bounds: Some(base.bounds),
        density_range: Some(base.norm_density_range),
        textural_range: Some(base.textural_range),
        shadow_point: Some(base.shadow_point),
        highlight_point: Some(base.highlight_point),
        anchor: base.anchor,
        black_hold: if config.exposure.hold_black {
            crate::features::exposure::constants::ZONE_BLACK_HOLD_TAPER
        } else {
            0.0
        },
        ..ExposureRun::plain(
            &config.exposure,
            &config.process,
            config.process.process_mode,
        )
    };
    let planes = build_planes(image, config, &print, roi);
    let print = planes.attach(print);
    let (toned, lab_out, density_histogram) = gpu.run_tail_input(
        image,
        config,
        &run,
        &print,
        crate::features::lab::lab_shader::LabRun {
            mono: is_mono(&config.process),
            scale_factor: image.height.max(image.width) as f64 / 4000.0,
            auto_masking: None,
        },
        roi,
    );
    to_metrics(context, &lab_out);
    context.metrics.insert(
        "histogram_density".to_string(),
        serde_json::to_value(density_histogram).unwrap_or(serde_json::Value::Null),
    );
    finish(config, toned)
}

fn to_metrics(context: &mut PipelineContext, lab: &crate::features::lab::lab_shader::LabOutput) {
    if let Some(share) = lab.auto_masking {
        context
            .metrics
            .insert("auto_masking".to_string(), serde_json::json!(share));
    }
}

fn finish(config: &WorkspaceConfig, toned: ImageBuffer) -> ImageBuffer {
    let image = finish_process(
        &toned,
        &config.finish,
        FinishContext {
            print_size_cm: 30.0,
            paper: crate::services::export::print::effective_paper_linear(
                &config.finish,
                &config.toning,
            ),
            crop_preview_full: false,
        },
        None,
    );
    let encoded = crate::features::exposure::analysis::working_oetf_encode(&image.pixels);
    ImageBuffer::from_rgb(image.width, image.height, encoded)
}

/// Build the local maps and shared mask grid a GPU print binds.
#[must_use]
pub fn build_planes(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    print: &ExposureRun<'_>,
    roi: Option<crate::features::geometry::logic::Roi>,
) -> GpuPlanes {
    let mut planes = GpuPlanes {
        grid_rect: roi
            .map(|(y1, y2, x1, x2)| {
                [
                    x1.max(0) as f64,
                    y1.max(0) as f64,
                    (x2 - x1).max(0) as f64,
                    (y2 - y1).max(0) as f64,
                ]
            })
            .unwrap_or([0.0, 0.0, image.width as f64, image.height as f64]),
        diffusion_mix: if config.exposure.diffusion > 0.0 {
            crate::features::exposure::print::diffusion_mix(config.exposure.diffusion) as f32
        } else {
            0.0
        },
        ..GpuPlanes::default()
    };
    let frame_dims = (image.height, image.width);
    if !config.local.masks.is_empty() {
        let geometry = crate::features::local::logic::LocalGeometry {
            rotation: config.geometry.rotation,
            fine_rotation: config.geometry.fine_rotation,
            flip_horizontal: config.geometry.flip_horizontal,
            flip_vertical: config.geometry.flip_vertical,
            distortion_k1: config.geometry.distortion_k1,
            converge_v: config.geometry.converge_v,
            converge_h: config.geometry.converge_h,
        };
        let maps = crate::features::local::processor::build(
            &config.local,
            frame_dims.0,
            frame_dims.1,
            (image.height, image.width),
            geometry,
        );
        planes.local = Some((
            maps.stops_plane(),
            maps.flash_plane(),
            maps.grade_delta_plane(),
        ));
        planes.key_alpha = maps.key_alpha();
    }
    if config.local.masks.is_empty()
        && config.exposure.contrast_mask_strength <= 0.0
        && config.exposure.diffusion <= 0.0
    {
        planes.planarity = crate::features::exposure::exposure_shader::ExposurePlanarity::of(print);
        return planes;
    }
    let Some(bounds) = print.bounds else {
        return planes;
    };
    let unmix = crate::features::exposure::metering::effective_crosstalk_matrix(
        &config.process,
        Some(config.process.process_mode),
    );
    let weights =
        crate::features::exposure::bw_filter::plane_weights(&config.process, &config.exposure);
    let geometry = crate::features::exposure::normalization::PlaneGeometry::from_config(
        &config.geometry,
        config.geometry.distortion_k1,
    );
    let roi_norm = crate::features::exposure::normalization::normalized_roi(roi, frame_dims);
    let contrast = (config.exposure.contrast_mask_strength > 0.0).then(|| {
        crate::features::exposure::normalization::contrast_mask_plane(
            image, bounds, unmix, geometry, roi_norm, weights,
        )
    });
    let diffusion = (config.exposure.diffusion > 0.0)
        .then(|| {
            crate::features::exposure::normalization::diffusion_grid(
                image, unmix, geometry, roi_norm,
            )
        })
        .and_then(|grid| {
            crate::features::exposure::normalization::diffusion_plane(&grid, bounds, weights)
        });
    let dims = contrast
        .as_ref()
        .map(|(_, height, width, _)| (*height, *width))
        .or_else(|| {
            (config.exposure.diffusion > 0.0).then(|| {
                let long = image.width.max(image.height);
                let scale = if long > 1024 {
                    1024.0 / long as f64
                } else {
                    1.0
                };
                (
                    ((image.height as f64 * scale).round() as usize).max(1),
                    ((image.width as f64 * scale).round() as usize).max(1),
                )
            })
        });
    let Some((height, width)) = dims else {
        planes.planarity = crate::features::exposure::exposure_shader::ExposurePlanarity::of(print);
        return planes;
    };
    let mut grid = vec![0.0; height * width * 4];
    if let Some((plane, _, _, centre)) = contrast {
        planes.mask_centre = centre;
        if plane.len() == height * width {
            planes.planarity.contrast = true;
            for (texel, value) in grid.chunks_exact_mut(4).zip(plane) {
                texel[0] = value;
            }
        }
    }
    if let Some(diffusion) = diffusion.filter(|plane| plane.len() == height * width * 3) {
        planes.planarity.diffusion = true;
        for (texel, rgb) in grid.chunks_exact_mut(4).zip(diffusion.chunks_exact(3)) {
            texel[1..4].copy_from_slice(rgb);
        }
    }
    planes.grid = Some((grid, height, width));
    planes.planarity = crate::features::exposure::exposure_shader::ExposurePlanarity {
        local: !config.local.masks.is_empty(),
        keyed: planes.key_alpha.is_some(),
        contrast: planes.planarity.contrast,
        diffusion: planes.planarity.diffusion,
    };
    planes
}
