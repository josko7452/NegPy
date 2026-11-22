//! A rail tab: one icon over a name, or a name beside it.
//!
//! The current tab takes the warm chip. A tab with edits carries a dot beside its name,
//! and an icon-only tab keeps its name for the tooltip and the overflow menu.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::theme::Theme;
use crate::view::widgets::overflow_bar::{OVERFLOW_WIDTH, hidden_tabs, visible_tabs};

const STACKED_ICON: f32 = 21.0;
const BESIDE_ICON: f32 = 17.0;
const DOT: f32 = 6.0;

/// An icon-only tab's width.
pub const ICON_ONLY_WIDTH: f32 = 38.0;

/// One rail tab's look and state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelTab {
    pub icon: Icon,
    pub label: &'static str,
    pub stacked: bool,
    pub icon_only: bool,
    pub divider: bool,
    pub marked: bool,
}

impl PanelTab {
    /// A tab with its name under the icon.
    #[must_use]
    pub fn stacked(icon: Icon, label: &'static str) -> Self {
        Self {
            icon,
            label,
            stacked: true,
            icon_only: false,
            divider: false,
            marked: false,
        }
    }

    /// A tab with its name beside the icon.
    #[must_use]
    pub fn beside(icon: Icon, label: &'static str) -> Self {
        Self {
            icon,
            label,
            stacked: false,
            icon_only: false,
            divider: false,
            marked: false,
        }
    }

    /// Show only the icon, keeping the name for the tooltip.
    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Draw a divider before this tab.
    #[must_use]
    pub fn with_divider(mut self) -> Self {
        self.divider = true;
        self
    }

    /// Mark the tab as holding an edit.
    #[must_use]
    pub fn marked(mut self) -> Self {
        self.marked = true;
        self
    }

    /// The chip's width a stacked tab needs for its name.
    #[must_use]
    pub fn stacked_width(&self) -> f32 {
        self.label.chars().count() as f32 * 8.0 + 2.0 * f32::from(Theme::default().space_md)
    }

    /// Paint the tab into `rect` and report a click.
    pub fn show_in(&self, ui: &mut Ui, rect: Rect, checked: bool) -> egui::Response {
        let theme = Theme::default();
        let response = ui.interact(
            rect,
            ui.id().with(("panel_tab", self.label)),
            Sense::click(),
        );
        let hover = response.hovered();
        if self.divider {
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left(), rect.top() + f32::from(theme.space_xl)),
                    egui::pos2(rect.left(), rect.bottom() - f32::from(theme.space_xl)),
                ],
                Stroke::new(1.0_f32, theme.border_primary),
            );
        }
        let pad = f32::from(theme.space_xs) + 0.5;
        let chip = rect.shrink2(Vec2::new(pad, f32::from(theme.space_sm) + 0.5));
        let radius = f32::from(theme.radius_lg) - pad / 2.0;
        if checked {
            ui.painter().rect_filled(chip, radius, theme.bg_checked);
            ui.painter().rect_stroke(
                chip,
                radius,
                Stroke::new(1.0_f32, theme.border_checked),
                egui::StrokeKind::Inside,
            );
        } else if hover {
            ui.painter()
                .rect_filled(chip, radius, theme.surface_hover_faint);
        }

        let color = if checked || hover {
            theme.text_primary
        } else {
            theme.text_hint
        };
        let name_width = if self.icon_only {
            0.0
        } else {
            self.label.chars().count() as f32 * 8.0
        };
        let (icon_rect, text_center) = if self.stacked {
            let top = (rect.height() - STACKED_ICON - f32::from(theme.space_xs) - 14.0) / 2.0;
            (
                Rect::from_min_size(
                    egui::pos2(rect.center().x - STACKED_ICON / 2.0, rect.top() + top),
                    Vec2::splat(STACKED_ICON),
                ),
                egui::pos2(
                    rect.center().x,
                    rect.top() + top + STACKED_ICON + f32::from(theme.space_xs) + 7.0,
                ),
            )
        } else {
            let left = rect.center().x
                - (BESIDE_ICON
                    + if name_width > 0.0 {
                        f32::from(theme.space_md) + name_width
                    } else {
                        0.0
                    })
                    / 2.0;
            (
                Rect::from_min_size(
                    egui::pos2(left, rect.center().y - BESIDE_ICON / 2.0),
                    Vec2::splat(BESIDE_ICON),
                ),
                egui::pos2(
                    left + BESIDE_ICON + f32::from(theme.space_md) + name_width / 2.0,
                    rect.center().y,
                ),
            )
        };
        ui.painter().text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            self.icon.glyph(),
            FontId::proportional(if self.stacked {
                STACKED_ICON - 5.0
            } else {
                BESIDE_ICON - 2.0
            }),
            color,
        );
        if !self.icon_only {
            ui.painter().text(
                text_center,
                Align2::CENTER_CENTER,
                self.label,
                FontId::proportional(f32::from(theme.font_size_small)),
                color,
            );
            if self.marked {
                let anchor = if self.stacked {
                    egui::pos2(text_center.x + name_width / 2.0, text_center.y - 7.0)
                } else {
                    egui::pos2(text_center.x + name_width / 2.0, text_center.y - 7.0)
                };
                ui.painter().circle_filled(
                    anchor + Vec2::new(f32::from(theme.space_xs), 0.0),
                    DOT / 2.0,
                    theme.mark_edited,
                );
            }
        }
        response.on_hover_text(self.label)
    }
}

