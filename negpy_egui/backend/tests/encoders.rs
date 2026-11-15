//! The export containers: structure, round trip and the ancillary chunks.

use negpy_backend::services::export::encoders::{
    Resolution, TiffCompression, encode_jpeg, encode_png, encode_tiff, zlib_compress,
};

fn rgb8(width: usize, height: usize) -> Vec<u8> {
    (0..width * height * 3)
        .map(|index| ((index * 37) % 251) as u8)
        .collect()
}

fn rgb16(width: usize, height: usize) -> Vec<u8> {
    (0..width * height * 3)
        .flat_map(|index| (((index * 977) % 65521) as u16).to_le_bytes())
        .collect()
}

#[test]
fn a_png_starts_with_the_signature_and_ends_with_iend() {
    let out = encode_png(&rgb8(8, 8), 8, 8, 3, 8, 6, None, None, None, None).expect("png");
    assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&out[out.len() - 8..out.len() - 4], b"IEND");
}

#[test]
fn a_png_gray_and_a_png_16_bit_are_accepted() {
    let gray = vec![128u8; 8 * 6];
    assert!(encode_png(&gray, 8, 6, 1, 8, 6, None, None, None, None).is_some());
    assert!(encode_png(&rgb16(8, 6), 8, 6, 3, 16, 6, None, None, None, None).is_some());
}

#[test]
fn a_png_rejects_a_wrong_pixel_count() {
    assert!(encode_png(&[0u8; 10], 8, 8, 3, 8, 6, None, None, None, None).is_none());
}

#[test]
fn the_density_chunk_holds_pixels_per_metre() {
    let out = encode_png(
        &rgb8(8, 8),
        8,
        8,
        3,
        8,
        6,
        None,
        Some(Resolution::default()),
        None,
        None,
    )
    .expect("png");
    let at = out.windows(4).position(|w| w == b"pHYs").expect("pHYs");
    assert_eq!(
        u32::from_be_bytes(out[at + 4..at + 8].try_into().unwrap()),
        11811
    );
    assert_eq!(out[at + 12], 1);
}

#[test]
fn the_ancillary_chunks_precede_the_image_data() {
    let icc = vec![1u8, 2, 3, 4];
    let out = encode_png(
        &rgb8(8, 8),
        8,
        8,
        3,
        8,
        6,
        Some(&icc),
        Some(Resolution::default()),
        None,
        None,
    )
    .expect("png");
    let iccp = out.windows(4).position(|w| w == b"iCCP").expect("iCCP");
    let phys = out.windows(4).position(|w| w == b"pHYs").expect("pHYs");
    let idat = out.windows(4).position(|w| w == b"IDAT").expect("IDAT");
    assert!(iccp < idat && phys < idat);
}

#[test]
fn jpeg_writes_a_baseline_header() {
    let out = encode_jpeg(&rgb8(8, 8), 8, 8, 3, 90, None, None).expect("jpeg");
    assert_eq!(&out[..2], &[0xFF, 0xD8]);
    assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9]);
    assert!(out.windows(2).any(|w| w == [0xFF, 0xC0]));
}

#[test]
fn tiff_uncompressed_carries_the_raster_verbatim() {
    let pixels = rgb16(6, 6);
    let out = encode_tiff(&pixels, 6, 6, 3, 16, TiffCompression::None, None, None).expect("tiff");
    assert_eq!(&out[..4], b"II*\0");
    assert!(out.ends_with(&pixels));
}

#[test]
fn tiff_deflate_is_smaller_for_a_flat_raster() {
    let flat = vec![17u8; 64 * 64 * 3];
    let plain = encode_tiff(&flat, 64, 64, 3, 8, TiffCompression::None, None, None).expect("tiff");
    let zipped =
        encode_tiff(&flat, 64, 64, 3, 8, TiffCompression::Deflate, None, None).expect("tiff");
    assert!(zipped.len() < plain.len());
    assert_eq!(&zipped[..4], b"II*\0");
}

#[test]
fn zlib_compression_round_trips() {
    let data = b"negpy".repeat(32);
    let out = zlib_compress(&data, 6);
    assert_eq!(&out[..1], &[0x78]);
    assert_eq!(out[1], 0x9c);
    let restored = flate2_decompress(&out);
    assert_eq!(restored, data);
}

fn flate2_decompress(data: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let _ = decoder.read_to_end(&mut out);
    out
}

#[test]
fn a_tiff_rejects_a_wrong_pixel_count() {
    assert!(encode_tiff(&[0u8; 8], 6, 6, 3, 8, TiffCompression::None, None, None).is_none());
}

#[test]
fn the_png_and_jpeg_bytes_match_python() {
    use negpy_backend::services::export::encoders::{encode_jpeg, encode_png};
    use sha1::{Digest, Sha1};
    let pixels = rgb8(8, 8);
    let png = encode_png(&pixels, 8, 8, 3, 8, 6, None, None, None, None).expect("png");
    let jpeg = encode_jpeg(&pixels, 8, 8, 3, 90, None, None).expect("jpeg");
    let hex = |data: &[u8]| {
        let digest = Sha1::digest(data);
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    assert_eq!(hex(&png), "1ec5d300d4c7b5a75ed5f533c5eecbfd3b690a35");
    assert_eq!(hex(&jpeg), "03123a7457b7411c9b34f0df1b934ca46dd6e799");
}
