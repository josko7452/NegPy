//! SGI LogLuv decode to linear RGB.
//!
//! Mirrors `negpy/infrastructure/loaders/logluv.py`: LogLuv32 RLE and LogLuv24
//! pixels become XYZ through per-code tables, then linear sRGB.

use std::sync::OnceLock;

use negpy_core::loaders::{decode_sgilog32, logluv_to_rgb};
use negpy_core::mathf::exp_f64;

/// SGI LogLuv32 compression, an RLE-packed 32-bit code.
pub const COMPRESSION_SGILOG: u16 = 34676;
/// SGI LogLuv24 compression, three bytes per pixel.
pub const COMPRESSION_SGILOG24: u16 = 34677;

const M_LN2: f64 = 0.693_147_180_559_945_3;
const UVSCALE: f64 = 410.0;
const U_NEU: f64 = 0.210_526_316;
const V_NEU: f64 = 0.473_684_211;
const UV_SQSIZ: f64 = 0.003_500;
const UV_VSTART: f64 = 0.016_940;
const UV_NVS: usize = 163;
const UV_NDIVS: usize = 16_289;

const XYZ_TO_LRGB: [f64; 9] = [
    2.690, -1.276, -0.414, -1.022, 1.978, 0.044, 0.061, -0.224, 1.163,
];

fn logl10_to_y(code: u32) -> f64 {
    if code == 0 {
        return 0.0;
    }
    exp_f64(M_LN2 / 64.0 * (f64::from(code) + 0.5) - M_LN2 * 12.0)
}

fn logl16_to_y(code: u32) -> f64 {
    let top = code as i32;
    let le = top & 0x7fff;
    if le == 0 {
        return 0.0;
    }
    let y = exp_f64(M_LN2 / 256.0 * (f64::from(le) + 0.5) - M_LN2 * 64.0);
    if top & 0x8000 != 0 { -y } else { y }
}

fn chroma(u: f64, v: f64) -> (f64, f64) {
    let s = 1.0 / (6.0 * u - 16.0 * v + 12.0);
    let x = 9.0 * u * s;
    let yv = 4.0 * v * s;
    (x / yv, (1.0 - x - yv) / yv)
}

