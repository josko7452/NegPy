//! A panel section: a header row over a card body.
//!
//! The header carries the title, the modified count, the reset and the guide button.
//! The body is the only place a panel puts its controls, and the card is the only
//! reset affordance.

use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

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
    pub icon: Option<Icon>,
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
            icon: None,
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

    /// Show a section icon before the title.
    #[must_use]
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
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
        let top = CornerRadius {
            nw: theme.radius_md,
            ne: theme.radius_md,
            sw: if self.expanded { 0 } else { theme.radius_md },
            se: if self.expanded { 0 } else { theme.radius_md },
        };
        ui.painter().rect_filled(header, top, theme.bg_header);
        ui.painter().line_segment(
            [header.left_bottom(), header.right_bottom()],
            Stroke::new(1.0_f32, theme.border_primary),
        );
        let edge = Rect::from_min_max(header.min, egui::pos2(header.max.x, header.max.y - 1.0));
        ui.painter().rect_stroke(
            edge,
            top,
            Stroke::new(1.0_f32, theme.border_primary),
            egui::StrokeKind::Inside,
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
        let mut title_left = f32::from(theme.space_lg + theme.space_xl);
        if let Some(icon) = self.icon {
            ui.painter().text(
                header.left_center() + Vec2::new(f32::from(theme.space_lg + theme.space_md), 0.0),
                Align2::LEFT_CENTER,
                icon.glyph(),
                FontId::proportional(f32::from(theme.font_size_small)),
                theme.text_hint,
            );
            title_left += 20.0;
        }
        ui.painter().text(
            header.left_center() + Vec2::new(title_left, 0.0),
            Align2::LEFT_CENTER,
            self.display_title(),
            FontId::proportional(f32::from(theme.font_size_header)),
            theme.text_primary,
        );
        let mut right = header.right() - f32::from(theme.space_lg);
        right = paint_header_icon(ui, right, header.center().y, Icon::ChevronDown, &theme);
        right = paint_header_icon(ui, right, header.center().y, Icon::Reset, &theme);
        right = paint_header_icon(ui, right, header.center().y, Icon::Circle, &theme);
        if self.modified > 0 {
            right = paint_header_icon(ui, right, header.center().y, Icon::Tag, &theme);
        }
        if self.has_guide {
            let _ = paint_header_icon(ui, right, header.center().y, Icon::Info, &theme);
        }
        if response.clicked() {
            self.expanded = !self.expanded;
        }
        if !self.expanded {
            return (response, None);
        }
        let body_rect = Rect::from_min_size(
            egui::pos2(header.left(), header.bottom()),
            Vec2::new(header.width(), 0.0),
        );
        let background = ui.painter().add(egui::Shape::Noop);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt((&self.title, "body"))
                .max_rect(body_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_width(body_rect.width());
        child.set_max_height(f32::INFINITY);
        let result = body(&mut child);
        let height = child.min_rect().height();
        let height = if height.is_finite() {
            height.max(0.0)
        } else {
            0.0
        };
        let body_rect = Rect::from_min_size(body_rect.min, Vec2::new(body_rect.width(), height));
        ui.advance_cursor_after_rect(body_rect);
        let bottom = CornerRadius {
            nw: 0,
            ne: 0,
            sw: theme.radius_md,
            se: theme.radius_md,
        };
        ui.painter().set(
            background,
            egui::Shape::Vec(vec![
                egui::Shape::rect_filled(body_rect, bottom, theme.bg_card),
                egui::Shape::rect_stroke(
                    body_rect,
                    bottom,
                    Stroke::new(1.0_f32, theme.border_primary),
                    egui::StrokeKind::Inside,
                ),
            ]),
        );
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
    fn sections_after_a_card_stay_in_the_scroll_area() {
        let ctx = egui::Context::default();
        let mut a = CollapsibleSection::new("A");
        let mut b = CollapsibleSection::new("B").collapsed();
        let mut c = CollapsibleSection::new("C").collapsed();
        let mut tops = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_max_height(200.0);
                a.show(ui, |ui| {
                    let _ = card(ui, |ui| {
                        ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 60.0),
                            Sense::hover(),
                        );
                    });
                });
                let (h, _) = b.show(ui, |_| {});
                tops.push(h.rect.top());
                let (h2, _) = c.show(ui, |_| {});
                tops.push(h2.rect.top());
            });
        });
        assert!(tops[0] < 200.0 && tops[1] < 200.0);
    }

    #[test]
    fn an_open_section_advances_by_its_body_not_the_rail() {
        let ctx = egui::Context::default();
        let mut first = CollapsibleSection::new("First");
        let mut second = CollapsibleSection::new("Second");
        let mut second_header = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_max_height(400.0);
                first.show(ui, |ui| {
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), Sense::hover());
                });
                let (header, _) = second.show(ui, |ui| {
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                });
                second_header = Some(header.rect.top());
            });
        });
        assert!(second_header.unwrap() < 400.0);
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
