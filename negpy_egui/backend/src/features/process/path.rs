//! Which renderer a frame goes through.
//!
//! Mirrors `negpy/features/process/path.py`: every CPU, GPU, decode and UI caller asks
//! this module so they cannot disagree.

use crate::domain::models::{ProcessConfig, ProcessMode, SourceEncoding};

/// The renderer a frame goes through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderPath {
    #[default]
    Print,
    Transfer,
    Positive,
}

/// The renderer for `process`: a negative prints, a slide transfers, and a Positive
/// slide decodes its finished positive with no print rendering.
#[must_use]
pub fn render_path(process: &ProcessConfig) -> RenderPath {
    if process.process_mode != ProcessMode::E6 {
        RenderPath::Print
    } else if process.positive_source {
        RenderPath::Positive
    } else {
        RenderPath::Transfer
    }
}

/// The source encoding a Positive TIFF decodes by; `None` off Positive.
#[must_use]
pub fn positive_decode(process: &ProcessConfig) -> Option<SourceEncoding> {
    (render_path(process) == RenderPath::Positive).then_some(process.source_encoding)
}

/// The frame renders in black and white: a B&W negative, or a Slide with B&W on.
#[must_use]
pub fn is_mono(process: &ProcessConfig) -> bool {
    process.process_mode == ProcessMode::BW
        || (process.process_mode == ProcessMode::E6 && process.slide_bw)
}

/// Whether the decode skips the camera's as-shot white balance.
///
/// True when Linear RAW is on, and always on an as-captured Slide: its render applies
/// the camera matrix, which folds the multipliers back in. A Positive decodes by its
/// Source Encoding instead.
#[must_use]
pub fn effective_linear_raw(process: &ProcessConfig) -> bool {
    process.linear_raw || render_path(process) == RenderPath::Transfer
}

/// Decode-mode identity for a cache key.
///
/// Keyed on the effective Linear RAW value, because the transfer path decodes without
/// white balance whatever the stored flag says.
#[must_use]
pub fn linear_raw_token(process: &ProcessConfig) -> String {
    let mut token = format!("|lr:{}", u8::from(effective_linear_raw(process)));
    if let Some(decode) = positive_decode(process) {
        token.push_str(&format!("|pd:{}", decode.as_str()));
    }
    token
}

/// CFA-interpolation identity for a cache key.
#[must_use]
pub fn demosaic_token(mode: &str) -> String {
    format!("|dm:{mode}")
}
