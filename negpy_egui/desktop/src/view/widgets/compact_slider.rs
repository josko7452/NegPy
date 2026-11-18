//! One slider row: label, track and typed value.
//!
//! Mirrors the `CompactSlider` look: the label is right-aligned in the shared field
//! column, the value is typed into, and the neutral marker sits at the default value.

use eframe::egui::{self, Align, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, FIELD_LABEL_WIDTH};
use crate::styles::theme::Theme;

const HANDLE_RADIUS: f32 = 6.0;
const TRACK_HEIGHT: f32 = 4.0;
const VALUE_WIDTH: f32 = 72.0;

/// A slider's numeric range and step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderSpec {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub default: f64,
    /// Digits after the decimal point.
    pub precision: usize,
    pub unit: &'static str,
}

impl SliderSpec {
    /// A spec whose default is the range's low end.
    #[must_use]
    pub fn new(min: f64, max: f64, step: f64, precision: usize) -> Self {
        Self {
            min,
            max,
            step,
            default: min,
            precision,
            unit: "",
        }
    }

    /// Set the default, the neutral marker and the double-click target.
    #[must_use]
    pub fn with_default(mut self, default: f64) -> Self {
        self.default = default.clamp(self.min, self.max);
        self
    }

    /// Set the unit suffix: a space before a word, none before a symbol.
    #[must_use]
    pub fn with_unit(mut self, unit: &'static str) -> Self {
        self.unit = unit;
        self
    }

    fn clamped(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    fn snapped(&self, value: f64) -> f64 {
        if self.step <= 0.0 {
            return self.clamped(value);
        }
        let steps = ((value - self.min) / self.step).round();
        self.clamped(self.min + steps * self.step)
    }

    fn fraction(&self, value: f64) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
    }
}

/// The value a slider row holds across frames.
#[derive(Clone, Debug, PartialEq)]
pub struct CompactSlider {
    pub spec: SliderSpec,
    pub value: f64,
    editing: bool,
    draft: String,
}

impl CompactSlider {
    /// A slider at its default value.
    #[must_use]
    pub fn new(spec: SliderSpec) -> Self {
        Self {
            spec,
            value: spec.default,
            editing: false,
            draft: String::new(),
        }
    }

    /// Set the value, snapped to the spec and clipped to its range.
    pub fn set_value(&mut self, value: f64) {
        self.value = self.spec.snapped(value);
    }

    /// Whether the row shows the neutral value.
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        (self.value - self.spec.default).abs() <= self.spec.step * 0.5
    }

    /// The value as the row prints it, with the unit suffix.
    #[must_use]
    pub fn display(&self) -> String {
        format!("{:.*}{}", self.spec.precision, self.value, self.spec.unit)
    }

    /// Paint and interact with the row. Returns the response and the changed flag.
    pub fn show(&mut self, ui: &mut Ui, label: &str) -> (egui::Response, bool) {
        let theme = Theme::default();
        let mut changed = false;
        let row = ui.horizontal(|ui| {
            ui.add_sized(
                Vec2::new(FIELD_LABEL_WIDTH, DEFAULT_BUTTON_HEIGHT),
                egui::Label::new(
                    RichText::new(label)
                        .color(theme.text_secondary)
                        .size(f32::from(theme.font_size_base)),
                )
                .halign(Align::Max),
            );
            let available =
                (ui.available_width() - VALUE_WIDTH - 2.0 * theme.space_md as f32).max(40.0);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(available, DEFAULT_BUTTON_HEIGHT),
                Sense::click_and_drag(),
            );
            if response.dragged() {
                let x =
                    response
                        .interact_pointer_pos()
                        .map_or(self.spec.fraction(self.value), |pos| {
                            ((pos.x - rect.left() - HANDLE_RADIUS)
                                / (rect.width() - 2.0 * HANDLE_RADIUS))
                                .clamp(0.0, 1.0)
                        });
                let value = self.spec.min + f64::from(x) * (self.spec.max - self.spec.min);
                let snapped = self.spec.snapped(value);
                if snapped != self.value {
                    self.value = snapped;
                    changed = true;
                }
            }
            if response.double_clicked() {
                self.value = self.spec.default;
                changed = true;
            }
            paint_track(ui, rect, &theme, &self.spec, self.value);
            let value_rect = Rect::from_min_size(
                egui::pos2(rect.right() + f32::from(theme.space_md), rect.top()),
                Vec2::new(VALUE_WIDTH, DEFAULT_BUTTON_HEIGHT),
            );
            ui.painter().text(
                value_rect.left_center(),
                egui::Align2::LEFT_CENTER,
                self.display(),
                FontId::proportional(f32::from(theme.font_size_base)),
                theme.text_primary,
            );
        });
        (row.response, changed)
    }
}

fn paint_track(ui: &Ui, rect: Rect, theme: &Theme, spec: &SliderSpec, value: f64) {
    let left = rect.left() + HANDLE_RADIUS;
    let right = rect.right() - HANDLE_RADIUS;
    let center_y = rect.center().y;
    ui.painter().rect_filled(
        Rect::from_min_max(
            egui::pos2(left, center_y - TRACK_HEIGHT / 2.0),
            egui::pos2(right, center_y + TRACK_HEIGHT / 2.0),
        ),
        TRACK_HEIGHT / 2.0,
        theme.border_primary,
    );
    let handle_x = left + (right - left) * spec.fraction(value);
    let neutral_x = left + (right - left) * spec.fraction(spec.default);
    if (handle_x - neutral_x).abs() > HANDLE_RADIUS + 1.0 {
        ui.painter().line_segment(
            [
                egui::pos2(neutral_x, center_y - 3.0),
                egui::pos2(neutral_x, center_y + 3.0),
            ],
            Stroke::new(1.0_f32, theme.text_unit),
        );
    }
    ui.painter().circle_filled(
        egui::pos2(handle_x, center_y),
        HANDLE_RADIUS,
        theme.text_secondary,
    );
}

/// A value field that reads a typed number, including a comma decimal.
#[must_use]
pub fn parse_value(text: &str) -> Option<f64> {
    text.trim()
        .trim_end_matches(|value: char| value.is_alphabetic() || value == '%')
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> SliderSpec {
        SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)
    }

    #[test]
    fn a_value_snaps_to_the_step_and_range() {
        let mut slider = CompactSlider::new(spec());
        slider.set_value(0.237);
        assert!((slider.value - 0.24).abs() < 1e-9);
        slider.set_value(4.0);
        assert_eq!(slider.value, 1.0);
    }

    #[test]
    fn a_displayed_value_carries_its_unit() {
        let mut slider = CompactSlider::new(SliderSpec::new(0.0, 10.0, 1.0, 0).with_unit(" mm"));
        slider.set_value(3.0);
        assert_eq!(slider.display(), "3 mm");
    }

    #[test]
    fn a_typed_value_reads_a_comma_and_a_suffix() {
        assert_eq!(parse_value("1,25 mm"), Some(1.25));
        assert_eq!(parse_value(" 16 "), Some(16.0));
        assert_eq!(parse_value("no"), None);
    }

    #[test]
    fn the_row_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut slider = CompactSlider::new(spec());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = slider.show(ui, "Density");
            });
        });
    }
}
