//! Matrix/TRC transforms between two ICC profiles.
//!
//! Mirrors `negpy/infrastructure/display/icc_lut.py`: decode the source TRC, apply the
//! PCS matrix with a gamut clip, then encode through the destination TRC. The tables
//! are built once per profile pair and handed to the core kernels.

use negpy_core::display::{matrix_trc_u8 as core_trc_u8, matrix_trc_u16 as core_trc_u16};

use super::icc_profile::{
    extract_pcs_matrix, extract_trc_decode_samples, invert, is_matrix_trc_profile, multiply,
};

/// Encode table entries of the 8-bit transform, as the Python `_ENC_LUT_SIZE`.
pub const ENC_LUT_SIZE: usize = 65_536;
/// Encode table entries of the 16-bit transform, as `_ENC16_SIZE`.
pub const ENC16_SIZE: usize = 16_385;
/// Dense decode samples of the 16-bit transform, as `_DEC_DENSE`.
pub const DEC_DENSE: usize = 1 << 20;

/// The tables of one matrix/TRC transform: decode, matrix, encode.
#[derive(Clone, Debug, PartialEq)]
pub struct MatrixTrcTables {
    /// Three rows of equal length: the source TRC decode at each input code.
    pub decode: Vec<f32>,
    /// The row-major PCS matrix.
    pub matrix: [[f32; 3]; 3],
    /// Three rows of equal length: the destination TRC encode.
    pub encode: Vec<f32>,
}

impl MatrixTrcTables {
    /// The decode row length, one entry per input code.
    #[must_use]
    pub fn decode_len(&self) -> usize {
        self.decode.len() / 3
    }

    /// The encode row length.
    #[must_use]
    pub fn encode_len(&self) -> usize {
        self.encode.len() / 3
    }
}

/// `numpy.interp` over a nondecreasing `xs`: a repeated knot reads its last value.
#[must_use]
pub fn interp(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    if xs.is_empty() || ys.is_empty() {
        return 0.0;
    }
    let last = xs.len().min(ys.len()) - 1;
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[last] {
        return ys[last];
    }
    let mut low = 0usize;
    let mut high = last;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if xs[middle] <= x {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if xs[low] == x || low == last {
        return ys[low];
    }
    let span = xs[low + 1] - xs[low];
    if span <= 0.0 {
        return ys[low];
    }
    let t = (x - xs[low]) / span;
    ys[low] + (ys[low + 1] - ys[low]) * t
}

fn linspace(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![start];
    }
    let step = (stop - start) / (count - 1) as f64;
    (0..count)
        .map(|index| start + index as f64 * step)
        .collect()
}

/// The matrix and decode rows shared by both transforms, or `None` for a non-matrix pair.
fn matrix_and_decode(
    source: &[u8],
    dest: &[u8],
    codes: &[f64],
) -> Option<([[f32; 3]; 3], Vec<Vec<f64>>)> {
    if !(is_matrix_trc_profile(source) && is_matrix_trc_profile(dest)) {
        return None;
    }
    let source_matrix = extract_pcs_matrix(source)?;
    let dest_matrix = extract_pcs_matrix(dest)?;
    let decode = extract_trc_decode_samples(source, codes)?;
    let matrix = multiply(invert(dest_matrix)?, source_matrix);
    let matrix =
        std::array::from_fn(|row| std::array::from_fn(|column| matrix[row][column] as f32));
    Some((matrix, decode))
}

fn flatten_rows(rows: Vec<Vec<f64>>) -> Vec<f32> {
    rows.into_iter()
        .flatten()
        .map(|value| value.clamp(0.0, 1.0) as f32)
        .collect()
}

/// The tables of the 8-bit transform: a per-code decode through a 256-step source curve,
/// the PCS matrix, and a 65536-entry destination encode.
#[must_use]
pub fn tables_u8(source: &[u8], dest: &[u8]) -> Option<MatrixTrcTables> {
    let codes = linspace(0.0, 1.0, 256);
    let (matrix, decode) = matrix_and_decode(source, dest, &codes)?;
    let dense_dest = extract_trc_decode_samples(dest, &linspace(0.0, 1.0, ENC_LUT_SIZE))?;
    let grid = linspace(0.0, 1.0, ENC_LUT_SIZE);
    let encode: Vec<Vec<f64>> = dense_dest
        .iter()
        .map(|row| {
            let accumulated: Vec<f64> = row.iter().map(|value| value.clamp(0.0, 1.0)).collect();
            let mut rising = accumulated;
            for index in 1..rising.len() {
                rising[index] = rising[index].max(rising[index - 1]);
            }
            grid.iter().map(|x| interp(*x, &rising, &grid)).collect()
        })
        .collect();
    Some(MatrixTrcTables {
        decode: flatten_rows(decode),
        matrix,
        encode: flatten_rows(encode),
    })
}

