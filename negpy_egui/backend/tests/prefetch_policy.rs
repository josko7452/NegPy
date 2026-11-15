//! Ports the admission cases in tests/test_prefetch_policy.py.

use negpy_backend::infrastructure::loaders::memory::PreviewMemoryEstimate;
use negpy_backend::kernel::system::memory::parse_vm_stat;
use negpy_backend::services::rendering::prefetch_policy::decide_prefetch;
use negpy_backend::services::rendering::preview_cache::PreviewCacheUsage;

fn cache(
    entries_remaining: usize,
    bytes_remaining: usize,
    reclaimable_entries: usize,
    reclaimable_bytes: usize,
) -> PreviewCacheUsage {
    PreviewCacheUsage {
        entries: 0,
        bytes_used: 0,
        entries_remaining,
        bytes_remaining,
        reclaimable_entries,
        reclaimable_bytes,
    }
}

fn estimate(cached: u64, temporary: u64) -> PreviewMemoryEstimate {
    PreviewMemoryEstimate {
        cached_bytes: cached,
        temporary_bytes: temporary,
        source_dimensions: Some((6000, 4000)),
    }
}

#[test]
fn a_preview_is_admitted_with_cache_and_ram_margin() {
    let decision = decide_prefetch(
        estimate(24 * 1024 * 1024, 512 * 1024 * 1024),
        cache(4, 512 * 1024 * 1024, 0, 0),
        Some(4 * 1024 * 1024 * 1024),
        false,
    );
    assert!(decision.allowed);
}

#[test]
fn a_preview_is_rejected_below_the_ram_margin() {
    let decision = decide_prefetch(
        estimate(24 * 1024 * 1024, 2 * 1024 * 1024 * 1024),
        cache(4, 512 * 1024 * 1024, 0, 0),
        Some(2 * 1024 * 1024 * 1024),
        false,
    );
    assert!(!decision.allowed);
    assert!(decision.reason.contains("system memory"));
}

#[test]
fn a_preview_is_rejected_when_the_cache_cannot_hold_it() {
    let decision = decide_prefetch(
        estimate(64 * 1024 * 1024, 512 * 1024 * 1024),
        cache(4, 32 * 1024 * 1024, 0, 0),
        None,
        false,
    );
    assert!(!decision.allowed);
    assert!(decision.reason.contains("cache byte"));
}

#[test]
fn a_cold_entry_can_be_replaced() {
    let decision = decide_prefetch(
        estimate(64 * 1024 * 1024, 512 * 1024 * 1024),
        cache(0, 0, 1, 64 * 1024 * 1024),
        Some(4 * 1024 * 1024 * 1024),
        false,
    );
    assert!(decision.allowed);
}

#[test]
fn a_full_cache_with_only_the_active_entry_rejects() {
    let decision = decide_prefetch(
        estimate(1, 512 * 1024 * 1024),
        cache(0, 0, 0, 0),
        Some(4 * 1024 * 1024 * 1024),
        false,
    );
    assert!(!decision.allowed);
    assert!(decision.reason.contains("cache entry"));
}

#[test]
fn an_integrated_gpu_keeps_extra_memory_free() {
    let estimate = estimate(24 * 1024 * 1024, 256 * 1024 * 1024);
    let discrete = decide_prefetch(
        estimate,
        cache(4, 512 * 1024 * 1024, 0, 0),
        Some(900 * 1024 * 1024),
        false,
    );
    let integrated = decide_prefetch(
        estimate,
        cache(4, 512 * 1024 * 1024, 0, 0),
        Some(900 * 1024 * 1024),
        true,
    );
    assert!(discrete.allowed);
    assert!(!integrated.allowed);
}

#[test]
fn an_unknown_ram_reading_rejects_with_the_requirement() {
    let decision = decide_prefetch(
        estimate(24 * 1024 * 1024, 512 * 1024 * 1024),
        cache(4, 512 * 1024 * 1024, 0, 0),
        None,
        false,
    );
    assert!(!decision.allowed);
    assert_eq!(decision.reason, "available system memory is unknown");
    assert!(decision.required_ram_bytes > 0);
}

#[test]
fn the_macos_parser_counts_reclaimable_pages() {
    let output = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 10.\nPages active: 100.\nPages inactive: 20.\nPages speculative: 5.\n";
    assert_eq!(parse_vm_stat(output), Some(35 * 16384));
}
