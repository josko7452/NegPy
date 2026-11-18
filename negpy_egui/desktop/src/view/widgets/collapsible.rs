//! A panel section: a header row over a card body.
//!
//! The header carries the title, the modified count, the reset and the guide button.
//! The body is the only place a panel puts its controls, and the card is the only
//! reset affordance.

use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{HEADER_BUTTON_SIZE, HEADER_HEIGHT};
use crate::styles::theme::Theme;

/// The state a section keeps across frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollapsibleSection {
    pub title: String,
    pub expanded: bool,
    pub modified: usize,
    pub has_guide: bool,
}

impl CollapsibleSection {
    /// An expanded section with no modified controls.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            expanded: true,
            modified: 0,
            has_guide: false,
        }
    }

    /// Start collapsed.
    #[must_use]
    pub fn collapsed(mut self) -> Self {
        self.expanded = false;
        self
    }

    /// Show the guide button in the header.
    #[must_use]
    pub fn with_guide(mut self) -> Self {
        self.has_guide = true;
        self
    }

    /// The title with the modified count, as the header prints it.
    #[must_use]
    pub fn display_title(&self) -> String {
        if self.modified == 0 {
            self.title.clone()
        } else {
            format!("{} · {}", self.title, self.modified)
        }
    }

    /// Paint the header and body. Returns the header response and whether the body is open.
    pub fn show<R>(
        &mut self,
        ui: &mut Ui,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> (egui::Response, Option<R>) {
        let theme = Theme::default();
        let (header, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), HEADER_HEIGHT),
            Sense::click(),
        );
        ui.painter().rect_filled(header, 0.0, theme.bg_header);
        ui.painter().line_segment(
            [header.left_bottom(), header.right_bottom()],
            Stroke::new(1.0_f32, theme.border_primary),
        );
        let chevron = if self.expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };
        ui.painter().text(
            header.left_center() + Vec2::new(f32::from(theme.space_lg), 0.0),
            Align2::LEFT_CENTER,
            chevron.glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_secondary,
        );
        ui.painter().text(
            header.left_center() + Vec2::new(f32::from(theme.space_lg + theme.space_xl), 0.0),
            Align2::LEFT_CENTER,
            self.display_title(),
            FontId::proportional(f32::from(theme.font_size_header)),
            theme.text_primary,
        );
        let mut right = header.right() - f32::from(theme.space_lg);
        if self.has_guide {
            right = paint_header_icon(ui, right, header.center().y, Icon::Info, &theme);
        }
        if self.modified > 0 {
            right = paint_header_icon(ui, right, header.center().y, Icon::TurnPage, &theme);
        }
        let _ = right;
        if response.clicked() {
            self.expanded = !self.expanded;
        }
        if !self.expanded {
            return (response, None);
        }
        let body_rect = Rect::from_min_size(
            egui::pos2(header.left(), header.bottom()),
            Vec2::new(header.width(), ui.available_height()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((&self.title, "body"))
                .max_rect(body_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_width(body_rect.width());
        let result = body(&mut child);
        let height = child.min_rect().height().max(0.0);
        ui.advance_cursor_after_rect(Rect::from_min_size(
            body_rect.min,
            Vec2::new(body_rect.width(), height),
        ));
        (response, Some(result))
    }
}

fn paint_header_icon(ui: &Ui, right: f32, center_y: f32, icon: Icon, theme: &Theme) -> f32 {
    let rect = Rect::from_center_size(
        egui::pos2(right - HEADER_BUTTON_SIZE / 2.0, center_y),
        Vec2::splat(HEADER_BUTTON_SIZE),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
    right - HEADER_BUTTON_SIZE
}

/// A section's body card, with the shared inset and surface.
#[must_use]
pub fn card<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = Theme::default();
    egui::Frame::NONE
        .fill(theme.bg_card)
        .inner_margin(egui::Margin::symmetric(
            i8::try_from(theme.space_xl).unwrap_or(12),
            i8::try_from(theme.space_sm).unwrap_or(4),
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
        .inner
}

/// A line of help under a control, in the hint role.
#[must_use]
pub fn hint(ui: &mut Ui, text: &str) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(text)
            .color(theme.text_hint)
            .size(f32::from(theme.font_size_small)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modified_section_prints_its_count() {
        let mut section = CollapsibleSection::new("Exposure");
        assert_eq!(section.display_title(), "Exposure");
        section.modified = 3;
        assert_eq!(section.display_title(), "Exposure · 3");
    }

    #[test]
    fn a_collapsed_section_skips_its_body() {
        let ctx = egui::Context::default();
        let mut section = CollapsibleSection::new("Exposure").collapsed();
        let mut painted = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, body) = section.show(ui, |_| painted = true);
                assert!(body.is_none());
            });
        });
        assert!(!painted);
    }

    #[test]
    fn an_open_section_paints_its_body() {
        let ctx = egui::Context::default();
        let mut section = CollapsibleSection::new("Exposure");
        let mut painted = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, body) = section.show(ui, |_| painted = true);
                assert!(body.is_some());
            });
        });
        assert!(painted);
    }
}
