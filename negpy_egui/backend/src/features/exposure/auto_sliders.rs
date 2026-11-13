//! Auto Density and Auto Grade as manual-equivalent slider values.

use crate::domain::models::{ExposureConfig, ProcessMode};
use crate::features::exposure::auto_metrics::AutoSliderMetrics;
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{
    auto_base_slope, compute_pivot, curve_params_from_metrics, effective_grade_range,
    effective_midtone_gamma, grade_to_slope, highlight_hold_offset, shadow_hold_offset,
    slope_to_grade,
};
use crate::features::exposure::papers::{PaperProfile, effective_paper_profile, slope_gain};

fn paper(exposure: &ExposureConfig, mode: Option<ProcessMode>) -> (&'static PaperProfile, f64) {
    let profile = effective_paper_profile(&exposure.paper_profile, mode);
    let d_min = if exposure.paper_dmin {
        profile.tonal.d_min
    } else {
        0.0
    };
    (profile, d_min)
}

fn density_shift(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> Option<f64> {
    let anchor = metrics.metered_anchor?;
    if !exposure.auto_exposure {
        return None;
    }
    let (profile, d_min) = paper(exposure, mode);
    let mg = effective_midtone_gamma(Some(profile), exposure.midtone_gamma);
    let slope = curve_params_from_metrics(exposure, mode, metrics).0[1];
    let metered = compute_pivot(slope, 1.0, d_min, Some(anchor), Some(profile), Some(mg));
    let reference = compute_pivot(slope, 1.0, d_min, None, Some(profile), Some(mg));
    Some((metered - reference) / c::DENSITY_MULTIPLIER)
}

fn auto_slope(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> Option<f64> {
    let (lum_range, textural) = (metrics.norm_density_range?, metrics.textural_range?);
    if !exposure.auto_normalize_contrast {
        return None;
    }
    let (profile, d_min) = paper(exposure, mode);
    let anchor = exposure
        .auto_exposure
        .then_some(metrics.metered_anchor)
        .flatten();
    Some(auto_base_slope(
        exposure.grade,
        true,
        Some(lum_range),
        Some(textural),
        metrics.shadow_point,
        anchor,
        d_min,
        Some(profile),
        Some(effective_midtone_gamma(
            Some(profile),
            exposure.midtone_gamma,
        )),
    ))
}

fn highlight_auto(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> Option<f64> {
    let point = metrics.highlight_point?;
    if !exposure.auto_normalize_contrast {
        return None;
    }
    let (profile, d_min) = paper(exposure, mode);
    let mg = effective_midtone_gamma(Some(profile), exposure.midtone_gamma);
    let (slopes, pivots, _) = curve_params_from_metrics(exposure, mode, metrics);
    Some(highlight_hold_offset(
        slopes[1],
        pivots[1],
        point,
        d_min,
        Some(profile),
        exposure.preflash,
        exposure.grade,
        Some(mg),
    ))
}

fn shadow_auto(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> Option<f64> {
    let point = metrics.shadow_point?;
    if !exposure.auto_normalize_contrast {
        return None;
    }
    let (profile, d_min) = paper(exposure, mode);
    let mg = effective_midtone_gamma(Some(profile), exposure.midtone_gamma);
    let (slopes, pivots, _) = curve_params_from_metrics(exposure, mode, metrics);
    Some(shadow_hold_offset(
        slopes[1],
        pivots[1],
        point,
        d_min,
        Some(profile),
        Some(mg),
    ))
}

/// The manual-equivalent value of every slider an active auto drives.
#[must_use]
pub fn print_shown_values(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> Vec<(String, f64)> {
    let mut shown = Vec::new();
    if let Some(shift) = density_shift(exposure, mode, metrics) {
        shown.push(("density".to_string(), exposure.density - shift));
    }
    if let Some(slope) = auto_slope(exposure, mode, metrics)
        && let Some(range) = metrics.norm_density_range
    {
        let (profile, _) = paper(exposure, mode);
        shown.push((
            "grade".to_string(),
            slope_to_grade(slope, Some(range), slope_gain(Some(profile))),
        ));
    }
    if let Some(value) = highlight_auto(exposure, mode, metrics) {
        shown.push((
            "highlight_density".to_string(),
            exposure.highlight_density + value,
        ));
    }
    if let Some(value) = shadow_auto(exposure, mode, metrics) {
        shown.push((
            "shadow_density".to_string(),
            exposure.shadow_density + value,
        ));
    }
    shown
}

/// Inverse of [`print_shown_values`] for one field.
#[must_use]
pub fn print_stored_value(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
    field: &str,
    shown: f64,
) -> f64 {
    match field {
        "density" => density_shift(exposure, mode, metrics).map_or(shown, |shift| shown + shift),
        "grade" => {
            let Some(range) = metrics.norm_density_range else {
                return shown;
            };
            if auto_slope(exposure, mode, metrics).is_none() {
                return shown;
            }
            let r_eff = effective_grade_range(true, Some(range), metrics.textural_range);
            let (profile, _) = paper(exposure, mode);
            let gain = slope_gain(Some(profile));
            slope_to_grade(grade_to_slope(shown, Some(range), gain), r_eff, gain)
        }
        "highlight_density" => highlight_auto(exposure, mode, metrics).map_or(shown, |v| shown - v),
        "shadow_density" => shadow_auto(exposure, mode, metrics).map_or(shown, |v| shown - v),
        _ => shown,
    }
}

/// Whether the frame prints its softest slope while the Grade slider still has travel.
#[must_use]
pub fn grade_at_softest(
    exposure: &ExposureConfig,
    mode: Option<ProcessMode>,
    metrics: &AutoSliderMetrics,
) -> bool {
    if metrics.norm_density_range.is_none() {
        return false;
    }
    let shown = print_shown_values(exposure, mode, metrics)
        .into_iter()
        .find_map(|(field, value)| (field == "grade").then_some(value))
        .unwrap_or(exposure.grade);
    if shown >= c::ISO_R_MAX - 0.5 {
        return false;
    }
    let current = curve_params_from_metrics(exposure, mode, metrics).0[1];
    let mut soft = exposure.clone();
    soft.grade = c::ISO_R_MAX;
    let soft_slope = curve_params_from_metrics(&soft, mode, metrics).0[1];
    current <= soft_slope + 1e-6
}
