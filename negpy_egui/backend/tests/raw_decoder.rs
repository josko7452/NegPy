//! Ports of tests/test_unsupported_raw_reason.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::raw_decoder::{
    RawDecoder, RawlerDecoder, UnsupportedRawDecoder, unsupported_raw_reason,
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

fn write_dng(path: &Path, width: u32, height: u32, sample: u16) {
    let entries = 13usize;
    let ifd_offset = 8usize;
    let ifd_end = ifd_offset + 2 + entries * 12 + 4;
    let cfa_offset = ifd_end;
    let neutral_offset = cfa_offset + 8;
    let pixels_offset = neutral_offset + 24;
    let pixels_len = usize::try_from(width * height * 2).unwrap();
    let mut data = vec![0u8; pixels_offset + pixels_len];
    data[0..8].copy_from_slice(&[b'I', b'I', 42, 0, ifd_offset as u8, 0, 0, 0]);
    data[ifd_offset..ifd_offset + 2].copy_from_slice(&(entries as u16).to_le_bytes());

    let mut write_entry = |index: usize, tag: u16, kind: u16, count: u32, value: u32| {
        let start = ifd_offset + 2 + index * 12;
        data[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        data[start + 2..start + 4].copy_from_slice(&kind.to_le_bytes());
        data[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
        data[start + 8..start + 12].copy_from_slice(&value.to_le_bytes());
    };
    write_entry(0, 256, 4, 1, width);
    write_entry(1, 257, 4, 1, height);
    write_entry(2, 258, 3, 1, 16);
    write_entry(3, 259, 3, 1, 1);
    write_entry(4, 262, 3, 1, 32_803);
    write_entry(5, 273, 4, 1, pixels_offset as u32);
    write_entry(6, 277, 3, 1, 1);
    write_entry(7, 278, 4, 1, height);
    write_entry(8, 279, 4, 1, pixels_len as u32);
    write_entry(9, 33_421, 3, 2, u32::from_le_bytes([2, 0, 2, 0]));
    write_entry(10, 33_422, 3, 4, cfa_offset as u32);
    write_entry(11, 50_706, 1, 4, u32::from_le_bytes([1, 4, 0, 0]));
    write_entry(12, 50_728, 5, 3, neutral_offset as u32);
    let next = ifd_offset + 2 + entries * 12;
    data[next..next + 4].copy_from_slice(&0u32.to_le_bytes());

    for (index, value) in [0u16, 1, 1, 2].into_iter().enumerate() {
        let start = cfa_offset + index * 2;
        data[start..start + 2].copy_from_slice(&value.to_le_bytes());
    }
    for (index, (numerator, denominator)) in [(1u32, 2u32), (1, 1), (1, 2)].into_iter().enumerate()
    {
        let start = neutral_offset + index * 8;
        data[start..start + 4].copy_from_slice(&numerator.to_le_bytes());
        data[start + 4..start + 8].copy_from_slice(&denominator.to_le_bytes());
    }
    for sample_bytes in data[pixels_offset..].chunks_exact_mut(2) {
        sample_bytes.copy_from_slice(&sample.to_le_bytes());
    }
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

#[test]
fn a_camera_dng_decodes_sensor_rgb_with_camera_white_balance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.dng");
    write_dng(&path, 16, 12, 8192);

    let loaded = RawlerDecoder.decode(&path).unwrap();
    let image = loaded.image.unwrap();
    assert_eq!((image.width, image.height), (16, 12));
    assert_eq!(loaded.orientation, 1);
    assert_eq!(loaded.color_space, None);
    assert!(image.rgb_xyz_matrix.is_none());
    assert_eq!(image.pixels.len(), 16 * 12 * 3);

    let expected = 8192.0 / 65535.0;
    assert!((image.pixels[0] - expected * 2.0).abs() < 1e-3);
    assert!((image.pixels[1] - expected).abs() < 1e-3);
    assert!((image.pixels[2] - expected * 2.0).abs() < 1e-3);
}
