//! Ports tests/test_densitometer.py and the stats read-out cases.

use negpy_backend::features::exposure::analysis::{
    encoded_of_zone, mid_gray_encoded, output_clip_fractions, zone_of_encoded,
};
use negpy_backend::features::exposure::densitometer::{
    compute_reading, format_reading, map_display_to_norm, zone_roman,
};
use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::features::exposure::stats::{clipping_row, negative_statistics};

const BOUNDS: LogNegativeBounds = LogNegativeBounds {
    floors: [0.0, 0.0, 0.0],
    ceils: [1.2, 1.0, 0.8],
};

#[test]
fn identity_mapping_without_border_or_roi() {
    assert_eq!(
        map_display_to_norm(0.5, 0.5, 100, 80, None, None, false, 100, 80),
        Some((50, 40))
    );
}

#[test]
fn border_inset_and_outside_are_handled() {
    let rect = (50.0, 40.0, 100.0, 80.0);
    assert_eq!(
        map_display_to_norm(0.5, 0.5, 200, 160, Some(rect), None, false, 100, 80),
        Some((50, 40))
    );
    assert_eq!(
        map_display_to_norm(0.1, 0.1, 200, 160, Some(rect), None, false, 100, 80),
        None
    );
}

#[test]
fn roi_offset_and_crop_full() {
    let roi = (10.0, 50.0, 20.0, 60.0);
    assert_eq!(
        map_display_to_norm(0.5, 0.5, 40, 40, None, Some(roi), false, 100, 80),
        Some((40, 30))
    );
    assert_eq!(
        map_display_to_norm(0.5, 0.5, 100, 80, None, Some(roi), true, 100, 80),
        Some((50, 40))
    );
}

#[test]
fn mapping_clamps_to_the_frame() {
    assert_eq!(
        map_display_to_norm(1.0, 1.0, 100, 80, None, None, false, 100, 80),
        Some((99, 79))
    );
}

#[test]
fn delta_density_scales_with_bounds() {
    let reading = compute_reading([0.5, 0.5, 0.5], BOUNDS, [0.5, 0.5, 0.5]);
    for (got, want) in reading.dd_rgb.iter().zip([0.6, 0.5, 0.4]) {
        assert!((got - want).abs() < 1e-9, "{got} != {want}");
    }
    assert!((reading.val_luma - 0.5).abs() < 1e-6);
}

#[test]
fn delta_density_reads_up_from_the_thinnest_tone() {
    let thin = compute_reading([1.0, 1.0, 1.0], BOUNDS, [0.5, 0.5, 0.5]);
    let dense = compute_reading([0.0, 0.0, 0.0], BOUNDS, [0.5, 0.5, 0.5]);
    assert_eq!(thin.dd_rgb, [0.0, 0.0, 0.0]);
    assert_eq!(dense.dd_rgb, [1.2, 1.0, 0.8]);
}

#[test]
fn a_positive_reads_the_same_way() {
    let bounds = LogNegativeBounds {
        floors: [1.2, 1.0, 0.8],
        ceils: [0.0, 0.0, 0.0],
    };
    let thin = compute_reading([0.0, 0.0, 0.0], bounds, [0.5, 0.5, 0.5]);
    let dense = compute_reading([1.0, 1.0, 1.0], bounds, [0.5, 0.5, 0.5]);
    assert_eq!(thin.dd_rgb, [0.0, 0.0, 0.0]);
    assert_eq!(dense.dd_rgb, [1.2, 1.0, 0.8]);
}

#[test]
fn one_crossed_channel_keeps_the_window_orientation() {
    let bounds = LogNegativeBounds {
        floors: [0.0, 0.0, 0.0],
        ceils: [1.2, -0.1, 0.8],
    };
    let reading = compute_reading([1.0, 1.0, 1.0], bounds, [0.5, 0.5, 0.5]);
    assert_eq!(reading.dd_rgb, [0.0, 0.0, 0.0]);
}

#[test]
fn mid_gray_reads_zone_five() {
    let encoded = mid_gray_encoded();
    assert!((zone_of_encoded(encoded) - 5.0).abs() < 1e-5);
    assert!((encoded_of_zone(5.0) - encoded).abs() < 1e-6);
}

#[test]
fn zone_roman_uses_thirds() {
    assert_eq!(zone_roman(5.0), "V");
    assert_eq!(zone_roman(4.33), "IV⅓");
    assert_eq!(zone_roman(7.66), "VII⅔");
    assert_eq!(zone_roman(10.0), "X");
    assert_eq!(zone_roman(-1.0), "0");
}

#[test]
fn the_one_line_reading_matches_python() {
    let reading = negpy_backend::features::exposure::densitometer::DensitometerReading {
        dd_rgb: [0.62, 0.85, 1.04],
        val_luma: 0.7,
        print_density: 1.32,
        zone: 4.33,
        val_rgb: None,
    };
    assert_eq!(format_reading(&reading), "ΔD 0.62·0.85·1.04 · D 1.32 · IV⅓");
}

#[test]
fn clipping_rows_name_the_worst_channel() {
    let row = clipping_row(Some(0.02), Some(0.005), true);
    assert_eq!(row.value, "2.0 · 0.5%");
    assert!(row.warn);
    assert_eq!(clipping_row(None, Some(0.1), false).value, "—");
}

#[test]
fn the_negative_readout_keeps_every_row() {
    let rows = negative_statistics(
        Some(0.9),
        Some(0.5),
        Some(0.0),
        Some(0.0),
        Some([0.0, 0.0, 0.0]),
        None,
        None,
        Some([0.2, 0.7, 1.4]),
        "read",
    );
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].name, "Negative");
    assert!(rows[0].value.starts_with("Δ0.90"));
    assert_eq!(rows[1].name, "Exposure");
}

#[test]
fn output_clip_fractions_use_each_channels_own_total() {
    let mut bins = vec![0.0; 3 * 256];
    bins[0] = 2.0;
    bins[1] = 8.0;
    bins[256 + 255] = 10.0;
    let (low, high) = output_clip_fractions(&bins, 256);
    assert!((low - 0.2).abs() < 1e-9);
    assert!((high - 1.0).abs() < 1e-9);
}
