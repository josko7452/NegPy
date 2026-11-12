//! Ports of tests/test_linear_output.py's Coolscan NEF and Flextight FFF rows.

use std::fs::File;

use negpy_backend::infrastructure::loaders::fff_loader::{
    find_full_res_ifd, is_flextight_fff, load as load_fff,
};
use negpy_backend::infrastructure::loaders::nef_loader::{
    find_rgb_subifd, is_coolscan_nef, load as load_nef,
};
use negpy_backend::infrastructure::loaders::tiff_walker::read_ifds;
use tiff::encoder::colortype::{Gray16, RGB8, RGB16};
use tiff::encoder::{Ifd, TiffEncoder, TiffKindStandard};
use tiff::tags::Tag;

fn open(path: &std::path::Path) -> TiffEncoder<File, TiffKindStandard> {
    TiffEncoder::new(File::create(path).unwrap()).unwrap()
}

fn coolscan_nef(path: &std::path::Path, channels: u16) {
    let mut encoder = open(path);
    let mut sub = encoder.extra_directory().unwrap();
    sub.write_tag(Tag::ImageWidth, 300u32).unwrap();
    sub.write_tag(Tag::ImageLength, 200u32).unwrap();
    sub.write_tag(Tag::BitsPerSample, 16u16).unwrap();
    sub.write_tag(Tag::SamplesPerPixel, channels).unwrap();
    sub.write_tag(Tag::PhotometricInterpretation, 2u16).unwrap();
    sub.write_tag(Tag::Compression, 1u16).unwrap();
    sub.write_tag(Tag::StripOffsets, 0u32).unwrap();
    sub.write_tag(Tag::RowsPerStrip, 200u32).unwrap();
    sub.write_tag(Tag::StripByteCounts, u32::from(channels) * 300 * 200 * 2)
        .unwrap();
    let sub = sub.finish_with_offsets().unwrap();

    let mut parent = encoder.image_directory().unwrap();
    parent.write_tag(Tag::ImageWidth, 75u32).unwrap();
    parent.write_tag(Tag::ImageLength, 50u32).unwrap();
    parent.write_tag(Tag::BitsPerSample, 8u16).unwrap();
    parent.write_tag(Tag::SamplesPerPixel, 3u16).unwrap();
    parent
        .write_tag(Tag::PhotometricInterpretation, 2u16)
        .unwrap();
    parent.write_tag(Tag::Compression, 1u16).unwrap();
    parent.write_tag(Tag::StripOffsets, 0u32).unwrap();
    parent.write_tag(Tag::RowsPerStrip, 50u32).unwrap();
    parent
        .write_tag(Tag::StripByteCounts, 75 * 50 * 3u32)
        .unwrap();
    parent.write_tag(Tag::SubIfd, Ifd(sub.offset)).unwrap();
    parent.finish().unwrap();
}

