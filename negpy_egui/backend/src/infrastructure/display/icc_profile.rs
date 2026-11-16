//! Low-level ICC profile parsing for matrix/TRC detection and primaries extraction.
//!
//! Mirrors `negpy/infrastructure/display/icc_profile.py`. The PCS is D50-relative,
//! so colorants go through the profile's `chad` when present and a Bradford fallback
//! otherwise, landing on the D65 the working space uses.

use std::collections::BTreeMap;

const D50_XYZ: [f64; 3] = [0.9642, 1.0, 0.8249];
const D65_XYZ: [f64; 3] = [0.95047, 1.0, 1.08883];
const BRADFORD_CONE: [[f64; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];

/// One tag's offset and size in the profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagEntry {
    pub offset: usize,
    pub size: usize,
}

/// The profile's tag table, keyed by its four-byte signature.
#[must_use]
pub fn tag_table(data: &[u8]) -> BTreeMap<[u8; 4], TagEntry> {
    let mut tags = BTreeMap::new();
    if data.len() < 132 {
        return tags;
    }
    let count = u32::from_be_bytes(data[128..132].try_into().unwrap_or_default()) as usize;
    for index in 0..count {
        let base = 132 + index * 12;
        if base + 12 > data.len() {
            break;
        }
        let signature: [u8; 4] = data[base..base + 4].try_into().unwrap_or_default();
        let offset =
            u32::from_be_bytes(data[base + 4..base + 8].try_into().unwrap_or_default()) as usize;
        let size =
            u32::from_be_bytes(data[base + 8..base + 12].try_into().unwrap_or_default()) as usize;
        tags.insert(signature, TagEntry { offset, size });
    }
    tags
}

fn signed_fixed(data: &[u8], offset: usize) -> Option<f64> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(i32::from_be_bytes(bytes) as f64 / 65536.0)
}

/// An XYZType tag as its three components.
#[must_use]
pub fn read_xyz_tag(data: &[u8], entry: TagEntry) -> Option<[f64; 3]> {
    if entry.size < 20 {
        return None;
    }
    Some([
        signed_fixed(data, entry.offset + 8)?,
        signed_fixed(data, entry.offset + 12)?,
        signed_fixed(data, entry.offset + 16)?,
    ])
}

/// A chromaticAdaptationTag as a 3x3 matrix.
#[must_use]
pub fn read_chad_tag(data: &[u8], entry: TagEntry) -> Option<[[f64; 3]; 3]> {
    if entry.size < 8 + 9 * 4 {
        return None;
    }
    Some(std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            signed_fixed(data, entry.offset + 8 + (row * 3 + column) * 4).unwrap_or(0.0)
        })
    }))
}

/// True for a matrix/TRC profile: colorants and a TRC, but no LUT tags.
#[must_use]
pub fn is_matrix_trc_profile(data: &[u8]) -> bool {
    let tags = tag_table(data);
    let has = |signature: &[u8; 4]| tags.contains_key(signature);
    let has_colorants = [b"rXYZ", b"gXYZ", b"bXYZ"].iter().all(|sig| has(sig));
    let has_trc = [b"rTRC", b"gTRC", b"bTRC"].iter().any(|sig| has(sig));
    let has_lut = [b"A2B0", b"B2A0"].iter().any(|sig| has(sig));
    has_colorants && has_trc && !has_lut
}

/// The PCS(D50)-relative RGB to XYZ matrix, unadapted.
#[must_use]
pub fn extract_pcs_matrix(data: &[u8]) -> Option<[[f64; 3]; 3]> {
    let tags = tag_table(data);
    let mut columns = Vec::with_capacity(3);
    for signature in [b"rXYZ", b"gXYZ", b"bXYZ"] {
        columns.push(read_xyz_tag(data, *tags.get(signature)?)?);
    }
    Some(std::array::from_fn(|row| {
        std::array::from_fn(|column| columns[column][row])
    }))
}

/// One TRC tag's decode at the encoded points `x`.
#[must_use]
pub fn decode_trc_at(data: &[u8], entry: TagEntry, x: &[f64]) -> Option<Vec<f64>> {
    let offset = entry.offset;
    if entry.size < 12 {
        return None;
    }
    match data.get(offset..offset + 4)? {
        b"curv" => {
            let count =
                u32::from_be_bytes(data.get(offset + 8..offset + 12)?.try_into().ok()?) as usize;
            if count == 0 {
                return Some(x.to_vec());
            }
            if count == 1 {
                let gamma = u16::from_be_bytes(data.get(offset + 12..offset + 14)?.try_into().ok()?)
                    as f64
                    / 256.0;
                return Some(x.iter().map(|value| value.powf(gamma)).collect());
            }
            if offset + 12 + count * 2 > data.len() {
                return None;
            }
            let values: Vec<f64> = (0..count)
                .map(|index| {
                    let start = offset + 12 + index * 2;
                    u16::from_be_bytes(data[start..start + 2].try_into().unwrap_or_default()) as f64
                        / 65535.0
                })
                .collect();
            Some(x.iter().map(|value| interp(*value, &values)).collect())
        }
        b"para" => {
            let kind = u16::from_be_bytes(data.get(offset + 8..offset + 10)?.try_into().ok()?);
            let count = match kind {
                0 => 1,
                1 => 3,
                2 => 4,
                3 => 5,
                4 => 7,
                _ => return None,
            };
            if offset + 12 + count * 4 > data.len() {
                return None;
            }
            let params: Vec<f64> = (0..count)
                .map(|index| signed_fixed(data, offset + 12 + index * 4).unwrap_or(0.0))
                .collect();
            Some(x.iter().map(|value| para(kind, &params, *value)).collect())
        }
        _ => None,
    }
}

