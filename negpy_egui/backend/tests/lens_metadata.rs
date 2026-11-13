//! Ports the lens-metadata availability and description rules.

use negpy_backend::domain::models::{LensCorrections, LensMetadata, LensWarp};

#[test]
fn the_default_reason_is_shown_when_nothing_is_available() {
    let lens = LensMetadata::default();
    assert!(!lens.available());
    assert!(!lens.distortion());
    assert!(!lens.ca());
    assert_eq!(lens.description(), "No embedded lens correction data.");
}

#[test]
fn an_available_lens_names_its_corrections() {
    let lens = LensMetadata {
        source: "Sony ARW".into(),
        warps: vec![
            LensWarp::Rectilinear {
                coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
                center: (0.5, 0.5),
            },
            LensWarp::Sony {
                distortion: vec![],
                ca_red: vec![100.0],
                ca_blue: vec![-100.0],
            },
        ],
        ..LensMetadata::default()
    };
    assert!(lens.available());
    assert!(lens.distortion());
    assert!(lens.ca());
    assert_eq!(lens.description(), "Sony ARW: distortion + lateral CA");
}

#[test]
fn one_correction_alone_is_named_alone() {
    let distortion = LensMetadata {
        source: "DNG".into(),
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    assert_eq!(distortion.description(), "DNG: distortion");
    let ca = LensMetadata {
        source: "ARW".into(),
        warps: vec![LensWarp::Sony {
            distortion: vec![],
            ca_red: vec![100.0],
            ca_blue: vec![-100.0],
        }],
        ..LensMetadata::default()
    };
    assert_eq!(ca.description(), "ARW: lateral CA");
}

#[test]
fn the_correction_flags_combine_for_the_warp_request() {
    assert!(!LensCorrections::default().any());
    assert!(
        LensCorrections {
            distortion: true,
            ca: false
        }
        .any()
    );
    assert!(
        LensCorrections {
            distortion: true,
            ca: true
        }
        .any()
    );
}
