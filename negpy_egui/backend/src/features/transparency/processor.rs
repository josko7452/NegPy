//! The slide render: normalized log density through the transfer curve.
//!
//! Mirrors `negpy/features/transparency/processor.py`.

use crate::domain::models::{ExposureConfig, ProcessConfig};
use crate::features::exposure::auto_metrics::AutoSliderMetrics;
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::filtration_offsets;
use crate::features::transparency::logic::{
    TransferSettings, apply_transfer_curve, contrast_free_cmy, transfer_auto_terms,
    transfer_cast_affine, transfer_curve_params, transfer_shape, transfer_widths,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The values a slide's transfer stage reads from the base meters.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransferContext {
    pub metrics: AutoSliderMetrics,
    pub positive_source: bool,
    pub white_target: Option<f64>,
    pub mono_weights: Option<[f32; 3]>,
}

/// Render a normalized log frame through the transfer curve.
#[must_use]
pub fn process(
    image: &ImageBuffer,
    exposure: &ExposureConfig,
    _process_config: &ProcessConfig,
    context: TransferContext,
) -> ImageBuffer {
    let (manual_offset, manual_contrast) = transfer_curve_params(exposure);
    let (offset, contrast, highlight_auto, shadow_auto) = transfer_auto_terms(
        exposure,
        manual_offset,
        manual_contrast,
        context.metrics.textural_range,
        context.metrics.metered_anchor,
        context.metrics.shadow_point,
        context.metrics.highlight_point,
    );
    let cmy = contrast_free_cmy(
        filtration_offsets(
            [exposure.wb_cyan, exposure.wb_magenta, exposure.wb_yellow],
            context.metrics.bounds,
        ),
        contrast,
    );
    let cmy_max = c::CMY_MAX_DENSITY;
    let shadow_cmy = [
        exposure.shadow_cyan * cmy_max,
        exposure.shadow_magenta * cmy_max,
        exposure.shadow_yellow * cmy_max,
    ];
    let highlight_cmy = [
        exposure.highlight_cyan * cmy_max,
        exposure.highlight_magenta * cmy_max,
        exposure.highlight_yellow * cmy_max,
    ];
    let (cast_gain, cast_offset) = context
        .metrics
        .bounds
        .map_or(([1.0; 3], [0.0; 3]), |bounds| {
            transfer_cast_affine(
                bounds,
                context.metrics.neutral_axis,
                exposure.cast_removal_strength,
                context.white_target,
            )
        });
    let cast_offset = cast_offset
        .map(|value| value * crate::features::transparency::logic::TRANSFER_DENSITY_RANGE);
    let (toe_width, shoulder_width) = transfer_widths(exposure);
    let (toe, shoulder) = transfer_shape(exposure);
    let settings = TransferSettings {
        cast_gain: cast_gain.map(|value| value as f32),
        cast_offset: cast_offset.map(|value| value as f32),
        exposure_offset: offset as f32,
        cmy: cmy.map(|value| value as f32),
        contrast: contrast as f32,
        toe: toe.map(|value| value as f32),
        shoulder: shoulder.map(|value| value as f32),
        toe_width: toe_width.map(|value| value as f32),
        shoulder_width: shoulder_width.map(|value| value as f32),
        shadow_density: (exposure.shadow_density + shadow_auto) as f32,
        highlight_density: (exposure.highlight_density + highlight_auto) as f32,
        shadow_cmy: shadow_cmy.map(|value| value as f32),
        highlight_cmy: highlight_cmy.map(|value| value as f32),
        separation: exposure.dye_separation,
        separation_trims: [
            exposure.dye_separation_trim_red,
            exposure.dye_separation_trim_green,
            exposure.dye_separation_trim_blue,
        ],
        damping: exposure.separation_damping,
        positive_source: context.positive_source,
        ..TransferSettings::default()
    };
    let mut input = image.clone();
    if let Some(weights) = context.mono_weights {
        let matrix = crate::features::exposure::bw_filter::collapse_matrix(weights);
        let mut out = vec![0.0; input.pixels.len()];
        negpy_core::analysis::unmix_log(&input.pixels, matrix, &mut out);
        input.pixels = out;
    }
    let scene = apply_transfer_curve(&input.pixels, &settings);
    let pixels = if context.positive_source {
        scene
    } else {
        crate::features::transparency::logic::display_rendering(&scene)
    };
    if context.mono_weights.is_some() {
        let mut luma = vec![0.0; input.width * input.height];
        negpy_core::analysis::luminance(&pixels, &mut luma);
        let gray: Vec<f32> = luma.iter().flat_map(|value| [*value; 3]).collect();
        return ImageBuffer::from_rgb(input.width, input.height, gray);
    }
    ImageBuffer::from_rgb(input.width, input.height, pixels)
}
