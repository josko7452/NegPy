//! Ports the composite preview cases in `tests/test_preview_cache.py`.

use std::path::Path;

use negpy_backend::domain::models::{HdrConfig, RgbScanConfig, StitchConfig};
use negpy_backend::domain::types::AppConfig;
use negpy_backend::services::rendering::preview_manager::{PreviewManager, PreviewRequest};
use tiff::encoder::TiffEncoder;
use tiff::encoder::colortype::RGB16;

fn config() -> AppConfig {
    AppConfig {
        preview_render_size: 64,
        preview_cache_max_entries: 8,
        preview_cache_max_bytes: 1 << 20,
        ..AppConfig::with_defaults()
    }
}

fn write_tiff(path: &Path, width: u32, height: u32, values: [u16; 3]) {
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for _ in 0..width * height {
        pixels.extend_from_slice(&values);
    }
    let mut encoder = TiffEncoder::new(std::fs::File::create(path).unwrap()).unwrap();
    encoder
        .write_image::<RGB16>(width, height, &pixels)
        .unwrap();
}

fn request(hash: &str) -> PreviewRequest {
    PreviewRequest {
        full_resolution: false,
        ..PreviewRequest::new(hash, "sRGB")
    }
}

#[test]
fn a_triplet_takes_one_channel_from_each_exposure() {
    let dir = tempfile::tempdir().unwrap();
    let red = dir.path().join("r.tif");
    let green = dir.path().join("g.tif");
    let blue = dir.path().join("b.tif");
    write_tiff(&red, 4, 4, [40000, 1000, 1000]);
    write_tiff(&green, 4, 4, [1000, 40000, 1000]);
    write_tiff(&blue, 4, 4, [1000, 1000, 40000]);
    let manager = PreviewManager::new(config());
    let triplet = RgbScanConfig {
        enabled: true,
        green_path: green.display().to_string(),
        blue_path: blue.display().to_string(),
        align: false,
    };
    let (image, dims, metadata) = manager
        .load_rgb(&red, &triplet, &request("triplet"), &[], false, None)
        .expect("triplet");
    assert_eq!(dims, (4, 4));
    assert!(image.pixels[0] > 0.5);
    assert!(image.pixels[1] > 0.5);
    assert!(image.pixels[2] > 0.5);
    assert!(metadata["camera_wb"][0].is_nan());
    let (cached, _, _) = manager
        .load_rgb(&red, &triplet, &request("triplet"), &[], false, None)
        .expect("cached triplet");
    assert_eq!(cached.pixels, image.pixels);
}

#[test]
fn a_bracket_merges_to_its_reference_exposure() {
    let dir = tempfile::tempdir().unwrap();
    let reference = dir.path().join("ref.tif");
    let longer = dir.path().join("long.tif");
    write_tiff(&reference, 4, 4, [16000, 16000, 16000]);
    write_tiff(&longer, 4, 4, [32000, 32000, 32000]);
    let manager = PreviewManager::new(config());
    let hdr = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec![longer.display().to_string()],
        hdr_ratios: vec![1.0, 2.0],
        hdr_align: false,
        ..HdrConfig::default()
    };
    let (image, dims, _) = manager
        .load_hdr(&reference, &hdr, &request("bracket"), &[], false, None)
        .expect("bracket");
    assert_eq!(dims, (4, 4));
    assert!(image.pixels.iter().all(|value| value.is_finite()));
    assert!(image.pixels[0] > 0.0);
}

#[test]
fn a_bracket_with_the_wrong_ratio_count_is_refused() {
    let manager = PreviewManager::new(config());
    let hdr = HdrConfig {
        hdr_enabled: true,
        hdr_paths: vec!["/missing/long.tif".into()],
        hdr_ratios: vec![1.0],
        ..HdrConfig::default()
    };
    let error = manager
        .load_hdr(
            Path::new("/missing/ref.tif"),
            &hdr,
            &request("bracket"),
            &[],
            false,
            None,
        )
        .expect_err("ratio count");
    assert!(error.contains("ratios"));
}

#[test]
fn a_stitch_replays_its_registration() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.tif");
    let second = dir.path().join("second.tif");
    write_tiff(&first, 4, 2, [16000, 16000, 16000]);
    write_tiff(&second, 4, 2, [24000, 24000, 24000]);
    let manager = PreviewManager::new(config());
    let stitch = StitchConfig {
        stitch_enabled: true,
        stitch_paths: vec![second.display().to_string()],
        stitch_transforms: vec![
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            vec![1.0, 0.0, 2.0, 0.0, 1.0, 0.0],
        ],
        stitch_canvas: [6, 2],
        stitch_sizes: vec![[4, 2], [4, 2]],
        stitch_triplets: vec![
            [String::new(), String::new()],
            [String::new(), String::new()],
        ],
        stitch_align: true,
    };
    let (image, dims, _) = manager
        .load_stitch(&first, &stitch, "", &request("stitch"), &[], false, None)
        .expect("stitch");
    assert_eq!(dims, (2, 6));
    assert_eq!((image.width, image.height), (6, 2));
    assert!(image.pixels.iter().all(|value| value.is_finite()));
}
