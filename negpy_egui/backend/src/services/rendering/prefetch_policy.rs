//! Mirrors `negpy/services/rendering/prefetch_policy.py`: when a prefetch is safe.

use crate::infrastructure::loaders::memory::PreviewMemoryEstimate;
use crate::services::rendering::preview_cache::PreviewCacheUsage;

/// Shared with every RAM-consuming background worker that needs the same floor.
pub const MIN_RAM_RESERVE_BYTES: u64 = 512 * 1024 * 1024;
const INTEGRATED_GPU_RESERVE_BYTES: u64 = 256 * 1024 * 1024;

/// One prefetch admission result and the RAM it would need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrefetchDecision {
    pub allowed: bool,
    pub reason: &'static str,
    pub required_ram_bytes: u64,
}

/// Admit a prefetch only when eviction and RAM margins can hold it.
#[must_use]
pub fn decide_prefetch(
    estimate: PreviewMemoryEstimate,
    cache: PreviewCacheUsage,
    available_ram_bytes: Option<u64>,
    integrated_gpu: bool,
) -> PrefetchDecision {
    if cache.entries_remaining + cache.reclaimable_entries < 1 {
        return PrefetchDecision {
            allowed: false,
            reason: "preview cache entry budget is full",
            required_ram_bytes: 0,
        };
    }
    if estimate.cached_bytes > (cache.bytes_remaining + cache.reclaimable_bytes) as u64 {
        return PrefetchDecision {
            allowed: false,
            reason: "preview cache byte budget is full",
            required_ram_bytes: 0,
        };
    }
    let working_bytes = estimate.temporary_bytes + estimate.cached_bytes;
    let mut reserve_bytes = MIN_RAM_RESERVE_BYTES.max(working_bytes / 4);
    if integrated_gpu {
        reserve_bytes += INTEGRATED_GPU_RESERVE_BYTES.max(estimate.cached_bytes.saturating_mul(2));
    }
    let required_ram_bytes = working_bytes + reserve_bytes;
    let Some(available) = available_ram_bytes else {
        return PrefetchDecision {
            allowed: false,
            reason: "available system memory is unknown",
            required_ram_bytes,
        };
    };
    if available < required_ram_bytes {
        return PrefetchDecision {
            allowed: false,
            reason: "available system memory is below the safety margin",
            required_ram_bytes,
        };
    }
    PrefetchDecision {
        allowed: true,
        reason: "memory margins are sufficient",
        required_ram_bytes,
    }
}
