//! Shared control factories and text helpers from `templates.py`.
//!
//! A panel calls one of these instead of styling a raw widget. The look stays in one
//! place, and a new size or color token needs the user's agreement.

use eframe::egui::{self, Align, Color32, FontId, RichText, Sense, Stroke, Ui, Vec2};

use super::layout::{
    BAR_BUTTON_HEIGHT, FIELD_LABEL_WIDTH, HEADER_BUTTON_SIZE, HEADER_HEIGHT, ICON_BUTTON_WIDTH,
};
use super::theme::Theme;

/// A control that draws a label with one of the theme's text roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    Primary,
    Secondary,
    Hint,
    Unit,
    Error,
    Success,
}

impl TextRole {
    /// The color this role reads.
    #[must_use]
    pub fn color(self, theme: &Theme) -> Color32 {
        match self {
            Self::Primary => theme.text_primary,
            Self::Secondary => theme.text_secondary,
            Self::Hint => theme.text_hint,
            Self::Unit => theme.text_unit,
            Self::Error => theme.error_text,
            Self::Success => theme.status_success,
        }
    }
}

/// A labeled action's rank in its panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionRank {
    #[default]
    Normal,
    Primary,
}

/// One-shot label button, the non-checkable twin of a labeled toggle.
pub fn labeled_action(ui: &mut Ui, label: &str, rank: ActionRank) -> egui::Response {
    let theme = Theme::default();
    let (fill, border, text) = match rank {
        ActionRank::Normal => (theme.bg_input, theme.border_primary, theme.text_primary),
        ActionRank::Primary => (
            theme.bg_primary,
            theme.border_checked_hover,
            theme.text_on_accent,
        ),
    };
    let button = egui::Button::new(RichText::new(label).color(text))
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .min_size(Vec2::new(0.0, super::layout::DEFAULT_BUTTON_HEIGHT));
    ui.add(button)
}

/// Checkable row toggle. The checked chip is the warm surface, never a bright accent.
pub fn labeled_toggle(ui: &mut Ui, label: &str, checked: &mut bool) -> egui::Response {
    let theme = Theme::default();
    let fill = if *checked {
        theme.bg_checked
    } else {
        theme.bg_input
    };
    let border = if *checked {
        theme.border_checked
    } else {
        theme.border_primary
    };
    let text = if *checked {
        theme.text_primary
    } else {
        theme.text_hint
    };
    let button = egui::Button::new(RichText::new(label).color(text))
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .min_size(Vec2::new(0.0, super::layout::DEFAULT_BUTTON_HEIGHT));
    let response = ui.add(button);
    if response.clicked() {
        *checked = !*checked;
    }
    response
}

/// An icon-only action at the shared icon-button width.
pub fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> egui::Response {
    let theme = Theme::default();
    let button = egui::Button::new(RichText::new(glyph).color(theme.text_primary))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(
            ICON_BUTTON_WIDTH,
            super::layout::DEFAULT_BUTTON_HEIGHT,
        ));
    ui.add(button).on_hover_text(tooltip)
}

/// A section header's small flat icon button.
pub fn header_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> egui::Response {
    let theme = Theme::default();
    let button = egui::Button::new(RichText::new(glyph).color(theme.text_primary))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .min_size(Vec2::splat(HEADER_BUTTON_SIZE));
    ui.add(button).on_hover_text(tooltip)
}

/// A section card's header row, with the shared header height and title role.
pub fn section_header(ui: &mut Ui, title: &str) -> egui::Response {
    let theme = Theme::default();
    ui.allocate_ui(Vec2::new(ui.available_width(), HEADER_HEIGHT), |ui| {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 0.0, theme.bg_header);
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0_f32, theme.border_primary),
        );
        ui.painter().text(
            rect.left_center() + Vec2::new(theme.space_lg as f32, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            FontId::proportional(f32::from(theme.font_size_header)),
            theme.text_primary,
        );
        ui.allocate_rect(rect, Sense::hover())
    })
    .inner
}

/// A label beside a combo or entry.
pub fn field_label(ui: &mut Ui, text: &str) -> egui::Response {
    let theme = Theme::default();
    ui.add_sized(
        Vec2::new(FIELD_LABEL_WIDTH, super::layout::DEFAULT_BUTTON_HEIGHT),
        egui::Label::new(
            RichText::new(text)
                .color(theme.text_secondary)
                .size(f32::from(theme.font_size_base)),
        )
        .halign(Align::Max),
    )
}

/// One line of help under a control.
pub fn hint_label(ui: &mut Ui, text: &str, role: TextRole) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(text)
            .color(role.color(&theme))
            .size(f32::from(theme.font_size_small)),
    )
}

/// An all-caps grouping label.
pub fn section_subheader(ui: &mut Ui, text: &str) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(text.to_uppercase())
            .color(theme.text_secondary)
            .size(f32::from(theme.font_size_base))
            .strong(),
    )
}

/// A label in the control's own color role.
pub fn text_label(ui: &mut Ui, text: &str, role: TextRole) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(text)
            .color(role.color(&theme))
            .size(f32::from(theme.font_size_base)),
    )
}

/// A button that fills the canvas bar's shared height.
pub fn bar_button(ui: &mut Ui, label: &str) -> egui::Response {
    let theme = Theme::default();
    let button = egui::Button::new(RichText::new(label).color(theme.text_primary))
        .fill(theme.bg_input)
        .stroke(Stroke::new(1.0_f32, theme.border_primary))
        .min_size(Vec2::new(0.0, BAR_BUTTON_HEIGHT));
    ui.add(button)
}

/// A panel's one call to action.
pub fn primary_action(ui: &mut Ui, label: &str) -> egui::Response {
    labeled_action(ui, label, ActionRank::Primary)
}

/// The label a toggle shows beside its icon.
#[must_use]
pub fn icon_text(label: &str) -> String {
    if label.is_empty() {
        String::new()
    } else {
        format!(" {label}")
    }
}

/// A color with an alpha channel applied.
#[must_use]
pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Theme::with_alpha(color, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_label_never_starts_without_its_gap() {
        assert_eq!(icon_text("Print"), " Print");
        assert_eq!(icon_text(""), "");
    }

    #[test]
    fn each_text_role_reads_its_theme_token() {
        let theme = Theme::default();
        assert_eq!(TextRole::Primary.color(&theme), theme.text_primary);
        assert_eq!(TextRole::Error.color(&theme), theme.error_text);
    }

    #[test]
    fn the_factories_draw_on_a_context() {
        let ctx = egui::Context::default();
        let mut checked = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = labeled_action(ui, "Apply", ActionRank::Primary);
                let _ = labeled_toggle(ui, "Camera", &mut checked);
                let _ = icon_button(ui, "×", "Clear");
                let _ = header_button(ui, "i", "Help");
                let _ = section_header(ui, "Analysis");
                let _ = field_label(ui, "Film stock");
                let _ = hint_label(ui, "Hint", TextRole::Hint);
                let _ = section_subheader(ui, "Rolls");
                let _ = text_label(ui, "Body", TextRole::Secondary);
                let _ = bar_button(ui, "Photo");
                let _ = primary_action(ui, "Export");
            });
        });
    }
}
