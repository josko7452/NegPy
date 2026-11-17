//! Decoded preview lookup, prefetch admission and detection decode.
//!
//! Mirrors the headless parts of `negpy/services/rendering/preview_manager.py`: every
//! cache write and lookup builds its key from one place, and a prefetch runs only when
//! the cache and RAM margins admit it.

use std::collections::HashMap;
use std::path::Path;

use crate::domain::models::{
    FlatFieldConfig, HdrConfig, LensCorrections, RgbScanConfig, SourceEncoding, WorkspaceConfig,
    is_rgb_triplet,
};
use crate::domain::types::{AppConfig, Dimensions};
use crate::infrastructure::loaders::factory::{self, LoadOptions};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::infrastructure::loaders::memory::estimate_preview_memory;
use crate::kernel::system::memory::available_system_memory_bytes;
use crate::services::assets::half_frame::{HalfSlice, slice_half, slice_half_dimensions};
use crate::services::rendering::prefetch_policy::{PrefetchDecision, decide_prefetch};
use crate::services::rendering::preview_cache::{
    PreviewBufferCache, PreviewCacheKey, PreviewCacheUsage,
};
use crate::services::rendering::source_identity::lens_decode_token;

/// The bytes a composite merge produces, before its render exposure is applied.
pub const COMPOSITE_CAMERA_WB_NONE: f64 = f64::NAN;

/// The fields every linear-preview cache key resolves from the workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewRequest {
    pub file_hash: Option<String>,
    pub color_space: String,
    pub use_camera_wb: bool,
    pub full_resolution: bool,
    pub half_slice: Option<HalfSlice>,
    pub demosaic: String,
    pub positive_decode: String,
    pub highlight_mode: i64,
    pub bake_camera_wb: bool,
    /// Decode on another frame's multipliers, as a bracket sibling pins to its reference.
    pub wb_override: Option<[f64; 3]>,
    pub lens_corrections: LensCorrections,
    pub lens_flatfield: FlatFieldConfig,
}

impl PreviewRequest {
    /// A request for one plain frame at the working color space.
    #[must_use]
    pub fn new(file_hash: impl Into<String>, color_space: impl Into<String>) -> Self {
        Self {
            file_hash: Some(file_hash.into()),
            color_space: color_space.into(),
            demosaic: "Auto".to_string(),
            ..Self::default()
        }
    }

    /// The cache key every put and lookup for this request builds.
    #[must_use]
    pub fn cache_key(&self) -> Option<PreviewCacheKey> {
        let file_hash = self.file_hash.as_deref().filter(|hash| !hash.is_empty())?;
        let mut key = PreviewCacheKey::new(
            file_hash,
            self.use_camera_wb,
            &self.color_space,
            self.full_resolution,
        );
        key.demosaic = self.demosaic.clone();
        key.half = self.half_slice.map_or(0, |slice| i64::from(slice.half));
        key.split_x = self.half_slice.map_or(0.5, |slice| slice.split_x);
        key.crop_rect = self.half_slice.and_then(|slice| slice.crop_rect);
        key.gutter_thickness = self.half_slice.map_or(0.0, |slice| slice.gutter_thickness);
        key.split_axis = self
            .half_slice
            .map_or_else(|| "x".to_string(), |slice| slice.split_axis.to_string());
        key.positive_decode = self.positive_decode.clone();
        key.highlight_mode = self.highlight_mode;
        key.bake_camera_wb = self.bake_camera_wb || self.wb_override.is_some();
        key.lens_token = lens_decode_token(self.lens_corrections, &self.lens_flatfield, None);
        Some(key)
    }
}

/// Decoded-preview cache and the decisions around it.
pub struct PreviewManager {
    app: AppConfig,
    cache: PreviewBufferCache,
}

impl PreviewManager {
    /// A manager with its own in-memory cache under `app`.
    #[must_use]
    pub fn new(app: AppConfig) -> Self {
        Self {
            cache: PreviewBufferCache::new(app.clone()),
            app,
        }
    }

