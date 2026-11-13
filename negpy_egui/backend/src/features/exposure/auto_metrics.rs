//! The measured values the Auto Density and Auto Grade read-outs consume.

use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::processor::NeutralAxisRefs;

/// The measured values the auto sliders read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AutoSliderMetrics {
    pub metered_anchor: Option<f64>,
    pub textural_range: Option<f64>,
    pub norm_density_range: Option<f64>,
    pub shadow_point: Option<f64>,
    pub highlight_point: Option<f64>,
    pub bounds: Option<LogNegativeBounds>,
    pub neutral_axis: Option<NeutralAxisRefs>,
}
