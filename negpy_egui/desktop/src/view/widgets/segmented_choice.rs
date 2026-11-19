//! One choice out of two to four, all in view.
//!
//! The track is the shared input surface and the current choice takes the warm chip.
//! A caller passes labels and the current index, never a row of exclusive toggles.

use eframe::egui::{self, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::theme::Theme;

/// A short, fixed choice the panel shows in full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentedChoice<'a> {
    pub labels: &'a [&'a str],
    pub icons: &'a [Option<Icon>],
    pub current: usize,
}

impl<'a> SegmentedChoice<'a> {
    /// A choice with the first segment current.
    #[must_use]
    pub fn new(labels: &'a [&'a str]) -> Self {
        Self {
            labels,
            icons: &[],
            current: 0,
        }
    }

    /// Paint the reference's leading icon on each segment.
    #[must_use]
    pub fn with_icons(mut self, icons: &'a [Option<Icon>]) -> Self {
        self.icons = icons;
        self
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
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("segmented", self.labels))
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.set_width(rect.width());
        let width = rect.width() / count as f32;
        let mut picked = None;
        for (index, label) in self.labels.iter().enumerate() {
            let segment = Rect::from_min_size(
                egui::pos2(rect.left() + index as f32 * width, rect.top()),
                Vec2::new(width, rect.height()),
            );
            let current = index == self.current;
            let segment_response =
                child.interact(segment, child.id().with(("segment", index)), Sense::click());
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
            child.painter().rect_filled(segment.shrink(1.0), 4.0, fill);
            let border = if current {
                theme.border_checked
            } else {
                theme.border_primary
            };
            child.painter().rect_stroke(
                segment.shrink(1.0),
                4.0,
                Stroke::new(1.0_f32, border),
                egui::StrokeKind::Inside,
            );
            let color = if current {
                theme.text_primary
            } else {
                theme.text_hint
            };
            let icon = self.icons.get(index).copied().flatten();
            let label_width = f32::from(theme.font_size_base) * 0.62 * label.chars().count() as f32;
            let icon_width = if icon.is_some() { 22.0 } else { 0.0 };
            let left = segment.center().x - (icon_width + label_width) / 2.0;
            if let Some(icon) = icon {
                child.painter().text(
                    egui::pos2(left + 7.0, segment.center().y),
                    egui::Align2::CENTER_CENTER,
                    icon.glyph(),
                    FontId::proportional(f32::from(theme.font_size_base)),
                    color,
                );
            }
            child.painter().text(
                egui::pos2(left + icon_width, segment.center().y),
                egui::Align2::LEFT_CENTER,
                *label,
                FontId::proportional(f32::from(theme.font_size_base)),
                color,
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
