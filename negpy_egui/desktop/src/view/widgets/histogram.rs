//! The output histogram strip behind a card header.

use eframe::egui::{self, Color32, Rect, Sense, Stroke, Vec2};

use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

/// The luminance row of an `(4, 256)` output histogram.
pub const LUMA_ROW: usize = 3;

/// The channel rows of an `(4, 256)` output histogram.
pub const CHANNEL_ROWS: [usize; 3] = [0, 1, 2];

/// One normalized trace and the color it paints.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub values: Vec<f32>,
    pub color: Color32,
}

/// Whether a buffer is an `(4, 256)` histogram this widget can paint.
#[must_use]
pub fn is_histogram(bins: &[f64]) -> bool {
    bins.len() == 4 * 256
}

/// The normalized trace a histogram row paints, or an empty one.
#[must_use]
pub fn normalized_trace(bins: &[f64], row: usize, log: bool) -> Vec<f32> {
    if !is_histogram(bins) || row >= 4 {
        return Vec::new();
    }
    let start = row * 256;
    let counts: Vec<f64> = bins[start..start + 256]
        .iter()
        .map(|value| if log { value.ln_1p() } else { *value })
        .collect();
    let peak = counts.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return vec![0.0; 256];
    }
    counts
        .into_iter()
        .map(|value| (value / peak) as f32)
        .collect()
}

/// The luma trace of the output histogram in the muted card color.
#[must_use]
pub fn luma_trace(bins: &[f64], log: bool) -> Option<Trace> {
    if !is_histogram(bins) {
        return None;
    }
    Some(Trace {
        values: normalized_trace(bins, LUMA_ROW, log),
        color: with_alpha(Theme::default().text_muted, 100),
    })
}

/// The R, G and B traces of the output histogram.
#[must_use]
pub fn channel_traces(bins: &[f64], log: bool) -> Option<Vec<Trace>> {
    if !is_histogram(bins) {
        return None;
    }
    let theme = Theme::default();
    let colors = [theme.channel_red, theme.channel_green, theme.channel_blue];
    Some(
        CHANNEL_ROWS
            .into_iter()
            .zip(colors)
            .map(|(row, color)| Trace {
                values: normalized_trace(bins, row, log),
                color: with_alpha(color, 120),
            })
            .collect(),
    )
}

/// Paint one trace as a filled strip from the bottom edge.
pub fn paint_trace(ui: &egui::Ui, area: Rect, trace: &Trace) {
    if trace.values.len() < 2 {
        return;
    }
    let step = area.width() / (trace.values.len() - 1) as f32;
    let mut points = Vec::with_capacity(trace.values.len() + 2);
    points.push(area.left_bottom());
    for (index, value) in trace.values.iter().enumerate() {
        points.push(egui::pos2(
            area.left() + index as f32 * step,
            area.bottom() - value * area.height(),
        ));
    }
    points.push(area.right_bottom());
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        trace.color,
        Stroke::NONE,
    ));
}

/// Paint the luma or channel traces on an existing rect, with no fill.
pub fn paint_strip(ui: &egui::Ui, area: Rect, bins: &[f64], log: bool, channels: bool) {
    if !is_histogram(bins) {
        return;
    }
    if channels {
        if let Some(traces) = channel_traces(bins, log) {
            for trace in traces {
                paint_trace(ui, area, &trace);
            }
        }
    } else if let Some(trace) = luma_trace(bins, log) {
        paint_trace(ui, area, &trace);
    }
}

/// Paint a histogram behind the current cursor position.
pub fn show(ui: &mut egui::Ui, bins: &[f64], log: bool, channels: bool) {
    let theme = Theme::default();
    let (area, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ui.available_height()),
        Sense::hover(),
    );
    ui.painter().rect_filled(area, 0.0, theme.bg_input);
    if !is_histogram(bins) {
        return;
    }
    if channels {
        if let Some(traces) = channel_traces(bins, log) {
            for trace in traces {
                paint_trace(ui, area, &trace);
            }
        }
    } else if let Some(trace) = luma_trace(bins, log) {
        paint_trace(ui, area, &trace);
    }
}

/// Whether a buffer is a `(4, 120)` density histogram.
#[must_use]
pub fn is_density_histogram(bins: &[f64]) -> bool {
    bins.len() == 4 * 120
}

/// The share of a channel above which its edge bin counts as clipping.
pub const CLIP_THRESHOLD: f64 = 0.005;

