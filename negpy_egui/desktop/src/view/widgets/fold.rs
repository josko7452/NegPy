//! Fine-tune controls behind a chevron: a rail under its parent, or a group under a
//! subheader.
//!
//! A fold starts closed unless its key says it starts open. It gates its content, never
//! its own button, so a disabled mode hides the content and keeps the chevron.

use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::theme::Theme;

/// A fold's saved open state, keyed by the control it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fold {
    pub key: String,
    pub open: bool,
    pub default_open: bool,
    pub warning: bool,
    pub edited: bool,
}

impl Fold {
    /// A fold that starts closed.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            open: false,
            default_open: false,
            warning: false,
            edited: false,
        }
    }

    /// Start open for this key.
    #[must_use]
    pub fn open_by_default(mut self) -> Self {
        self.default_open = true;
        self.open = true;
        self
    }

    /// Mark a closed fold whose content differs from its default.
    pub fn set_edited(&mut self, edited: bool) {
        self.edited = edited;
    }

    /// Mark a closed fold that reports a warning behind it.
    pub fn set_warning(&mut self, warning: bool) {
        self.warning = warning;
    }

    /// The chevron and label a group shows in its subheader.
    #[must_use]
    pub fn chevron(&self) -> Icon {
        if self.open {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        }
    }

    /// Whether the closed fold shows an edited dot or a warning mark.
    #[must_use]
    pub fn marked(&self) -> bool {
        !self.open && (self.edited || self.warning)
    }

    /// Paint the chevron button at the start of `row`. Returns whether the user toggled it.
    pub fn gutter(&mut self, ui: &mut Ui, row: Rect) -> bool {
        let theme = Theme::default();
        let button = Rect::from_min_size(row.min, Vec2::splat(row.height()));
        let response = ui.interact(button, ui.id().with(("fold", &self.key)), Sense::click());
        ui.painter().text(
            button.center(),
            Align2::CENTER_CENTER,
            self.chevron().glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            if response.hovered() {
                theme.text_primary
            } else {
                theme.text_hint
            },
        );
        if self.marked() {
            let center = egui::pos2(button.right() - 3.0, button.top() + 3.0);
            if self.warning {
                ui.painter().circle_filled(center, 4.0, theme.warn_amber);
            } else {
                ui.painter().circle_filled(center, 2.0, theme.mark_edited);
            }
        }
        if response.clicked() {
            self.open = !self.open;
            return true;
        }
        false
    }

    /// Paint a subheader row that folds the group under it. The row is returned so the
    /// caller can place its header actions after the title.
    pub fn group_header(&mut self, ui: &mut Ui, title: &str) -> egui::Response {
        let theme = Theme::default();
        let (row, response) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                crate::styles::layout::DEFAULT_BUTTON_HEIGHT,
            ),
            Sense::click(),
        );
        ui.painter().text(
            row.left_center(),
            Align2::LEFT_CENTER,
            self.chevron().glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_secondary,
        );
        ui.painter().text(
            row.left_center() + Vec2::new(f32::from(theme.space_xl), 0.0),
            Align2::LEFT_CENTER,
            title.to_uppercase(),
            FontId::proportional(f32::from(theme.font_size_base)),
            theme.text_secondary,
        );
        if self.marked() {
            let at = row.left_center()
                + Vec2::new(
                    f32::from(theme.space_xl)
                        + f32::from(theme.font_size_base) * title.len() as f32 * 0.55,
                    0.0,
                );
            if self.warning {
                ui.painter().circle_filled(at, 4.0, theme.warn_amber);
            } else {
                ui.painter().circle_filled(at, 2.0, theme.mark_edited);
            }
        }
        if response.clicked() {
            self.open = !self.open;
        }
        response
    }
}

/// A rail: related controls indented under the control they depend on.
#[must_use]
pub fn rail<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = Theme::default();
    let indent = f32::from(theme.space_lg);
    ui.horizontal(|ui| {
        let height = ui.available_height().max(1.0);
        let (bar, _) = ui.allocate_exact_size(Vec2::new(2.0, height), Sense::hover());
        ui.painter().rect_filled(bar, 1.0, theme.border_color);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - indent);
            body(ui)
        })
        .inner
    })
    .inner
}

/// The hint that a folded rail waits on its parent.
#[must_use]
pub fn fold_hint(ui: &mut Ui, text: &str) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(text)
            .color(theme.text_hint)
            .size(f32::from(theme.font_size_small)),
    )
}

/// Draw a row's label beside its control, before a fold gutter.
#[must_use]
pub fn row_label(ui: &mut Ui, label: &str) -> egui::Response {
    let theme = Theme::default();
    ui.label(
        RichText::new(label)
            .color(theme.text_secondary)
            .size(f32::from(theme.font_size_base)),
    )
}

/// A vertical line that ties a rail to the parent's gutter.
#[must_use]
pub fn rail_line(ui: &mut Ui) -> Stroke {
    let theme = Theme::default();
    ui.painter().line_segment(
        [ui.max_rect().left_top(), ui.max_rect().left_bottom()],
        Stroke::new(2.0_f32, theme.border_color),
    );
    Stroke::new(2.0_f32, theme.border_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fold_starts_closed_and_says_which_chevron_it_shows() {
        let fold = Fold::new("toe_width");
        assert!(!fold.open);
        assert_eq!(fold.chevron(), Icon::ChevronRight);
    }

    #[test]
    fn a_closed_fold_marks_an_edit_or_a_warning() {
        let mut fold = Fold::new("toe_width");
        fold.set_edited(true);
        assert!(fold.marked());
        fold.open = true;
        assert!(!fold.marked());
        fold.open = false;
        fold.set_warning(true);
        assert!(fold.marked());
    }

    #[test]
    fn an_open_by_default_fold_starts_open() {
        assert!(Fold::new("favorites").open_by_default().open);
    }

    #[test]
    fn the_fold_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut fold = Fold::new("toe");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = fold.group_header(ui, "Toe");
                let _ = rail(ui, |ui| {
                    let _ = row_label(ui, "Width");
                });
            });
        });
    }
}
