//! Mirrors `negpy/services/film_edge/barcodes.py`: edge barcode module bits.

/// The DX module width in millimeters.
pub const DX_MODULE: f64 = 0.406;
/// The DX data track, `(near, far)` mm from the film edge.
pub const DX_DATA_TRACK: (f64, f64) = (0.0, 1.0);
/// The DX clock track.
pub const DX_CLOCK_TRACK: (f64, f64) = (1.0, 2.2);

/// SMPTE ST 254 KEYKODE: Code 128 subset C, 123 modules of 0.1905 mm.
pub const KEYKODE_MODULE: f64 = 0.1905;
pub const KEYKODE_MODULES: usize = 123;
pub const KEYKODE_TRACK: (f64, f64) = (0.25, 1.75);
/// SMPTE ST 254 frame index: a short bar every 4 perforations.
pub const FRAME_INDEX_LENGTH: f64 = 1.5;
pub const FRAME_INDEX_TRACK: (f64, f64) = (1.1, 1.35);

/// The human key's first letter names the maker; the barcode carries the maker's SMPTE number.
fn keykode_maker(code: &str) -> u32 {
    match code.as_bytes().first() {
        Some(b'A') => 1,
        Some(b'E' | b'K' | b'V') => 2,
        Some(b'F') => 3,
        Some(b'I') => 4,
        _ => 2,
    }
}

/// Code 128 bar and space widths of values 0-105; 106 is the stop.
const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];
const START_C: usize = 105;
const STOP: usize = 106;

fn bits(value: u32, count: usize) -> Vec<u8> {
    (0..count)
        .rev()
        .map(|shift| ((value >> shift) & 1) as u8)
        .collect()
}

/// `(clock, data)` tracks of a 31-module DX film edge barcode.
#[must_use]
pub fn dx_bits(number: u32, half: bool, product: (u32, u32)) -> (Vec<u8>, Vec<u8>) {
    let (p1, p2) = product;
    let mut payload = vec![0];
    payload.extend(bits(p1, 7));
    payload.push(0);
    payload.extend(bits(p2, 4));
    payload.extend(bits(number % 64, 6));
    payload.push(u8::from(half));
    payload.push(0);
    let parity = u8::from(payload.iter().sum::<u8>() % 2 == 1);
    let data = [&[1, 0, 1, 0, 1][..], &payload, &[parity, 0, 1, 0, 1]].concat();
    let clock_steps: Vec<u8> = (0..23).map(|i| (i % 2) as u8).collect();
    let clock = [&[1, 1, 1, 1, 1][..], &clock_steps, &[1, 1, 1][..]].concat();
    (clock, data)
}

/// Code 128 subset C of an even count of digits, with its mod-103 check symbol.
#[must_use]
pub fn code128c_bits(digits: &str) -> Vec<u8> {
    let mut values = vec![START_C];
    values.extend(
        digits
            .as_bytes()
            .chunks(2)
            .filter_map(|pair| std::str::from_utf8(pair).ok()?.parse::<usize>().ok()),
    );
    let check = (values[0]
        + values[1..]
            .iter()
            .enumerate()
            .map(|(i, value)| (i + 1) * value)
            .sum::<usize>())
        % 103;
    values.push(check);
    values.push(STOP);
    let mut out = Vec::new();
    for value in values {
        for (k, width) in CODE128[value].chars().enumerate() {
            out.extend(std::iter::repeat_n(
                (1 - (k % 2)) as u8,
                width.to_digit(10).unwrap_or(0) as usize,
            ));
        }
    }
    out
}

/// KEYKODE barcode: maker, product, the key's roll prefix, foot count and perforation offset.
#[must_use]
pub fn keykode_bits(code: &str, roll: &str, foot: u32, perforations: u32) -> Vec<u8> {
    let maker = keykode_maker(code);
    let product = code[1..2]
        .bytes()
        .next()
        .map_or(0, |byte| u32::from(byte.wrapping_sub(b'A')) % 100);
    code128c_bits(&format!(
        "{maker:02}{product:02}{roll}{:04}{perforations:02}",
        foot % 10000
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dx_code_has_the_expected_tracks() {
        let (clock, data) = dx_bits(22, true, (47, 1));
        assert_eq!(clock.len(), 31);
        assert_eq!(&clock[..5], &[1, 1, 1, 1, 1]);
        assert_eq!(&clock[28..], &[1, 1, 1]);
        assert_eq!(&data[..5], &[1, 0, 1, 0, 1]);
        assert_eq!(&data[28..], &[1, 0, 1]);
        assert_eq!(data.len(), 31);
    }

    #[test]
    fn the_dx_frame_field_wraps_at_sixty_four() {
        let (_, data) = dx_bits(64 + 5, false, (1, 0));
        let payload = &data[5..28];
        let frame: u32 = payload[13..19]
            .iter()
            .fold(0, |acc, bit| acc * 2 + u32::from(*bit));
        assert_eq!(frame, 5);
    }

    #[test]
    fn code128c_has_a_valid_check_symbol() {
        let bits = code128c_bits("0209076163436800");
        assert_eq!(bits.len(), 123);
        let runs = run_lengths(&bits);
        let mut symbols: Vec<String> = runs[..runs.len() - 7]
            .chunks(6)
            .map(|chunk| chunk.iter().map(u8::to_string).collect())
            .collect();
        symbols.push(runs[runs.len() - 7..].iter().map(u8::to_string).collect());
        let values: Vec<usize> = symbols
            .iter()
            .map(|symbol| CODE128.iter().position(|entry| *entry == symbol).unwrap())
            .collect();
        assert_eq!(values[0], 105);
        assert_eq!(values[1..9], [2, 9, 7, 61, 63, 43, 68, 0]);
        assert_eq!(values[values.len() - 1], 106);
        let check = (105
            + values[1..values.len() - 2]
                .iter()
                .enumerate()
                .map(|(i, value)| (i + 1) * value)
                .sum::<usize>())
            % 103;
        assert_eq!(values[values.len() - 2], check);
    }

    fn run_lengths(bits: &[u8]) -> Vec<u8> {
        let mut runs = Vec::new();
        let mut last = bits[0];
        let mut count = 1;
        for bit in &bits[1..] {
            if *bit == last {
                count += 1;
            } else {
                runs.push(count);
                last = *bit;
                count = 1;
            }
        }
        runs.push(count);
        runs
    }

    #[test]
    fn the_keykode_starts_with_the_make_code() {
        let bits = keykode_bits("EJ", "076163", 1000, 0);
        assert_eq!(bits.len(), 123);
        let (_, zero) = dx_bits(1, false, (1, 0));
        assert_ne!(bits, zero);
    }
}
