//! Ports the warp-capability and center-fixed cases of tests/test_metadata_lens.py.

use negpy_backend::domain::models::{IDENTITY, LensCorrections, LensMetadata, LensWarp};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn image() -> ImageBuffer {
    ImageBuffer::from_rgb(60, 40, (0..60 * 40 * 3).map(|i| 0.2 + i as f32 * 0.0001).collect())
}

#[test]
fn identity_warps_choose_nothing() {
    let lens = LensMetadata {
        warps: vec![
            LensWarp::Rectilinear {
                coefficients: vec![IDENTITY],
                center: (0.5, 0.5),
            },
            LensWarp::Rectilinear {
                coefficients: vec![IDENTITY; 3],
                center: (0.5, 0.5),
            },
            LensWarp::Sony {
                distortion: vec![0.0; 16],
                ca_red: vec![0.0; 16],
                ca_blue: vec![0.0; 16],
            },
        ],
        ..LensMetadata::default()
    };
    assert!(!lens.distortion());
    assert!(!lens.ca());
    assert!(!lens.available());
    let source = image();
    assert_eq!(lens.apply(&source, 1, LensCorrections { distortion: true, ca: true }), source);
}

#[test]
fn a_warp_capability_names_its_own_axis() {
    let distortion = LensWarp::Rectilinear {
        coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
        center: (0.5, 0.5),
    };
    assert!(distortion.has_distortion());
    assert!(!distortion.has_ca());
    let ca = LensWarp::Rectilinear {
        coefficients: vec![
            [1.01, 0.0, 0.0, 0.0, 0.0, 0.0],
            IDENTITY,
            [0.99, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
        center: (0.5, 0.5),
    };
    assert!(!ca.has_distortion());
    assert!(ca.has_ca());
    let sony = LensWarp::Sony {
        distortion: vec![],
        ca_red: vec![100.0; 16],
        ca_blue: vec![-100.0; 16],
    };
    assert!(!sony.has_distortion());
    assert!(sony.has_ca());
}

#[test]
fn a_strong_distortion_moves_the_edge_and_holds_the_center() {
    let source = ImageBuffer::from_rgb(90, 60, (0..90 * 60 * 3).map(|i| i as f32 / 1000.0).collect());
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, 0.05, 0.01, 0.0, 0.001, -0.002]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    let out = lens.apply(&source, 1, LensCorrections { distortion: true, ca: false });
    assert_eq!(out.width, source.width);
    assert_eq!(out.height, source.height);
    assert_ne!(out.pixels, source.pixels);
    let center = (30 * 90 + 45) * 3;
    assert!((out.pixels[center] - source.pixels[center]).abs() < 0.25);
}

#[test]
fn a_fill_scale_never_exceeds_one() {
    let lens = LensMetadata {
        warps: vec![LensWarp::Sony {
            distortion: (0..16).map(|i| i as f64 * 100.0).collect(),
            ca_red: vec![],
            ca_blue: vec![],
        }],
        ..LensMetadata::default()
    };
    let scale = lens.fill_scale((60, 90), LensCorrections { distortion: true, ca: false });
    assert!((0.5..=1.0).contains(&scale));
}
