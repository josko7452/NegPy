//! Ports the DNG opcode, Sony tag, Panasonic word and bind cases in
//! `tests/test_metadata_lens.py`.

use std::path::PathBuf;

use negpy_backend::domain::models::{LensMetadata, LensWarp};
use negpy_backend::infrastructure::loaders::lens_metadata::{
    bind_decode, parse_opcodes, read_lens_metadata,
};

fn identity() -> [f64; 6] {
    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0]
}

fn opcode(coefficients: &[[f64; 6]], center: (f64, f64), code: u32, flags: u32) -> Vec<u8> {
    let mut values = Vec::new();
    for plane in coefficients {
        values.extend_from_slice(plane);
    }
    values.push(center.0);
    values.push(center.1);
    let mut payload = Vec::new();
    payload.extend_from_slice(&(coefficients.len() as u32).to_be_bytes());
    for value in values {
        payload.extend_from_slice(&value.to_be_bytes());
    }
    let mut out = Vec::new();
    for field in [1, code, 0x0103_0000, flags, payload.len() as u32] {
        out.extend_from_slice(&field.to_be_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

fn dng_bytes(path: &std::path::Path, data: &[u8]) {
    // Minimal little-endian TIFF: one entry for OpcodeList3, one RAW plane.
    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    let entries: [(u16, u16, u32, u32); 4] = [
        (256, 3, 1, 60),
        (257, 3, 1, 40),
        (262, 3, 1, 32_803),
        (51022, 7, data.len() as u32, 0),
    ];
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, kind, count, value) in entries {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        if tag == 51022 {
            let at = 8 + 2 + entries.len() * 12 + 4;
            out.extend_from_slice(&(at as u32).to_le_bytes());
        } else {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    std::fs::write(path, out).unwrap();
}

#[test]
fn dng_identity_is_empty_and_capabilities_are_separate() {
    assert!(
        parse_opcodes(&opcode(&[identity()], (0.5, 0.5), 1, 0))
            .unwrap()
            .is_empty()
    );
    let ca = [
        [1.002, 0.0, 0.0, 0.0, 0.0, 0.0],
        identity(),
        [0.998, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let lens = LensMetadata {
        warps: parse_opcodes(&opcode(&ca, (0.5, 0.5), 1, 0)).unwrap(),
        ..LensMetadata::default()
    };
    assert!(lens.available() && lens.ca() && !lens.distortion());
    let distortion = LensMetadata {
        warps: parse_opcodes(&opcode(
            &[[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
            (0.5, 0.5),
            1,
            0,
        ))
        .unwrap(),
        ..LensMetadata::default()
    };
    assert!(distortion.distortion() && !distortion.ca());
}

#[test]
fn malformed_opcode_lists_are_rejected() {
    assert!(parse_opcodes(&[0, 0, 0, 1]).is_err());
    assert!(parse_opcodes(&opcode(&[identity()], (0.5, 0.5), 2, 0)).is_err());
    assert!(parse_opcodes(&opcode(&[identity()], (0.5, 0.5), 1, 4)).is_err());
    assert!(parse_opcodes(&opcode(&[identity()], (1.5, 0.5), 1, 0)).is_err());
    assert!(parse_opcodes(&[]).is_err());
}

#[test]
fn a_dng_reader_reports_a_warp_and_active_area() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("source.dng");
    dng_bytes(
        &path,
        &opcode(&[[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]], (0.5, 0.5), 1, 0),
    );
    let lens = read_lens_metadata(Some(&path));
    assert!(lens.available());
    assert!(lens.distortion(), "{:?}", lens);
    assert_eq!(lens.active_area, Some([0, 0, 40, 60]));
}

#[test]
fn a_cache_reads_again_when_the_file_revision_changes() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("source.dng");
    dng_bytes(
        &path,
        &opcode(&[[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]], (0.5, 0.5), 1, 0),
    );
    assert!(read_lens_metadata(Some(&path)).available());
    dng_bytes(&path, &opcode(&[identity()], (0.5, 0.5), 1, 0));
    assert!(!read_lens_metadata(Some(&path)).available());
}

#[test]
fn bind_decode_checks_the_active_area() {
    let lens = LensMetadata {
        warps: vec![LensWarp::Rectilinear {
            coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
            center: (0.5, 0.5),
        }],
        active_area: Some([4, 8, 104, 208]),
        ..LensMetadata::default()
    };
    let bound = bind_decode(&lens, [4, 8, 104, 208], false);
    assert_eq!(bound.buffer_area, Some([4, 8, 104, 208]));
    let mismatch = bind_decode(&lens, [4, 8, 104, 308], false);
    assert!(!mismatch.available());
}

#[test]
fn unsupported_extensions_report_why() {
    let path = PathBuf::from("frame.jpg");
    let lens = read_lens_metadata(Some(&path));
    assert!(lens.reason.contains("not supported"));
    assert!(!lens.available());
}

#[test]
fn a_panasonic_sample_parses_the_distortion_polynomial() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("sample.rw2");
    let words = [
        29475i16, -32031, 227, 629, 152, -614, 202, -13055, -888, 177, 344, 123, 3605, 677, -14365,
        29981,
    ];
    rw2_bytes(&path, &words, 0x0119, 7, 32, 22, false);
    let lens = read_lens_metadata(Some(&path));
    assert!(lens.available());
    assert_eq!(lens.source, "Panasonic RW2");
    assert!(lens.distortion(), "{lens:?}");
    assert!(!lens.ca());
    let LensWarp::Panasonic { scale, a, b, c } = lens.warps[0] else {
        panic!("panasonic warp");
    };
    assert!((scale - 1.0190956).abs() < 1e-6);
    assert!((a - -0.027099609375).abs() < 1e-9);
    assert!((b - 0.004638671875).abs() < 1e-9);
    assert!((c - 0.003753662109375).abs() < 1e-9);
}

#[test]
fn a_panasonic_off_flag_is_unavailable() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("off.rw2");
    let mut words = [0i16; 16];
    words.copy_from_slice(&[
        29475, -32031, 227, 629, 152, -614, 202, -13055, -888, 177, 344, 123, 3605, 677, -14365,
        29981,
    ]);
    words[7] = 0x0102;
    rw2_bytes(&path, &words, 0x0119, 7, 32, 22, false);
    assert!(!read_lens_metadata(Some(&path)).available());
}

#[test]
fn malformed_panasonic_files_do_not_enable_correction() {
    let dir = tempfile::TempDir::new().unwrap();
    let words = [0i16; 16];
    for (index, (tag, kind, count, offset, big_endian)) in [
        (0x0119, 7, 32, 22, true),
        (0x0119, 7, 16, 22, false),
        (0x0119, 3, 32, 22, false),
        (0x0100, 7, 32, 22, false),
        (0x0119, 7, 32, 9999, false),
    ]
    .into_iter()
    .enumerate()
    {
        let path = dir.path().join(format!("bad{index}.rw2"));
        rw2_bytes(&path, &words, tag, kind, count, offset, big_endian);
        assert!(!read_lens_metadata(Some(&path)).available(), "{index}");
    }
}

fn rw2_bytes(
    path: &std::path::Path,
    words: &[i16; 16],
    tag: u16,
    kind: u16,
    count: u32,
    value_offset: u32,
    big_endian: bool,
) {
    let order = if big_endian { ">" } else { "<" };
    let mut data = Vec::new();
    data.extend_from_slice(if big_endian {
        b"MM\x00\x55"
    } else {
        b"II\x55\x00"
    });
    push_u32(&mut data, 8, big_endian);
    push_u16(&mut data, 1, big_endian);
    push_u16(&mut data, tag, big_endian);
    push_u16(&mut data, kind, big_endian);
    push_u32(&mut data, count, big_endian);
    push_u32(&mut data, value_offset, big_endian);
    for word in words {
        data.extend_from_slice(&word.to_le_bytes());
    }
    let _ = order;
    std::fs::write(path, data).unwrap();
}

fn push_u16(data: &mut Vec<u8>, value: u16, big: bool) {
    data.extend_from_slice(&if big {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    });
}

fn push_u32(data: &mut Vec<u8>, value: u32, big: bool) {
    data.extend_from_slice(&if big {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    });
}

#[test]
fn sony_padded_arrays_and_ca_only_are_independent() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut dist: Vec<i16> = vec![11, -100, -90, -80, -70, -60, -50, -40, -30, -20, -10, 0];
    dist.extend(std::iter::repeat_n(0i16, 5));
    let mut ca = Vec::new();
    ca.push(22i16);
    ca.extend(std::iter::repeat_n(100i16, 11));
    ca.extend(std::iter::repeat_n(-100i16, 11));
    ca.extend(std::iter::repeat_n(0i16, 10));
    let path = dir.path().join("source.arw");
    arw_bytes(&path, &[(0x7037, &dist), (0x7035, &ca)]);
    let lens = read_lens_metadata(Some(&path));
    assert!(lens.distortion(), "{lens:?}");
    assert!(lens.ca());
    let LensWarp::Sony { distortion, .. } = &lens.warps[0] else {
        panic!("sony warp");
    };
    assert_eq!(distortion.len(), 11);

    let ca_only = dir.path().join("ca.arw");
    arw_bytes(&ca_only, &[(0x7035, &ca)]);
    let lens = read_lens_metadata(Some(&ca_only));
    assert!(lens.ca());
    assert!(!lens.distortion());
}

#[test]
fn a_sony_unavailable_flag_overrides_leftover_coefficients() {
    let dir = tempfile::TempDir::new().unwrap();
    let dist: Vec<i16> = std::iter::once(16)
        .chain(std::iter::repeat_n(100i16, 16))
        .collect();
    let path = dir.path().join("source.arw");
    arw_bytes_typed(&path, &[(0x7037, 8, &dist), (0x7036, 3, &[255, 0, 0])]);
    assert!(!read_lens_metadata(Some(&path)).available());
}

#[test]
fn a_dng_does_not_reuse_inherited_sony_coefficients() {
    let dir = tempfile::TempDir::new().unwrap();
    let dist: Vec<i16> = std::iter::once(16)
        .chain(std::iter::repeat_n(100i16, 16))
        .collect();
    let path = dir.path().join("converted.dng");
    arw_bytes(&path, &[(0x7037, &dist)]);
    assert!(!read_lens_metadata(Some(&path)).available());
}

fn arw_bytes(path: &std::path::Path, tags: &[(u16, &[i16])]) {
    let typed: Vec<(u16, u16, &[i16])> = tags
        .iter()
        .map(|(tag, values)| (*tag, 8, *values))
        .collect();
    arw_bytes_typed(path, &typed);
}

fn arw_bytes_typed(path: &std::path::Path, tags: &[(u16, u16, &[i16])]) {
    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    let base_entries = 4 + tags.len();
    out.extend_from_slice(&(base_entries as u16).to_le_bytes());
    for (tag, kind, count, value) in [
        (256u16, 3u16, 1u32, 60u32),
        (257, 3, 1, 40),
        (277, 3, 1, 1),
        (262, 3, 1, 32_803),
    ] {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    let payload_start = 8 + 2 + base_entries * 12 + 4;
    let mut payload = Vec::new();
    for (tag, kind, values) in tags {
        out.extend_from_slice(&tag.to_le_bytes());
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        let at = payload_start + payload.len();
        out.extend_from_slice(&(at as u32).to_le_bytes());
        for value in *values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&payload);
    std::fs::write(path, out).unwrap();
}
