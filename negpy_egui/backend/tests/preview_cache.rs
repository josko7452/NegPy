//! Ports the LRU, slot-budget and protection checks in tests/test_preview_cache.py.

use negpy_backend::domain::types::AppConfig;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::rendering::preview_cache::{PreviewBufferCache, PreviewCacheKey};

fn small_config() -> AppConfig {
    AppConfig {
        preview_cache_max_entries: 2,
        preview_cache_max_bytes: 4096,
        preview_cache_max_full_res_entries: 1,
        ..AppConfig::with_defaults()
    }
}

fn buffer(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

fn key(name: &str, full: bool) -> PreviewCacheKey {
    PreviewCacheKey::new(name, false, "Adobe RGB", full)
}

#[test]
fn a_miss_returns_none_and_a_hit_returns_the_buffer() {
    let cache = PreviewBufferCache::new(small_config());
    assert!(cache.get(&key("frame", false)).is_none());
    cache.put(
        &key("frame", false),
        buffer(2, 2, 0.5),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    let (stored, dims, _) = cache.get(&key("frame", false)).expect("cached");
    assert_eq!(dims, (2, 2));
    assert_eq!(stored.pixels, vec![0.5; 12]);
    assert!(cache.contains(&key("frame", false)));
}

#[test]
fn peek_does_not_change_the_lru_order() {
    let cache = PreviewBufferCache::new(small_config());
    cache.put(
        &key("a", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("b", false),
        buffer(2, 2, 0.2),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    assert!(cache.peek(&key("a", false)).is_some());
    cache.put(
        &key("c", false),
        buffer(2, 2, 0.3),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    // Peeking a did not refresh it, so the oldest entry a went first.
    assert!(cache.get(&key("a", false)).is_none());
    assert!(cache.get(&key("b", false)).is_some());
    assert!(cache.get(&key("c", false)).is_some());
}

#[test]
fn an_entry_above_the_byte_cap_is_not_cached() {
    let cache = PreviewBufferCache::new(AppConfig {
        preview_cache_max_bytes: 12,
        ..small_config()
    });
    cache.put(
        &key("big", false),
        buffer(2, 2, 0.5),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    assert!(cache.get(&key("big", false)).is_none());
}

#[test]
fn the_full_resolution_slot_budget_keeps_a_small_preview() {
    let cache = PreviewBufferCache::new(small_config());
    cache.put(
        &key("frame", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("one", true),
        buffer(4, 4, 0.2),
        (4, 4),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("two", true),
        buffer(4, 4, 0.3),
        (4, 4),
        Default::default(),
        &[],
        false,
    );
    assert!(cache.get(&key("one", true)).is_none());
    assert!(cache.get(&key("two", true)).is_some());
    assert!(cache.get(&key("frame", false)).is_some());
}

#[test]
fn usage_counts_entries_that_a_prefetch_can_reclaim() {
    let cache = PreviewBufferCache::new(small_config());
    cache.put(
        &key("active", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("other", false),
        buffer(2, 2, 0.2),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    let usage = cache.usage(&["active"], false);
    assert_eq!(usage.entries, 2);
    assert_eq!(usage.bytes_used, 96);
    assert_eq!(usage.entries_remaining, 0);
    assert_eq!(usage.reclaimable_entries, 1);
    assert_eq!(usage.reclaimable_bytes, 48);
}

#[test]
fn a_protected_full_resolution_entry_survives_eviction() {
    let cache = PreviewBufferCache::new(small_config());
    cache.put(
        &key("active", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("hq", true),
        buffer(2, 2, 0.2),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("neighbor", false),
        buffer(2, 2, 0.3),
        (2, 2),
        Default::default(),
        &[],
        true,
    );
    let usage = cache.usage(&[], true);
    assert_eq!(usage.reclaimable_entries, 1);
    cache.put(
        &key("next", false),
        buffer(2, 2, 0.4),
        (2, 2),
        Default::default(),
        &[],
        true,
    );
    assert!(cache.get(&key("hq", true)).is_some());
    assert!(cache.get(&key("next", false)).is_some());
}

#[test]
fn a_composite_hash_matches_its_protected_primary() {
    let cache = PreviewBufferCache::new(AppConfig {
        preview_cache_max_entries: 1,
        ..small_config()
    });
    cache.put(
        &key("hdr|primary|digest", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &["primary"],
        false,
    );
    cache.put(
        &key("other", false),
        buffer(2, 2, 0.2),
        (2, 2),
        Default::default(),
        &["primary"],
        false,
    );
    assert!(cache.get(&key("hdr|primary|digest", false)).is_some());
}

#[test]
fn invalidating_a_path_hash_drops_only_that_source() {
    let cache = PreviewBufferCache::new(small_config());
    cache.put(
        &key("a", false),
        buffer(2, 2, 0.1),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.put(
        &key("b", false),
        buffer(2, 2, 0.2),
        (2, 2),
        Default::default(),
        &[],
        false,
    );
    cache.invalidate_path_hash("a");
    assert!(cache.get(&key("a", false)).is_none());
    assert!(cache.get(&key("b", false)).is_some());
}
