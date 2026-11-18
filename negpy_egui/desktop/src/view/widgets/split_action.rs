//! A fused main action and menu arrow.
//!
//! The two halves share one height and one border, so a panel's call to action with
//! its menu reads as one control. The main half and the arrow return separate
//! responses, because a menu is not a second action.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, ICON_BUTTON_WIDTH};
use crate::styles::templates::ActionRank;
use crate::styles::theme::Theme;

/// The two halves of a split action.
#[derive(Clone, Debug)]
pub struct SplitAction {
    pub main: egui::Response,
    pub menu: egui::Response,
}

/// Paint a split action across the available width.
pub fn split_action(ui: &mut Ui, icon: Icon, label: &str, rank: ActionRank) -> SplitAction {
    let theme = Theme::default();
    let width = ui.available_width().max(ICON_BUTTON_WIDTH * 2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, DEFAULT_BUTTON_HEIGHT), Sense::hover());
    let menu_rect = Rect::from_min_size(
        egui::pos2(rect.right() - ICON_BUTTON_WIDTH, rect.top()),
        Vec2::new(ICON_BUTTON_WIDTH, rect.height()),
    );
    let main_rect = Rect::from_min_max(rect.min, egui::pos2(menu_rect.left(), rect.bottom()));
    let (fill, border, text) = match rank {
        ActionRank::Normal => (theme.bg_input, theme.border_primary, theme.text_primary),
        ActionRank::Primary => (
            theme.bg_primary,
            theme.border_checked_hover,
            theme.text_on_accent,
        ),
    };
    let main = ui.interact(main_rect, ui.id().with(("split", label, 0)), Sense::click());
    let menu = ui.interact(menu_rect, ui.id().with(("split", label, 1)), Sense::click());
    let fill = if main.hovered() || menu.hovered() {
        match rank {
            ActionRank::Normal => theme.bg_hover,
            ActionRank::Primary => theme.bg_primary_hover,
        }
    } else {
        fill
    };
    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().line_segment(
        [menu_rect.left_top(), menu_rect.left_bottom()],
        Stroke::new(1.0_f32, border),
    );
    ui.painter().text(
        main_rect.center(),
        Align2::CENTER_CENTER,
        format!("{} {}", icon.glyph(), label),
        FontId::proportional(f32::from(theme.font_size_base)),
        text,
    );
    ui.painter().text(
        menu_rect.center(),
        Align2::CENTER_CENTER,
        Icon::ChevronDown.glyph(),
        FontId::proportional(f32::from(theme.font_size_small)),
        text,
    );
    SplitAction { main, menu }
}

/// The two half widths for a total, keeping the menu arrow at the icon-button width.
#[must_use]
pub fn split_widths(total: f32) -> (f32, f32) {
    let menu = ICON_BUTTON_WIDTH.min(total / 2.0);
    (total - menu, menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_half_keeps_the_icon_button_width() {
        assert_eq!(
            split_widths(200.0),
            (200.0 - ICON_BUTTON_WIDTH, ICON_BUTTON_WIDTH)
        );
    }

    #[test]
    fn a_narrow_total_splits_in_half() {
        let (main, menu) = split_widths(40.0);
        assert_eq!(main, 20.0);
        assert_eq!(menu, 20.0);
    }

    #[test]
    fn the_split_action_paints_on_a_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let action = split_action(ui, Icon::Export, "Export", ActionRank::Primary);
                assert!(!action.main.clicked());
                assert!(!action.menu.clicked());
            });
        });
    }
}
