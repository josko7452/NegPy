//! The Frame tab's History page: saved work prints over the frame's edit steps.

use eframe::egui::{self, Align2, FontId, Ui, Vec2};

use negpy_backend::session::workspace::HistoryRow;

use crate::styles::templates::ActionRank;
use crate::styles::templates::labeled_action;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::flat_rows::{FlatRow, paint_fill, paint_pill, two_line_rects};
use crate::view::widgets::tab_header::TabSummary;

/// One step in the frame's edit history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryStep {
    pub number: u32,
    pub label: String,
    pub detail: String,
    pub current: bool,
}

impl HistoryStep {
    /// A step that is not the current one.
    #[must_use]
    pub fn new(number: u32, label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            number,
            label: label.into(),
            detail: detail.into(),
            current: false,
        }
    }

    /// Mark this step as the one the frame is on.
    #[must_use]
    pub fn current(mut self) -> Self {
        self.current = true;
        self
    }
}

/// The History page's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryState {
    pub work_prints: Vec<String>,
    pub steps: Vec<HistoryStep>,
    pub work_prints_open: bool,
    pub history_open: bool,
    pub load_metadata: bool,
    pub selected_step: Option<i64>,
    pub save_requested: bool,
    pub restore_requested: Option<i64>,
    pub selected_work_print: Option<String>,
}

impl Default for HistoryState {
    fn default() -> Self {
        Self::reference()
    }
}

impl HistoryState {
    /// The history the reference frame has.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            work_prints: Vec::new(),
            steps: vec![
                HistoryStep::new(14, "Roll Analysis, Metering", "").current(),
                HistoryStep::new(13, "—", ""),
                HistoryStep::new(12, "Crop", ""),
                HistoryStep::new(11, "Crop", "Auto Crop"),
                HistoryStep::new(10, "Crop", "Auto Crop"),
                HistoryStep::new(9, "Crop", "Auto Crop"),
                HistoryStep::new(8, "Crop", "Auto Crop"),
                HistoryStep::new(7, "Crop", "Auto Crop"),
                HistoryStep::new(6, "Crop", "Crop Ratio"),
                HistoryStep::new(5, "Calibration", "Crosstalk"),
                HistoryStep::new(4, "Calibration", "Crosstalk"),
                HistoryStep::new(3, "Setup", "Film Mode"),
                HistoryStep::new(2, "Crop, Straighten", ""),
                HistoryStep::new(1, "Crop, Straighten", ""),
                HistoryStep::new(0, "As opened", ""),
            ],
            work_prints_open: true,
            history_open: true,
            load_metadata: false,
            selected_step: None,
            save_requested: false,
            restore_requested: None,
            selected_work_print: None,
        }
    }

    /// Replace the rows with a session's stored history, newest first.
    pub fn load_session(&mut self, rows: &[HistoryRow], work_prints: &[String]) {
        self.work_prints = work_prints.to_vec();
        self.steps = rows
            .iter()
            .map(|row| {
                let mut step = HistoryStep::new(
                    u32::try_from(row.index).unwrap_or(0),
                    row.name.clone(),
                    row.detail.clone(),
                );
                step.current = row.current;
                step
            })
            .collect();
    }

    /// Paint the page. Returns whether a control changed the session.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = TabSummary::new(2).show(ui) != Default::default();
        let mut work_prints = CollapsibleSection::new("Work Prints").with_guide();
        work_prints.expanded = self.work_prints_open;
        let (_, prints) = work_prints.show(ui, |ui| {
            card(ui, |ui| {
                let mut changed = false;
                if self.work_prints.is_empty() {
                    let _ = hint(ui, "Keep this edit as a named version.");
                } else {
                    for name in &self.work_prints {
                        let row = FlatRow::new(name.clone(), "");
                        let response = row.show(ui, 30.0);
                        if response.response.clicked() {
                            self.selected_work_print = Some(name.clone());
                            self.restore_requested = Some(-1);
                            changed = true;
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if labeled_action(ui, "Save Work Print...", ActionRank::Normal).clicked() {
                        self.save_requested = true;
                        changed = true;
                    }
                });
                changed
            })
        });
        self.work_prints_open = work_prints.expanded;
        changed |= prints.unwrap_or(false);
        let mut history = CollapsibleSection::new("Edit History").with_guide();
        history.expanded = self.history_open;
        let (_, steps) = history.show(ui, |ui| {
            card(ui, |ui| {
                let mut changed = false;
                let mut restore = None;
                for step in &self.steps {
                    if step_row(ui, step) {
                        changed = true;
                        self.selected_step = Some(i64::from(step.number));
                        restore = Some(i64::from(step.number));
                    }
                }
                if let Some(index) = restore {
                    self.restore_requested = Some(index);
                }
                let _ = hint(
                    ui,
                    "Click a step to go back to it; ... or right-click exports it. The last 100 edits are kept.",
                );
                changed
            })
        });
        self.history_open = history.expanded;
        changed |= steps.unwrap_or(false);
        ui.add_space(f32::from(theme.space_sm));
        changed
    }

    /// Take the one requested restore: a work print by name, or a history step by index.
    pub fn take_restore(&mut self) -> Option<RestoreTarget> {
        let index = self.restore_requested.take()?;
        if index < 0 {
            let name = self.selected_work_print.take()?;
            Some(RestoreTarget::WorkPrint(name))
        } else {
            Some(RestoreTarget::Step(index))
        }
    }

    /// Take the one Save Work Print request.
    pub fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_requested)
    }
}

