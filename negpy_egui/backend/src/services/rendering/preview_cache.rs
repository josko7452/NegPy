//! Mirrors `negpy/services/rendering/preview_cache.py`: the decoded-preview LRU.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::domain::types::{AppConfig, Dimensions};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// What identifies one decoded preview buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewCacheKey {
    pub file_hash: String,
    pub use_camera_wb: bool,
    pub workspace_color_space: String,
    pub full_resolution: bool,
    pub demosaic: String,
    pub half: i64,
    pub split_x: f64,
    pub crop_rect: Option<[f64; 4]>,
    pub gutter_thickness: f64,
    pub split_axis: String,
    pub positive_decode: String,
    pub highlight_mode: i64,
    pub bake_camera_wb: bool,
    pub lens_token: String,
}

impl PreviewCacheKey {
    /// Build a key from the fields every caller must decide.
    #[must_use]
    pub fn new(
        file_hash: &str,
        use_camera_wb: bool,
        color_space: &str,
        full_resolution: bool,
    ) -> Self {
        Self {
            file_hash: file_hash.to_string(),
            use_camera_wb,
            workspace_color_space: color_space.to_string(),
            full_resolution,
            demosaic: "Auto".to_string(),
            half: 0,
            split_x: 0.5,
            crop_rect: None,
            gutter_thickness: 0.0,
            split_axis: "x".to_string(),
            positive_decode: String::new(),
            highlight_mode: 0,
            bake_camera_wb: false,
            lens_token: String::new(),
        }
    }

    /// The rounded tuple Python compares, so two values that display the same share a slot.
    #[must_use]
    fn as_tuple(&self) -> CacheTuple {
        CacheTuple {
            file_hash: self.file_hash.clone(),
            use_camera_wb: self.use_camera_wb,
            workspace_color_space: self.workspace_color_space.clone(),
            full_resolution: self.full_resolution,
            demosaic: self.demosaic.clone(),
            half: self.half,
            split_x: round6(self.split_x).to_bits(),
            crop_rect: self
                .crop_rect
                .map(|rect| rect.map(|value| round6(value).to_bits())),
            gutter_thickness: round6(self.gutter_thickness).to_bits(),
            split_axis: self.split_axis.clone(),
            positive_decode: self.positive_decode.clone(),
            highlight_mode: self.highlight_mode,
            bake_camera_wb: self.bake_camera_wb,
            lens_token: self.lens_token.clone(),
        }
    }
}

fn round6(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheTuple {
    file_hash: String,
    use_camera_wb: bool,
    workspace_color_space: String,
    full_resolution: bool,
    demosaic: String,
    half: i64,
    split_x: u64,
    crop_rect: Option<[u64; 4]>,
    gutter_thickness: u64,
    split_axis: String,
    positive_decode: String,
    highlight_mode: i64,
    bake_camera_wb: bool,
    lens_token: String,
}

#[derive(Clone, Debug)]
struct Entry {
    buffer: ImageBuffer,
    dims: Dimensions,
    metadata: HashMap<String, Vec<f64>>,
    byte_size: usize,
}

/// The cache's occupancy, as the prefetch policy reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewCacheUsage {
    pub entries: usize,
    pub bytes_used: usize,
    pub entries_remaining: usize,
    pub bytes_remaining: usize,
    pub reclaimable_entries: usize,
    pub reclaimable_bytes: usize,
}

/// One in-memory LRU for decoded linear preview buffers.
pub struct PreviewBufferCache {
    app: AppConfig,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    order: VecDeque<CacheTuple>,
    data: HashMap<CacheTuple, Entry>,
}

impl PreviewBufferCache {
    /// A cache bounded by `app`'s entry and byte limits.
    #[must_use]
    pub fn new(app: AppConfig) -> Self {
        Self {
            app,
            state: Mutex::new(State::default()),
        }
    }

