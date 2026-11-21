//! The waveform, parade and vectorscope plots of the Analysis card.
//!
//! Each takes the bins the render worker measured on the display frame, so a
//! chart cannot show another frame's pixels.

use eframe::egui::{self, Color32, Rect, Vec2};

use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

const WAVEFORM_BINS: usize = 128;
const WAVEFORM_COLS: usize = 256;
const HUE_BINS: usize = 64;
const CHROMA_BINS: usize = 32;

/// The luma counts per column and tone, peak-normalized and log-scaled on request.
#[must_use]
pub fn waveform_trace(bins: &[f64], log: bool) -> Option<Vec<f32>> {
    if bins.len() != WAVEFORM_BINS * WAVEFORM_COLS {
        return None;
    }
    let scaled: Vec<f64> = bins
        .iter()
        .map(|value| if log { value.ln_1p() } else { *value })
        .collect();
    let peak = scaled.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return None;
    }
    Some(
        scaled
            .into_iter()
            .map(|value| (value / peak) as f32)
            .collect(),
    )
}

/// The luma counts of one column as a tone trace, row 0 at paper black.
#[must_use]
pub fn column_trace(bins: &[f64], column: usize, log: bool) -> Vec<f32> {
    if bins.len() != WAVEFORM_BINS * WAVEFORM_COLS || column >= WAVEFORM_COLS {
        return Vec::new();
    }
    let values: Vec<f64> = (0..WAVEFORM_BINS)
        .map(|row| bins[row * WAVEFORM_COLS + column])
        .map(|value| if log { value.ln_1p() } else { value })
        .collect();
    let peak = values.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return Vec::new();
    }
    values
        .into_iter()
        .map(|value| (value / peak) as f32)
        .collect()
}

/// Paint the luma waveform as a tone trace per column.
pub fn paint_waveform(ui: &egui::Ui, area: Rect, bins: Option<&[f64]>, log: bool) {
    let Some(bins) = bins.filter(|bins| waveform_trace(bins, log).is_some()) else {
        return;
    };
    let theme = Theme::default();
    let color = with_alpha(theme.channel_green, 110);
    for column in 0..WAVEFORM_COLS {
        let trace = column_trace(bins, column, log);
        if trace.len() < 2 {
            continue;
        }
        let x = area.left() + column as f32 / (WAVEFORM_COLS - 1) as f32 * area.width();
        let mut points = Vec::with_capacity(trace.len());
        for (row, value) in trace.iter().enumerate() {
            let y = area.bottom() - row as f32 / (WAVEFORM_BINS - 1) as f32 * area.height();
            points.push(egui::pos2(x, y - value * 4.0));
        }
        ui.painter()
            .add(egui::Shape::line(points, egui::Stroke::new(1.0_f32, color)));
    }
}

/// Paint the R, G and B parade side by side over the waveform's tone axis.
pub fn paint_parade(ui: &egui::Ui, area: Rect, bins: Option<&[f64]>, log: bool) {
    let Some(bins) = bins.filter(|bins| bins.len() == 3 * WAVEFORM_BINS * WAVEFORM_COLS) else {
        return;
    };
    let theme = Theme::default();
    let colors = [theme.channel_red, theme.channel_green, theme.channel_blue];
    let plane = WAVEFORM_BINS * WAVEFORM_COLS;
    let gap = 3.0;
    let width = (area.width() - 2.0 * gap) / 3.0;
    for (channel, color) in colors.into_iter().enumerate() {
        let slice = &bins[channel * plane..(channel + 1) * plane];
        let Some(trace) = waveform_trace(slice, log) else {
            continue;
        };
        let strip = Rect::from_min_size(
            egui::pos2(area.left() + channel as f32 * (width + gap), area.top()),
            Vec2::new(width, area.height()),
        );
        let color = with_alpha(color, 110);
        for column in 0..WAVEFORM_COLS {
            let start = column * WAVEFORM_BINS;
            let peak = trace[start..start + WAVEFORM_BINS]
                .iter()
                .copied()
                .fold(0.0, f32::max);
            if peak <= 0.0 {
                continue;
            }
            let x = strip.left() + column as f32 / (WAVEFORM_COLS - 1) as f32 * strip.width();
            let mut points = Vec::with_capacity(WAVEFORM_BINS);
            for row in 0..WAVEFORM_BINS {
                let value = trace[start + row];
                let y = strip.bottom() - row as f32 / (WAVEFORM_BINS - 1) as f32 * strip.height();
                points.push(egui::pos2(x, y - value * 4.0));
            }
            ui.painter()
                .add(egui::Shape::line(points, egui::Stroke::new(1.0_f32, color)));
        }
    }
}

/// The a*b* density of a vectorscope histogram, normalized so the densest cell shows.
#[must_use]
pub fn vectorscope_trace(counts: &[f64], log: bool) -> Option<Vec<f32>> {
    if counts.len() != HUE_BINS * CHROMA_BINS {
        return None;
    }
    let scaled: Vec<f64> = counts
        .iter()
        .map(|value| if log { value.ln_1p() } else { *value })
        .collect();
    let peak = scaled.iter().copied().fold(0.0, f64::max);
    (peak > 0.0).then(|| {
        scaled
            .into_iter()
            .map(|value| (value / peak) as f32)
            .collect()
    })
}

/// Paint the a*b* plane as a polar scope: hue around, chroma outward.
pub fn paint_vectorscope(ui: &egui::Ui, area: Rect, counts: Option<&[f64]>, log: bool) {
    let Some(trace) = counts.and_then(|counts| vectorscope_trace(counts, log)) else {
        return;
    };
    let theme = Theme::default();
    let center = area.center();
    let radius = area.width().min(area.height()) * 0.46;
    let color = with_alpha(theme.channel_green, 120);
    for hue in 0..HUE_BINS {
        for chroma in 1..CHROMA_BINS {
            let value = trace[hue * CHROMA_BINS + chroma];
            if value <= 0.01 {
                continue;
            }
            let angle = hue as f32 / HUE_BINS as f32 * std::f32::consts::TAU;
            let r = chroma as f32 / (CHROMA_BINS - 1) as f32 * radius;
            let point = center + Vec2::new(angle.cos(), -angle.sin()) * r;
            let size = 1.0 + 3.0 * value;
            ui.painter().circle_filled(
                point,
                size,
                Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    (200.0 * value) as u8,
                ),
            );
        }
    }
    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(1.0_f32, theme.border_primary),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_waveform_normalizes_to_one_peak() {
        let mut bins = vec![0.0; WAVEFORM_BINS * WAVEFORM_COLS];
        bins[10] = 4.0;
        let trace = waveform_trace(&bins, false).unwrap();
        assert!((trace[10] - 1.0).abs() < 1e-6);
        assert!(trace.iter().all(|value| *value <= 1.0));
    }

    #[test]
    fn a_column_trace_reads_its_own_column() {
        let mut bins = vec![0.0; WAVEFORM_BINS * WAVEFORM_COLS];
        bins[5 * WAVEFORM_COLS + 7] = 3.0;
        let trace = column_trace(&bins, 7, false);
        assert!((trace[5] - 1.0).abs() < 1e-6);
        assert!(
            trace
                .iter()
                .enumerate()
                .all(|(row, value)| row == 5 || *value == 0.0)
        );
    }

    #[test]
    fn an_empty_scope_paints_nothing() {
        assert!(waveform_trace(&[0.0; WAVEFORM_BINS * WAVEFORM_COLS], false).is_none());
        assert!(vectorscope_trace(&[0.0; HUE_BINS * CHROMA_BINS], true).is_none());
    }
}
