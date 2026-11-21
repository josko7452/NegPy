//! The GPU-first preview: measure on the CPU, render the base on the GPU.
//!
//! A print frame without local maps, a print LUT or a carrier runs here. The
//! CPU measures the base and builds the solved print; the GPU runs geometry,
//! normalization, the print curve, CLAHE, Lab and toning with the crop. The
//! finish stage stays on the CPU.

use crate::domain::models::{RenderIntent, WorkspaceConfig};
use crate::features::exposure::exposure_shader::ExposureRun;
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::papers::{PrintColor, effective_paper_profile, resolve_print_color};
use crate::features::finish::processor::{FinishContext, process as finish_process};
use crate::features::geometry::processor::GeometryContext;
use crate::features::process::hue::apply_hue_trim;
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
    fn metrics(&self, context: &mut PipelineContext) {
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
    if render_path(&config.process) != RenderPath::Print
        || config.exposure.render_intent != RenderIntent::Print
        || context.crop_preview_full
    {
        return false;
    }
    if !config.local.masks.is_empty() || config.finish.carrier_width > 0.0 {
        return false;
    }
    // The alt-process passes and the grain measurement are not on the device yet.
    if config.altproc.alt_process != crate::domain::models::AltProcess::None
        && config.process.process_mode == crate::domain::models::ProcessMode::BW
    {
        return false;
    }
    let paper = effective_paper_profile(
        &config.exposure.paper_profile,
        Some(config.process.process_mode),
    );
    matches!(resolve_print_color(Some(paper)), (None, None))
        && matches!(paper.print_color, PrintColor::Matrix(_))
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
    let (toned, lab_out) = gpu.run_tail_input(
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
    let toned = apply_hue_trim(&toned, config.process.hue_trim);
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