/// Whether each of the R, G and B rows clips at its black and white edge bins.
#[must_use]
pub fn clip_marks(bins: &[f64]) -> ([bool; 3], [bool; 3]) {
    let mut low = [false; 3];
    let mut high = [false; 3];
    if !is_histogram(bins) {
        return (low, high);
    }
    for channel in 0..3 {
        let row = &bins[channel * 256..(channel + 1) * 256];
        let total = row.iter().sum::<f64>().max(1.0);
        low[channel] = row.first().copied().unwrap_or(0.0) / total > CLIP_THRESHOLD;
        high[channel] = row.last().copied().unwrap_or(0.0) / total > CLIP_THRESHOLD;
    }
    (low, high)
}

/// Paint a filled triangle against `edge` for each channel that clips there.
pub fn paint_clip_marks(ui: &egui::Ui, area: Rect, bins: &[f64]) {
    let (low, high) = clip_marks(bins);
    let theme = Theme::default();
    let colors = [theme.channel_red, theme.channel_green, theme.channel_blue];
    let size = 5.0;
    let gap = size + 2.0;
    for (channel, color) in colors.into_iter().enumerate() {
        let y = area.top() + 4.0 + channel as f32 * gap;
        if low[channel] {
            let points = vec![
                egui::pos2(area.left() + 3.0, y),
                egui::pos2(area.left() + 3.0, y + size),
                egui::pos2(area.left() + 3.0 + size, y + size / 2.0),
            ];
            ui.painter()
                .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
        if high[channel] {
            let points = vec![
                egui::pos2(area.right() - 3.0, y),
                egui::pos2(area.right() - 3.0, y + size),
                egui::pos2(area.right() - 3.0 - size, y + size / 2.0),
            ];
            ui.painter()
                .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
    }
}

/// Paint the density histogram's luma row along the bottom edge of a chart.
pub fn paint_density(ui: &egui::Ui, area: Rect, bins: &[f64], log: bool) {
    if !is_density_histogram(bins) {
        return;
    }
    let start = 3 * 120;
    let counts: Vec<f64> = bins[start..start + 120]
        .iter()
        .map(|value| if log { value.ln_1p() } else { *value })
        .collect();
    let peak = counts.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return;
    }
    let mut points = Vec::with_capacity(122);
    points.push(area.left_bottom());
    for (index, value) in counts.iter().enumerate() {
        points.push(egui::pos2(
            area.left() + index as f32 / 119.0 * area.width(),
            area.bottom() - (value / peak) as f32 * area.height(),
        ));
    }
    points.push(area.right_bottom());
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        with_alpha(Theme::default().text_hint, 70),
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bins(peak: f64) -> Vec<f64> {
        let mut bins = vec![0.0; 4 * 256];
        bins[3 * 256 + 128] = peak;
        bins
    }

    #[test]
    fn a_trace_peaks_at_one() {
        let values = normalized_trace(&bins(40.0), LUMA_ROW, false);
        assert_eq!(values.len(), 256);
        assert!((values[128] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_channel_clips_only_when_its_edge_bin_is_loaded() {
        let mut values = vec![0.0; 4 * 256];
        values[2 * 256] = 50.0;
        values[2 * 256 + 128] = 1000.0;
        values[0 * 256 + 255] = 40.0;
        values[0 * 256 + 128] = 1000.0;
        let (low, high) = clip_marks(&values);
        assert!(!low[0] && !low[1] && low[2]);
        assert!(high[0] && !high[1] && !high[2]);
    }

    #[test]
    fn a_missing_histogram_paints_nothing() {
        assert!(luma_trace(&[], false).is_none());
        assert!(channel_traces(&[1.0; 10], false).is_none());
        assert!(normalized_trace(&[], LUMA_ROW, false).is_empty());
    }

    #[test]
    fn the_luma_and_channel_traces_paint_on_a_context() {
        let ctx = egui::Context::default();
        let bins = bins(20.0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let area = Rect::from_min_size(ui.cursor().min, Vec2::new(120.0, 24.0));
                for trace in luma_trace(&bins, false)
                    .into_iter()
                    .chain(channel_traces(&bins, true).unwrap_or_default())
                {
                    paint_trace(ui, area, &trace);
                }
            });
        });
        assert!(luma_trace(&bins, false).is_some());
        assert_eq!(channel_traces(&bins, false).unwrap().len(), 3);
    }

    #[test]
    fn the_log_scale_lifts_the_small_bins() {
        let mut bins = bins(100.0);
        bins[3 * 256 + 10] = 1.0;
        let linear = normalized_trace(&bins, LUMA_ROW, false);
        let log = normalized_trace(&bins, LUMA_ROW, true);
        assert!(log[10] > linear[10]);
    }
}