/// The published LogLuv u/v table: `(u start, codes in the row, first code)`.
const UV_ROW: [(f64, u32, u32); UV_NVS] = [
    (0.247663, 4, 0),
    (0.243779, 6, 4),
    (0.241684, 7, 10),
    (0.237874, 9, 17),
    (0.235906, 10, 26),
    (0.232153, 12, 36),
    (0.228352, 14, 48),
    (0.226259, 15, 62),
    (0.222371, 17, 77),
    (0.220410, 18, 94),
    (0.214710, 21, 112),
    (0.212714, 22, 133),
    (0.210721, 23, 155),
    (0.204976, 26, 178),
    (0.202986, 27, 204),
    (0.199245, 29, 231),
    (0.195525, 31, 260),
    (0.193560, 32, 291),
    (0.189878, 34, 323),
    (0.186216, 36, 357),
    (0.186216, 36, 393),
    (0.182592, 38, 429),
    (0.179003, 40, 467),
    (0.175466, 42, 507),
    (0.172001, 44, 549),
    (0.172001, 44, 593),
    (0.168612, 46, 637),
    (0.168612, 46, 683),
    (0.163575, 49, 729),
    (0.158642, 52, 778),
    (0.158642, 52, 830),
    (0.158642, 52, 882),
    (0.153815, 55, 934),
    (0.153815, 55, 989),
    (0.149097, 58, 1044),
    (0.149097, 58, 1102),
    (0.142746, 62, 1160),
    (0.142746, 62, 1222),
    (0.142746, 62, 1284),
    (0.138270, 65, 1346),
    (0.138270, 65, 1411),
    (0.138270, 65, 1476),
    (0.132166, 69, 1541),
    (0.132166, 69, 1610),
    (0.126204, 73, 1679),
    (0.126204, 73, 1752),
    (0.126204, 73, 1825),
    (0.120381, 77, 1898),
    (0.120381, 77, 1975),
    (0.120381, 77, 2052),
    (0.120381, 77, 2129),
    (0.112962, 82, 2206),
    (0.112962, 82, 2288),
    (0.112962, 82, 2370),
    (0.107450, 86, 2452),
    (0.107450, 86, 2538),
    (0.107450, 86, 2624),
    (0.107450, 86, 2710),
    (0.100343, 91, 2796),
    (0.100343, 91, 2887),
    (0.100343, 91, 2978),
    (0.095126, 95, 3069),
    (0.095126, 95, 3164),
    (0.095126, 95, 3259),
    (0.095126, 95, 3354),
    (0.088276, 100, 3449),
    (0.088276, 100, 3549),
    (0.088276, 100, 3649),
    (0.088276, 100, 3749),
    (0.081523, 105, 3849),
    (0.081523, 105, 3954),
    (0.081523, 105, 4059),
    (0.081523, 105, 4164),
    (0.074861, 110, 4269),
    (0.074861, 110, 4379),
    (0.074861, 110, 4489),
    (0.074861, 110, 4599),
    (0.068290, 115, 4709),
    (0.068290, 115, 4824),
    (0.068290, 115, 4939),
    (0.068290, 115, 5054),
    (0.063573, 119, 5169),
    (0.063573, 119, 5288),
    (0.063573, 119, 5407),
    (0.063573, 119, 5526),
    (0.057219, 124, 5645),
    (0.057219, 124, 5769),
    (0.057219, 124, 5893),
    (0.057219, 124, 6017),
    (0.050985, 129, 6141),
    (0.050985, 129, 6270),
    (0.050985, 129, 6399),
    (0.050985, 129, 6528),
    (0.050985, 129, 6657),
    (0.044859, 134, 6786),
    (0.044859, 134, 6920),
    (0.044859, 134, 7054),
    (0.044859, 134, 7188),
    (0.040571, 138, 7322),
    (0.040571, 138, 7460),
    (0.040571, 138, 7598),
    (0.040571, 138, 7736),
    (0.036339, 142, 7874),
    (0.036339, 142, 8016),
    (0.036339, 142, 8158),
    (0.036339, 142, 8300),
    (0.032139, 146, 8442),
    (0.032139, 146, 8588),
    (0.032139, 146, 8734),
    (0.032139, 146, 8880),
    (0.027947, 150, 9026),
    (0.027947, 150, 9176),
    (0.027947, 150, 9326),
    (0.023739, 154, 9476),
    (0.023739, 154, 9630),
    (0.023739, 154, 9784),
    (0.023739, 154, 9938),
    (0.019504, 158, 10092),
    (0.019504, 158, 10250),
    (0.019504, 158, 10408),
    (0.016976, 161, 10566),
    (0.016976, 161, 10727),
    (0.016976, 161, 10888),
    (0.016976, 161, 11049),
    (0.012639, 165, 11210),
    (0.012639, 165, 11375),
    (0.012639, 165, 11540),
    (0.009991, 168, 11705),
    (0.009991, 168, 11873),
    (0.009991, 168, 12041),
    (0.009016, 170, 12209),
    (0.009016, 170, 12379),
    (0.009016, 170, 12549),
    (0.006217, 173, 12719),
    (0.006217, 173, 12892),
    (0.005097, 175, 13065),
    (0.005097, 175, 13240),
    (0.005097, 175, 13415),
    (0.003909, 177, 13590),
    (0.003909, 177, 13767),
    (0.002340, 177, 13944),
    (0.002389, 170, 14121),
    (0.001068, 164, 14291),
    (0.001653, 157, 14455),
    (0.000717, 150, 14612),
    (0.001614, 143, 14762),
    (0.000270, 136, 14905),
    (0.000484, 129, 15041),
    (0.001103, 123, 15170),
    (0.001242, 115, 15293),
    (0.001188, 109, 15408),
    (0.001011, 103, 15517),
    (0.000709, 97, 15620),
    (0.000301, 89, 15717),
    (0.002416, 82, 15806),
    (0.003251, 76, 15888),
    (0.003246, 69, 15964),
    (0.004141, 62, 16033),
    (0.005963, 55, 16095),
    (0.008839, 47, 16150),
    (0.010490, 40, 16197),
    (0.016994, 31, 16237),
    (0.023659, 21, 16268),
];

fn uv_row(code: usize) -> (usize, u32) {
    let mut row = UV_ROW.partition_point(|entry| entry.2 <= code as u32);
    row = row.saturating_sub(1).min(UV_NVS - 1);
    (row, code as u32 - UV_ROW[row].2)
}

struct CodeTables {
    y: Vec<f64>,
    x: Vec<f64>,
    z: Vec<f64>,
}

