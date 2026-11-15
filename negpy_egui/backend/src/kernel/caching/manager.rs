//! Mirrors `negpy/kernel/caching/manager.py`: the active image's stage checkpoints.

use crate::kernel::caching::logic::CacheEntry;

/// Intermediate stage results for the active image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PipelineCache {
    pub source_hash: String,
    pub process_mode: String,
    pub base: Option<CacheEntry>,
    pub exposure: Option<CacheEntry>,
    pub clahe: Option<CacheEntry>,
    pub lab: Option<CacheEntry>,
}

impl PipelineCache {
    /// Drop every checkpoint and the source identity they belong to.
    pub fn clear(&mut self) {
        self.base = None;
        self.exposure = None;
        self.clahe = None;
        self.lab = None;
        self.source_hash.clear();
    }
}
