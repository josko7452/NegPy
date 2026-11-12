//! Ports of the bundled print-LUT assertions in `tests/test_print_lut.py`.

use negpy_backend::features::exposure::lut::parse;

const FRONTIER: &[u8] = include_bytes!("../../../negpy/features/exposure/print_luts/frontier.npz");
const E4070: &[u8] = include_bytes!("../../../negpy/features/exposure/print_luts/kodak_e4070.npz");
const PAKON: &[u8] = include_bytes!("../../../negpy/features/exposure/print_luts/pakon.npz");

#[test]
fn a_bundled_lut_has_the_baked_shape() {
    let lut = parse("frontier", FRONTIER).unwrap();
    assert_eq!(lut.n, 33);
    assert_eq!(lut.table.len(), 33 * 33 * 33 * 3);
    assert!(lut.e_max.iter().all(|value| *value > 0.0));
    assert!(lut.table.iter().all(|value| value.is_finite()));
    assert!(lut.source.contains("Frontier"));
}

#[test]
fn a_spectral_lut_carries_its_flat_matrix() {
    let lut = parse("kodak_e4070", E4070).unwrap();
    let rows = lut.flat_rows.unwrap();
    for row in rows {
        let sum: f64 = row.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }
}

#[test]
fn every_bundled_lut_loads() {
    let cases: [(&str, &[u8]); 3] = [
        ("frontier", FRONTIER),
        ("kodak_e4070", E4070),
        ("pakon", PAKON),
    ];
    for (key, bytes) in cases {
        let lut = parse(key, bytes).unwrap();
        assert_eq!(lut.key, key);
        assert_eq!(lut.n, 33);
    }
}

#[test]
fn a_missing_entry_is_an_error() {
    assert!(parse("empty", b"not a zip").is_err());
}
