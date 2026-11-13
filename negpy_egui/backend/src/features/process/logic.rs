//! Process-path decisions that both engines and the decode must share.

use crate::domain::models::{ProcessConfig, ProcessMode};
use crate::features::process::path::{RenderPath, render_path};

/// Highlight levels libraw accepts; `1` is never a control level.
pub const VALID_HIGHLIGHT_LEVELS: [i64; 8] = [0, 2, 3, 4, 5, 6, 7, 8];

/// The highlight mode the decode should request.
#[must_use]
pub fn effective_highlight_reconstruction(process: &ProcessConfig) -> i64 {
    if process.process_mode != ProcessMode::E6 {
        return 0;
    }
    if VALID_HIGHLIGHT_LEVELS.contains(&process.highlight_reconstruction) {
        process.highlight_reconstruction
    } else {
        0
    }
}

/// Reconstruction-level identity for a cache key.
#[must_use]
pub fn highlight_reconstruction_token(process: &ProcessConfig) -> String {
    format!("|hr:{}", effective_highlight_reconstruction(process))
}

/// Whether reconstruction decodes with the real white balance baked in.
#[must_use]
pub fn highlight_reconstruction_bakes_wb(process: &ProcessConfig) -> bool {
    if process.linear_raw
        || process.positive_source
        || effective_highlight_reconstruction(process) == 0
    {
        return false;
    }
    render_path(process) != RenderPath::Print
}

/// Cache-key identity for the white-balance decision.
#[must_use]
pub fn highlight_reconstruction_bakes_wb_token(process: &ProcessConfig) -> String {
    format!(
        "|hrwb:{}",
        u8::from(highlight_reconstruction_bakes_wb(process))
    )
}

/// The rawpy `bright` value that offsets libraw's highlight-mode scaling.
#[must_use]
pub fn highlight_reconstruction_bright_gain(wb: Option<&[f64]>, highlight_mode: i64) -> f64 {
    if highlight_mode == 0 {
        return 1.0;
    }
    let Some(wb) = wb.filter(|wb| wb.len() >= 3) else {
        return 1.0;
    };
    let values = [wb[0], wb[1], wb[2]];
    if !values.iter().all(|value| value.is_finite() && *value > 0.0) {
        return 1.0;
    }
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        / values.iter().copied().fold(f64::INFINITY, f64::min)
}