/// The version a History click asked to reopen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreTarget {
    Step(i64),
    WorkPrint(String),
}

fn step_row(ui: &mut Ui, step: &HistoryStep) -> bool {
    let theme = Theme::default();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::click());
    paint_fill(ui.painter(), rect, step.current, response.hovered(), false);
    let (_name_rect, _detail_rect) = two_line_rects(rect, rect.left() + 12.0, rect.right() - 12.0);
    let number = egui::Rect::from_min_size(
        rect.left_top() + Vec2::new(f32::from(theme.space_lg), 0.0),
        Vec2::new(22.0, rect.height()),
    );
    ui.painter().text(
        number.right_center(),
        Align2::RIGHT_CENTER,
        format!("{}", step.number),
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
    let left = number.right() + f32::from(theme.space_lg);
    let right = rect.right() - f32::from(theme.space_lg) - 48.0;
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(left, rect.top()),
        egui::pos2(right, rect.bottom()),
    );
    ui.painter().text(
        label_rect.left_center(),
        Align2::LEFT_CENTER,
        &step.label,
        FontId::proportional(f32::from(theme.font_size_base)),
        if step.current {
            theme.text_primary
        } else {
            theme.text_secondary
        },
    );
    if !step.detail.is_empty() {
        let detail_x = label_rect.left() + step.label.chars().count() as f32 * 7.0 + 6.0;
        ui.painter().text(
            egui::pos2(detail_x, rect.center().y),
            Align2::LEFT_CENTER,
            format!("· {}", step.detail),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
    }
    if step.current {
        paint_pill(
            ui.painter(),
            rect.right() - f32::from(theme.space_lg),
            rect.center().y,
            "Now",
            true,
        );
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_page_keeps_the_reference_steps() {
        let state = HistoryState::default();
        assert_eq!(state.steps.len(), 15);
        assert!(state.steps[0].current);
        assert_eq!(state.steps.last().unwrap().label, "As opened");
    }

    #[test]
    fn the_history_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = HistoryState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }

    #[test]
    fn a_session_replaces_the_reference_rows() {
        let mut state = HistoryState::default();
        let rows = vec![
            HistoryRow {
                index: 2,
                name: "Crop".to_string(),
                detail: String::new(),
                current: false,
                undone: true,
            },
            HistoryRow {
                index: 1,
                name: "Tone".to_string(),
                detail: "Density".to_string(),
                current: true,
                undone: false,
            },
            HistoryRow {
                index: 0,
                name: "As opened".to_string(),
                detail: String::new(),
                current: false,
                undone: false,
            },
        ];
        state.load_session(&rows, &["First print".to_string()]);
        assert_eq!(state.steps.len(), 3);
        assert_eq!(state.steps[0].number, 2);
        assert!(state.steps[1].current);
        assert_eq!(state.work_prints, vec!["First print".to_string()]);
    }
}
