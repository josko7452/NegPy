//! Capture colorimetry: the camera's own color matrix.

use crate::domain::color::{WORKING_TO_XYZ, XYZ_TO_WORKING};

const MATRIX_EPSILON: f64 = 1e-9;

fn determinant(matrix: [[f64; 3]; 3]) -> f64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn inverse(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = determinant(matrix);
    if !det.is_finite() || det.abs() < MATRIX_EPSILON {
        return None;
    }
    let scale = 1.0 / det;
    Some([
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * scale,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * scale,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * scale,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * scale,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * scale,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * scale,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * scale,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * scale,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * scale,
        ],
    ])
}

fn multiply(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum()
        })
    })
}

/// Working-from-camera primaries, without the white-balance fold.
#[must_use]
pub fn primaries(cam_xyz: Option<[[f64; 3]; 3]>) -> Option<[[f64; 3]; 3]> {
    let camera = cam_xyz?;
    if !camera.iter().flatten().all(|value| value.is_finite()) {
        return None;
    }
    if determinant(camera).abs() < MATRIX_EPSILON {
        return None;
    }
    let working_to_xyz = inverse(XYZ_TO_WORKING.map(|row| row.map(f64::from)))?;
    let forward = multiply(camera, working_to_xyz);
    let mut normalized = forward;
    for row in &mut normalized {
        let sum = row.iter().sum::<f64>();
        if !sum.is_finite() || sum.abs() < MATRIX_EPSILON {
            return None;
        }
        for value in row {
            *value /= sum;
        }
    }
    inverse(normalized)
}

/// The as-shot multipliers normalized to green.
#[must_use]
pub fn wb_gains(camera_wb: Option<&[f64]>) -> Option<[f64; 3]> {
    let wb = camera_wb?;
    if wb.len() < 3 {
        return None;
    }
    let [red, green, blue] = [wb[0], wb[1], wb[2]];
    if ![red, green, blue].iter().all(|value| value.is_finite())
        || red.min(green).min(blue) <= 0.0
        || green == 0.0
    {
        return None;
    }
    Some([red / green, 1.0, blue / green])
}

/// Working-space-from-camera 3x3 for a camera's XYZ-to-camera matrix.
#[must_use]
pub fn camera_to_working_matrix(
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
) -> Option<[[f64; 3]; 3]> {
    let primaries = primaries(cam_xyz)?;
    let Some(gains) = wb_gains(camera_wb) else {
        return Some(primaries);
    };
    let mut out = primaries;
    for row in &mut out {
        for (value, gain) in row.iter_mut().zip(gains) {
            *value *= gain;
        }
    }
    Some(out)
}

/// The gains and primaries split at the white-balance fold.
#[must_use]
pub fn camera_matrix_parts(
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
) -> (Option<[f64; 3]>, Option<[[f64; 3]; 3]>) {
    let Some(primaries) = primaries(cam_xyz) else {
        return (None, None);
    };
    (wb_gains(camera_wb), Some(primaries))
}

/// `_WORKING_TO_XYZ` in the matrix type both paths share.
#[must_use]
pub fn working_to_xyz() -> [[f64; 3]; 3] {
    WORKING_TO_XYZ.map(|row| row.map(f64::from))
}
