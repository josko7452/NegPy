//! Mirrors `negpy/services/rendering/engine.py`: the stage order and its checkpoints.
//!
//! The base, exposure, CLAHE and Lab stages each cache their result by config hash, so a
//! slider that feeds one stage re-runs that stage and the ones after it only. The base
//! reports the measured bounds the exposure stage reads.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::domain::models::{RenderIntent, WorkspaceConfig};
use crate::features::altprocess::process as alt_process;
use crate::features::exposure::auto_metrics::AutoSliderMetrics;
use crate::features::exposure::bw_filter::plane_weights;
use crate::features::exposure::metering::{Roi, effective_crosstalk_matrix};
use crate::features::exposure::normalization::{
    LogNegativeBounds, PlaneGeometry, contrast_mask_plane, diffusion_grid, diffusion_plane,
    expand_mask_plane, normalized_roi,
};
use crate::features::exposure::papers::effective_paper_profile;
use crate::features::exposure::print::{
    DiffusionPlane, FlatRenderSettings, MaskPlane, PrintRenderSettings, render_flat,
    render_negative,
};
use crate::features::exposure::processor::{
    NormalizationMetrics, NormalizationSettings, normalize_negative,
};
use crate::features::finish::processor::{
    FinishContext, RebateMetrics, process as finish_process, rebate_tone,
};
use crate::features::geometry::processor::{
    GeometryContext, crop_image, process as geometry_process,
};
use crate::features::lab::logic::apply_clahe;
use crate::features::lab::processor::{LabContext, process as lab_process};
use crate::features::process::fade::{
    MeasuredWhite, fade_white_target, render_white_space, with_fade_white,
};
use crate::features::process::hue::apply_hue_trim;
use crate::features::process::path::{RenderPath, is_mono, render_path};
use crate::features::toning::processor::process as toning_process;
use crate::features::transparency::base::{TransferBaseSettings, process as transfer_base};
use crate::features::transparency::logic::{slide_exposure, slide_mono_weights};
use crate::features::transparency::processor::{TransferContext, process as transfer_process};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::kernel::caching::logic::{CacheEntry, calculate_config_hash, calculate_value_hash};
use crate::kernel::caching::manager::PipelineCache;
use crate::services::rendering::base_read::base_log_for;

/// The frame values a render reads and writes beyond the config.
#[derive(Clone, Debug, Default)]
pub struct PipelineContext {
    pub active_roi: Option<Roi>,
    pub metrics: BTreeMap<String, Value>,
    pub crop_preview_full: bool,
    pub cache_stages: bool,
    pub readback_metrics: bool,
    pub wants_uv_grid: bool,
    pub film_base: Option<[f64; 3]>,
    pub source_base_log: Option<[f64; 3]>,
    pub fade_white: Option<MeasuredWhite>,
}

impl PipelineContext {
    /// The defaults of an interactive preview that reads its metrics back.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_roi: None,
            metrics: BTreeMap::new(),
            crop_preview_full: false,
            cache_stages: true,
            readback_metrics: true,
            wants_uv_grid: true,
            film_base: None,
            source_base_log: None,
            fade_white: None,
        }
    }
}

/// The values one cached stage produced and the metrics it reports.
#[derive(Clone, Debug, PartialEq)]
pub struct StageOutcome {
    pub image: ImageBuffer,
    pub metrics: BTreeMap<String, Value>,
    pub active_roi: Option<Roi>,
}

/// A key-comparable rendering of the fields one stage reads.
trait StageKey {
    fn stage_hash(&self) -> String;
}

impl StageKey for WorkspaceConfig {
    fn stage_hash(&self) -> String {
        calculate_config_hash(self)
    }
}

impl StageKey for crate::domain::models::GeometryConfig {
    fn stage_hash(&self) -> String {
        calculate_value_hash(self)
    }
}

impl StageKey for crate::domain::models::LabConfig {
    fn stage_hash(&self) -> String {
        calculate_value_hash(self)
    }
}

