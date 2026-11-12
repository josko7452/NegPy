//! Ports of tests/test_loader_u16_sources.py's Noritsu row.

use negpy_backend::infrastructure::loaders::noritsu_loader::load;

#[test]
fn a_known_size_decodes_chunky_bgr16_as_rgb() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.raw");
    let (width, height) = (3711usize, 5028usize);
    let mut bytes = Vec::with_capacity(width * height * 6);
    for _ in 0..width * height {
        bytes.extend([0x01, 0x00, 0x02, 0x00, 0x03, 0x00]);
    }
    std::fs::write(&path, bytes).unwrap();
    let image = load(&path).unwrap().image.unwrap();
    assert_eq!((image.width, image.height), (width, height));
    let samples = image.source_u16.unwrap();
    assert_eq!(&samples[..6], &[3, 2, 1, 3, 2, 1]);
}
