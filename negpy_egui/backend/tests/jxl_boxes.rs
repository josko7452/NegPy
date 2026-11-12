//! Ports of tests/test_jxl_loader.py's container rows.

use negpy_backend::infrastructure::loaders::jxl_boxes::{
    JXL_SIGNATURE, is_jxl, jxl_boxes, read_jxl_exif, read_jxl_xmp,
};

fn boxed(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

#[test]
fn a_container_is_split_into_boxes() {
    let data = [boxed(b"JXL ", b""), boxed(b"jxlc", b"pixels")].concat();
    let boxes = jxl_boxes(&data).unwrap();
    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[1].kind, *b"jxlc");
    assert_eq!(&data[boxes[1].start..boxes[1].end], b"pixels");
}

#[test]
fn a_malformed_container_is_rejected() {
    let data = [JXL_SIGNATURE, &[0, 0, 0xff, 0xff], b"jxlc"].concat();
    assert!(jxl_boxes(&data).is_err());
    assert!(is_jxl(&data));
}

#[test]
fn an_exif_box_carries_the_tiff_header() {
    let tiff = b"II*\0rest";
    let mut payload = vec![0, 0, 0, 0];
    payload.extend_from_slice(tiff);
    let data = [JXL_SIGNATURE, &boxed(b"Exif", &payload)].concat();
    assert_eq!(read_jxl_exif(&data).as_deref(), Some(&tiff[..]));
}

#[test]
fn a_brotli_box_is_decompressed() {
    let tiff = b"II*\0payload";
    let mut inner = vec![0, 0, 0, 0];
    inner.extend_from_slice(tiff);
    // Written with `/usr/bin/brotli -q 11`; the payload is the four-byte
    // offset and the TIFF header.
    let compressed = b"\x21\x38\x00\x04\x00\x00\x00\x00II*\x00payload\x03";
    let mut payload = b"Exif".to_vec();
    payload.extend_from_slice(compressed);
    let data = [JXL_SIGNATURE, &boxed(b"brob", &payload)].concat();
    assert_eq!(read_jxl_exif(&data).as_deref(), Some(&tiff[..]));
}

#[test]
fn xmp_is_read_from_its_own_box() {
    let xmp = b"<x:xmpmeta/>";
    let data = [JXL_SIGNATURE, &boxed(b"xml ", xmp)].concat();
    assert_eq!(read_jxl_xmp(&data).as_deref(), Some(&xmp[..]));
    assert!(read_jxl_exif(&data).is_none());
}