    /// A cached buffer, moved to the newest end of the LRU order.
    pub fn get(
        &self,
        key: &PreviewCacheKey,
    ) -> Option<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>)> {
        let mut state = self.state.lock().ok()?;
        let tuple = key.as_tuple();
        let entry = state.data.get(&tuple)?;
        let result = (entry.buffer.clone(), entry.dims, entry.metadata.clone());
        state.order.retain(|candidate| *candidate != tuple);
        state.order.push_back(tuple);
        Some(result)
    }

    /// A cached buffer without changing its LRU position.
    #[must_use]
    pub fn peek(
        &self,
        key: &PreviewCacheKey,
    ) -> Option<(ImageBuffer, Dimensions, HashMap<String, Vec<f64>>)> {
        let state = self.state.lock().ok()?;
        state
            .data
            .get(&key.as_tuple())
            .map(|entry| (entry.buffer.clone(), entry.dims, entry.metadata.clone()))
    }

    /// Insert or replace a buffer, then evict beyond the entry and byte limits.
    pub fn put(
        &self,
        key: &PreviewCacheKey,
        buffer: ImageBuffer,
        dims: Dimensions,
        metadata: HashMap<String, Vec<f64>>,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
    ) {
        let tuple = key.as_tuple();
        let byte_size = buffer.pixels.len() * size_of::<f32>();
        if byte_size > self.app.preview_cache_max_bytes as usize {
            return;
        }
        let mut state = self.state.lock().expect("preview cache lock");
        if key.full_resolution {
            let limit = self.app.preview_cache_max_full_res_entries.max(1) as usize;
            let full: Vec<CacheTuple> = state
                .order
                .iter()
                .filter(|candidate| candidate.full_resolution && **candidate != tuple)
                .cloned()
                .collect();
            for other in full.iter().take(full.len().saturating_sub(limit - 1)) {
                remove(&mut state, other);
            }
        }
        state.order.retain(|candidate| *candidate != tuple);
        state.data.insert(
            tuple.clone(),
            Entry {
                buffer,
                dims,
                metadata,
                byte_size,
            },
        );
        state.order.push_back(tuple);
        self.evict_if_needed(&mut state, protected_file_hashes, preserve_full_resolution);
    }

    /// Drop every entry of one source hash.
    pub fn invalidate_path_hash(&self, file_hash: &str) {
        let mut state = self.state.lock().expect("preview cache lock");
        let drop: Vec<CacheTuple> = state
            .order
            .iter()
            .filter(|key| key.file_hash == file_hash)
            .cloned()
            .collect();
        for key in drop {
            remove(&mut state, &key);
        }
    }

    /// Drop every entry.
    pub fn clear(&self) {
        let mut state = self.state.lock().expect("preview cache lock");
        state.order.clear();
        state.data.clear();
    }

    /// Whether `key` is already cached.
    #[must_use]
    pub fn contains(&self, key: &PreviewCacheKey) -> bool {
        let state = self.state.lock().expect("preview cache lock");
        state.data.contains_key(&key.as_tuple())
    }

    /// Occupancy, counting what a prefetch could reclaim.
    #[must_use]
    pub fn usage(
        &self,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
    ) -> PreviewCacheUsage {
        let state = self.state.lock().expect("preview cache lock");
        let bytes_used = state
            .data
            .values()
            .map(|entry| entry.byte_size)
            .sum::<usize>();
        let mut reclaimable_entries = 0;
        let mut reclaimable_bytes = 0;
        for key in &state.order {
            if !self.is_protected(key, protected_file_hashes, preserve_full_resolution) {
                reclaimable_entries += 1;
                reclaimable_bytes += state.data[key].byte_size;
            }
        }
        PreviewCacheUsage {
            entries: state.data.len(),
            bytes_used,
            entries_remaining: (self.app.preview_cache_max_entries as usize)
                .saturating_sub(state.data.len()),
            bytes_remaining: (self.app.preview_cache_max_bytes as usize).saturating_sub(bytes_used),
            reclaimable_entries,
            reclaimable_bytes,
        }
    }

    fn is_protected(
        &self,
        key: &CacheTuple,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
    ) -> bool {
        if preserve_full_resolution && key.full_resolution {
            return true;
        }
        if protected_file_hashes.is_empty() {
            return false;
        }
        if protected_file_hashes.contains(&key.file_hash.as_str()) {
            return true;
        }
        let Some((kind, rest)) = key.file_hash.split_once('|') else {
            return false;
        };
        let Some(hash) = rest.split('|').next() else {
            return false;
        };
        matches!(kind, "rgb" | "hdr" | "stitch") && protected_file_hashes.contains(&hash)
    }

    fn evict_if_needed(
        &self,
        state: &mut State,
        protected_file_hashes: &[&str],
        preserve_full_resolution: bool,
    ) {
        let max_entries = self.app.preview_cache_max_entries.max(0) as usize;
        let max_bytes = self.app.preview_cache_max_bytes.max(0) as usize;
        while state.order.len() > max_entries {
            let Some(key) = state
                .order
                .iter()
                .find(|key| {
                    !self.is_protected(key, protected_file_hashes, preserve_full_resolution)
                })
                .cloned()
            else {
                break;
            };
            remove(state, &key);
        }
        while state
            .data
            .values()
            .map(|entry| entry.byte_size)
            .sum::<usize>()
            > max_bytes
        {
            let Some(key) = state
                .order
                .iter()
                .find(|key| {
                    !self.is_protected(key, protected_file_hashes, preserve_full_resolution)
                })
                .cloned()
            else {
                break;
            };
            remove(state, &key);
        }
    }
}

fn remove(state: &mut State, tuple: &CacheTuple) {
    state.data.remove(tuple);
    state.order.retain(|candidate| candidate != tuple);
}
