//! Ports the armed Auto Crop and meter-box cases from `tests/test_autocrop_square.py`
//! and `tests/test_meter_rect.py`.

use std::collections::HashMap;

use negpy_backend::domain::models::{GeometryConfig, WorkspaceConfig};
use negpy_backend::features::geometry::logic::{autocrop_detection_key, meter_detection_key};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::rendering::auto_geometry::{
    ArmedCrop, resolve_armed_autocrop_with, resolve_meter_rect_with,
};

fn frame() -> ImageBuffer {
    ImageBuffer::from_rgb(60, 40, vec![0.0; 60 * 40 * 3])
}

fn armed() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.geometry.crop_from_auto = true;
    config.geometry.crop_rect = None;
    config.geometry.autocrop_square = true;
    config
}

#[test]
fn a_fresh_arm_is_squared_then_cropped_on_the_squared_frame() {
    let seen = std::cell::RefCell::new(Vec::new());
    let (settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &armed(),
        1600.0,
        |_image, geometry, _mode| {
            let mut squared = geometry.clone();
            squared.fine_rotation = 1.5;
            squared.converge_v = 0.7;
            Some((squared, [1.5, 0.7, 0.0]))
        },
        |_image, geometry, _size| {
            seen.borrow_mut().push(geometry.clone());
            Some([0.1, 0.1, 0.9, 0.9])
        },
    );
    let resolved = resolved.expect("an armed crop resolves");
    assert_eq!(
        seen.borrow()[0].fine_rotation,
        1.5,
        "the crop saw the squared frame"
    );
    assert_eq!(resolved.skew, Some([1.5, 0.7, 0.0]));
    assert_eq!(resolved.key, autocrop_detection_key(&settings.geometry));
    assert_eq!(settings.geometry.crop_rect, Some(vec![0.1, 0.1, 0.9, 0.9]));
}

#[test]
fn square_off_crops_without_fitting() {
    let mut config = armed();
    config.geometry.autocrop_square = false;
    let outfitted = std::cell::Cell::new(false);
    let (_settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &config,
        1600.0,
        |_image, _geometry, _mode| {
            outfitted.set(true);
            None
        },
        |_image, _geometry, _size| Some([0.1, 0.1, 0.9, 0.9]),
    );
    assert!(!outfitted.get());
    assert_eq!(resolved.expect("a crop").skew, None);
}

#[test]
fn a_frame_with_its_own_angle_is_not_squared() {
    let mut config = armed();
    config.geometry.fine_rotation = 0.3;
    let outfitted = std::cell::Cell::new(false);
    let (_settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &config,
        1600.0,
        |_image, _geometry, _mode| {
            outfitted.set(true);
            None
        },
        |_image, _geometry, _size| Some([0.1, 0.1, 0.9, 0.9]),
    );
    assert!(!outfitted.get());
    assert_eq!(resolved.expect("a crop").skew, None);
}

#[test]
fn a_re_detect_of_a_squared_arm_keeps_the_angle_it_has() {
    let mut config = armed();
    config.geometry.autocrop_squared = true;
    config.geometry.crop_rect = Some(vec![0.2, 0.2, 0.8, 0.8]);
    config.geometry.crop_detect_key = "old".to_string();
    let outfitted = std::cell::Cell::new(false);
    let (_settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &config,
        1600.0,
        |_image, _geometry, _mode| {
            outfitted.set(true);
            None
        },
        |_image, _geometry, _size| Some([0.1, 0.1, 0.9, 0.9]),
    );
    assert!(!outfitted.get());
    assert!(resolved.is_some());
}

#[test]
fn an_untrusted_fit_crops_the_frame_as_it_is() {
    let (settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &armed(),
        1600.0,
        |_image, _geometry, _mode| None,
        |_image, _geometry, _size| Some([0.1, 0.1, 0.9, 0.9]),
    );
    let resolved = resolved.expect("an attempted arm reports");
    assert_eq!(resolved.skew, None);
    assert!(resolved.squared);
    assert_eq!(settings.geometry.fine_rotation, 0.0);
}

#[test]
fn a_failed_squaring_with_no_crop_still_reports_the_attempt() {
    let (settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &armed(),
        1600.0,
        |_image, _geometry, _mode| None,
        |_image, _geometry, _size| None,
    );
    assert_eq!(
        resolved,
        Some(ArmedCrop {
            rect: None,
            key: autocrop_detection_key(&settings.geometry),
            skew: None,
            squared: true,
        })
    );
}

#[test]
fn an_arm_already_squared_is_not_squared_again() {
    let mut config = armed();
    config.geometry.autocrop_squared = true;
    let outfitted = std::cell::Cell::new(false);
    let (_settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &config,
        1600.0,
        |_image, _geometry, _mode| {
            outfitted.set(true);
            None
        },
        |_image, _geometry, _size| None,
    );
    assert!(!outfitted.get());
    assert_eq!(resolved, None);
}