impl StageKey for f64 {
    fn stage_hash(&self) -> String {
        calculate_value_hash(self)
    }
}

/// The base stage's key: the fields its geometry and normalization read.
#[must_use]
pub fn base_key(settings: &WorkspaceConfig, context: &PipelineContext) -> String {
    let mut geometry = settings.geometry.clone();
    if context.crop_preview_full {
        geometry.crop_rect = None;
        geometry.autocrop_offset = 0;
    }
    let process = settings.process.clone();
    // The crosstalk matrix, the fade fields and the measured targets all move bounds.
    let value = serde_json::json!({
        "geometry": geometry,
        "process": process,
        "crosstalk_strength": process.crosstalk_strength,
        "crosstalk_process": process.crosstalk_process,
        "film_base": context.film_base,
        "cast_removal": slide_exposure(&process, &settings.exposure).cast_removal_strength > 0.0,
    });
    calculate_value_hash(&value)
}

/// The Plates the engine derives once per base: the Contrast Mask and Diffusion planes.
#[derive(Clone, Debug, Default)]
struct Planes {
    mask: Option<(String, Vec<f32>, usize, usize, f32)>,
    diffusion: Option<(String, Option<Vec<f32>>, usize, usize)>,
    local: Option<(String, crate::features::local::processor::LocalMaps)>,
}

/// The local channels, copied once for the print settings' borrowed slices.
#[derive(Default)]
struct LocalPlanes {
    stops: Option<Vec<f32>>,
    grade: Option<Vec<f32>>,
    flash: Option<Vec<f32>>,
    grade_deltas: Option<Vec<f32>>,
    key_alpha: Option<(Vec<f32>, usize)>,
    key_params: Option<Vec<[f64; 4]>>,
}

impl LocalPlanes {
    fn from(
        maps: Option<&crate::features::local::processor::LocalMaps>,
        local: &crate::domain::models::LocalAdjustmentsConfig,
        exposure: &crate::domain::models::ExposureConfig,
        mode: crate::domain::models::ProcessMode,
        metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
    ) -> Self {
        maps.map_or_else(Self::default, |maps| Self {
            stops: Some(maps.stops_plane()),
            grade: Some(maps.grade_plane(exposure.grade)),
            flash: Some(maps.flash_plane()),
            grade_deltas: Some(maps.grade_delta_plane()),
            key_alpha: maps.key_alpha(),
            key_params: crate::features::exposure::placement::limited_mask_params(
                local,
                exposure,
                Some(mode),
                metrics,
            ),
        })
    }
}

/// The derived planes an exposure pass reads on its own frame size.
#[derive(Clone, Debug, Default)]
struct ExposurePlanes {
    mask: Option<Vec<f32>>,
    mask_centre: f32,
    diffusion: Option<Vec<f32>>,
    diffusion_dims: (usize, usize),
    diffusion_rect: [f64; 4],
    local: Option<crate::features::local::processor::LocalMaps>,
}

/// The engine holds the stage checkpoints and the planes of the active source.
#[derive(Debug, Default)]
pub struct DarkroomEngine {
    pub cache: PipelineCache,
    planes: Planes,
}

