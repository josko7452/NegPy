//! The right rail's top tab strip: one icon over one label per tab.
//!
//! The selected tab takes the warm chip and a checked edge. The strip keeps the
//! shared tab height, so the rail's cards start at the same place on every tab.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
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
}