fn write_scanner_tiff(path: &std::path::Path, subifd: bool, width: u32, height: u32) {
    let root_entries = if subifd { 10usize } else { 9usize };
    let sub_entries = 10usize;
    let ifd0 = 8usize;
    let ifd0_end = ifd0 + 2 + root_entries * 12 + 4;
    let sub_offset = (ifd0_end + 3) & !3;
    let root_pixels_offset = ifd0_end;
    let pixels_offset = if subifd {
        sub_offset + 2 + sub_entries * 12 + 4
    } else {
        root_pixels_offset
    };
    let payload = usize::try_from(width * height * 3 * 2).unwrap();
    let mut data = vec![0u8; pixels_offset + payload];
    data[..8].copy_from_slice(&[b'I', b'I', 42, 0, ifd0 as u8, 0, 0, 0]);
    data[ifd0..ifd0 + 2].copy_from_slice(&(root_entries as u16).to_le_bytes());
    fn entry(data: &mut [u8], at: usize, tag: u16, kind: u16, count: u32, value: u32) {
        data[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        data[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        data[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        data[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    let root = [
        (256, 4, 1, width),
        (257, 4, 1, height),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, 2),
        (273, 4, 1, root_pixels_offset as u32),
        (277, 3, 1, 3),
        (278, 4, 1, height),
        (279, 4, 1, width * height * 6),
    ];
    for (index, (tag, kind, count, value)) in root.into_iter().enumerate() {
        entry(&mut data, ifd0 + 2 + index * 12, tag, kind, count, value);
    }
    if subifd {
        entry(&mut data, ifd0 + 2 + 9 * 12, 330, 4, 1, sub_offset as u32);
        data[sub_offset..sub_offset + 2].copy_from_slice(&(sub_entries as u16).to_le_bytes());
        let sub = [
            (256, 4, 1, width),
            (257, 4, 1, height),
            (258, 3, 1, 16),
            (259, 3, 1, 1),
            (262, 3, 1, 2),
            (273, 4, 1, pixels_offset as u32),
            (277, 3, 1, 3),
            (278, 4, 1, height),
            (279, 4, 1, width * height * 6),
            (330, 4, 1, 0),
        ];
        for (index, (tag, kind, count, value)) in sub.into_iter().enumerate() {
            entry(
                &mut data,
                sub_offset + 2 + index * 12,
                tag,
                kind,
                count,
                value,
            );
        }
    }
    let next = if subifd {
        sub_offset + 2 + sub_entries * 12
    } else {
        ifd0 + 2 + root_entries * 12
    };
    data[next..next + 4].copy_from_slice(&0u32.to_le_bytes());
    for (index, value) in [16u16, 32, 48]
        .into_iter()
        .cycle()
        .take(width as usize * height as usize)
        .enumerate()
    {
        data[pixels_offset + index * 2..pixels_offset + index * 2 + 2]
            .copy_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, data).unwrap();
}

#[test]
fn a_coolscan_nef_is_detected_by_its_rgb_subifd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("coolscan.nef");
    coolscan_nef(&path, 3);
    let ifds = read_ifds(&path).unwrap();
    assert_eq!(find_rgb_subifd(&ifds).unwrap().width, 300);
    assert!(is_coolscan_nef(&path));
}

#[test]
fn a_coolscan_subifd_decodes_its_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("coolscan.nef");
    write_scanner_tiff(&path, true, 4, 3);
    let image = load_nef(&path).unwrap().image.unwrap();
    assert_eq!((image.width, image.height), (4, 3));
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..3], &[16, 32, 48]);
}

#[test]
fn a_camera_nef_or_an_empty_nef_is_not_a_coolscan() {
    let dir = tempfile::tempdir().unwrap();
    let camera = dir.path().join("camera.nef");
    let mut encoder = open(&camera);
    encoder
        .write_image::<Gray16>(300, 200, &vec![0; 300 * 200])
        .unwrap();
    assert!(!is_coolscan_nef(&camera));

    let other = dir.path().join("photo.tiff");
    let mut encoder = open(&other);
    encoder
        .write_image::<RGB16>(10, 10, &vec![0; 10 * 10 * 3])
        .unwrap();
    assert!(!is_coolscan_nef(&other));
}

#[test]
fn a_flextight_fff_is_detected_by_its_largest_16_bit_rgb_ifd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.fff");
    let mut encoder = open(&path);
    encoder
        .write_image::<RGB16>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    encoder
        .write_image::<RGB8>(75, 50, &vec![0; 75 * 50 * 3])
        .unwrap();
    let ifds = read_ifds(&path).unwrap();
    assert_eq!(find_full_res_ifd(&ifds).unwrap().width, 600);
    assert!(is_flextight_fff(&path));
}

#[test]
fn a_flextight_full_resolution_page_decodes_its_pixels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.fff");
    write_scanner_tiff(&path, false, 4, 3);
    let image = load_fff(&path).unwrap().image.unwrap();
    assert_eq!((image.width, image.height), (4, 3));
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..3], &[16, 32, 48]);
}

#[test]
fn an_8_bit_or_non_fff_file_is_not_a_flextight() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("small.fff");
    let mut encoder = open(&small);
    encoder
        .write_image::<RGB8>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    assert!(!is_flextight_fff(&small));

    let tiff = dir.path().join("photo.tiff");
    let mut encoder = open(&tiff);
    encoder
        .write_image::<RGB16>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    assert!(!is_flextight_fff(&tiff));
}