impl DarkroomEngine {
    /// A fresh engine with no checkpoint.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the pipeline over one linear frame. `source_hash` identifies the source the
    /// decode produced; a change clears the checkpoints and the planes.
    pub fn process(
        &mut self,
        img: &ImageBuffer,
        settings: &WorkspaceConfig,
        source_hash: &str,
        context: &mut PipelineContext,
    ) -> ImageBuffer {
        let mut pipeline_changed = false;
        if !context.cache_stages {
            self.cache.clear();
            self.planes = Planes::default();
        }
        if self.cache.source_hash != source_hash {
            self.cache.clear();
            self.cache.source_hash = source_hash.to_string();
            self.planes = Planes::default();
            pipeline_changed = true;
        }
        let mode = settings.process.process_mode.as_str().to_string();
        if self.cache.process_mode != mode {
            self.cache.process_mode = mode;
            self.cache.base = None;
            self.cache.exposure = None;
            self.cache.clahe = None;
            self.cache.lab = None;
            pipeline_changed = true;
        }
        let geometry_context = GeometryContext {
            scale_factor: render_scale_factor(img.height, img.width),
        };
        let key = base_key(settings, context);
        let mut current = img.clone();
        (current, _) = run_stage(
            &mut self.cache,
            "base",
            &current,
            &key,
            pipeline_changed,
            context,
            |image, ctx| run_base(image, settings, source_hash, ctx, geometry_context),
        );
        self.store_plane_metrics(img, settings, context);
        let exposure_key = self.exposure_key(settings);
        let measured = settings.clone();
        let frame_dims = self
            .cache
            .base
            .as_ref()
            .map_or((img.height, img.width), |entry| {
                (entry.data.height, entry.data.width)
            });
        self.store_local_planes(settings, frame_dims, (img.height, img.width));
        let planes = self.exposure_planes(settings, context, frame_dims);
        (current, _) = run_stage(
            &mut self.cache,
            "exposure",
            &current,
            &exposure_key,
            pipeline_changed,
            context,
            |image, ctx| run_exposure(image, &measured, &planes, ctx),
        );
        let flat = settings.exposure.render_intent == RenderIntent::Flat;
        if !flat {
            let clahe_key = settings.lab.clahe_strength.stage_hash();
            (current, _) = run_stage(
                &mut self.cache,
                "clahe",
                &current,
                &clahe_key,
                pipeline_changed,
                context,
                |image, _| StageOutcome {
                    image: apply_clahe(image, settings.lab.clahe_strength),
                    metrics: BTreeMap::new(),
                    active_roi: None,
                },
            );
            let lab_key = settings.lab.stage_hash();
            let mono = is_mono(&settings.process);
            (current, _) = run_stage(
                &mut self.cache,
                "lab",
                &current,
                &lab_key,
                pipeline_changed,
                context,
                |image, _| {
                    let lab = lab_process(
                        image,
                        &settings.lab,
                        mono,
                        LabContext {
                            scale_factor: render_scale_factor(image.height, image.width),
                        },
                    );
                    StageOutcome {
                        image: lab.image,
                        metrics: BTreeMap::new(),
                        active_roi: None,
                    }
                },
            );
            let paper = effective_paper_profile(
                &settings.exposure.paper_profile,
                Some(settings.process.process_mode),
            );
            current = alt_process(
                &current,
                &settings.altproc,
                settings.process.process_mode,
                paper,
            );
            current = toning_process(
                &current,
                &settings.toning,
                settings.altproc.alt_process,
                mono,
            );
        }
        if !context.crop_preview_full {
            current = crop_image(
                &current,
                context
                    .active_roi
                    .map(|(y1, y2, x1, x2)| (y1 as i64, y2 as i64, x1 as i64, x2 as i64)),
            );
        }
        if !flat {
            let paper = crate::services::export::print::effective_paper_linear(
                &settings.finish,
                &settings.toning,
            );
            let carrier_live = settings.finish.carrier_width > 0.0 && !context.crop_preview_full;
            let tone = carrier_live.then(|| {
                rebate_tone(
                    settings,
                    RebateMetrics {
                        final_bounds: base_bounds_from(context),
                    },
                )
            });
            current = finish_process(
                &current,
                &settings.finish,
                FinishContext {
                    print_size_cm: 30.0,
                    paper,
                    crop_preview_full: context.crop_preview_full,
                },
                tone.as_ref().map(|tone| tone.as_slice()),
            );
            let encoded = crate::features::exposure::analysis::working_oetf_encode(&current.pixels);
            current = ImageBuffer::from_rgb(current.width, current.height, encoded);
        }
        context
            .metrics
            .insert("content_rect".to_string(), Value::Null);
        current
    }

    fn exposure_key(&self, settings: &WorkspaceConfig) -> String {
        let mut copy = settings.clone();
        copy.exposure = settings.exposure.clone();
        calculate_value_hash(&serde_json::json!({
            "exposure": copy.exposure,
            "local": copy.local,
            "hue_trim": copy.process.hue_trim,
            "slide_bw": copy.process.slide_bw,
        }))
    }

