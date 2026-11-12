//! Ports of tests/test_sidecar.py.

use std::fs;

use negpy_backend::domain::models::workspace::{BOUNDS_VERSION, WorkspaceConfig};
use negpy_backend::domain::models::{ExposureConfig, LocalAdjustmentsConfig, LocalMask};
use negpy_backend::services::assets::sidecar::{
    load_sidecar, read_sidecar, sidecar_path_for, write_sidecar,
};

fn rich_config() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.exposure = ExposureConfig {
        density: 0.42,
        grade: 130.0,
        ..ExposureConfig::default()
    };
    config.geometry.fine_rotation = 1.5;
    config.geometry.crop_rect = Some(vec![0.1, 0.2, 0.8, 0.9]);
    config.local = LocalAdjustmentsConfig {
        masks: vec![LocalMask {
            vertices: vec![[0.0, 0.0], [0.5, 0.5]],
            stops: -0.7,
            feather: 0.05,
            ..LocalMask::default()
        }],
    };
    config
}

#[test]
fn the_path_is_next_to_the_source_and_names_a_half() {
    assert_eq!(
        sidecar_path_for(std::path::Path::new("/photos/IMG_001.NEF"), 0),
        std::path::PathBuf::from("/photos/IMG_001.negpy")
    );
    assert_eq!(
        sidecar_path_for(std::path::Path::new("/photos/IMG_001.NEF"), 2),
        std::path::PathBuf::from("/photos/IMG_001.negpy").with_file_name("IMG_001.2.negpy")
    );
}

#[test]
fn a_sidecar_round_trips_next_to_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("IMG_001.NEF");
    let config = rich_config();
    let path = write_sidecar(&source, &config, 0).unwrap();
    assert_eq!(path, dir.path().join("IMG_001.negpy"));

    let loaded = load_sidecar(&source, 0).unwrap();
    assert_eq!(loaded.exposure.density, 0.42);
    assert_eq!(loaded.exposure.grade, 130.0);
    assert_eq!(loaded.geometry.crop_rect, Some(vec![0.1, 0.2, 0.8, 0.9]));
    assert_eq!(loaded.local.masks[0].stops, -0.7);
}

#[test]
fn the_payload_records_the_bounds_version() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("IMG_001.NEF");
    let config = rich_config();
    write_sidecar(&source, &config, 0).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sidecar_path_for(&source, 0)).unwrap()).unwrap();
    assert_eq!(data["bounds_version"], BOUNDS_VERSION);
    assert_eq!(data["density"], 0.42);
}

#[test]
fn a_missing_or_malformed_sidecar_has_no_edit() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_sidecar(&dir.path().join("nope.NEF"), 0).is_none());
    let source = dir.path().join("IMG_003.NEF");
    let path = sidecar_path_for(&source, 0);
    fs::write(&path, "{ not valid json").unwrap();
    assert!(load_sidecar(&source, 0).is_none());
}

#[test]
fn older_bounds_are_dropped_unless_lock_bounds_keeps_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.negpy");
    let mut config = rich_config();
    config.process.local_floors = [-1.1, -1.2, -1.0];
    config.process.local_ceils = [-0.2, -0.4, -0.6];
    write_sidecar(&dir.path().join("frame.tif"), &config, 0).unwrap();
    let text = fs::read_to_string(sidecar_path_for(&dir.path().join("frame.tif"), 0)).unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&text).unwrap();
    data.as_object_mut().unwrap().remove("bounds_version");
    fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();

    let loaded = read_sidecar(&path).unwrap();
    assert!(!loaded.process.is_local_initialized());

    config.process.lock_bounds = true;
    write_sidecar(&dir.path().join("frame.tif"), &config, 0).unwrap();
    let text = fs::read_to_string(sidecar_path_for(&dir.path().join("frame.tif"), 0)).unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&text).unwrap();
    data.as_object_mut().unwrap().remove("bounds_version");
    fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
    assert_eq!(
        read_sidecar(&path).unwrap().process.local_floors,
        [-1.1, -1.2, -1.0]
    );
}