fn logluv32_tables() -> &'static CodeTables {
    static TABLES: OnceLock<CodeTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let y = (0..1 << 16).map(logl16_to_y).collect();
        let (mut x, mut z) = (Vec::with_capacity(1 << 16), Vec::with_capacity(1 << 16));
        for code in 0..1 << 16 {
            let u = (f64::from(code >> 8 & 0xff) + 0.5) / UVSCALE;
            let v = (f64::from(code & 0xff) + 0.5) / UVSCALE;
            let (xx, zz) = chroma(u, v);
            x.push(xx);
            z.push(zz);
        }
        CodeTables { y, x, z }
    })
}

fn logluv24_tables() -> &'static CodeTables {
    static TABLES: OnceLock<CodeTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let y = (0..1 << 10).map(logl10_to_y).collect();
        let (mut x, mut z) = (Vec::with_capacity(1 << 14), Vec::with_capacity(1 << 14));
        for code in 0..1 << 14 {
            let (row, inside) = uv_row(code);
            let (u, v) = if code >= UV_NDIVS {
                (U_NEU, V_NEU)
            } else {
                (
                    UV_ROW[row].0 + (inside as f64 + 0.5) * UV_SQSIZ,
                    UV_VSTART + (row as f64 + 0.5) * UV_SQSIZ,
                )
            };
            let (xx, zz) = chroma(u, v);
            x.push(xx);
            z.push(zz);
        }
        CodeTables { y, x, z }
    })
}

/// LogLuv32 pixels to linear sRGB float32, unbounded.
#[must_use]
pub fn logluv32_to_rgb(packed: &[u32]) -> Vec<f32> {
    let tables = logluv32_tables();
    let mut dst = vec![0f32; packed.len() * 3];
    logluv_to_rgb(
        packed,
        &tables.y,
        16,
        &tables.x,
        &tables.z,
        XYZ_TO_LRGB,
        &mut dst,
    );
    dst
}

/// LogLuv24 pixels in the low 24 bits to linear sRGB float32, unbounded.
#[must_use]
pub fn logluv24_to_rgb(packed: &[u32]) -> Vec<f32> {
    let tables = logluv24_tables();
    let mut dst = vec![0f32; packed.len() * 3];
    logluv_to_rgb(
        packed,
        &tables.y,
        14,
        &tables.x,
        &tables.z,
        XYZ_TO_LRGB,
        &mut dst,
    );
    dst
}

/// Decode one LogLuv32 RLE strip of `pixels` pixels in scanlines of `width`.
#[must_use]
pub fn decode_strip_logluv32(compressed: &[u8], pixels: usize, width: usize) -> Vec<u32> {
    let mut dst = vec![0u32; pixels];
    if width > 0 {
        decode_sgilog32(compressed, width, &mut dst);
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packets(pixels: &[u32], width: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for row in pixels.chunks(width) {
            for shift in [24, 16, 8, 0] {
                for chunk in row.chunks(127) {
                    out.push(chunk.len() as u8);
                    out.extend(chunk.iter().map(|value| (value >> shift) as u8));
                }
            }
        }
        out
    }

    #[test]
    fn black_stays_black() {
        assert_eq!(logluv32_to_rgb(&[0]), [0.0, 0.0, 0.0]);
        assert_eq!(logluv24_to_rgb(&[0]), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_gray_code_stays_neutral() {
        let rgb = logluv32_to_rgb(&[0x3f00_56c1]);
        let spread = rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2]);
        assert!((0.4..0.7).contains(&rgb[1]), "{rgb:?}");
        assert!(spread < 0.1, "{rgb:?}");
    }

    #[test]
    fn a_literal_packet_round_trips_one_scanline() {
        let codes = [0x0012_3456u32, 0x009a_bcde, 0x00ff_0000];
        let strip = packets(&codes, codes.len());
        assert_eq!(
            decode_strip_logluv32(&strip, codes.len(), codes.len()),
            codes
        );
    }

    #[test]
    fn a_run_packet_expands() {
        let mut strip = vec![2u8];
        strip.extend([0x12, 0x34, 0x56, 0x78]);
        for shift in [16, 8, 0] {
            strip.push(128 + 2);
            strip.push((0x11u32 >> (24 - shift)) as u8);
        }
        let codes = decode_strip_logluv32(&strip, 2, 2);
        assert_eq!(codes.len(), 2);
    }
}