    fn store_local_planes(
        &mut self,
        settings: &WorkspaceConfig,
        frame_dims: (usize, usize),
        orig_dims: (usize, usize),
    ) {
        if settings.local.masks.is_empty()
            || settings.exposure.render_intent == RenderIntent::Flat
            || render_path(&settings.process) != RenderPath::Print
        {
            self.planes.local = None;
            return;
        }
        let (height, width) = frame_dims;
        let geometry = crate::features::local::logic::LocalGeometry {
            rotation: settings.geometry.rotation,
            fine_rotation: settings.geometry.fine_rotation,
            flip_horizontal: settings.geometry.flip_horizontal,
            flip_vertical: settings.geometry.flip_vertical,
            distortion_k1: settings.geometry.distortion_k1,
            converge_v: settings.geometry.converge_v,
            converge_h: settings.geometry.converge_h,
        };
        let key = format!(
            "{:?}|{:?}|{height}x{width}",
            settings.geometry, settings.local
        );
        if self.planes.local.as_ref().map(|(cached, _)| cached) == Some(&key) {
            return;
        }
        let maps = crate::features::local::processor::build(
            &settings.local,
            height,
            width,
            orig_dims,
            geometry,
        );
        self.planes.local = Some((key, maps));
    }

    fn store_plane_metrics(
        &mut self,
        image: &ImageBuffer,
        settings: &WorkspaceConfig,
        context: &PipelineContext,
    ) {
        let Some(bounds) = base_bounds_from(context) else {
            return;
        };
        let mask_bounds = context
            .metrics
            .get("pre_trim_bounds")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .or(Some(bounds));
        let geometry =
            PlaneGeometry::from_config(&settings.geometry, settings.geometry.distortion_k1);
        let frame_dims = self
            .cache
            .base
            .as_ref()
            .map_or((image.height, image.width), |entry| {
                (entry.data.height, entry.data.width)
            });
        let roi = normalized_roi(
            context
                .active_roi
                .map(|(y1, y2, x1, x2)| (y1 as i64, y2 as i64, x1 as i64, x2 as i64)),
            frame_dims,
        );
        let unmix =
            effective_crosstalk_matrix(&settings.process, Some(settings.process.process_mode));
        let weights = plane_weights(&settings.process, &settings.exposure);
        let key = format!("{roi:?}|{geometry:?}|{weights:?}");
        if settings.exposure.contrast_mask_strength > 0.0
            && settings.exposure.render_intent != RenderIntent::Flat
            && render_path(&settings.process) == RenderPath::Print
        {
            if let Some(mask_bounds) = mask_bounds {
                if self.planes.mask.as_ref().map(|cached| &cached.0) != Some(&key) {
                    let (plane, height, width, centre) =
                        contrast_mask_plane(image, mask_bounds, unmix, geometry, roi, weights);
                    self.planes.mask = Some((key.clone(), plane, height, width, centre));
                }
            }
        }
        if settings.exposure.diffusion > 0.0
            && settings.exposure.render_intent != RenderIntent::Flat
            && render_path(&settings.process) == RenderPath::Print
        {
            if self.planes.diffusion.as_ref().map(|cached| &cached.0) != Some(&key) {
                let grid = diffusion_grid(image, unmix, geometry, roi);
                let plane = diffusion_plane(&grid, bounds, weights);
                self.planes.diffusion = Some((key, plane, grid.height, grid.width));
            }
        }
    }

