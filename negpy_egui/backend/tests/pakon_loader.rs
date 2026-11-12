//! Ports of tests/test_pakon_loader.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::pakon_loader::load;

const HEIGHT: usize = 1000;
const WIDTH: usize = 1500;

fn sample(value: u16) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn planar_ramp() -> Vec<u16> {
    let column: Vec<u16> = (0..WIDTH as u16).collect();
    let mut samples = Vec::with_capacity(3 * HEIGHT * WIDTH);
    for plane in 0..3u16 {
        for _ in 0..HEIGHT {
            samples.extend(column.iter().map(|value| value + plane * 100));
        }
    }
    samples
}

fn write(path: &Path, header: bool, samples: &[u16]) {
    let mut bytes = Vec::new();
    if header {
        for value in [16u32, WIDTH as u32, HEIGHT as u32, 48] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend(samples.iter().flat_map(|value| sample(*value)));
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_tlx_header_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("headered.raw");
    write(&path, true, &planar_ramp());
    let image = load(&path).unwrap().image.unwrap();
    assert_eq!((image.width, image.height), (WIDTH, HEIGHT));
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..3], &[0, 100, 200]);
}

#[test]
fn a_headerless_planar_file_is_guessed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("headerless.raw");
    write(&path, false, &planar_ramp());
    let image = load(&path).unwrap().image.unwrap();
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..3], &[0, 100, 200]);
}

#[test]
fn an_interleaved_file_is_read_as_bgr() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("interleaved.raw");
    let mut samples = Vec::with_capacity(HEIGHT * WIDTH * 3);
    for index in 0..HEIGHT * WIDTH {
        samples.extend([index as u16 % 1000, 100, 200]);
    }
    write(&path, false, &samples);
    let image = load(&path).unwrap().image.unwrap();
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..3], &[200, 100, 0]);
}

#[test]
fn a_trimmed_frame_keeps_two_thousand_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trimmed.raw");
    let width = 2017usize;
    let mut samples = Vec::with_capacity(3 * 2000 * width);
    for plane in 0..3u16 {
        samples.extend(std::iter::repeat_n(plane * 1000, 2000 * width));
    }
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    bytes.extend(samples.iter().flat_map(|value| sample(*value)));
    std::fs::write(&path, bytes).unwrap();
    let image = load(&path).unwrap().image.unwrap();
    assert_eq!((image.width, image.height), (width, 2000));
    assert_eq!(&image.source_u16.unwrap()[..3], &[0, 1000, 2000]);
}
