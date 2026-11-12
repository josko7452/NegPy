//! Ports of tests/test_icc_identify.py and tests/test_read_orientation.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::helpers::{
    identify_color_space_from_icc, read_orientation,
};

fn tiff_with_orientation(orientation: u16) -> Vec<u8> {
    let mut data = b"II*\0\x08\0\0\0".to_vec();
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&274u16.to_le_bytes());
    data.extend_from_slice(&3u16.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&orientation.to_le_bytes());
    data.extend_from_slice(&[0, 0]);
    data.extend_from_slice(&0u32.to_le_bytes());
    data
}

#[test]
fn the_bundled_profiles_are_recognized() {
    let cases: [(&[u8], &str); 4] = [
        (
            include_bytes!("../../../icc/AdobeCompat-v4.icc"),
            "Adobe RGB",
        ),
        (
            include_bytes!("../../../icc/ProPhoto-v4.icc"),
            "ProPhoto RGB",
        ),
        (include_bytes!("../../../icc/Rec2020-v4.icc"), "Rec 2020"),
        (include_bytes!("../../../icc/DisplayP3-v4.icc"), "P3 D65"),
    ];
    for (profile, expected) in cases {
        assert_eq!(identify_color_space_from_icc(profile), Some(expected));
    }
}

#[test]
fn no_profile_names_no_space() {
    assert_eq!(identify_color_space_from_icc(&[]), None);
    assert_eq!(identify_color_space_from_icc(b"not an icc profile"), None);
}

#[test]
fn a_tiff_orientation_is_read_from_the_first_ifd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.tif");
    std::fs::write(&path, tiff_with_orientation(6)).unwrap();
    assert_eq!(read_orientation(&path), 6);
}

#[test]
fn a_jpeg_exif_orientation_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.jpg");
    let tiff = tiff_with_orientation(8);
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff);
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    jpeg.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    jpeg.extend_from_slice(&payload);
    jpeg.extend_from_slice(&[0xff, 0xda, 0, 2]);
    std::fs::write(&path, jpeg).unwrap();
    assert_eq!(read_orientation(&path), 8);
}

#[test]
fn a_file_without_exif_stays_upright() {
    assert_eq!(read_orientation(Path::new("/missing.jpg")), 1);
}