    fn exposure_planes(
        &self,
        settings: &WorkspaceConfig,
        context: &PipelineContext,
        frame_dims: (usize, usize),
    ) -> ExposurePlanes {
        let mut out = ExposurePlanes::default();
        if settings.exposure.render_intent == RenderIntent::Flat
            || render_path(&settings.process) != RenderPath::Print
        {
            return out;
        }
        let (height, width) = frame_dims;
        let (y1, y2, x1, x2) = context
            .active_roi
            .map_or((0, height, 0, width), |(y1, y2, x1, x2)| (y1, y2, x1, x2));
        let roi = if (y1, x1, y2, x2) == (0, 0, height, width) {
            None
        } else {
            Some((y1 as i64, y2 as i64, x1 as i64, x2 as i64))
        };
        if let Some((_, plane, height, width, centre)) = &self.planes.mask {
            if let Some(expanded) = expand_mask_plane(plane, (*height, *width), frame_dims, roi) {
                out.mask = Some(expanded);
                out.mask_centre = *centre;
            }
        }
        if let Some((_, Some(plane), height, width)) = &self.planes.diffusion {
            out.diffusion = Some(plane.clone());
            out.diffusion_dims = (*height, *width);
            out.diffusion_rect = [x1 as f64, y1 as f64, (x2 - x1) as f64, (y2 - y1) as f64];
        }
        out.local = self.planes.local.as_ref().map(|(_, maps)| maps.clone());
        out
    }
}

fn render_scale_factor(height: usize, width: usize) -> f64 {
    height.max(width) as f64 / 4000.0
}

fn metrics_to_map(metrics: &NormalizationMetrics) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "norm_density_range".to_string(),
            serde_json::json!(metrics.norm_density_range),
        ),
        (
            "metered_anchor".to_string(),
            serde_json::json!(metrics.metered_anchor),
        ),
        (
            "textural_range".to_string(),
            serde_json::json!(metrics.textural_range),
        ),
        (
            "shadow_point".to_string(),
            serde_json::json!(metrics.shadow_point),
        ),
        (
            "highlight_point".to_string(),
            serde_json::json!(metrics.highlight_point),
        ),
        (
            "final_bounds".to_string(),
            serde_json::to_value(metrics.final_bounds).unwrap_or(Value::Null),
        ),
        (
            "pre_trim_bounds".to_string(),
            serde_json::to_value(metrics.pre_trim_bounds).unwrap_or(Value::Null),
        ),
    ])
}

