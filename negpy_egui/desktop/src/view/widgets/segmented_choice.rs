//! One choice out of two to four, all in view.
//!
//! The track is the shared input surface and the current choice takes the warm chip.
//! A caller passes labels and the current index, never a row of exclusive toggles.

use eframe::egui::{self, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::theme::Theme;

/// A short, fixed choice the panel shows in full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentedChoice<'a> {
    pub labels: &'a [&'a str],
    pub current: usize,
}

impl<'a> SegmentedChoice<'a> {
    /// A choice with the first segment current.
    #[must_use]
    pub fn new(labels: &'a [&'a str]) -> Self {
        Self { labels, current: 0 }
    }

    /// The current label, or an empty string for a choice with no segments.
    #[must_use]
    pub fn current_label(&self) -> &'a str {
        self.labels.get(self.current).copied().unwrap_or("")
    }

    /// Paint the track and return the segment the user clicked, if any.
    pub fn show(&mut self, ui: &mut Ui) -> Option<usize> {
        let theme = Theme::default();
        let count = self.labels.len();
        if count == 0 {
            return None;
        }
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
            Sense::click(),
        );
        let width = rect.width() / count as f32;
        let mut picked = None;
        for (index, label) in self.labels.iter().enumerate() {
            let segment = Rect::from_min_size(
                egui::pos2(rect.left() + index as f32 * width, rect.top()),
                Vec2::new(width, rect.height()),
            );
            let current = index == self.current;
            let segment_response =
                ui.interact(segment, ui.id().with(("segment", index)), Sense::click());
            let hovered = segment_response.hovered();
            let fill = if current {
                if hovered {
                    theme.bg_checked_hover
                } else {
                    theme.bg_checked
                }
            } else if hovered {
                theme.bg_hover
            } else {
                theme.bg_input
            };
            ui.painter().rect_filled(segment.shrink(1.0), 4.0, fill);
            let border = if current {
                theme.border_checked
            } else {
                theme.border_primary
            };
            ui.painter().rect_stroke(
                segment.shrink(1.0),
                4.0,
                Stroke::new(1.0_f32, border),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                segment.center(),
                egui::Align2::CENTER_CENTER,
                *label,
                FontId::proportional(f32::from(theme.font_size_base)),
                if current {
                    theme.text_primary
                } else {
                    theme.text_hint
                },
            );
            if segment_response.clicked() {
                picked = Some(index);
            }
        }
        if let Some(index) = picked {
            self.current = index;
        }
        let _ = response;
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_reports_its_current_label() {
        let choice = SegmentedChoice::new(&["Color", "B&W", "Slide"]);
        assert_eq!(choice.current_label(), "Color");
    }

    #[test]
    fn an_empty_choice_has_no_label() {
        assert_eq!(SegmentedChoice::new(&[]).current_label(), "");
    }

    #[test]
    fn the_track_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut choice = SegmentedChoice::new(&["Camera", "Film Scanner"]);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = choice.show(ui);
            });
        });
    }
}