/// The tables of the 16-bit transform: an exact per-code decode, the PCS matrix, and a
/// 16385-entry destination encode sampled at linear `t^2`.
#[must_use]
pub fn tables_u16(source: &[u8], dest: &[u8]) -> Option<MatrixTrcTables> {
    let codes: Vec<f64> = (0..65_536).map(|code| code as f64 / 65_535.0).collect();
    let (matrix, decode) = matrix_and_decode(source, dest, &codes)?;
    let dense_grid = linspace(0.0, 1.0, DEC_DENSE);
    let dense_dest = extract_trc_decode_samples(dest, &dense_grid)?;
    let t = linspace(0.0, 1.0, ENC16_SIZE);
    let encode: Vec<Vec<f64>> = dense_dest
        .iter()
        .map(|row| {
            let mut rising: Vec<f64> = row.iter().map(|value| value.clamp(0.0, 1.0)).collect();
            for index in 1..rising.len() {
                rising[index] = rising[index].max(rising[index - 1]);
            }
            t.iter()
                .map(|value| interp(value * value, &rising, &dense_grid))
                .collect()
        })
        .collect();
    Some(MatrixTrcTables {
        decode: flatten_rows(decode),
        matrix,
        encode: flatten_rows(encode),
    })
}

/// `apply_matrix_trc_u8`: an 8-bit RGB image through the pair's tables.
#[must_use]
pub fn apply_matrix_trc_u8(img: &[u8], tables: &MatrixTrcTables) -> Option<Vec<u8>> {
    if img.len() % 3 != 0
        || tables.decode_len() != 256
        || tables.encode_len() < 2
        || tables.encode.len() != tables.encode_len() * 3
    {
        return None;
    }
    let mut out = vec![0u8; img.len()];
    core_trc_u8(img, &tables.decode, tables.matrix, &tables.encode, &mut out);
    Some(out)
}

/// `apply_matrix_trc_u16`: a 16-bit RGB image through the pair's tables.
#[must_use]
pub fn apply_matrix_trc_u16(img: &[u16], tables: &MatrixTrcTables) -> Option<Vec<u16>> {
    if img.len() % 3 != 0
        || tables.decode_len() != 65_536
        || tables.encode_len() < 2
        || tables.encode.len() != tables.encode_len() * 3
    {
        return None;
    }
    let mut out = vec![0u16; img.len()];
    core_trc_u16(img, &tables.decode, tables.matrix, &tables.encode, &mut out);
    Some(out)
}

/// Build the pair's tables and transform an 8-bit image. `None` when either profile is
/// not matrix/TRC, so the caller falls back to the 3D LUT path.
#[must_use]
pub fn apply_profile_pair_u8(img: &[u8], source: &[u8], dest: &[u8]) -> Option<Vec<u8>> {
    apply_matrix_trc_u8(img, &tables_u8(source, dest)?)
}

/// The 16-bit twin of [`apply_profile_pair_u8`].
#[must_use]
pub fn apply_profile_pair_u16(img: &[u16], source: &[u8], dest: &[u8]) -> Option<Vec<u16>> {
    apply_matrix_trc_u16(img, &tables_u16(source, dest)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read(root.join("icc").join(name)).expect("bundled profile")
    }

    #[test]
    fn the_interpolation_reads_the_last_of_a_repeated_knot() {
        let xs = [0.0, 0.5, 0.5, 0.7, 1.0];
        let ys = [0.0, 0.1, 0.2, 0.3, 1.0];
        assert!((interp(0.0, &xs, &ys) - 0.0).abs() < 1e-12);
        assert!((interp(0.25, &xs, &ys) - 0.05).abs() < 1e-12);
        assert!((interp(0.5, &xs, &ys) - 0.2).abs() < 1e-12);
        assert!((interp(0.9, &xs, &ys) - 0.766_666_666_666_666_8).abs() < 1e-12);
        assert!((interp(2.0, &xs, &ys) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn an_identity_pair_builds_the_two_table_shapes() {
        let icc = profile("sRGB-v4.icc");
        let tables = tables_u8(&icc, &icc).expect("matrix/TRC pair");
        assert_eq!(tables.decode_len(), 256);
        assert_eq!(tables.encode_len(), ENC_LUT_SIZE);
        let dense = tables_u16(&icc, &icc).expect("matrix/TRC pair");
        assert_eq!(dense.decode_len(), 65_536);
        assert_eq!(dense.encode_len(), ENC16_SIZE);
    }

    #[test]
    fn the_identity_transform_keeps_an_eight_bit_code() {
        let icc = profile("sRGB-v4.icc");
        let img = [0u8, 0, 0, 255, 255, 255, 128, 64, 192, 17, 200, 99];
        let out = apply_profile_pair_u8(&img, &icc, &icc).expect("transform");
        for (before, after) in img.iter().zip(&out) {
            assert!(before.abs_diff(*after) <= 1, "{before} became {after}");
        }
    }

    #[test]
    fn the_identity_transform_keeps_a_sixteen_bit_code() {
        let icc = profile("sRGB-v4.icc");
        let img = [0u16, 0, 0, 65535, 65535, 65535, 32768, 16384, 49152];
        let out = apply_profile_pair_u16(&img, &icc, &icc).expect("transform");
        for (before, after) in img.iter().zip(&out) {
            assert!(before.abs_diff(*after) <= 300, "{before} became {after}");
        }
    }

    #[test]
    fn a_non_matrix_profile_does_not_take_the_matrix_path() {
        let srgb = profile("sRGB-v4.icc");
        let gray = profile("GrayGamma2.2.icc");
        assert!(tables_u8(&gray, &srgb).is_none());
        assert!(tables_u8(b"not an icc file", &srgb).is_none());
    }
}
