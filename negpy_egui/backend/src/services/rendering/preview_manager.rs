//! Decoded preview lookup, prefetch admission and detection decode.
//!
//! Mirrors the headless parts of `negpy/services/rendering/preview_manager.py`: every
//! cache write and lookup builds its key from one place, and a prefetch runs only when
//! the cache and RAM margins admit it.

use std::collections::HashMap;
use std::path::Path;

use crate::domain::models::{FlatFieldConfig, LensCorrections, SourceEncoding, WorkspaceConfig};
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
        key.bake_camera_wb = self.bake_camera_wb;
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
        let loaded = factory::load(
            path,
            LoadOptions {
                linear_raw: !request.use_camera_wb,
                positive_decode: (!request.positive_decode.is_empty())
                    .then(|| SourceEncoding::coerce(&request.positive_decode)),
                load_ir: false,
            },
        )?;
        let image = loaded
            .image
            .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
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

    /// Drop every cached buffer.
    pub fn clear(&self) {
        self.cache.clear();
    }
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
        lens_corrections: crate::services::rendering::source_identity::metadata_lens_corrections(
            config,
        ),
        lens_flatfield: config.flatfield.clone(),
    }
}
