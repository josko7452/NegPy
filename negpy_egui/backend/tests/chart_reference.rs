//! The chart's published colors and the CIEDE2000 oracle.

use negpy_backend::features::process::chart_ref::{
    CHARTS, COLORCHECKER_24, SPYDERCHECKR_24, delta_e_color, delta_e2000, delta_e2000_with_kl,
    reference_lab,
};

const SHARMA: [([f64; 3], [f64; 3], f64); 4] = [
    ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
    ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
    ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
    (
        [60.2574, -34.0099, 36.2677],
        [60.4626, -34.1751, 39.4387],
        1.2644,
    ),
];

#[test]
fn delta_e2000_matches_the_published_pairs() {
    for (lab1, lab2, expected) in SHARMA {
        let value = delta_e2000(&[lab1], &[lab2])[0];
        assert!(
            (value - expected).abs() < 1e-4,
            "{lab1:?} {lab2:?}: {value}"
        );
    }
}

#[test]
fn the_lightness_weight_scales_only_the_lightness_term() {
    let lab1 = [[50.0, 20.0, -10.0]];
    let lab2 = [[70.0, 20.0, -10.0]];
    assert!(delta_e2000_with_kl(&lab1, &lab2, 2.0)[0] < delta_e2000(&lab1, &lab2)[0]);
}

#[test]
fn color_only_delta_e_leaves_lightness_out() {
    let reference = [[50.0, 20.0, -10.0], [50.0, 20.0, -10.0]];
    let lab = [[70.0, 20.0, -10.0], [80.0, 26.0, -4.0]];
    let values = delta_e_color(&reference, &lab);
    assert_eq!(values[0], 0.0);
    assert!((values[1] - delta_e2000(&[reference[1]], &[[50.0, 26.0, -4.0]])[0]).abs() < 1e-12);
}

#[test]
fn reference_lab_keeps_a_neutral_neutral() {
    const COLORS: [[f64; 3]; 24] = [[50.0, 0.0, 0.0]; 24];
    const NAMES: [&str; 24] = ["g"; 24];
    let spec = negpy_backend::features::process::chart_ref::ChartSpec {
        name: "gray",
        patch_names: &NAMES,
        lab_d50: &COLORS,
        exposure_patches: &[0],
    };
    assert!((reference_lab(&spec)[0][0] - 50.0).abs() < 0.05);
    assert!(reference_lab(&spec)[0][1].abs() < 0.05);
    assert!(reference_lab(&spec)[0][2].abs() < 0.05);
}

#[test]
fn charts_are_complete() {
    for chart in CHARTS {
        assert_eq!(chart.patch_names.len(), chart.lab_d50.len());
        assert_eq!(chart.lab_d50.len(), 24);
        for &index in chart.exposure_patches {
            let [_, a, b] = chart.lab_d50[index];
            assert!(a.abs() < 2.5 && b.abs() < 3.0);
        }
    }
    assert_eq!(SPYDERCHECKR_24.name, "SpyderCheckr 24");
    assert_eq!(COLORCHECKER_24.name, "ColorChecker 24");
}
