//! Ports the affine and canvas cases of tests/test_stitch.py.

use negpy_backend::domain::models::StitchConfig;
use negpy_backend::features::stitch::{
    Affine, chain_affine, compute_canvas, scale_affine, scale_transforms,
};

#[test]
fn scale_affine_moves_proxy_coordinates_to_full_resolution() {
    let matrix: Affine = [1.0, 0.0, 10.0, 0.0, 1.0, 20.0];
    let scaled = scale_affine(matrix, 0.5, 0.25);
    assert_eq!(scaled[0], 0.5);
    assert_eq!(scaled[4], 0.5);
    assert_eq!(scaled[2], 20.0);
    assert_eq!(scaled[5], 40.0);
}

#[test]
fn chain_affine_composes_right_to_left() {
    let a: Affine = [1.0, 0.0, 10.0, 0.0, 1.0, 20.0];
    let b: Affine = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0];
    let out = chain_affine(a, b);
    assert_eq!(out, [2.0, 0.0, 10.0, 0.0, 2.0, 20.0]);
}

#[test]
fn the_canvas_is_the_shifted_union_of_the_warped_parts() {
    let sizes = [(100, 50), (100, 50)];
    let transforms = [
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [1.0, 0.0, 80.0, 0.0, 1.0, 5.0],
    ];
    let ((width, height), shifted) = compute_canvas(&sizes, &transforms);
    assert_eq!((width, height), (180, 55));
    assert_eq!(shifted[0], [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(shifted[1], [1.0, 0.0, 80.0, 0.0, 1.0, 5.0]);
}

#[test]
fn a_negative_origin_is_shifted_up_and_left() {
    let sizes = [(100, 50)];
    let transforms = [[1.0, 0.0, -20.0, 0.0, 1.0, -10.0]];
    let ((width, height), shifted) = compute_canvas(&sizes, &transforms);
    assert_eq!((width, height), (100, 50));
    assert_eq!(shifted[0][2], 0.0);
    assert_eq!(shifted[0][5], 0.0);
}

#[test]
fn stored_transforms_scale_with_the_decoded_parts() {
    let config = StitchConfig {
        stitch_enabled: true,
        stitch_paths: vec!["/part1".to_string()],
        stitch_transforms: vec![
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            vec![1.0, 0.0, 800.0, 0.0, 1.0, 0.0],
        ],
        stitch_canvas: [1800, 1200],
        stitch_sizes: vec![[1800, 1200], [1800, 1200]],
        ..StitchConfig::default()
    };
    let (transforms, canvas) = scale_transforms(&config, &[(900, 600), (900, 600)]);
    assert_eq!(canvas, (900, 600));
    assert_eq!(transforms[0], [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(transforms[1][2], 400.0);
    assert_eq!(transforms[1][0], 1.0);
}
