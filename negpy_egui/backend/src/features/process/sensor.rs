//! Sensor crosstalk calibration and correction identity.

use crate::domain::models::{ProcessConfig, ProcessMode, SensorUnmix};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

const EPS: f64 = 1e-4;
const MAX_CONDITION: f64 = 1e3;

/// The mean linear RGB over the central `center` fraction.
#[must_use]
pub fn measure_capture(img: &ImageBuffer, center: f64) -> [f64; 3] {
    let margin = (1.0 - center) / 2.0;
    let x0 = (img.width as f64 * margin) as usize;
    let x1 = img.width.saturating_sub(x0);
    let y0 = (img.height as f64 * margin) as usize;
    let y1 = img.height.saturating_sub(y0);
    let mut sum = [0.0; 3];
    let mut count = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let at = (y * img.width + x) * 3;
            for (channel, value) in sum.iter_mut().enumerate() {
                *value += img.pixels[at + channel] as f64;
            }
            count += 1;
        }
    }
    if count == 0 {
        return [0.0; 3];
    }
    sum.map(|value| value / count as f64)
}

/// Correction matrix from three bare-light capture means.
pub fn build_sensor_matrix(
    red: [f64; 3],
    green: [f64; 3],
    blue: [f64; 3],
) -> Result<[f64; 9], String> {
    let columns = [red, green, blue];
    let diagonal = [red[0], green[1], blue[2]];
    let maxima: [f64; 3] = std::array::from_fn(|column| {
        (0..3)
            .map(|row| columns[column][row])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    if (0..3).any(|index| diagonal[index] <= EPS * maxima[index]) {
        return Err(
            "a capture has no signal in its own channel — check the R/G/B assignment".into(),
        );
    }
    let normalized: [[f64; 3]; 3] = std::array::from_fn(|row| {
        std::array::from_fn(|column| columns[column][row] / diagonal[column])
    });
    let inverse = invert_3x3(normalized).ok_or_else(|| {
        "captures are not independent — three distinct single-band exposures are required"
            .to_string()
    })?;
    let condition = condition_estimate(normalized);
    if !condition.is_finite() || condition > MAX_CONDITION {
        return Err(
            "captures are not independent — three distinct single-band exposures are required"
                .to_string(),
        );
    }
    let mut flat = [0.0; 9];
    for row in 0..3 {
        for column in 0..3 {
            flat[row * 3 + column] = inverse[row][column];
        }
    }
    Ok(flat)
}

fn determinant(matrix: [[f64; 3]; 3]) -> f64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn invert_3x3(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = determinant(matrix);
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    Some([
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inv,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inv,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inv,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inv,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inv,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inv,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inv,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inv,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inv,
        ],
    ])
}

fn condition_estimate(matrix: [[f64; 3]; 3]) -> f64 {
    let frobenius = matrix
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    let Some(inverse) = invert_3x3(matrix) else {
        return f64::INFINITY;
    };
    let inverse_frobenius = inverse
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    frobenius * inverse_frobenius
}

/// `unmix_block_reason`: why the unmix cannot apply.
#[must_use]
pub fn unmix_block_reason(process: &ProcessConfig) -> &'static str {
    if process.process_mode == ProcessMode::E6 {
        "transparency"
    } else if !process.linear_raw {
        "linear_raw"
    } else {
        ""
    }
}

/// Whether the unmix can apply at all.
#[must_use]
pub fn sensor_unmix_available(process: &ProcessConfig) -> bool {
    unmix_block_reason(process).is_empty()
}

/// The baked matrix, or `None` when the basis makes it invalid to apply.
#[must_use]
pub fn effective_sensor_matrix(process: &ProcessConfig) -> Option<&[f64]> {
    sensor_unmix_available(process)
        .then_some(process.sensor_matrix.as_deref()?)
        .or(None)
}

/// Identity of the baked sensor matrix, folded into the render source hash.
#[must_use]
pub fn sensor_token(process: &ProcessConfig) -> String {
    let Some(matrix) = effective_sensor_matrix(process) else {
        return String::new();
    };
    let values: Vec<String> = matrix
        .iter()
        .map(|value| {
            let text = format!("{value:.6}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        })
        .collect();
    let mut token = format!("|sn:{}", values.join(","));
    if process.sensor_unmix != SensorUnmix::Linear {
        token.push_str(&format!("|{}", process.sensor_unmix.as_str()));
    }
    token
}

/// Apply a baked 3x3 unmix to a linear capture.
#[must_use]
pub fn apply_sensor_correction(
    img: &ImageBuffer,
    matrix: Option<&[f64]>,
    mode: SensorUnmix,
) -> ImageBuffer {
    let Some(matrix) = matrix.filter(|matrix| matrix.len() == 9) else {
        return img.clone();
    };
    let m: [[f32; 3]; 3] =
        std::array::from_fn(|row| std::array::from_fn(|column| matrix[row * 3 + column] as f32));
    let mut out = vec![0.0; img.pixels.len()];
    match mode {
        SensorUnmix::Linear => negpy_core::sensor::unmix(&img.pixels, &mut out, m),
        SensorUnmix::Density | SensorUnmix::TwoScale => {
            negpy_core::sensor::unmix(&img.pixels, &mut out, m);
        }
    }
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// The sensor matrix linearised in log space around `base`.
#[must_use]
pub fn density_unmix_matrix(matrix: &[f64], base: [f64; 3]) -> [f64; 9] {
    let m: [[f64; 3]; 3] =
        std::array::from_fn(|row| std::array::from_fn(|column| matrix[row * 3 + column]));
    let at_base: [f64; 3] =
        std::array::from_fn(|row| m[row][0] * base[0] + m[row][1] * base[1] + m[row][2] * base[2]);
    let mut out = [0.0; 9];
    for row in 0..3 {
        for column in 0..3 {
            out[row * 3 + column] = m[row][column] * base[column] / at_base[row];
        }
    }
    out
}
