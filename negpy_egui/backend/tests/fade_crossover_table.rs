//! Ports the table-level rules of `tests/test_fade_crossover.py` and `test_fade_neutrals.py`.

use negpy_backend::features::process::fade::{
    CROSSOVER_WEIGHTS, cast_name, crossover_amount, crossover_bands, crossover_casts,
    crossover_reference, crossover_zero, neutral_spread, shape_cubic,
};

fn table() -> Vec<f64> {
    vec![
        0.5, 0.1, 0.20, 0.04, 0.00, -0.04, 0.4, 0.30, 0.02, 0.01, -0.03, 0.7, 0.40, -0.02, 0.02,
        0.00, 1.0, 0.20, -0.10, -0.10, 0.20,
    ]
}

#[test]
fn the_bands_are_five_values_each_and_sum_to_zero_under_the_weights() {
    let rows = crossover_bands(&table());
    assert_eq!(rows.len(), 4);
    for row in &rows {
        let sum: f64 = row[2..]
            .iter()
            .zip(CROSSOVER_WEIGHTS)
            .map(|(value, weight)| value * weight)
            .sum();
        assert!(sum.abs() < 1e-9);
    }
    assert_eq!(crossover_zero(&table()), 0.5);
}

#[test]
fn the_reference_is_interpolated_at_the_zero() {
    let reference = crossover_reference(&table());
    assert!((reference[0] - 0.01).abs() < 0.01);
    assert!(crossover_amount(Some(&table())) > 0.0);
    assert!(crossover_amount(None) == 0.0);
}

#[test]
fn an_identity_table_has_no_crossover_and_no_cast() {
    let flat = vec![
        0.5, 0.1, 0.1, 0.01, 0.01, 0.01, 0.4, 0.3, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0,
    ];
    assert_eq!(crossover_amount(Some(&flat)), 0.0);
    let (shadow, highlight) = crossover_casts(Some(&flat));
    assert!(shadow.is_some_and(|lean| lean.iter().all(|value| value.abs() < 0.01)));
    assert!(highlight.is_some_and(|lean| lean.iter().all(|value| value.abs() < 0.01)));
    assert!(neutral_spread(&flat).abs() < 1e-9);
}

#[test]
fn the_shadow_and_highlight_casts_keep_their_hue() {
    let (shadow, highlight) = crossover_casts(Some(&table()));
    assert!(shadow.is_some() && highlight.is_some());
    assert!(cast_name(shadow).is_some());
    assert!(cast_name(highlight).is_some());
    assert_eq!(cast_name(None), None);
    assert_eq!(cast_name(Some([0.01, 0.01, 0.01])), None);
}

#[test]
fn a_monotone_cubic_is_flat_beyond_its_ends() {
    let xs = [0.0, 1.0, 2.0];
    let ys = [0.0, 1.0, 2.0];
    assert_eq!(shape_cubic(&xs, &ys, -1.0), 0.0);
    assert_eq!(shape_cubic(&xs, &ys, 3.0), 2.0);
    assert!((shape_cubic(&xs, &ys, 1.0) - 1.0).abs() < 1e-9);
}

#[test]
fn a_flat_table_has_a_zero_spread() {
    let flat = vec![0.0, 0.1, 0.0, 0.01, 0.01, 0.01, 1.0, 0.0, 0.01, 0.01, 0.01];
    assert_eq!(neutral_spread(&flat), 0.0);
}