    /// The cached buffer, moved to the newest LRU position.
    pub fn get(
        &self,
        request: &PreviewRequest,
    ) -> Option<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>)> {
        self.cache.get(&request.cache_key()?)
    }

    /// The cached buffer without reordering the cache.
    #[must_use]
    pub fn peek(
        &self,
        request: &PreviewRequest,
    ) -> Option<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>)> {
        self.cache.peek(&request.cache_key()?)
    }

    /// Connect the cache occupancy to the prefetch policy.
    #[must_use]
    pub fn usage(
        &self,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
    ) -> PreviewCacheUsage {
        self.cache
            .usage(protected_file_hashes, preserve_full_resolution)
    }

    /// Decide whether one preview can be warmed, before decoding it.
    #[must_use]
    pub fn decide_prefetch(
        &self,
        path: &Path,
        protected_file_hashes: &[&str],
        integrated_gpu: bool,
    ) -> PrefetchDecision {
        let estimate =
            estimate_preview_memory(path, self.app.preview_render_size.max(1) as u64, "display");
        decide_prefetch(
            estimate,
            self.usage(protected_file_hashes, true),
            available_system_memory_bytes(),
            integrated_gpu,
        )
    }

    /// Warm one preview when its cache and RAM budgets admit it. A cache hit is a
    /// successful warm without a decode.
    pub fn prefetch(
        &self,
        path: &Path,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        integrated_gpu: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<bool, String> {
        if request.cache_key().is_none() {
            return Ok(false);
        }
        if self.peek(request).is_some() {
            return Ok(true);
        }
        if is_cancelled.is_some_and(|cancelled| cancelled()) {
            return Err("preview load cancelled".to_string());
        }
        let decision = self.decide_prefetch(path, protected_file_hashes, integrated_gpu);
        if !decision.allowed {
            log::debug!("preview prefetch skipped: {}", decision.reason);
            return Ok(false);
        }
        if is_cancelled.is_some_and(|cancelled| cancelled()) {
            return Err("preview load cancelled".to_string());
        }
        self.load(path, request, protected_file_hashes, true, is_cancelled)?;
        Ok(true)
    }

    /// Decode and cache one linear preview. A hit returns the cached buffer.
    pub fn load(
        &self,
        path: &Path,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>), String> {
        if let Some(hit) = self.get(request) {
            return Ok(hit);
        }
        if is_cancelled.is_some_and(|cancelled| cancelled()) {
            return Err("preview load cancelled".to_string());
        }
        let decodes_with_wb = crate::infrastructure::loaders::camera_wb::decodes_with_wb(
            request.use_camera_wb,
            request.bake_camera_wb,
            request.wb_override.as_ref().map(|wb| wb.as_slice()),
        );
        let loaded = factory::load(
            path,
            LoadOptions {
                linear_raw: !decodes_with_wb,
                positive_decode: (!request.positive_decode.is_empty())
                    .then(|| SourceEncoding::coerce(&request.positive_decode)),
                load_ir: false,
            },
        )?;
        let mut image = loaded
            .image
            .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
        if image.pixels.is_empty()
            && let Some(samples) = image.source_u16.take()
        {
            image.pixels = samples
                .iter()
                .map(|value| *value as f32 / 65535.0)
                .collect();
        }
        let wb = request.wb_override.or_else(|| {
            (request.bake_camera_wb && !request.use_camera_wb)
                .then(|| crate::infrastructure::loaders::camera_wb::camera_wb_for(path))
                .flatten()
        });
        let image = crate::infrastructure::loaders::camera_wb::apply_camera_wb(&image, wb);
        let (image, dims) = apply_half(&image, request.half_slice);
        let mut metadata = HashMap::new();
        if let Some(space) = loaded.color_space {
            metadata.insert("color_space".to_string(), vec![space.len() as f64]);
        }
        if let Some(key) = request.cache_key() {
            self.cache.put(
                &key,
                image.clone(),
                dims,
                metadata.clone(),
                protected_file_hashes,
                preserve_full_resolution,
            );
        }
        Ok((image, dims, metadata))
    }

    /// Open one file once and return both its first-paint splash and its linear preview.
    ///
    /// A cache hit returns no splash, because the linear buffer is already fast enough.
    pub fn load_splash_and_linear(
        &self,
        path: &Path,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> (
        Option<ImageBuffer>,
        Result<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>), String>,
    ) {
        if let Some(hit) = self.get(request) {
            return (None, Ok(hit));
        }
        let splash = (!request.full_resolution
            && !request.lens_corrections.any()
            && !request.bake_camera_wb)
            .then(|| {
                crate::services::rendering::splash::try_splash_preview(
                    path,
                    crate::services::rendering::splash::SPLASH_MAX_EDGE,
                )
                .map(|splash| splash.image)
            })
            .flatten();
        let linear = self.load(
            path,
            request,
            protected_file_hashes,
            preserve_full_resolution,
            is_cancelled,
        );
        (splash, linear)
    }

    /// Drop every cached buffer.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Merge a narrowband R/G/B triplet into one linear preview. Each exposure decodes
    /// neutral, since only one narrowband channel carries signal.
    pub fn load_rgb(
        &self,
        red_path: &Path,
        rgbscan: &RgbScanConfig,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>), String> {
        if !is_rgb_triplet(rgbscan) {
            return Err("RGB triplet paths are incomplete.".to_string());
        }
        let token = crate::domain::models::rgbscan_token(rgbscan);
        let merged_key =
            self.composite_key(&format!("rgb|{}", request_hash(request)), &token, request);
        if let Some(key) = &merged_key
            && let Some(hit) = self.cache.get(key)
        {
            return Ok(hit);
        }
        let (red, dims, mut metadata) = self.load(
            red_path,
            &PreviewRequest {
                use_camera_wb: false,
                ..request.clone()
            },
            protected_file_hashes,
            preserve_full_resolution,
            is_cancelled,
        )?;
        let green = self.load(
            Path::new(&rgbscan.green_path),
            &PreviewRequest {
                file_hash: None,
                use_camera_wb: false,
                ..request.clone()
            },
            protected_file_hashes,
            preserve_full_resolution,
            is_cancelled,
        )?;
        let blue = self.load(
            Path::new(&rgbscan.blue_path),
            &PreviewRequest {
                file_hash: None,
                use_camera_wb: false,
                ..request.clone()
            },
            protected_file_hashes,
            preserve_full_resolution,
            is_cancelled,
        )?;
        let green = match_size(&green.0, &red)?;
        let blue = match_size(&blue.0, &red)?;
        let merged = crate::features::rgbscan::assemble_rgb(&red, &green, &blue, rgbscan.align)
            .ok_or_else(|| "RGB-scan exposures differ in shape.".to_string())?;
        metadata.insert("camera_wb".to_string(), vec![COMPOSITE_CAMERA_WB_NONE]);
        if let Some(key) = merged_key {
            self.cache.put(
                &key,
                merged.clone(),
                dims,
                metadata.clone(),
                protected_file_hashes,
                preserve_full_resolution,
            );
        }
        Ok((merged, dims, metadata))
    }

    /// Merge a bracket into one linear preview in the reference frame's exposure units.
    pub fn load_hdr(
        &self,
        reference_path: &Path,
        hdr: &HdrConfig,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>), String> {
        let ratios = &hdr.hdr_ratios;
        if ratios.len() != hdr.hdr_paths.len() + 1 {
            return Err(format!(
                "{} frames but {} ratios",
                hdr.hdr_paths.len() + 1,
                ratios.len()
            ));
        }
        let mut paths = vec![reference_path.display().to_string()];
        paths.extend(hdr.hdr_paths.iter().cloned());
        let anchor = crate::features::hdr::logic::resolve_anchor(&paths, ratios, hdr);
        let token = crate::domain::models::hdr_merge_token(hdr);
        let merged_key =
            self.composite_key(&format!("hdr|{}", request_hash(request)), &token, request);
        if let Some(key) = &merged_key
            && let Some((raw, dims, metadata)) = self.cache.get(key)
        {
            return Ok((scale_merge(raw, ratios, anchor), dims, metadata));
        }
        let (reference, dims, metadata) = self.load(
            reference_path,
            request,
            protected_file_hashes,
            preserve_full_resolution,
            is_cancelled,
        )?;
        let mut frames = vec![frame(&reference)];
        for path in &hdr.hdr_paths {
            let loaded = self.load(
                Path::new(path),
                &PreviewRequest {
                    file_hash: None,
                    ..request.clone()
                },
                protected_file_hashes,
                preserve_full_resolution,
                is_cancelled,
            )?;
            frames.push(frame(&match_size(&loaded.0, &reference)?));
        }
        let merged = crate::features::hdr::logic::merge_frames(&frames, ratios, 0, hdr.hdr_align)
            .ok_or_else(|| "bracket frames differ in shape.".to_string())?;
        let merged_image =
            ImageBuffer::from_rgb(merged.width, merged.height, merged.pixels.clone());
        if let Some(key) = merged_key {
            self.cache.put(
                &key,
                merged_image.clone(),
                dims,
                metadata.clone(),
                protected_file_hashes,
                preserve_full_resolution,
            );
        }
        Ok((scale_merge(merged_image, ratios, anchor), dims, metadata))
    }

    /// Assemble a stitch composite at preview scale, applying the flat field per part.
    pub fn load_stitch(
        &self,
        primary_path: &Path,
        config: &crate::domain::models::StitchConfig,
        flatfield_profile_id: &str,
        request: &PreviewRequest,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>), String> {
        if !crate::domain::models::stitch_active(config) {
            return Err("Stitch registration is incomplete.".to_string());
        }
        let flatfield_config = flatfield_config(flatfield_profile_id);
        let flatfield =
            crate::services::assets::flatfield::FlatFieldProfiles::new(&self.app.flatfield_dir)
                .load_gain(flatfield_profile_id);
        let flatfield_token = crate::features::flatfield::logic::flatfield_token(
            &flatfield_config,
            flatfield.as_ref(),
        );
        let token = format!(
            "{}{}",
            crate::domain::models::stitch_token(config),
            flatfield_token
        );
        let merged_key = self.composite_key(
            &format!("stitch|{}", request_hash(request)),
            &token,
            request,
        );
        if let Some(key) = &merged_key
            && let Some(hit) = self.cache.get(key)
        {
            return Ok(hit);
        }
        let mut parts = Vec::with_capacity(config.stitch_paths.len() + 1);
        let mut irs = Vec::with_capacity(config.stitch_paths.len() + 1);
        let mut metadata = HashMap::new();
        for (index, path) in std::iter::once(primary_path.display().to_string())
            .chain(config.stitch_paths.iter().cloned())
            .enumerate()
        {
            let [green, blue] = config
                .stitch_triplets
                .get(index)
                .cloned()
                .unwrap_or_default();
            let part = if !green.is_empty() && !blue.is_empty() {
                let triplet = RgbScanConfig {
                    enabled: true,
                    green_path: green,
                    blue_path: blue,
                    align: config.stitch_align,
                };
                self.load_rgb(
                    Path::new(&path),
                    &triplet,
                    &PreviewRequest {
                        file_hash: None,
                        ..request.clone()
                    },
                    protected_file_hashes,
                    preserve_full_resolution,
                    is_cancelled,
                )?
            } else {
                self.load(
                    Path::new(&path),
                    &PreviewRequest {
                        file_hash: None,
                        ..request.clone()
                    },
                    protected_file_hashes,
                    preserve_full_resolution,
                    is_cancelled,
                )?
            };
            let part_image = crate::features::flatfield::logic::apply_flatfield(
                &part.0,
                &flatfield_config,
                flatfield.as_ref(),
                None,
                None,
            );
            parts.push(part_image);
            irs.push(None);
            if index == 0 {
                metadata = part.2;
            }
        }
        let (merged, ir) = crate::features::stitch::stitch_composite(&parts, &irs, config);
        if let Some(ir) = ir {
            metadata.insert(
                "ir_preview".to_string(),
                ir.iter().map(|v| f64::from(*v)).collect(),
            );
        }
        let dims = (
            config.stitch_canvas[1].max(0),
            config.stitch_canvas[0].max(0),
        );
        if let Some(key) = merged_key {
            self.cache.put(
                &key,
                merged.clone(),
                dims,
                metadata.clone(),
                protected_file_hashes,
                preserve_full_resolution,
            );
        }
        Ok((merged, dims, metadata))
    }

    fn composite_key(
        &self,
        base: &str,
        token: &str,
        request: &PreviewRequest,
    ) -> Option<PreviewCacheKey> {
        if token.is_empty() {
            return None;
        }
        let mut key = PreviewCacheKey::new(
            base,
            request.use_camera_wb,
            &request.color_space,
            request.full_resolution,
        );
        key.demosaic = request.demosaic.clone();
        key.positive_decode = request.positive_decode.clone();
        key.highlight_mode = request.highlight_mode;
        key.bake_camera_wb = request.bake_camera_wb;
        Some(key)
    }
}

