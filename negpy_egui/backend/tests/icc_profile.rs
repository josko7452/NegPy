//! Matrix/TRC detection, D65 primaries and TRC decoding.

use negpy_backend::infrastructure::display::icc_profile::{
    extract_pcs_matrix, extract_primaries_matrix, extract_trc_decode_samples,
    is_matrix_trc_profile, tag_table,
};

fn fixed(value: f64) -> [u8; 4] {
    ((value * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(x: f64, y: f64, z: f64) -> Vec<u8> {
    let mut out = b"XYZ \0\0\0\0".to_vec();
    out.extend_from_slice(&fixed(x));
    out.extend_from_slice(&fixed(y));
    out.extend_from_slice(&fixed(z));
    out
}

fn trc_gamma(gamma: f64) -> Vec<u8> {
    let mut out = b"curv\0\0\0\0".to_vec();
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&((gamma * 256.0).round() as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

fn chad_tag(matrix: [[f64; 3]; 3]) -> Vec<u8> {
    let mut out = b"sf32\0\0\0\0".to_vec();
    for row in matrix {
        for value in row {
            out.extend_from_slice(&fixed(value));
        }
    }
    out
}

fn build_icc(
    r: (f64, f64, f64),
    g: (f64, f64, f64),
    b: (f64, f64, f64),
    gamma: f64,
    chad: Option<[[f64; 3]; 3]>,
    add_a2b0: bool,
) -> Vec<u8> {
    let mut tags: Vec<(&[u8], Vec<u8>)> = vec![
        (b"rXYZ", xyz_tag(r.0, r.1, r.2)),
        (b"gXYZ", xyz_tag(g.0, g.1, g.2)),
        (b"bXYZ", xyz_tag(b.0, b.1, b.2)),
        (b"wtpt", xyz_tag(0.9642, 1.0, 0.8249)),
        (b"rTRC", trc_gamma(gamma)),
        (b"gTRC", trc_gamma(gamma)),
        (b"bTRC", trc_gamma(gamma)),
    ];
    if let Some(matrix) = chad {
        tags.push((b"chad", chad_tag(matrix)));
    }
    if add_a2b0 {
        tags.push((
            b"A2B0",
            b"mft2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"
                .to_vec(),
        ));
    }
    let table_size = tags.len() * 12;
    let mut offset = 128 + 4 + table_size;
    let mut entries = Vec::new();
    for (_, payload) in &tags {
        entries.push((offset, payload.len()));
        offset += (payload.len() + 3) / 4 * 4;
    }
    let mut header = vec![0u8; 128];
    header[0..4].copy_from_slice(&(offset as u32).to_be_bytes());
    header[12..16].copy_from_slice(b"mntr");
    header[16..20].copy_from_slice(b"RGB ");
    header[36..40].copy_from_slice(b"acsp");
    header[40..44].copy_from_slice(b"APPL");
    let mut data = header;
    data.extend_from_slice(&(tags.len() as u32).to_be_bytes());
    for ((signature, _), (tag_offset, size)) in tags.iter().zip(&entries) {
        data.extend_from_slice(signature);
        data.extend_from_slice(&(*tag_offset as u32).to_be_bytes());
        data.extend_from_slice(&(*size as u32).to_be_bytes());
    }
    for (_, payload) in &tags {
        data.extend_from_slice(payload);
        while data.len() % 4 != 0 {
            data.push(0);
        }
    }
    data
}

const R: (f64, f64, f64) = (0.43603516, 0.22248840, 0.01391602);
const G: (f64, f64, f64) = (0.38511658, 0.71690369, 0.09706116);
const B: (f64, f64, f64) = (0.14305115, 0.06060791, 0.71392822);
const SRGB_CHAD: [[f64; 3]; 3] = [
    [1.04788208, 0.0229187, -0.05021667],
    [0.02958679, 0.99047852, -0.01707458],
    [-0.00924683, 0.01507568, 0.75167847],
];

fn assert_close(actual: [[f64; 3]; 3], expected: [[f64; 3]; 3], tolerance: f64) {
    for row in 0..3 {
        for column in 0..3 {
            assert!(
                (actual[row][column] - expected[row][column]).abs() <= tolerance,
                "{actual:?} != {expected:?}"
            );
        }
    }
}

#[test]
fn matrix_trc_profiles_are_recognized_and_lut_profiles_are_not() {
    assert!(is_matrix_trc_profile(&build_icc(R, G, B, 2.2, None, false)));
    assert!(!is_matrix_trc_profile(&build_icc(R, G, B, 2.2, None, true)));
    let gray = b"not a profile";
    assert!(!is_matrix_trc_profile(gray));
    assert!(extract_primaries_matrix(gray).is_none());
}

#[test]
fn the_pcs_matrix_keeps_the_tag_values() {
    let matrix = extract_pcs_matrix(&build_icc(R, G, B, 2.2, None, false)).unwrap();
    assert!((matrix[0][0] - R.0).abs() < 1e-5);
    assert!((matrix[1][0] - R.1).abs() < 1e-5);
    assert!((matrix[2][2] - B.2).abs() < 1e-5);
}

#[test]
fn chad_recovers_the_d65_native_primaries() {
    let icc = build_icc(R, G, B, 2.2, Some(SRGB_CHAD), false);
    let expected = [
        [0.4124564, 0.3575761, 0.1804375],
        [0.2126729, 0.7151522, 0.0721750],
        [0.0193339, 0.1191920, 0.9503041],
    ];
    assert_close(extract_primaries_matrix(&icc).unwrap(), expected, 5e-4);
}

#[test]
fn without_chad_the_bradford_fallback_still_lands_on_d65() {
    let matrix = extract_primaries_matrix(&build_icc(R, G, B, 2.2, None, false)).unwrap();
    let white = [
        matrix[0][0] + matrix[0][1] + matrix[0][2],
        matrix[1][0] + matrix[1][1] + matrix[1][2],
        matrix[2][0] + matrix[2][1] + matrix[2][2],
    ];
    let expected = [0.95047, 1.0, 1.08883];
    for index in 0..3 {
        assert!((white[index] - expected[index]).abs() < 2e-3);
    }
}

#[test]
fn the_trc_gamma_is_decoded_at_each_sample() {
    let icc = build_icc(R, G, B, 2.2, None, false);
    let samples = extract_trc_decode_samples(&icc, &[0.0, 0.25, 0.5, 1.0]).unwrap();
    assert_eq!(samples.len(), 3);
    for row in samples {
        assert!((row[0] - 0.0).abs() < 1e-9);
        let gamma = (2.2f64 * 256.0).round() / 256.0;
        assert!((row[1] - 0.25f64.powf(gamma)).abs() < 1e-9);
        assert!((row[3] - 1.0).abs() < 1e-9);
    }
}

#[test]
fn tags_are_read_by_signature() {
    let icc = build_icc(R, G, B, 2.2, None, false);
    let tags = tag_table(&icc);
    assert!(tags.contains_key(b"rXYZ"));
    assert!(tags.contains_key(b"chad") == false);
}
