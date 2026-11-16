//! Container resolution from JFIF and PNG headers.

use negpy_backend::features::metadata::container_resolution::{
    from_container, from_jfif, from_png,
};
use negpy_backend::features::metadata::resolution::{CENTIMETRE, INCH, Resolution};

fn jfif(unit: u8, x: u16, y: u16) -> Vec<u8> {
    let mut data = vec![0xFF, 0xD8];
    data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
    data.extend_from_slice(b"JFIF\0");
    data.extend_from_slice(&[1, 2, unit]);
    data.extend_from_slice(&x.to_be_bytes());
    data.extend_from_slice(&y.to_be_bytes());
    data.extend_from_slice(&[0, 0]);
    data.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
    data
}

fn png(unit: u8, x: u32, y: u32) -> Vec<u8> {
    let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
    data.extend_from_slice(&9u32.to_be_bytes());
    data.extend_from_slice(b"pHYs");
    data.extend_from_slice(&x.to_be_bytes());
    data.extend_from_slice(&y.to_be_bytes());
    data.push(unit);
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(&0u32.to_be_bytes());
    data.extend_from_slice(b"IDAT");
    data.extend_from_slice(&[0, 0, 0, 0]);
    data
}

#[test]
fn jfif_keeps_its_native_unit() {
    assert_eq!(
        from_jfif(&jfif(2, 100, 50)),
        Some(Resolution {
            x: (100, 1),
            y: (50, 1),
            unit: CENTIMETRE
        })
    );
    assert_eq!(
        from_jfif(&jfif(1, 72, 72)),
        Some(Resolution {
            x: (72, 1),
            y: (72, 1),
            unit: INCH
        })
    );
}

#[test]
fn jfif_unit_zero_is_an_aspect_ratio() {
    assert_eq!(from_jfif(&jfif(0, 1, 1)), None);
}

#[test]
fn png_phys_is_pixels_per_metre() {
    let resolution = from_png(&png(1, 2835, 2835)).unwrap();
    assert_eq!(resolution.unit, 0);
    let cm = resolution.to_centimetres();
    assert_eq!(cm.unit, CENTIMETRE);
    assert!((cm.x_dpi() - 72.0).abs() < 0.01);
    assert_eq!(from_png(&png(0, 1, 1)), None);
}

#[test]
fn from_container_dispatches_by_magic() {
    let dir = tempfile::tempdir().unwrap();
    let jpeg = dir.path().join("image.jpg");
    std::fs::write(&jpeg, jfif(1, 300, 300)).unwrap();
    assert_eq!(
        from_container(&jpeg),
        Some(Resolution {
            x: (300, 1),
            y: (300, 1),
            unit: INCH
        })
    );
    let png_path = dir.path().join("image.png");
    std::fs::write(&png_path, png(1, 11811, 11811)).unwrap();
    assert_eq!(from_container(&png_path).unwrap().unit, 0);
    let other = dir.path().join("image.tif");
    std::fs::write(&other, b"II*\0").unwrap();
    assert_eq!(from_container(&other), None);
}