#[test]
fn a_resolved_rect_with_its_key_is_not_detected_again() {
    let config = WorkspaceConfig::default();
    let mut frozen = config.clone();
    frozen.geometry.crop_from_auto = true;
    frozen.geometry.crop_rect = Some(vec![0.1, 0.1, 0.9, 0.9]);
    frozen.geometry.crop_detect_key = autocrop_detection_key(&frozen.geometry);
    let called = std::cell::Cell::new(false);
    let (settings, resolved) = resolve_armed_autocrop_with(
        &frame(),
        &frozen,
        1600.0,
        |_image, _geometry, _mode| None,
        |_image, _geometry, _size| {
            called.set(true);
            None
        },
    );
    assert!(!called.get());
    assert_eq!(resolved, None);
    assert_eq!(settings, frozen);
}

fn meter_settings() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.process.local_floors = [0.2, 0.3, 0.4];
    config.process.local_ceils = [1.2, 1.3, 1.4];
    config
}

#[test]
fn a_frame_with_no_crop_resolves_its_meter_rect_and_re_meters() {
    let config = meter_settings();
    let (settings, boxed) = resolve_meter_rect_with(
        &frame(),
        &config,
        1600.0,
        None,
        "source",
        |_image, _geometry, _size| Some([0.2, 0.2, 0.8, 0.8]),
    );
    let boxed = boxed.expect("a meter box");
    let key = meter_detection_key(&settings.geometry, 60, 40);
    assert_eq!(settings.process.meter_rect, Some(vec![0.2, 0.2, 0.8, 0.8]));
    assert_eq!(settings.process.meter_key, key);
    assert_eq!(boxed.key, key);
    assert_eq!(
        boxed.geometry_key,
        autocrop_detection_key(&settings.geometry)
    );
    assert_eq!(settings.process.local_floors, [0.0; 3]);
    assert_eq!(settings.process.local_ceils, [0.0; 3]);
}

#[test]
fn a_resolved_meter_rect_is_not_detected_again() {
    let config = meter_settings();
    let (resolved, _) = resolve_meter_rect_with(
        &frame(),
        &config,
        1600.0,
        None,
        "source",
        |_image, _geometry, _size| Some([0.2, 0.2, 0.8, 0.8]),
    );
    let called = std::cell::Cell::new(false);
    let (again, boxed) = resolve_meter_rect_with(
        &frame(),
        &resolved,
        1600.0,
        None,
        "source",
        |_image, _geometry, _size| {
            called.set(true);
            None
        },
    );
    assert!(!called.get());
    assert_eq!(boxed, None);
    assert_eq!(again, resolved);
}

#[test]
fn a_cropped_frame_or_a_freehand_region_needs_no_meter_rect() {
    let config = meter_settings();
    let mut cropped = config.clone();
    cropped.geometry = GeometryConfig {
        crop_rect: Some(vec![0.2, 0.2, 0.8, 0.8]),
        ..GeometryConfig::default()
    };
    assert_eq!(
        resolve_meter_rect_with(&frame(), &cropped, 1600.0, None, "s", |_, _, _| Some(
            [0.0; 4]
        )),
        (cropped.clone(), None)
    );
    let mut freehand = config.clone();
    freehand.process.analysis_rect = Some(vec![0.3, 0.3, 0.7, 0.7]);
    assert_eq!(
        resolve_meter_rect_with(&frame(), &freehand, 1600.0, None, "s", |_, _, _| Some(
            [0.0; 4]
        )),
        (freehand.clone(), None)
    );
    let mut armed_config = config.clone();
    armed_config.geometry.crop_from_auto = true;
    assert_eq!(
        resolve_meter_rect_with(&frame(), &armed_config, 1600.0, None, "s", |_, _, _| Some(
            [0.0; 4]
        )),
        (armed_config.clone(), None)
    );
}

#[test]
fn a_full_bleed_frame_keeps_its_bounds() {
    let config = meter_settings();
    let mut frozen = config.clone();
    frozen.process.meter_rect = None;
    frozen.process.meter_key = meter_detection_key(&frozen.geometry, 60, 40);
    let (settings, _boxed) = resolve_meter_rect_with(
        &frame(),
        &frozen,
        1600.0,
        None,
        "source",
        |_image, _geometry, _size| None,
    );
    assert_eq!(settings, frozen);
}

#[test]
fn locked_bounds_survive_a_new_meter_rect() {
    let mut config = meter_settings();
    config.process.lock_bounds = true;
    let (settings, _boxed) = resolve_meter_rect_with(
        &frame(),
        &config,
        1600.0,
        None,
        "source",
        |_image, _geometry, _size| Some([0.2, 0.2, 0.8, 0.8]),
    );
    assert_eq!(settings.process.local_floors, [0.2, 0.3, 0.4]);
    assert_eq!(settings.process.local_ceils, [1.2, 1.3, 1.4]);
}

#[test]
fn a_source_is_detected_once_across_renders_that_never_froze() {
    let config = meter_settings();
    let mut memo = HashMap::new();
    let mut calls = 0;
    for _ in 0..3 {
        let (_settings, boxed) = resolve_meter_rect_with(
            &frame(),
            &config,
            1600.0,
            Some(&mut memo),
            "source",
            |_image, _geometry, _size| {
                calls += 1;
                Some([0.2, 0.2, 0.8, 0.8])
            },
        );
        assert!(boxed.is_some());
    }
    assert_eq!(calls, 1);
}