fn run_base(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    source_hash: &str,
    context: &mut PipelineContext,
    geometry_context: GeometryContext,
) -> StageOutcome {
    let print = render_path(&settings.process) == RenderPath::Print;
    if context.cache_stages && context.readback_metrics && print {
        let base = context
            .film_base
            .or(context.source_base_log)
            .or_else(|| base_log_for(Some(source_hash), image));
        context.metrics.insert(
            "base_log".to_string(),
            serde_json::to_value(base).unwrap_or(Value::Null),
        );
        context.metrics.insert(
            "base_source".to_string(),
            Value::String(
                if context.film_base.is_some() {
                    "picked"
                } else {
                    "read"
                }
                .to_string(),
            ),
        );
    }
    let geometry = geometry_process(image, &settings.geometry, geometry_context);
    context.active_roi = geometry.active_roi.map(|(y1, y2, x1, x2)| {
        (
            y1.max(0) as usize,
            y2.max(0) as usize,
            x1.max(0) as usize,
            x2.max(0) as usize,
        )
    });
    context.metrics.insert(
        "geometry_params".to_string(),
        serde_json::json!({
            "rotation": geometry.geometry_params.rotation,
            "fine_rotation": geometry.geometry_params.fine_rotation,
            "flip_horizontal": geometry.geometry_params.flip_horizontal,
            "flip_vertical": geometry.geometry_params.flip_vertical,
            "converge_v": geometry.geometry_params.converge_v,
            "converge_h": geometry.geometry_params.converge_h,
        }),
    );
    context.metrics.insert(
        "distortion_k1".to_string(),
        serde_json::json!(geometry.distortion_k1),
    );
    context.metrics.insert(
        "active_roi".to_string(),
        serde_json::to_value(geometry.active_roi).unwrap_or(Value::Null),
    );
    let outcome = if print {
        normalize_negative(
            &geometry.image.pixels,
            geometry.image.width,
            geometry.image.height,
            NormalizationSettings {
                config: &settings.process,
                mode: settings.process.process_mode,
                active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
                    (
                        y1.max(0) as usize,
                        y2.max(0) as usize,
                        x1.max(0) as usize,
                        x2.max(0) as usize,
                    )
                }),
                base_log: context
                    .metrics
                    .get("base_log")
                    .and_then(|value| serde_json::from_value(value.clone()).ok())
                    .flatten(),
            },
        )
    } else {
        let base = transfer_base(
            &geometry.image,
            &settings.process,
            settings.process.process_mode,
            TransferBaseSettings {
                active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
                    (
                        y1.max(0) as usize,
                        y2.max(0) as usize,
                        x1.max(0) as usize,
                        x2.max(0) as usize,
                    )
                }),
                cast_strength: slide_exposure(&settings.process, &settings.exposure)
                    .cast_removal_strength,
                ..TransferBaseSettings::default()
            },
        );
        let metrics = NormalizationMetrics {
            scan_clip_fractions: [0.0; 3],
            norm_density_range: base.metrics.norm_density_range,
            density_above_base: None,
            final_bounds: base.metrics.bounds,
            pre_trim_bounds: base.metrics.bounds,
            base_bounds: base.metrics.bounds,
            metered_anchor: base.metrics.metered_anchor,
            textural_range: base.metrics.textural_range,
            shadow_point: base.metrics.shadow_point,
            highlight_point: base.metrics.highlight_point,
            neutral_axis: None,
        };
        crate::features::exposure::processor::NormalizationResult {
            image: base.image.pixels,
            metrics,
        }
    };
    let mut metrics = metrics_to_map(&outcome.metrics);
    if print {
        let outcome_roi = geometry.active_roi.map(|(y1, y2, x1, x2)| {
            (
                y1.max(0) as usize,
                y2.max(0) as usize,
                x1.max(0) as usize,
                x2.max(0) as usize,
            )
        });
        if let Some(bins) = crate::features::exposure::analysis::density_histogram(
            &outcome.image,
            geometry.image.width,
            geometry.image.height,
            outcome_roi,
        ) {
            metrics.insert(
                "histogram_density".to_string(),
                serde_json::to_value(bins).unwrap_or(Value::Null),
            );
        }
    }
    StageOutcome {
        image: ImageBuffer::from_rgb(geometry.image.width, geometry.image.height, outcome.image),
        metrics,
        active_roi: geometry.active_roi.map(|(y1, y2, x1, x2)| {
            (
                y1.max(0) as usize,
                y2.max(0) as usize,
                x1.max(0) as usize,
                x2.max(0) as usize,
            )
        }),
    }
}

