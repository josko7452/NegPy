//! Ports `tests/test_render_memo.py` to the headless render memo.

use std::sync::{Arc, Mutex};

use negpy_backend::domain::types::AppConfig;
use negpy_backend::session::render_memo::{MemoBudget, RenderMemo};

fn config(standard: i64, large: i64) -> AppConfig {
    AppConfig {
        render_memo_max_entries: standard,
        preview_cache_max_full_res_entries: large,
        ..AppConfig::with_defaults()
    }
}

#[derive(Clone)]
struct Owned {
    dropped: Arc<Mutex<usize>>,
}

impl Owned {
    fn new(dropped: &Arc<Mutex<usize>>) -> Self {
        Self {
            dropped: Arc::clone(dropped),
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        *self.dropped.lock().unwrap() += 1;
    }
}

#[test]
fn a_hit_requires_a_matching_key() {
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("frameA", "key1", 7);
    assert_eq!(memo.get("frameA", "key1"), Some(&7));
    assert_eq!(memo.get("frameA", "key2"), None);
    assert_eq!(memo.get("frameB", "key1"), None);
}

#[test]
fn a_store_overwrites_the_sources_previous_key() {
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("frameA", "key1", 1);
    memo.store("frameA", "key2", 2);
    assert_eq!(memo.get("frameA", "key1"), None);
    assert_eq!(memo.get("frameA", "key2"), Some(&2));
}

#[test]
fn the_least_recent_source_evicts_first() {
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("A", "k", 1);
    memo.store("B", "k", 2);
    assert_eq!(memo.get("A", "k"), Some(&1));
    memo.store("C", "k", 3);
    assert_eq!(memo.get("B", "k"), None);
    assert_eq!(memo.get("A", "k"), Some(&1));
    assert_eq!(memo.get("C", "k"), Some(&3));
}

#[test]
fn large_entries_use_the_full_resolution_budget() {
    let mut memo = RenderMemo::new(config(5, 2));
    for index in 0..5 {
        memo.store(&format!("f{index}"), "k", index);
    }
    assert!(memo.get("f0", "k").is_some());

    memo.set_budget(MemoBudget::Large);
    memo.store("f5", "k", 5);
    let kept = (0..6)
        .filter(|index| memo.get(&format!("f{index}"), "k").is_some())
        .count();
    assert_eq!(kept, 2);
}

#[test]
fn the_budget_floor_keeps_the_current_and_previous_frame() {
    let mut memo = RenderMemo::new(config(1, 1));
    memo.store("A", "k", 1);
    memo.store("B", "k", 2);
    assert!(memo.get("A", "k").is_some());
    assert!(memo.get("B", "k").is_some());
}

#[test]
fn empty_identifiers_are_not_stored() {
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("", "key1", 1);
    memo.store("frameA", "", 2);
    assert!(memo.is_empty());
}

#[test]
fn rekey_moves_the_payload_to_a_new_identity() {
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("frameA", "pre-bounds", 4);
    memo.rekey("frameA", "post-bounds", "pre-bounds");
    assert_eq!(memo.get("frameA", "pre-bounds"), None);
    assert_eq!(memo.get("frameA", "post-bounds"), Some(&4));
    memo.rekey("frameB", "whatever", "");
    assert_eq!(memo.get("frameB", "whatever"), None);
}

#[test]
fn invalidation_and_clear_release_the_payload() {
    let dropped = Arc::new(Mutex::new(0));
    let mut memo = RenderMemo::new(config(2, 2));
    memo.store("A", "k", Owned::new(&dropped));
    memo.store("B", "k", Owned::new(&dropped));
    memo.invalidate("A");
    assert_eq!(*dropped.lock().unwrap(), 1);
    assert!(memo.get("B", "k").is_some());
    memo.clear();
    assert_eq!(*dropped.lock().unwrap(), 2);
}

#[test]
fn variants_share_the_budget_and_rekey_one_variant() {
    let mut memo = RenderMemo::with_variants(config(2, 2), true);
    memo.store("A", "off", 1);
    memo.store("A", "ca", 2);
    assert_eq!(memo.get("A", "off"), Some(&1));
    memo.store("A", "distortion", 3);
    assert_eq!(memo.get("A", "off"), Some(&1));
    assert_eq!(memo.get("A", "ca"), None);
    assert_eq!(memo.get("A", "distortion"), Some(&3));

    memo.rekey("A", "settled-on", "distortion");
    assert_eq!(memo.get("A", "distortion"), None);
    assert_eq!(memo.get("A", "off"), Some(&1));
    assert_eq!(memo.get("A", "settled-on"), Some(&3));
    memo.rekey("A", "unspecified", "");
    assert_eq!(memo.get("A", "unspecified"), None);
}
