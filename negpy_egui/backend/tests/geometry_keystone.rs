//! Ports the CPU keystone inverse and edge-solver cases of tests/test_keystone.py.

use negpy_backend::features::geometry::logic::{
    keystone_inverse_normalized, keystone_matrix, keystone_quad,
};
use negpy_backend::features::geometry::mapping::map_point_keystone;
use negpy_backend::features::geometry::skew::{
    EdgeSet, KeystoneSolveShape, solve_keystone_from_edges,
};

fn multiply(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum()
        })
    })
}

#[test]
fn zero_keystone_inverse_is_identity() {
    assert_eq!(
        keystone_inverse_normalized(0.0, 0.0),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
}

#[test]
fn the_shader_inverse_undoes_the_forward_matrix() {
    let width = 144;
    let height = 96;
    let forward = keystone_matrix(9.0, -4.0, width, height);
    let inverse_normalized = keystone_inverse_normalized(9.0, -4.0);
    let to_index = [
        [width as f64, 0.0, -0.5],
        [0.0, height as f64, -0.5],
        [0.0, 0.0, 1.0],
    ];
    let from_index = [
        [1.0 / width as f64, 0.0, 0.5 / width as f64],
        [0.0, 1.0 / height as f64, 0.5 / height as f64],
        [0.0, 0.0, 1.0],
    ];
    let inverse_index = multiply(multiply(to_index, inverse_normalized), from_index);
    let product = multiply(inverse_index, forward);
    let scale = product[2][2];
    for row in 0..3 {
        for column in 0..3 {
            let expected = if row == column { 1.0 } else { 0.0 };
            assert!(
                (product[row][column] / scale - expected).abs() < 1e-8,
                "{row},{column}: {}",
                product[row][column] / scale
            );
        }
    }
}

#[test]
fn the_edge_solver_recovers_tilt_and_swing() {
    let height = 300;
    let width = 400;
    let (true_v, true_h) = (8.0, -6.0);
    let quad = keystone_quad(true_v, true_h);
    let points: Vec<(f64, f64)> = quad
        .iter()
        .map(|point| {
            (
                f64::from(point[0]) * width as f64 / width as f64,
                f64::from(point[1]) * height as f64 / height as f64,
            )
        })
        .collect();
    let normalized = |index: usize| (points[index].0, points[index].1);
    let edges = EdgeSet {
        left: Some((normalized(0), normalized(3))),
        right: Some((normalized(1), normalized(2))),
        top: Some((normalized(0), normalized(1))),
        bottom: Some((normalized(3), normalized(2))),
    };

    let (solved_v, solved_h) = solve_keystone_from_edges(
        &edges,
        KeystoneSolveShape {
            orig_shape: (height, width),
            ..KeystoneSolveShape::default()
        },
        0.0,
        0.0,
    )
    .unwrap();

    assert!((solved_v - true_v).abs() < 1e-2, "{solved_v}");
    assert!((solved_h - true_h).abs() < 1e-2, "{solved_h}");
}

#[test]
fn a_solve_with_a_missing_edge_is_rejected() {
    let edges = EdgeSet {
        left: Some(((0.0, 0.0), (0.0, 1.0))),
        right: Some(((1.0, 0.0), (1.0, 1.0))),
        ..EdgeSet::default()
    };
    assert!(solve_keystone_from_edges(&edges, KeystoneSolveShape::default(), 0.0, 0.0).is_err());
}

#[test]
fn a_projective_point_stays_finite_away_from_the_canvas() {
    let mapped = map_point_keystone(-20.0, 320.0, 10.0, -6.0, 400, 300);
    assert!(mapped.0.is_finite() && mapped.1.is_finite());
}
