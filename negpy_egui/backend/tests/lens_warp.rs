//! Ports the warp-capability and center-fixed cases of tests/test_metadata_lens.py.

use negpy_backend::domain::models::{IDENTITY, LensCorrections, LensMetadata, LensWarp};
use negpy_backend::features::rgbscan::apply_lens;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn image() -> ImageBuffer {
    ImageBuffer::from_rgb(
        60,
        40,
        (0..60 * 40 * 3).map(|i| 0.2 + i as f32 * 0.0001).collect(),
    )
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
    assert_eq!(
        lens.apply(
            &source,
            1,
            LensCorrections {
                distortion: true,
                ca: true
            }
        ),
        source
    );
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
    let source = ImageBuffer::from_rgb(
        90,
        60,
        (0..90 * 60 * 3).map(|i| i as f32 / 1000.0).collect(),
    );
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, 0.05, 0.01, 0.0, 0.001, -0.002]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    let out = lens.apply(
        &source,
        1,
        LensCorrections {
            distortion: true,
            ca: false,
        },
    );
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
    let scale = lens.fill_scale(
        (60, 90),
        LensCorrections {
            distortion: true,
            ca: false,
        },
    );
    assert!((0.5..=1.0).contains(&scale));
}

fn ramp(width: usize, height: usize) -> ImageBuffer {
    ImageBuffer::from_rgb(
        width,
        height,
        (0..width * height * 3)
            .map(|i| 0.1 + (i % (width * 3)) as f32 / (width * 3) as f32 * 0.7)
            .collect(),
    )
}

#[test]
fn the_lens_entry_point_is_the_python_name() {
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    let image = ramp(90, 60);
    let selected = LensCorrections {
        distortion: true,
        ca: true,
    };
    assert_eq!(
        apply_lens(&image, &lens, 1, selected),
        lens.apply(&image, 1, selected)
    );
}

#[test]
fn no_correction_keeps_the_source() {
    let image = ramp(30, 20);
    assert_eq!(
        apply_lens(
            &image,
            &LensMetadata::default(),
            1,
            LensCorrections::default()
        ),
        image
    );
}

#[test]
fn a_lens_remap_moves_a_smooth_ramp_continuously() {
    let image = ramp(90, 60);
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, 0.04, 0.01, 0.0, 0.001, -0.002]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    let out = apply_lens(
        &image,
        &lens,
        1,
        LensCorrections {
            distortion: true,
            ca: false,
        },
    );
    let row = 30 * out.width;
    let steps: Vec<f32> = (1..out.width)
        .map(|col| (out.pixels[(row + col) * 3] - out.pixels[(row + col - 1) * 3]).abs())
        .collect();
    assert!(steps.iter().all(|step| *step < 0.05));
}

#[test]
fn orientation_wraps_the_same_correction() {
    let base = ramp(90, 60);
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, -0.05, 0.0, 0.0, 0.0, 0.0]],
            center: (0.5, 0.5),
        }],
        ..LensMetadata::default()
    };
    let selected = LensCorrections {
        distortion: true,
        ca: false,
    };
    let oriented = orient(&base, 6);
    let expected = orient(&apply_lens(&base, &lens, 1, selected), 6);
    assert_eq!(apply_lens(&oriented, &lens, 6, selected), expected);
}

fn orient(img: &ImageBuffer, orientation: u8) -> ImageBuffer {
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let (height, width) = negpy_core::orient::oriented_dims(img.height, img.width, orientation);
    let mut dst = vec![0u8; height * width * 12];
    negpy_core::orient::orient(&bytes, img.height, img.width, 12, orientation, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}
