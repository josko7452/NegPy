//! Ports of tests/test_assets.py and tests/test_thumbnail_fingerprint.py.

use std::fs;

use image::{Rgb, RgbImage};
use negpy_backend::infrastructure::storage::local_asset_store::{LocalAssetStore, file_hash};

const THUMB_SIZE: i64 = 256;

fn store(dir: &std::path::Path) -> LocalAssetStore {
    let store = LocalAssetStore::new(dir.join("cache"), dir.join("icc"), THUMB_SIZE);
    store.initialize().unwrap();
    store
}

fn image() -> RgbImage {
    RgbImage::from_pixel(THUMB_SIZE as u32, THUMB_SIZE as u32, Rgb([10, 20, 30]))
}

#[test]
fn a_thumbnail_at_the_current_size_is_returned() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store.save_thumbnail("current", &image(), None).unwrap();
    assert_eq!(
        store.get_thumbnail("current").unwrap().dimensions(),
        (THUMB_SIZE as u32, THUMB_SIZE as u32)
    );
}

#[test]
fn a_stale_size_or_truncated_thumbnail_is_a_miss() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let stale = RgbImage::from_pixel(128, 128, Rgb([10, 20, 30]));
    store.save_thumbnail("stale", &stale, None).unwrap();
    assert!(store.get_thumbnail("stale").is_none());
    assert!(store.get_thumbnail("absent").is_none());

    store.save_thumbnail("cut", &image(), None).unwrap();
    let path = store.thumb_dir().join("cut.jpg");
    let data = fs::read(&path).unwrap();
    fs::write(&path, &data[..data.len() / 2]).unwrap();
    assert!(store.get_thumbnail("cut").is_none());
}

#[test]
fn a_fingerprint_rides_in_the_comment_and_clears_with_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store
        .save_thumbnail("h1", &image(), Some("deadbeef"))
        .unwrap();
    assert_eq!(
        store.get_thumbnail_fingerprint("h1").as_deref(),
        Some("deadbeef")
    );
    store.save_thumbnail("h2", &image(), None).unwrap();
    assert!(store.get_thumbnail_fingerprint("h2").is_none());
    assert!(store.get_thumbnail_fingerprint("absent").is_none());

    store.clear_thumbnails().unwrap();
    assert!(store.get_thumbnail_fingerprint("h1").is_none());
}

#[test]
fn thumbnail_stats_count_files_and_clearing_leaves_the_cache_usable() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    store.save_thumbnail("h1", &image(), None).unwrap();
    store.save_thumbnail("h2", &image(), None).unwrap();
    let (count, bytes) = store.thumbnail_stats();
    assert_eq!(count, 2);
    assert!(bytes > 0);

    store.clear_thumbnails().unwrap();
    assert_eq!(store.thumbnail_stats(), (0, 0));
    store.save_thumbnail("h3", &image(), None).unwrap();
    assert!(store.get_thumbnail("h3").is_some());
}

#[test]
fn the_content_hash_covers_the_interior_sample() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.nef");
    let mut data = vec![0u8; 5 * 1024 * 1024];
    data[..8].copy_from_slice(b"NEGPYRAW");
    fs::write(&path, &data).unwrap();
    let first = file_hash(&path).unwrap();
    data[2 * 1024 * 1024 + 17] = 1;
    fs::write(&path, &data).unwrap();
    assert_ne!(first, file_hash(&path).unwrap());
}
