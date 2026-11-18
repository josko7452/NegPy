//! A label beside a combo or entry, in the column every slider row shares.
//!
//! A card aligns its rows by one width, so a combo's label and a slider's label
//! line up. A grouped row subtracts the group's indent from its label cell.

use eframe::egui::{self, Align, RichText, Ui, Vec2};

use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, FIELD_LABEL_WIDTH};
use crate::styles::theme::Theme;

/// The shared label and value columns of a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowColumns {
    pub label_width: u32,
    pub value_width: u32,
}

impl RowColumns {
    /// The default label and value columns.
    #[must_use]
    pub fn new() -> Self {
        Self {
            label_width: FIELD_LABEL_WIDTH as u32,
            value_width: 72,
        }
    }

    /// The label width inside a rail or a group, which indents its own rows.
    #[must_use]
    pub fn inside(&self, indent: u32) -> Self {
        Self {
            label_width: self.label_width.saturating_sub(indent),
            value_width: self.value_width,
        }
    }
}

impl Default for RowColumns {
    fn default() -> Self {
        Self::new()
    }
}

/// A field label at a column width, right-aligned.
pub fn field_label_at(ui: &mut Ui, text: &str, width: f32) -> egui::Response {
    let theme = Theme::default();
    ui.add_sized(
        Vec2::new(width, DEFAULT_BUTTON_HEIGHT),
        egui::Label::new(
            RichText::new(text)
                .color(theme.text_secondary)
                .size(f32::from(theme.font_size_base)),
        )
        .halign(Align::Max),
    )
}

/// A label and the field it names. `trailing` sits in the value column, so a pair of
/// steppers or a turn button does not narrow the field.
pub fn field_row<R>(
    ui: &mut Ui,
    columns: RowColumns,
    label: &str,
    field: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let _ = field_label_at(ui, label, columns.label_width as f32);
        ui.add_space(f32::from(Theme::default().space_md));
        field(ui)
    })
    .inner
}

/// The widest label and value among a card's rows, for one column width across the card.
#[must_use]
pub fn widest_columns(labels: &[&str], values: &[&str], base: RowColumns) -> RowColumns {
    let widest_value = values
        .iter()
        .map(|value| value.chars().count() as u32 * 8)
        .max()
        .unwrap_or(base.value_width);
    let widest_label = labels
        .iter()
        .map(|label| label.chars().count() as u32 * 8)
        .max()
        .unwrap_or(base.label_width);
    RowColumns {
        label_width: base.label_width.max(widest_label),
        value_width: base.value_width.max(widest_value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grouped_row_shortens_its_label_cell() {
        let columns = RowColumns::new().inside(8);
        assert_eq!(columns.label_width, FIELD_LABEL_WIDTH as u32 - 8);
    }

    #[test]
    fn a_widening_never_shrinks_the_columns() {
        let columns = widest_columns(&["Density"], &["1.00"], RowColumns::new());
        assert_eq!(columns.label_width, FIELD_LABEL_WIDTH as u32);
    }

    #[test]
    fn the_row_paints_on_a_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = field_row(ui, RowColumns::new(), "Film stock", |ui| {
                    ui.label("Kodak Portra 400")
                });
            });
        });
    }
}
