//! The right rail's top tab strip: one icon over one label per tab.
//!
//! The selected tab takes the warm chip and a checked edge. The strip keeps the
//! shared tab height, so the rail's cards start at the same place on every tab.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, HEADER_BUTTON_SIZE};
use crate::styles::theme::Theme;

/// The tab strip's height.
pub const TAB_STRIP_HEIGHT: f32 = 58.0;

/// One tab: its label and the icon above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabHeaderItem {
    pub label: &'static str,
    pub icon: Icon,
}

/// Paint the strip and return the tab the user clicked, if any.
pub fn tab_header(ui: &mut Ui, items: &[TabHeaderItem], current: usize) -> Option<usize> {
    let theme = Theme::default();
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), TAB_STRIP_HEIGHT),
        Sense::hover(),
    );
    if items.is_empty() {
        return None;
    }
    let width = rect.width() / items.len() as f32;
    let mut picked = None;
    for (index, item) in items.iter().enumerate() {
        let tab = Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * width, rect.top()),
            Vec2::new(width, rect.height()),
        );
        let response = ui.interact(tab, ui.id().with(("tab", index)), Sense::click());
        let selected = index == current;
        let fill = if selected {
            if response.hovered() {
                theme.bg_checked_hover
            } else {
                theme.bg_checked
            }
        } else if response.hovered() {
            theme.bg_hover
        } else {
            theme.bg_panel
        };
        ui.painter().rect_filled(tab, 0.0, fill);
        if selected {
            ui.painter().rect_stroke(
                tab,
                0.0,
                Stroke::new(1.0_f32, theme.border_checked),
                egui::StrokeKind::Inside,
            );
        }
        let color = if selected {
            theme.text_primary
        } else {
            theme.text_hint
        };
        ui.painter().text(
            tab.center() - Vec2::new(0.0, 9.0),
            Align2::CENTER_CENTER,
            item.icon.glyph(),
            FontId::proportional(16.0),
            color,
        );
        ui.painter().text(
            tab.center() + Vec2::new(0.0, 15.0),
            Align2::CENTER_CENTER,
            item.label,
            FontId::proportional(f32::from(theme.font_size_small)),
            color,
        );
        if response.clicked() {
            picked = Some(index);
        }
    }
    picked
}

/// One tab's summary bar: how many cards hold an edit, and the actions that reach them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabSummary {
    pub edited: usize,
    pub cards: usize,
    pub title: String,
    pub apply: bool,
    pub reset: bool,
    pub collapse: bool,
    /// Whether the cards under this summary are collapsed.
    pub collapsed: bool,
}

impl TabSummary {
    /// A tab with every card unedited.
    #[must_use]
    pub fn new(cards: usize) -> Self {
        Self {
            edited: 0,
            cards,
            title: String::new(),
            apply: true,
            reset: false,
            collapse: true,
            collapsed: false,
        }
    }

    /// Draw the chevron for a collapsed page.
    #[must_use]
    pub fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Hide the Apply action on a tab whose settings do not copy to other frames.
    #[must_use]
    pub fn without_apply(mut self) -> Self {
        self.apply = false;
        self
    }

    /// Show the Reset action when a card holds an edit.
    #[must_use]
    pub fn with_reset(mut self) -> Self {
        self.reset = true;
        self
    }

    /// The state line under the tab strip.
    #[must_use]
    pub fn label(&self) -> String {
        if !self.title.is_empty() {
            self.title.clone()
        } else if self.edited == 0 {
            "No cards edited".to_string()
        } else {
            format!(
                "{} of {} edited",
                self.edited,
                if self.cards == 1 {
                    "1 card".to_string()
                } else {
                    format!("{} cards", self.cards)
                }
            )
        }
    }

    /// Replace the state line with a tab that owns no editable cards.
    #[must_use]
    pub fn titled(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Paint the bar and return a click on Apply, Reset or the collapse button.
    pub fn show(&self, ui: &mut Ui) -> TabSummaryAction {
        let theme = Theme::default();
        let (row, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT + 8.0),
            Sense::hover(),
        );
        let left = row.left() + f32::from(theme.space_xl);
        if self.edited > 0 {
            ui.painter().circle_filled(
                egui::pos2(left + 3.0, row.center().y),
                3.0,
                theme.mark_edited,
            );
        }
        ui.painter().text(
            egui::pos2(left + 12.0, row.center().y),
            Align2::LEFT_CENTER,
            self.label(),
            FontId::proportional(f32::from(theme.font_size_base)),
            theme.text_secondary,
        );
        let mut right = row.right() - f32::from(theme.space_xl);
        let mut action = TabSummaryAction::None;
        if self.collapse {
            let icon = if self.collapsed {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            };
            let (next, clicked) = summary_button(ui, right, row.center().y, icon, &theme);
            if clicked {
                action = TabSummaryAction::Collapse;
            }
            right = next;
        }
        if self.apply {
            let (next, clicked) = summary_button(ui, right, row.center().y, Icon::Film, &theme);
            if clicked {
                action = TabSummaryAction::Apply;
            }
            right = next;
        }
        if self.reset {
            let (_next, clicked) = summary_button(ui, right, row.center().y, Icon::Reset, &theme);
            if clicked {
                action = TabSummaryAction::Reset;
            }
        }
        action
    }
}

/// What a tab summary bar clicked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSummaryAction {
    #[default]
    None,
    Apply,
    Reset,
    Collapse,
}

fn summary_button(
    ui: &mut Ui,
    right: f32,
    center_y: f32,
    icon: Icon,
    theme: &Theme,
) -> (f32, bool) {
    let rect = Rect::from_center_size(
        egui::pos2(right - HEADER_BUTTON_SIZE / 2.0, center_y),
        Vec2::splat(HEADER_BUTTON_SIZE),
    );
    let response = ui.allocate_rect(rect, Sense::click());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::proportional(f32::from(theme.font_size_small)),
        if response.hovered() {
            theme.text_primary
        } else {
            theme.text_hint
        },
    );
    (right - HEADER_BUTTON_SIZE, response.clicked())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: [TabHeaderItem; 2] = [
        TabHeaderItem {
            label: "Library",
            icon: Icon::Grid,
        },
        TabHeaderItem {
            label: "Roll",
            icon: Icon::Film,
        },
    ];

    #[test]
    fn the_strip_keeps_the_shared_height() {
        assert_eq!(TAB_STRIP_HEIGHT, 58.0);
    }

    #[test]
    fn the_strip_paints_on_a_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(tab_header(ui, &ITEMS, 1), None);
            });
        });
    }

    #[test]
    fn a_tab_summary_counts_only_the_cards_it_names() {
        let mut summary = TabSummary::new(4);
        assert_eq!(summary.label(), "No cards edited");
        summary.edited = 2;
        assert_eq!(summary.label(), "2 of 4 cards edited");
        summary.cards = 1;
        assert_eq!(summary.label(), "2 of 1 card edited");
    }

    #[test]
    fn a_tab_summary_keeps_apply_unless_the_tab_says_otherwise() {
        assert!(TabSummary::new(2).apply);
        assert!(!TabSummary::new(2).without_apply().apply);
        assert!(TabSummary::new(2).with_reset().reset);
    }

    #[test]
    fn a_tab_summary_paints_on_a_context() {
        let ctx = egui::Context::default();
        let summary = TabSummary::new(3);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(summary.show(ui), TabSummaryAction::None);
            });
        });
    }
}