fn request_hash(request: &PreviewRequest) -> String {
    request.file_hash.clone().unwrap_or_default()
}

fn flatfield_config(profile_id: &str) -> FlatFieldConfig {
    FlatFieldConfig {
        apply: !profile_id.is_empty(),
        profile_id: profile_id.to_string(),
        ..FlatFieldConfig::default()
    }
}

fn frame(image: &ImageBuffer) -> crate::features::hdr::logic::Frame {
    crate::features::hdr::logic::Frame::new(image.width, image.height, image.pixels.clone())
}

fn match_size(image: &ImageBuffer, reference: &ImageBuffer) -> Result<ImageBuffer, String> {
    if image.width == reference.width && image.height == reference.height {
        return Ok(image.clone());
    }
    if (image.width * reference.height).abs_diff(image.height * reference.width)
        > reference.width * image.height / 50
    {
        return Err("composite frames differ in shape.".to_string());
    }
    let resized = negpy_core::resize::resize_area(
        &image.pixels,
        image.height,
        image.width,
        3,
        reference.height,
        reference.width,
    )
    .ok_or_else(|| "composite frames differ in shape.".to_string())?;
    Ok(ImageBuffer::from_rgb(
        reference.width,
        reference.height,
        resized,
    ))
}

fn scale_merge(image: ImageBuffer, ratios: &[f64], anchor: Option<f64>) -> ImageBuffer {
    let merged = frame(&image);
    let scaled = crate::features::hdr::logic::apply_render_exposure(&merged, ratios, anchor);
    ImageBuffer::from_rgb(scaled.width, scaled.height, scaled.pixels)
}

