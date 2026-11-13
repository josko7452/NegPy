//! Ports the observable rules of `tests/test_flatfield_profiles.py`.

use negpy_backend::features::flatfield::logic::{Evenness, apply_flatfield, gain_token};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::assets::flatfield::{FlatFieldProfiles, SENSOR_LAYOUT};

fn gain(height: usize, width: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(height * width * 3);
    for y in 0..height {
        for x in 0..width {
            let value = 1.0 + 0.5 * (x as f32 / width as f32) + 0.25 * (y as f32 / height as f32);
            pixels.extend([value; 3]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn store() -> (tempfile::TempDir, FlatFieldProfiles) {
    let dir = tempfile::tempdir().unwrap();
    let store = FlatFieldProfiles::new(dir.path().join("flatfield"));
    (dir, store)
}

#[test]
fn an_imported_gain_round_trips_with_its_metadata() {
    let (_dir, store) = store();
    let gain = gain(24, 32);
    let id = store.import_gain(&gain, "Rig A", -0.08, "/refs/flat.dng");

    let loaded = store.load_gain(&id).unwrap();
    assert_eq!(loaded.gain.pixels, gain.pixels);
    assert_eq!((loaded.gain.width, loaded.gain.height), (32, 24));
    assert_eq!(loaded.token, gain_token(&gain));
    assert!(loaded.sensor_layout);

    let profile = store.get(&id).unwrap();
    assert_eq!(
        (profile.id, profile.name, profile.k1, profile.source),
        (id, "Rig A".to_string(), -0.08, "/refs/flat.dng".to_string())
    );
}

#[test]
fn opaque_ids_are_unique_per_profile() {
    let (_dir, store) = store();
    let first = store.import_gain(&gain(8, 8), "dup", 0.0, "");
    let second = store.import_gain(&gain(8, 8), "dup", 0.0, "");
    assert_ne!(first, second);
    let listed = store.list_profiles();
    assert_eq!(
        listed
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>(),
        ["dup", "dup"]
    );
    let mut ids: Vec<&str> = listed.iter().map(|(id, _)| id.as_str()).collect();
    ids.sort_unstable();
    let mut expected = [first.as_str(), second.as_str()];
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

#[test]
fn profiles_are_sorted_by_name() {
    let (_dir, store) = store();
    store.import_gain(&gain(8, 8), "Zeta", 0.0, "");
    store.import_gain(&gain(8, 8), "alpha", 0.0, "");
    assert_eq!(
        store
            .list_profiles()
            .into_iter()
            .map(|(_, name)| name)
            .collect::<Vec<_>>(),
        ["alpha", "Zeta"]
    );
}

#[test]
fn a_missing_profile_reads_as_none_and_deletes_idempotently() {
    let (_dir, store) = store();
    assert!(store.load_gain("ghost").is_none());
    assert!(store.load_check("ghost").is_none());
    assert!(store.get("ghost").is_none());
    let id = store.import_gain(&gain(8, 8), "rig", 0.0, "");
    store.delete(&id);
    store.delete(&id);
    assert!(store.load_gain(&id).is_none());
}

#[test]
fn metadata_reads_do_not_require_a_gain() {
    let (_dir, store) = store();
    std::fs::create_dir_all(&store.directory).unwrap();
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let entry = zip::write::SimpleFileOptions::default();
    zip.start_file("name.npy".to_string(), entry).unwrap();
    {
        use npyz::WriterBuilder;
        use std::io::Write;
        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut writer = npyz::WriteOptions::new()
            .dtype(npyz::DType::parse("'<U8'").unwrap())
            .writer(&mut buffer)
            .begin_1d()
            .unwrap();
        writer.push("MetaOnly").unwrap();
        writer.finish().unwrap();
        zip.write_all(&buffer.into_inner()).unwrap();
    }
    zip.start_file("k1.npy".to_string(), entry).unwrap();
    {
        use npyz::WriterBuilder;
        let mut writer = npyz::WriteOptions::new()
            .default_dtype()
            .shape(&[1])
            .writer(&mut zip)
            .begin_nd()
            .unwrap();
        writer.extend([0.05f64]).unwrap();
        writer.finish().unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    std::fs::write(store.directory.join("meta-only.npz"), bytes).unwrap();

    let profile = store.get("meta-only").unwrap();
    assert_eq!((profile.name.as_str(), profile.k1), ("MetaOnly", 0.05));
    assert!(
        store
            .list_profiles()
            .contains(&("meta-only".to_string(), "MetaOnly".to_string()))
    );
    assert!(store.load_gain("meta-only").is_none());
}

#[test]
fn a_checked_profile_stores_its_reference_copy_and_check() {
    let (_dir, store) = store();
    let checked = Evenness {
        low: -0.01,
        high: 0.01,
        color: 0.005,
        clipped: false,
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    store
        .write(
            &id,
            &gain(8, 8),
            "rig",
            0.0,
            "/refs/rig.dng",
            Some(&ImageBuffer::from_rgb(8, 8, vec![0.4; 8 * 8 * 3])),
            Some(checked),
        )
        .unwrap();
    let stored = store.load_check(&id).unwrap();
    assert!(stored.reference.is_some());
    assert_eq!(stored.check, Some(checked));
    assert!(stored.sensor_layout);
    assert!(
        store
            .load_check(&id)
            .unwrap()
            .reference
            .unwrap()
            .pixels
            .iter()
            .all(|v| (*v - 0.4).abs() < 1e-3)
    );
}

#[test]
fn an_imported_gain_has_no_reference_copy_or_check() {
    let (_dir, store) = store();
    let id = store.import_gain(&gain(8, 8), "old", 0.0, "");
    let stored = store.load_check(&id).unwrap();
    assert!(stored.reference.is_none());
    assert!(stored.check.is_none());
}

#[test]
fn a_stored_profile_applies_through_the_provider() {
    let (_dir, store) = store();
    let id = store.import_gain(&gain(24, 32), "rig", 0.0, "");
    let gain = store.load_gain(&id).unwrap();
    let image = ImageBuffer::from_rgb(32, 24, vec![0.5; 24 * 32 * 3]);
    let mut config = negpy_backend::domain::models::FlatFieldConfig::default();
    config.apply = true;
    config.profile_id = id;
    let out = apply_flatfield(&image, &config, Some(&gain), Some(1), None);
    for (value, expected) in out.pixels.iter().zip(gain.gain.pixels.iter()) {
        assert!((*value - 0.5 * expected).abs() < 1e-6);
    }
}

#[test]
fn the_layout_marker_is_the_python_marker() {
    assert_eq!(SENSOR_LAYOUT, "sensor");
}
