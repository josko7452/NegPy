//! Mirrors `negpy/features/exposure/bw_filter.py`: the B&W render's channel collapse.

use crate::domain::models::{BwFilter, ExposureConfig, ProcessConfig, ProcessMode};

/// R, G, B weights on the normalized log channels of a color negative. Each row sums to
/// exactly 1, so a gray does not move.
pub const BW_FILTER_WEIGHTS: [(BwFilter, [f32; 3]); 7] = [
    (BwFilter::None, [0.31, 0.39, 0.30]),
    (BwFilter::Yellow8, [0.47, 0.57, -0.04]),
    (BwFilter::DeepYellow15, [0.56, 0.50, -0.06]),
    (BwFilter::Orange21, [0.72, 0.32, -0.04]),
    (BwFilter::Red25, [1.06, -0.06, 0.00]),
    (BwFilter::YellowGreen11, [0.25, 0.78, -0.03]),
    (BwFilter::Green58, [0.04, 1.08, -0.12]),
];

/// The weights of one Wratten filter.
#[must_use]
pub fn filter_weights(filter: BwFilter) -> [f32; 3] {
    BW_FILTER_WEIGHTS
        .iter()
        .find_map(|(candidate, weights)| (*candidate == filter).then_some(*weights))
        .expect("every BwFilter variant has a row")
}

/// The B&W render's channel weights. A filter acts only on a color source, where the UI
/// shows it.
#[must_use]
pub fn collapse_weights(process: &ProcessConfig, exposure: &ExposureConfig) -> [f32; 3] {
    if is_mono(process) && process.source_color == Some(true) {
        filter_weights(exposure.bw_filter)
    } else {
        filter_weights(BwFilter::None)
    }
}

/// 3x3 with every row equal to `weights`: a matrix product then gives three equal
/// channels.
#[must_use]
pub fn collapse_matrix(weights: [f32; 3]) -> [[f32; 3]; 3] {
    [weights; 3]
}

/// The weights the mask and diffusion planes collapse with, or None outside B&W.
#[must_use]
pub fn plane_weights(process: &ProcessConfig, exposure: &ExposureConfig) -> Option<[f32; 3]> {
    (process.process_mode == ProcessMode::BW).then(|| collapse_weights(process, exposure))
}

/// The exposure the print renders. B&W drops the color filtration, which the Filtration
/// card hides there.
#[must_use]
pub fn printed_exposure(process: &ProcessConfig, exposure: &ExposureConfig) -> ExposureConfig {
    let mut printed = exposure.clone();
    if process.process_mode == ProcessMode::BW {
        printed.wb_cyan = 0.0;
        printed.wb_magenta = 0.0;
        printed.wb_yellow = 0.0;
        printed.shadow_cyan = 0.0;
        printed.shadow_magenta = 0.0;
        printed.shadow_yellow = 0.0;
        printed.highlight_cyan = 0.0;
        printed.highlight_magenta = 0.0;
        printed.highlight_yellow = 0.0;
    }
    printed
}

/// The frame renders in black and white: a B&W negative, or a slide with B&W on.
#[must_use]
pub fn is_mono(process: &ProcessConfig) -> bool {
    process.process_mode == ProcessMode::BW
        || (process.process_mode == ProcessMode::E6 && process.slide_bw)
}
