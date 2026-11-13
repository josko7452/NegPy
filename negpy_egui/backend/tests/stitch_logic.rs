//! Ports the affine and canvas cases of tests/test_stitch.py.

use negpy_backend::domain::models::StitchConfig;
use negpy_backend::features::stitch::{
    Affine, blend_ir, chain_affine, compute_canvas, scale_affine, scale_transforms,
    stitch_composite, warp_into_canvas,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

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

fn translated_config(first: (usize, usize), second: (usize, usize)) -> StitchConfig {
    StitchConfig {
        stitch_enabled: true,
        stitch_paths: vec!["/part1".to_string()],
        stitch_transforms: vec![
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            vec![1.0, 0.0, first.0 as f64 / 2.0, 0.0, 1.0, 0.0],
        ],
        stitch_canvas: [(first.0 + second.0 / 2) as i64, first.1 as i64],
        stitch_sizes: vec![
            [first.0 as i64, first.1 as i64],
            [second.0 as i64, second.1 as i64],
        ],
        ..StitchConfig::default()
    }
}

#[test]
fn a_translation_only_composite_keeps_each_part_sharp() {
    let first = ImageBuffer::from_rgb(
        40,
        20,
        (0..40 * 20 * 3).map(|i| i as f32 / 2400.0).collect(),
    );
    let second = ImageBuffer::from_rgb(40, 20, vec![0.5; 40 * 20 * 3]);
    let (out, ir) = stitch_composite(
        &[first, second],
        &[None, None],
        &translated_config((40, 20), (40, 20)),
    );
    assert_eq!(ir, None);
    assert_eq!((out.width, out.height), (60, 20));
    assert!(out.pixels.iter().all(|value| value.is_finite()));
}

#[test]
fn an_ir_blend_returns_none_until_every_part_has_a_plane() {
    let first = ImageBuffer::from_rgb(4, 2, vec![0.2; 4 * 2 * 3]);
    let second = ImageBuffer::from_rgb(4, 2, vec![0.8; 4 * 2 * 3]);
    let (_, ir) = stitch_composite(
        &[first, second],
        &[Some(vec![0.5; 8]), None],
        &translated_config((4, 2), (4, 2)),
    );
    assert_eq!(ir, None);
}

#[test]
fn a_composite_carries_the_blended_ir_plane() {
    let first = ImageBuffer::from_rgb(4, 2, vec![0.2; 4 * 2 * 3]);
    let second = ImageBuffer::from_rgb(4, 2, vec![0.8; 4 * 2 * 3]);
    let (_, ir) = stitch_composite(
        &[first, second],
        &[Some(vec![0.5; 8]), Some(vec![0.25; 8])],
        &translated_config((4, 2), (4, 2)),
    );
    assert!(ir.is_some_and(|plane| plane.iter().all(|value| value.is_finite())));
}

#[test]
fn a_seam_selects_one_plane_and_a_missing_seam_reads_coverage() {
    let transforms: [Affine; 2] = [
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [1.0, 0.0, 2.0, 0.0, 1.0, 0.0],
    ];
    let sizes = [(2, 2), (2, 2)];
    let irs = vec![vec![0.1; 4], vec![0.9; 4]];
    let seams = [vec![true; 8], vec![true; 8]];
    let blended = blend_ir(&irs, &sizes, &transforms, (4, 2), Some(&seams), None);
    assert!(blended.iter().all(|value| *value <= 0.9));
    let masks: Vec<Vec<bool>> = sizes.iter().map(|(w, h)| vec![true; w * h]).collect();
    let coverage = blend_ir(&irs, &sizes, &transforms, (4, 2), None, Some(&masks));
    assert!(coverage.iter().all(|value| value.is_finite()));
}

#[test]
fn a_warped_part_keeps_the_canvas_size() {
    let image = ImageBuffer::from_rgb(3, 2, vec![0.4; 3 * 2 * 3]);
    let (out, mask) = warp_into_canvas(&image, [1.0, 0.0, 1.0, 0.0, 1.0, 0.0], (5, 3));
    assert_eq!((out.width, out.height), (5, 3));
    assert_eq!(mask.len(), 15);
}
