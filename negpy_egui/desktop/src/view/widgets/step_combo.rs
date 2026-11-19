//! A dropdown drawn as one outlined field split into ‹ | choice | ›.
//!
//! The side arrows step to the neighboring enabled choice and wrap. The face opens
//! the whole list. A caller passes labels and the stored value, never a bare combo.

use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, HEADER_BUTTON_SIZE};
use crate::styles::theme::Theme;

/// One choice and whether its arrows and list row are active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    /// The stored value when it differs from the label, such as a gear id.
    pub id: Option<String>,
    pub enabled: bool,
}

impl Choice {
    /// An enabled choice.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            id: None,
            enabled: true,
        }
    }

    /// An enabled choice that stores `id` instead of its label.
    #[must_use]
    pub fn with_id(label: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            id: Some(id.into()),
            enabled: true,
        }
    }

    /// A grayed choice the arrows step past.
    #[must_use]
    pub fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            id: None,
            enabled: false,
        }
    }
}

/// A dropdown with a stepper on each side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepCombo {
    pub choices: Vec<Choice>,
    pub current: usize,
}

impl StepCombo {
    /// A combo with the first enabled choice current.
    #[must_use]
    pub fn new(choices: Vec<Choice>) -> Self {
        let current = choices
            .iter()
            .position(|choice| choice.enabled)
            .unwrap_or(0);
        Self { choices, current }
    }

    /// The current choice's label.
    #[must_use]
    pub fn current_label(&self) -> &str {
        self.choices
            .get(self.current)
            .map_or("", |choice| choice.label.as_str())
    }

    /// The current choice's stored value: its id when it has one, else its label.
    #[must_use]
    pub fn current_value(&self) -> &str {
        self.choices
            .get(self.current)
            .map_or("", |choice| choice.id.as_deref().unwrap_or(&choice.label))
    }

    /// Make the choice whose stored value is `value` current.
    pub fn pick(&mut self, value: &str) -> bool {
        let Some(index) = self
            .choices
            .iter()
            .position(|choice| choice.id.as_deref().unwrap_or(&choice.label) == value)
        else {
            return false;
        };
        let changed = index != self.current;
        self.current = index;
        changed
    }

    /// The neighboring active item, stepping past grayed rows and wrapping.
    #[must_use]
    pub fn step_target(&self, step: isize) -> Option<usize> {
        let count = self.choices.len();
        if count == 0 {
            return None;
        }
        let start = self.current as isize;
        for k in 1..=count as isize {
            let index = (start + k * step).rem_euclid(count as isize) as usize;
            if index != self.current && self.choices[index].enabled {
                return Some(index);
            }
        }
        None
    }

    /// Move to the neighboring choice. Returns whether the index changed.
    pub fn step(&mut self, step: isize) -> bool {
        let Some(target) = self.step_target(step) else {
            return false;
        };
        self.current = target;
        true
    }

    /// Paint the field and return whether the current choice changed.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
            Sense::hover(),
        );
        let arrow = HEADER_BUTTON_SIZE;
        let prev = Rect::from_min_size(rect.min, Vec2::new(arrow, rect.height()));
        let face = Rect::from_min_max(
            egui::pos2(rect.left() + arrow, rect.top()),
            egui::pos2(rect.right() - arrow, rect.bottom()),
        );
        let next = Rect::from_min_size(
            egui::pos2(rect.right() - arrow, rect.top()),
            Vec2::new(arrow, rect.height()),
        );

        paint_frame(&theme, ui, rect);
        paint_arrow(
            ui,
            prev,
            Icon::ChevronLeft,
            self.step_target(-1).is_some(),
            &theme,
        );
        paint_arrow(
            ui,
            next,
            Icon::ChevronRight,
            self.step_target(1).is_some(),
            &theme,
        );
        ui.painter().text(
            face.left_center() + Vec2::new(f32::from(theme.space_md), 0.0),
            Align2::LEFT_CENTER,
            self.current_label(),
            FontId::proportional(f32::from(theme.font_size_base)),
            theme.text_primary,
        );
        ui.painter().text(
            face.right_center() - Vec2::new(f32::from(theme.space_md), 0.0),
            Align2::RIGHT_CENTER,
            Icon::ChevronDown.glyph(),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_secondary,
        );

        let mut changed = false;
        if ui.allocate_rect(prev, Sense::click()).clicked() {
            changed |= self.step(-1);
        }
        if ui.allocate_rect(next, Sense::click()).clicked() {
            changed |= self.step(1);
        }
        if ui.allocate_rect(face, Sense::click()).clicked() {
            changed |= self.open_menu(ui, face);
        }
        changed
    }

    fn open_menu(&mut self, ui: &mut Ui, anchor: Rect) -> bool {
        let labels: Vec<RichText> = self
            .choices
            .iter()
            .map(|choice| {
                RichText::new(&choice.label).color(if choice.enabled {
                    Theme::default().text_primary
                } else {
                    Theme::default().text_muted
                })
            })
            .collect();
        let mut picked = None;
        egui::Area::new(ui.id().with("step_combo_menu"))
            .fixed_pos(anchor.left_bottom())
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    for (index, label) in labels.into_iter().enumerate() {
                        if !self.choices[index].enabled {
                            continue;
                        }
                        if ui.selectable_label(index == self.current, label).clicked() {
                            picked = Some(index);
                        }
                    }
                });
            });
        let Some(index) = picked else {
            return false;
        };
        let changed = index != self.current;
        self.current = index;
        changed
    }
}

fn paint_frame(theme: &Theme, ui: &Ui, rect: Rect) {
    let hovered = ui.rect_contains_pointer(rect);
    let border = if hovered {
        theme.border_hover
    } else {
        theme.border_primary
    };
    ui.painter().rect_filled(rect, 4.0, theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
}

fn paint_arrow(ui: &Ui, rect: Rect, icon: Icon, enabled: bool, theme: &Theme) {
    let color = if enabled {
        theme.text_secondary
    } else {
        theme.text_muted
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::proportional(f32::from(theme.font_size_small)),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo() -> StepCombo {
        StepCombo::new(vec![
            Choice::new("Auto"),
            Choice::disabled("Disallowed"),
            Choice::new("Manual"),
        ])
    }

    #[test]
    fn a_step_skips_a_grayed_choice_and_wraps() {
        let mut combo = combo();
        assert_eq!(combo.step_target(1), Some(2));
        assert!(combo.step(1));
        assert_eq!(combo.current_label(), "Manual");
        assert_eq!(combo.step_target(1), Some(0));
    }

    #[test]
    fn a_combo_of_one_enabled_choice_does_not_step() {
        let mut combo = StepCombo::new(vec![Choice::new("Only")]);
        assert_eq!(combo.step_target(1), None);
        assert!(!combo.step(-1));
    }

    #[test]
    fn a_choice_stores_its_id_and_can_be_picked_by_it() {
        let mut combo = StepCombo::new(vec![
            Choice::new("Search..."),
            Choice::with_id("Leica M-A", "cam-leica-ma"),
        ]);
        assert_eq!(combo.current_value(), "Search...");
        assert!(combo.pick("cam-leica-ma"));
        assert_eq!(combo.current_label(), "Leica M-A");
        assert_eq!(combo.current_value(), "cam-leica-ma");
    }

    #[test]
    fn the_face_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut combo = combo();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = combo.show(ui);
            });
        });
    }
}