/// A strip of panel tabs in one track, returning a clicked tab or an overflow pick.
///
/// A narrow strip keeps the current tab in view and moves the rest into the »
/// menu, as Python's `OverflowBar` does.
pub fn panel_tab_strip(
    ui: &mut Ui,
    tabs: &[PanelTab],
    current: usize,
    height: f32,
) -> Option<usize> {
    let theme = Theme::default();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, f32::from(theme.radius_lg), theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        f32::from(theme.radius_lg),
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    let min_item = tabs
        .iter()
        .map(|tab| {
            if tab.icon_only {
                ICON_ONLY_WIDTH
            } else {
                tab.stacked_width()
            }
        })
        .fold(0.0_f32, f32::max)
        .max(36.0);
    let visible = visible_tabs(tabs.len(), Some(current), rect.width(), min_item);
    let hidden = hidden_tabs(tabs.len(), Some(current), rect.width(), min_item);
    let room = rect.width()
        - if hidden.is_empty() {
            0.0
        } else {
            OVERFLOW_WIDTH
        };
    let width = if visible.is_empty() {
        0.0
    } else {
        room / visible.len() as f32
    };
    let mut x = rect.left();
    let mut picked = None;
    for index in visible {
        let width = if tabs[index].icon_only {
            ICON_ONLY_WIDTH
        } else {
            width
        };
        let tab_rect =
            Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(width, rect.height()));
        if tabs[index]
            .show_in(ui, tab_rect, index == current)
            .clicked()
        {
            picked = Some(index);
        }
        x += width;
    }
    if !hidden.is_empty() {
        let more = Rect::from_min_size(
            egui::pos2(rect.right() - OVERFLOW_WIDTH, rect.top()),
            Vec2::new(OVERFLOW_WIDTH, rect.height()),
        );
        let response = ui.interact(more, ui.id().with("tab-overflow"), Sense::click());
        ui.painter().text(
            more.center(),
            egui::Align2::CENTER_CENTER,
            Icon::More.glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_secondary,
        );
        if let Some(index) = overflow_menu(&response, tabs, &hidden) {
            picked = Some(index);
        }
    }
    picked
}

/// The tabs hidden from the strip, one row each.
fn overflow_menu(response: &egui::Response, tabs: &[PanelTab], hidden: &[usize]) -> Option<usize> {
    egui::Popup::menu(response)
        .show(|ui| {
            ui.set_min_width(170.0);
            let mut picked = None;
            for index in hidden {
                let tab = &tabs[*index];
                if ui
                    .button(format!("{} {}", tab.icon.glyph(), tab.label))
                    .clicked()
                {
                    picked = Some(*index);
                }
            }
            picked
        })
        .and_then(|response| response.inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_paints_only_the_tabs_that_fit() {
        let tabs = [
            PanelTab::stacked(Icon::Grid, "Library"),
            PanelTab::stacked(Icon::Film, "Roll"),
        ];
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(panel_tab_strip(ui, &tabs, 1, 58.0), None);
            });
        });
    }

    #[test]
    fn an_icon_only_tab_keeps_its_fixed_width() {
        assert_eq!(
            PanelTab::beside(Icon::Image, "Frame").icon_only().icon_only,
            true
        );
        assert_eq!(
            PanelTab::stacked(Icon::Grid, "Library").stacked_width(),
            68.0
        );
    }
}