fn run_exposure(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    planes: &ExposurePlanes,
    context: &mut PipelineContext,
) -> StageOutcome {
    let metrics = auto_metrics_from(context);
    let white = render_white_space(&settings.process, None, None);
    let mut measured = settings.clone();
    measured.process = with_fade_white(&settings.process, context.fade_white.clone());
    let output = if settings.exposure.render_intent == RenderIntent::Flat
        || render_path(&settings.process) == RenderPath::Print
    {
        let local = LocalPlanes::from(
            planes.local.as_ref(),
            &measured.local,
            &measured.exposure,
            measured.process.process_mode,
            &metrics,
        );
        let print = PrintRenderSettings {
            config: &measured.exposure,
            process: &measured.process,
            density_range: metrics.norm_density_range,
            textural_range: metrics.textural_range,
            shadow_point: metrics.shadow_point,
            highlight_point: metrics.highlight_point,
            anchor: metrics.metered_anchor,
            bounds: metrics.bounds,
            neutral_axis: metrics.neutral_axis,
            mode: measured.process.process_mode,
            base_black_hold: if measured.exposure.hold_black {
                crate::features::exposure::constants::ZONE_BLACK_HOLD_TAPER
            } else {
                0.0
            },
            mask: planes.mask.as_deref().map(|data| MaskPlane {
                data,
                centre: planes.mask_centre,
            }),
            diffusion: planes.diffusion.as_deref().map(|data| DiffusionPlane {
                data,
                height: planes.diffusion_dims.0,
                width: planes.diffusion_dims.1,
                rect: planes.diffusion_rect,
            }),
            local_exposure: local.stops.as_deref(),
            local_grade: local.grade.as_deref(),
            local_flash: local.flash.as_deref(),
            key_params: local.key_params.as_deref(),
            key_alpha: local
                .key_alpha
                .as_ref()
                .map(|(alpha, count)| (alpha.as_slice(), *count)),
            grade_deltas: local.grade_deltas.as_deref(),
        };
        if settings.exposure.render_intent == RenderIntent::Flat {
            ImageBuffer::from_rgb(
                image.width,
                image.height,
                render_flat(
                    &image.pixels,
                    &FlatRenderSettings {
                        exposure: &measured.exposure,
                        process: &measured.process,
                        mode: measured.process.process_mode,
                        bounds: metrics.bounds,
                    },
                ),
            )
        } else {
            ImageBuffer::from_rgb(
                image.width,
                image.height,
                render_negative(&image.pixels, image.width, &print),
            )
        }
    } else {
        transfer_process(
            image,
            &slide_exposure(&measured.process, &measured.exposure),
            &measured.process,
            TransferContext {
                metrics,
                positive_source: measured.process.positive_source,
                white_target: fade_white_target(&measured.process, &white),
                mono_weights: slide_mono_weights(&measured.process, &measured.exposure),
            },
        )
    };
    StageOutcome {
        image: apply_hue_trim(&output, measured.process.hue_trim),
        metrics: BTreeMap::new(),
        active_roi: None,
    }
}

fn run_stage(
    cache: &mut PipelineCache,
    field: &str,
    image: &ImageBuffer,
    key: &str,
    changed: bool,
    context: &mut PipelineContext,
    run: impl FnOnce(&ImageBuffer, &mut PipelineContext) -> StageOutcome,
) -> (ImageBuffer, bool) {
    if !context.cache_stages {
        return (run(image, context).image, true);
    }
    let entry = match field {
        "base" => cache.base.as_ref(),
        "exposure" => cache.exposure.as_ref(),
        "clahe" => cache.clahe.as_ref(),
        "lab" => cache.lab.as_ref(),
        _ => None,
    };
    if !changed {
        if let Some(entry) = entry {
            if entry.config_hash == key {
                for (name, value) in &entry.metrics {
                    context.metrics.insert(name.clone(), value.clone());
                }
                context.active_roi = entry.active_roi;
                return (entry.data.clone(), false);
            }
        }
    }
    let outcome = run(image, context);
    for (name, value) in &outcome.metrics {
        context.metrics.insert(name.clone(), value.clone());
    }
    let entry = CacheEntry {
        config_hash: key.to_string(),
        data: outcome.image.clone(),
        metrics: outcome.metrics.clone(),
        active_roi: outcome.active_roi,
    };
    match field {
        "base" => cache.base = Some(entry),
        "exposure" => cache.exposure = Some(entry),
        "clahe" => cache.clahe = Some(entry),
        "lab" => cache.lab = Some(entry),
        _ => {}
    }
    (outcome.image, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_factor_follows_the_long_edge() {
        assert!((render_scale_factor(2000, 1500) - 0.5).abs() < 1e-9);
    }
}

fn base_bounds_from(context: &PipelineContext) -> Option<LogNegativeBounds> {
    context
        .metrics
        .get("final_bounds")
        .and_then(|value| serde_json::from_value::<LogNegativeBounds>(value.clone()).ok())
}

fn auto_metrics_from(context: &PipelineContext) -> AutoSliderMetrics {
    let read = |name: &str| context.metrics.get(name).and_then(Value::as_f64);
    AutoSliderMetrics {
        metered_anchor: read("metered_anchor"),
        textural_range: read("textural_range"),
        norm_density_range: read("norm_density_range"),
        shadow_point: read("shadow_point"),
        highlight_point: read("highlight_point"),
        bounds: base_bounds_from(context),
        neutral_axis: None,
    }
}
