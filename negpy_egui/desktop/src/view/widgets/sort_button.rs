//! A section toolbar's Sort control: one order, then ascending or descending.
//!
//! The button stays icon-only. The menu holds the orders and the two directions as
//! mutually exclusive choices, so the current state reads from the ticks.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, ICON_BUTTON_WIDTH};
use crate::styles::theme::Theme;

/// One sort order, by its stored key and the label the menu shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortOrder {
    pub key: &'static str,
    pub label: &'static str,
}

impl SortOrder {
    /// An order under its stored key.
    #[must_use]
    pub fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

/// A sort control's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortButton {
    pub orders: Vec<SortOrder>,
    pub order: &'static str,
    pub descending: bool,
}

impl SortButton {
    /// A sort with the first order, ascending.
    #[must_use]
    pub fn new(orders: Vec<SortOrder>) -> Self {
        let order = orders.first().map_or("", |order| order.key);
        Self {
            orders,
            order,
            descending: false,
        }
    }

    /// Set the current order and direction without emitting anything.
    pub fn show_order(&mut self, order: &'static str, descending: bool) {
        if self.orders.iter().any(|choice| choice.key == order) {
            self.order = order;
        }
        self.descending = descending;
    }

    /// A change the menu asked for.
    pub fn menu(&self, ui: &mut Ui) -> Option<SortChange> {
        let theme = Theme::default();
        let mut change = None;
        egui::Area::new(ui.id().with("sort_menu"))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    for choice in &self.orders {
                        if ui
                            .selectable_label(choice.key == self.order, choice.label)
                            .clicked()
                        {
                            change = Some(SortChange::Order(choice.key));
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(!self.descending, "Ascending").clicked() {
                        change = Some(SortChange::Direction(false));
                    }
                    if ui.selectable_label(self.descending, "Descending").clicked() {
                        change = Some(SortChange::Direction(true));
                    }
                });
            });
        let _ = theme;
        change
    }

    /// Paint the sort button and open the menu on a click.
    pub fn show(&mut self, ui: &mut Ui, tooltip: &str) -> Option<SortChange> {
        let theme = Theme::default();
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ICON_BUTTON_WIDTH, DEFAULT_BUTTON_HEIGHT),
            Sense::click(),
        );
        let fill = if response.hovered() {
            theme.bg_hover
        } else {
            theme.bg_input
        };
        ui.painter()
            .rect_filled(rect, f32::from(theme.radius_md), fill);
        ui.painter().rect_stroke(
            rect,
            f32::from(theme.radius_md),
            Stroke::new(1.0_f32, theme.border_primary),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            Icon::Menu.glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_primary,
        );
        let _ = response.clone().on_hover_text(tooltip);
        if response.clicked() {
            return self.menu(ui);
        }
        None
    }
}

/// A choice the sort menu reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortChange {
    Order(&'static str),
    Direction(bool),
}

/// The box the sort button occupies in a toolbar.
#[must_use]
pub fn sort_box() -> Rect {
    Rect::from_min_size(
        egui::Pos2::ZERO,
        Vec2::new(ICON_BUTTON_WIDTH, DEFAULT_BUTTON_HEIGHT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sort() -> SortButton {
        SortButton::new(vec![
            SortOrder::new("name", "Name"),
            SortOrder::new("date", "Date"),
        ])
    }

    #[test]
    fn a_new_sort_uses_the_first_order_and_ascends() {
        let sort = sort();
        assert_eq!(sort.order, "name");
        assert!(!sort.descending);
    }

    #[test]
    fn showing_a_saved_order_keeps_a_known_key() {
        let mut sort = sort();
        sort.show_order("date", true);
        assert_eq!(sort.order, "date");
        assert!(sort.descending);
        sort.show_order("missing", false);
        assert_eq!(sort.order, "date");
    }
}
