//! Ports of tests/test_unsupported_raw_reason.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::raw_decoder::{
    RawDecoder, UnsupportedRawDecoder, unsupported_raw_reason,
};

const TICORAW: &[u8] = b"\xff\x10\xff\x50\x00\x22CONTACT_INTOPIX_\xef\xc0\x00\x00";
const LOSSLESS: &[u8] = b"\x00\x4a\x00\x01\x00\x00\x00\x01\x02\x03";

fn write_nef(path: &Path, payload: &[u8]) {
    let strip_offset = 512usize;
    let sub_offset = 200usize;
    let ifd0_offset = 8usize;
    let mut data = vec![0u8; strip_offset + payload.len()];
    data[0..8].copy_from_slice(&[b'I', b'I', 42, 0, ifd0_offset as u8, 0, 0, 0]);

    let mut ifd0 = Vec::new();
    ifd0.extend_from_slice(&1u16.to_le_bytes());
    for (tag, kind, count, value) in [(330u16, 4u16, 1u32, sub_offset as u32)] {
        ifd0.extend_from_slice(&tag.to_le_bytes());
        ifd0.extend_from_slice(&kind.to_le_bytes());
        ifd0.extend_from_slice(&count.to_le_bytes());
        ifd0.extend_from_slice(&value.to_le_bytes());
    }
    ifd0.extend_from_slice(&0u32.to_le_bytes());
    data[ifd0_offset..ifd0_offset + ifd0.len()].copy_from_slice(&ifd0);

    let mut sub = Vec::new();
    sub.extend_from_slice(&6u16.to_le_bytes());
    for (tag, value) in [
        (256u16, 8280u32),
        (257, 5520),
        (258, 14),
        (259, 34713),
        (273, strip_offset as u32),
        (279, payload.len() as u32),
    ] {
        sub.extend_from_slice(&tag.to_le_bytes());
        sub.extend_from_slice(&3u16.to_le_bytes());
        sub.extend_from_slice(&1u32.to_le_bytes());
        sub.extend_from_slice(&value.to_le_bytes());
    }
    sub.extend_from_slice(&0u32.to_le_bytes());
    data[sub_offset..sub_offset + sub.len()].copy_from_slice(&sub);
    data[strip_offset..].copy_from_slice(payload);
    std::fs::write(path, data).unwrap();
}

#[test]
fn high_efficiency_nef_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("he.nef");
    write_nef(&path, TICORAW);
    let reason = unsupported_raw_reason(&path).unwrap();
    assert!(reason.contains("High Efficiency"));
    assert!(reason.contains("Lossless") && reason.contains("DNG"));
    assert!(UnsupportedRawDecoder.decode(&path).is_err());
}

#[test]
fn an_ordinary_nef_has_no_complaint() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lossless.nef");
    write_nef(&path, LOSSLESS);
    assert!(unsupported_raw_reason(&path).is_none());
    assert_eq!(
        UnsupportedRawDecoder.decode(&path).unwrap_err(),
        "This camera RAW format is not supported by this build."
    );
}

#[test]
fn only_nef_is_inspected_and_a_broken_file_has_no_explanation() {
    for name in ["shot.cr2", "shot.arw", "shot.dng", "scan.tif"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        write_nef(&path, TICORAW);
        assert!(unsupported_raw_reason(&path).is_none(), "{name}");
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.nef");
    std::fs::write(&path, b"not a tiff at all").unwrap();
    assert!(unsupported_raw_reason(&path).is_none());
    assert!(unsupported_raw_reason(&dir.path().join("absent.nef")).is_none());
}