fn apply_half(image: &ImageBuffer, half_slice: Option<HalfSlice>) -> (ImageBuffer, Dimensions) {
    let dims = (image.height as i64, image.width as i64);
    let Some(slice) = half_slice else {
        return (image.clone(), dims);
    };
    let sliced = slice_half(image, &slice);
    let dimensions = slice_half_dimensions((image.height, image.width), &slice);
    (sliced, (dimensions.0 as i64, dimensions.1 as i64))
}

/// The half slice used by the manager, exposed for a geometry-parity test.
#[doc(hidden)]
#[must_use]
pub fn apply_half_for_test(
    image: &ImageBuffer,
    half_slice: Option<HalfSlice>,
) -> (ImageBuffer, Dimensions) {
    apply_half(image, half_slice)
}

/// A no-WB linear decode for autodetect outside the preview cache.
#[must_use]
pub fn decode_for_detection(path: &Path, preview_render_size: i64) -> Option<ImageBuffer> {
    let loaded = factory::load(
        path,
        LoadOptions {
            linear_raw: true,
            positive_decode: None,
            load_ir: false,
        },
    )
    .ok()?;
    let image = loaded.image?;
    let max_edge = preview_render_size.max(1) as usize;
    Some(crate::features::retouch::processor::downsample_to_long_edge(&image, max_edge))
}

/// The detection decode with an explicit downsample size, so tests do not need a large file.
#[must_use]
pub fn detection_plane(image: &ImageBuffer, preview_render_size: usize) -> Vec<f32> {
    crate::features::retouch::processor::detection_downsample(image, preview_render_size)
}

/// The request for one workspace decode, with all source-changing fields folded in.
#[must_use]
pub fn request_for(config: &WorkspaceConfig, file_hash: impl Into<String>) -> PreviewRequest {
    PreviewRequest {
        file_hash: Some(file_hash.into()),
        color_space: crate::infrastructure::display::color_mgmt::WORKING_COLOR_SPACE.to_string(),
        use_camera_wb: !config.process.linear_raw,
        full_resolution: false,
        half_slice: None,
        demosaic: config.process.demosaic_preview.as_str().to_string(),
        positive_decode: config.process.source_encoding.as_str().to_string(),
        highlight_mode: config.process.highlight_reconstruction,
        bake_camera_wb: false,
        wb_override: None,
        lens_corrections: crate::services::rendering::source_identity::metadata_lens_corrections(
            config,
        ),
        lens_flatfield: config.flatfield.clone(),
    }
}
