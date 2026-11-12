//! Ports of the B&W collapse assertions in `negpy/features/exposure/bw_filter.py`.

use negpy_backend::domain::models::{BwFilter, ExposureConfig, ProcessConfig, ProcessMode};
use negpy_backend::features::exposure::bw_filter::{
    BW_FILTER_WEIGHTS, collapse_matrix, collapse_weights, filter_weights, is_mono, plane_weights,
    printed_exposure,
};

fn process(mode: ProcessMode, source_color: Option<bool>) -> ProcessConfig {
    ProcessConfig {
        process_mode: mode,
        source_color,
        ..ProcessConfig::default()
    }
}

#[test]
fn every_weights_row_sums_to_one() {
    for (filter, weights) in BW_FILTER_WEIGHTS {
        let sum: f32 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-6,
            "{filter} weights sum to {sum}, not 1"
        );
        assert_eq!(filter_weights(filter), weights);
    }
}

#[test]
fn a_filter_acts_only_on_a_color_bw_source() {
    let exposure = ExposureConfig {
        bw_filter: BwFilter::Orange21,
        ..ExposureConfig::default()
    };
    let bw_color = process(ProcessMode::BW, Some(true));
    assert_eq!(collapse_weights(&bw_color, &exposure), [0.72, 0.32, -0.04]);

    for neutral in [
        process(ProcessMode::BW, Some(false)),
        process(ProcessMode::BW, None),
        process(ProcessMode::C41, Some(true)),
    ] {
        assert_eq!(
            collapse_weights(&neutral, &exposure),
            filter_weights(BwFilter::None)
        );
    }
}

#[test]
fn a_mono_slide_takes_the_filter() {
    let slide = ProcessConfig {
        process_mode: ProcessMode::E6,
        slide_bw: true,
        source_color: Some(true),
        ..ProcessConfig::default()
    };
    let exposure = ExposureConfig {
        bw_filter: BwFilter::Red25,
        ..ExposureConfig::default()
    };
    assert!(is_mono(&slide));
    assert_eq!(collapse_weights(&slide, &exposure), [1.06, -0.06, 0.0]);
}

#[test]
fn the_collapse_matrix_repeats_the_weights() {
    assert_eq!(collapse_matrix([0.31, 0.39, 0.30]), [[0.31, 0.39, 0.30]; 3],);
}

#[test]
fn only_a_bw_mode_collapses_a_plane() {
    let exposure = ExposureConfig::default();
    assert_eq!(
        plane_weights(&process(ProcessMode::BW, Some(true)), &exposure),
        Some([0.31, 0.39, 0.30])
    );
    assert_eq!(
        plane_weights(&process(ProcessMode::C41, None), &exposure),
        None
    );
}

#[test]
fn a_bw_print_drops_color_filtration() {
    let exposure = ExposureConfig {
        wb_cyan: 0.1,
        wb_magenta: 0.2,
        wb_yellow: 0.3,
        shadow_cyan: 0.4,
        shadow_magenta: 0.5,
        shadow_yellow: 0.6,
        highlight_cyan: 0.7,
        highlight_magenta: 0.8,
        highlight_yellow: 0.9,
        density: 1.2,
        ..ExposureConfig::default()
    };
    let printed = printed_exposure(&process(ProcessMode::BW, Some(true)), &exposure);
    assert_eq!(
        [
            printed.wb_cyan,
            printed.wb_magenta,
            printed.wb_yellow,
            printed.shadow_cyan,
            printed.shadow_magenta,
            printed.shadow_yellow,
            printed.highlight_cyan,
            printed.highlight_magenta,
            printed.highlight_yellow,
        ],
        [0.0; 9]
    );
    assert_eq!(printed.density, 1.2);

    let color = printed_exposure(&process(ProcessMode::C41, Some(true)), &exposure);
    assert_eq!(color.wb_yellow, 0.3);
    assert_eq!(color.highlight_yellow, 0.9);
}
