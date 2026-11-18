//! A single-line label that ends in … when its text does not fit.
//!
//! The full text stays the label's value. The tooltip repeats it only when the
//! shown text is shorter.

use eframe::egui::{self, FontId, Sense, Ui, Vec2};

use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::theme::Theme;

/// The width a text takes at the base font, near enough for a clip decision.
#[must_use]
pub fn text_width(text: &str) -> f32 {
    text.chars().count() as f32 * 8.0
}

/// The text a width can show, with a trailing … when it clips.
#[must_use]
pub fn elide(text: &str, width: f32) -> String {
    if text_width(text) <= width {
        return text.to_string();
    }
    let room = (width / 8.0).floor().max(1.0) as usize;
    let mut clipped: String = text.chars().take(room.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

/// Paint one elided line, with the full text on hover when it clipped.
pub fn elided_label(ui: &mut Ui, text: &str) -> egui::Response {
    let theme = Theme::default();
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
        Sense::hover(),
    );
    let shown = elide(text, rect.width());
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        &shown,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
    if shown == text {
        response
    } else {
        response.on_hover_text(text)
    }
}

/// The alignment a caller asks for. Right alignment keeps a row's value column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelAlign {
    #[default]
    Left,
    Right,
}

/// Paint an elided line at one alignment.
pub fn elided_label_at(ui: &mut Ui, text: &str, align: LabelAlign) -> egui::Response {
    let theme = Theme::default();
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
        Sense::hover(),
    );
    let shown = elide(text, rect.width());
    ui.painter().text(
        if align == LabelAlign::Left {
            rect.left_center()
        } else {
            rect.right_center()
        },
        match align {
            LabelAlign::Left => egui::Align2::LEFT_CENTER,
            LabelAlign::Right => egui::Align2::RIGHT_CENTER,
        },
        &shown,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
    if shown == text {
        response
    } else {
        response.on_hover_text(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_text_is_not_elided() {
        assert_eq!(elide("Portra 400", 200.0), "Portra 400");
    }

    #[test]
    fn a_long_text_ends_in_an_ellipsis() {
        let clipped = elide("a very long film stock name", 80.0);
        assert!(clipped.ends_with('…'));
        assert!(text_width(&clipped) <= 80.0);
    }
}
