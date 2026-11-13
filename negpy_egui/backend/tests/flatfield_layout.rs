//! Ports the observable rules of `tests/test_flatfield_layout_migration.py`.

use negpy_backend::features::flatfield::logic::{flatfield_token, gain_token};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::assets::flatfield::{FlatFieldProfiles, SENSOR_LAYOUT};
use negpy_backend::services::assets::migrations::flatfield::migrate_flatfield_sensor_layout;

fn sensor_gain(height: usize, width: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(height * width * 3);
    for y in 0..height {
        for x in 0..width {
            let value = 1.0 + 0.5 * (x as f32 / width as f32) + 0.25 * (y as f32 / height as f32);
            pixels.extend([value; 3]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn apply_exif(image: &ImageBuffer, orientation: u8) -> ImageBuffer {
    let src: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect();
    let (height, width) = negpy_core::orient::oriented_dims(image.height, image.width, orientation);
    let mut dst = vec![0u8; height * width * 12];
    negpy_core::orient::orient(&src, image.height, image.width, 12, orientation, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|bytes| f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}

fn turned(image: &ImageBuffer) -> ImageBuffer {
    apply_exif(image, 6)
}

fn store(directory: &std::path::Path) -> FlatFieldProfiles {
    FlatFieldProfiles {
        directory: directory.join("flatfield"),
        bake: |_| None,
    }
}

fn write_legacy(store: &FlatFieldProfiles, id: &str, gain: &ImageBuffer, source: &str) {
    let reference = ImageBuffer::from_rgb(
        gain.width,
        gain.height,
        gain.pixels.iter().map(|value| value * 0.4).collect(),
    );
    store
        .write(id, gain, "rig", 0.0, source, Some(&reference), None)
        .unwrap();
    // A profile from before the layout marker has no layout member.
    let path = store.directory.join(format!("{id}.npz"));
    let data = std::fs::read(&path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
    let mut kept = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        let name = member.name().to_string();
        if name == "layout.npy" {
            continue;
        }
        kept.start_file(name, options).unwrap();
        std::io::copy(&mut member, &mut kept).unwrap();
    }
    std::fs::write(path, kept.finish().unwrap().into_inner()).unwrap();
}

fn exif_tiff(path: &std::path::Path, orientation: u16) {
    // Little-endian TIFF: header, one IFD entry, next-IFD offset.
    let mut data = b"II*\x00\x08\x00\x00\x00\x01\x00".to_vec();
    data.extend(0x0112u16.to_le_bytes());
    data.extend(3u16.to_le_bytes());
    data.extend(1u32.to_le_bytes());
    data.extend(orientation.to_le_bytes());
    data.extend(0u16.to_le_bytes());
    data.extend(0u32.to_le_bytes());
    std::fs::write(path, data).unwrap();
}

#[test]
fn a_turned_reference_moves_to_sensor_layout() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    exif_tiff(&source, 6);
    let sensor = sensor_gain(24, 36);
    write_legacy(&store, "p", &turned(&sensor), &source.display().to_string());

    migrate_flatfield_sensor_layout(&store);

    let entry = store.load_gain("p").unwrap();
    assert_eq!(entry.gain.pixels, sensor.pixels);
    assert_eq!(entry.token, gain_token(&sensor));
    assert!(entry.sensor_layout);
    assert_eq!(store.get("p").unwrap().name, "rig");
    assert_eq!(store.load_check("p").unwrap().sensor_layout, true);
}

#[test]
fn an_upright_reference_keeps_its_gain() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    exif_tiff(&source, 1);
    let gain = sensor_gain(24, 36);
    write_legacy(&store, "p", &gain, &source.display().to_string());

    migrate_flatfield_sensor_layout(&store);

    let entry = store.load_gain("p").unwrap();
    assert_eq!(entry.gain.pixels, gain.pixels);
    assert!(entry.sensor_layout);
}

#[test]
fn a_missing_reference_stays_unmarked_until_it_returns() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    let gain = turned(&sensor_gain(24, 36));
    write_legacy(&store, "p", &gain, &source.display().to_string());

    migrate_flatfield_sensor_layout(&store);
    let entry = store.load_gain("p").unwrap();
    assert_eq!(entry.gain.pixels, gain.pixels);
    assert!(!entry.sensor_layout);

    exif_tiff(&source, 6);
    migrate_flatfield_sensor_layout(&store);
    let entry = store.load_gain("p").unwrap();
    assert_eq!(entry.gain.pixels, sensor_gain(24, 36).pixels);
    assert!(entry.sensor_layout);
}

#[test]
fn an_unmarked_profile_applies_as_baked() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let gain = turned(&sensor_gain(24, 36));
    write_legacy(
        &store,
        "p",
        &gain,
        &dir.path().join("gone.tif").display().to_string(),
    );

    let config = negpy_backend::domain::models::FlatFieldConfig {
        apply: true,
        profile_id: "p".to_string(),
    };
    let image = ImageBuffer::from_rgb(
        gain.width,
        gain.height,
        vec![0.5; gain.width * gain.height * 3],
    );
    let out = negpy_backend::features::flatfield::logic::apply_flatfield(
        &image,
        &config,
        store.load_gain("p").as_ref(),
        Some(6),
        None,
    );
    for (value, expected) in out.pixels.iter().zip(gain.pixels.iter()) {
        assert!((*value - 0.5 * expected).abs() < 1e-6);
    }
    assert!(flatfield_token(&config, store.load_gain("p").as_ref()).ends_with(":ref"));
}

#[test]
fn a_marked_profile_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    exif_tiff(&source, 6);
    write_legacy(
        &store,
        "p",
        &turned(&sensor_gain(24, 36)),
        &source.display().to_string(),
    );
    migrate_flatfield_sensor_layout(&store);
    let first = store.load_gain("p").unwrap().gain;

    migrate_flatfield_sensor_layout(&store);

    assert_eq!(store.load_gain("p").unwrap().gain.pixels, first.pixels);
}

#[test]
fn a_profile_keeps_its_stored_check() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    exif_tiff(&source, 6);
    let gain = turned(&sensor_gain(24, 36));
    let reference = ImageBuffer::from_rgb(24, 36, vec![0.4; 24 * 36 * 3]);
    let check = negpy_backend::features::flatfield::logic::Evenness {
        low: -0.01,
        high: 0.01,
        color: 0.005,
        clipped: false,
    };
    store
        .write(
            "p",
            &gain,
            "rig",
            0.0,
            &source.display().to_string(),
            Some(&reference),
            Some(check),
        )
        .unwrap();
    let path = store.directory.join("p.npz");
    let data = std::fs::read(&path).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
    let mut kept = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        if member.name() == "layout.npy" {
            continue;
        }
        let name = member.name().to_string();
        kept.start_file(name, options).unwrap();
        std::io::copy(&mut member, &mut kept).unwrap();
    }
    std::fs::write(path, kept.finish().unwrap().into_inner()).unwrap();

    migrate_flatfield_sensor_layout(&store);

    assert_eq!(store.load_check("p").unwrap().check, Some(check));
    assert_eq!(SENSOR_LAYOUT, "sensor");
}

#[test]
fn a_cut_short_write_leaves_the_original_file_listed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let source = dir.path().join("flat.tif");
    exif_tiff(&source, 6);
    let gain = turned(&sensor_gain(24, 36));
    write_legacy(&store, "p", &gain, &source.display().to_string());

    migrate_flatfield_sensor_layout(&store);

    let listing: Vec<String> = std::fs::read_dir(&store.directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(listing, ["p.npz"]);
}