fn interp(x: f64, values: &[f64]) -> f64 {
    if values.is_empty() {
        return x;
    }
    let position = x.clamp(0.0, 1.0) * (values.len() - 1) as f64;
    let low = position.floor() as usize;
    let high = position.ceil() as usize;
    let fraction = position - low as f64;
    values[low] + (values[high] - values[low]) * fraction
}

fn para(kind: u16, p: &[f64], x: f64) -> f64 {
    match kind {
        0 => x.powf(p[0]),
        1 => {
            let (g, a, b) = (p[0], p[1], p[2]);
            if x >= -b / a {
                ((a * x + b).max(0.0)).powf(g)
            } else {
                0.0
            }
        }
        2 => {
            let (g, a, b, c) = (p[0], p[1], p[2], p[3]);
            if x >= -b / a {
                ((a * x + b).max(0.0)).powf(g) + c
            } else {
                c
            }
        }
        3 => {
            let (g, a, b, c, d) = (p[0], p[1], p[2], p[3], p[4]);
            if x >= d {
                ((a * x + b).max(0.0)).powf(g)
            } else {
                c * x
            }
        }
        _ => {
            let (g, a, b, c, d, e, f) = (p[0], p[1], p[2], p[3], p[4], p[5], p[6]);
            if x >= d {
                ((a * x + b).max(0.0)).powf(g) + e
            } else {
                c * x + f
            }
        }
    }
}

/// (3, len(x)) TRC decodes, or None when a tag is missing or unsupported.
#[must_use]
pub fn extract_trc_decode_samples(data: &[u8], x: &[f64]) -> Option<Vec<Vec<f64>>> {
    let tags = tag_table(data);
    let mut rows = Vec::with_capacity(3);
    for signature in [b"rTRC", b"gTRC", b"bTRC"] {
        rows.push(decode_trc_at(data, *tags.get(signature)?, x)?);
    }
    Some(rows)
}

/// The D65-referenced RGB to XYZ primaries from the colorant tags.
#[must_use]
pub fn extract_primaries_matrix(data: &[u8]) -> Option<[[f64; 3]; 3]> {
    let tags = tag_table(data);
    let mut columns = Vec::with_capacity(3);
    for signature in [b"rXYZ", b"gXYZ", b"bXYZ"] {
        columns.push(read_xyz_tag(data, *tags.get(signature)?)?);
    }
    let pcs: [[f64; 3]; 3] =
        std::array::from_fn(|row| std::array::from_fn(|column| columns[column][row]));
    let mut native = pcs;
    let mut native_white = D50_XYZ;
    if let Some(entry) = tags.get(b"chad") {
        if let Some(chad) = read_chad_tag(data, *entry) {
            if let Some(inverse) = invert_3x3(chad) {
                native = mul(inverse, pcs);
                native_white = mul_vec(inverse, D50_XYZ);
            }
        }
    }
    Some(mul(bradford(native_white, D65_XYZ), native))
}

fn bradford(source: [f64; 3], destination: [f64; 3]) -> [[f64; 3]; 3] {
    let lms_source = mul_vec(BRADFORD_CONE, source);
    let lms_destination = mul_vec(BRADFORD_CONE, destination);
    let scale: [f64; 3] = std::array::from_fn(|index| lms_destination[index] / lms_source[index]);
    let diagonal: [[f64; 3]; 3] = std::array::from_fn(|row| {
        std::array::from_fn(|column| if row == column { scale[row] } else { 0.0 })
    });
    mul(
        invert_3x3(BRADFORD_CONE).unwrap_or(BRADFORD_CONE),
        mul(diagonal, BRADFORD_CONE),
    )
}

fn mul(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum()
        })
    })
}

fn mul_vec(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| {
        (0..3)
            .map(|column| matrix[row][column] * vector[column])
            .sum()
    })
}

fn invert_3x3(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let determinant = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);
    if determinant.abs() < 1e-18 {
        return None;
    }
    let inverse = 1.0 / determinant;
    Some([
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inverse,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inverse,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inverse,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inverse,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inverse,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inverse,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inverse,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inverse,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inverse,
        ],
    ])
}
