//! The Frame tab's Geometry page: the crop and the frame's own straightening.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use negpy_backend::domain::models::geometry::{AutocropMode, with_auto_crop};

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::field_row::{RowColumns, field_row};
use crate::view::widgets::fold::Fold;
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::tab_header::TabSummary;

/// The crop options in the Crop card.
const TOGGLE_CHIP_HEIGHT: f32 = 40.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CropState {
    pub draw: bool,
    pub auto_crop: bool,
    pub ratio: StepCombo,
    pub guide: StepCombo,
    pub mode: StepCombo,
    pub offset: CompactSlider,
    pub rebate_trim: CompactSlider,
    pub square_first: bool,
    pub settings: Fold,
}

impl Default for CropState {
    fn default() -> Self {
        Self {
            draw: false,
            auto_crop: false,
            ratio: StepCombo::new(vec![
                Choice::new("Original"),
                Choice::new("1:1"),
                Choice::new("4:5"),
                Choice::new("2:3"),
            ]),
            guide: StepCombo::new(vec![
                Choice::new("Thirds"),
                Choice::new("Golden Ratio"),
                Choice::new("Diagonals"),
            ]),
            mode: StepCombo::new(vec![Choice::new("Image only"), Choice::new("Film edge")]),
            offset: CompactSlider::new(
                SliderSpec::new(-5.0, 100.0, 1.0, 0)
                    .with_default(0.0)
                    .with_unit(" px"),
            ),
            rebate_trim: CompactSlider::new(
                SliderSpec::new(0.0, 150.0, 5.0, 0)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            square_first: false,
            settings: Fold::new("crop_auto_settings").open_by_default(),
        }
    }
}

/// The Straighten card's state.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightenState {
    pub level: bool,
    pub draw_lines: bool,
    pub fine_rotation: CompactSlider,
    pub tilt: CompactSlider,
    pub swing: CompactSlider,
    pub crop_empty: bool,
}

impl Default for StraightenState {
    fn default() -> Self {
        Self {
            level: false,
            draw_lines: false,
            fine_rotation: CompactSlider::new(
                SliderSpec::new(-5.0, 5.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("°"),
            ),
            tilt: CompactSlider::new(
                SliderSpec::new(-15.0, 15.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            swing: CompactSlider::new(
                SliderSpec::new(-15.0, 15.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            crop_empty: true,
        }
    }
}

/// The Geometry page's state.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryState {
    pub crop: CropState,
    pub straighten: StraightenState,
    pub crop_open: bool,
    pub straighten_open: bool,
}

impl Default for GeometryState {
    fn default() -> Self {
        Self {
            crop: CropState::default(),
            straighten: StraightenState::default(),
            crop_open: true,
            straighten_open: true,
        }
    }
}

impl GeometryState {
    /// Replace the controls with the geometry values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let geometry = &edit.config.geometry;
        self.crop.auto_crop = geometry.crop_from_auto;
        self.crop.draw = geometry.crop_rect.is_some() && !geometry.crop_from_auto;
        self.crop.mode.current = usize::from(geometry.autocrop_mode == AutocropMode::Film);
        self.crop.square_first = geometry.autocrop_square;
        self.crop.offset.set_value(geometry.autocrop_offset as f64);
        self.crop
            .rebate_trim
            .set_value(geometry.autocrop_rebate_trim * 100.0);
        self.straighten
            .fine_rotation
            .set_value(geometry.fine_rotation);
        self.straighten.crop_empty = !geometry.crop_to_valid;
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        let geometry = &mut edit.config.geometry;
        let auto_crop = self.crop.auto_crop;
        let draw = self.crop.draw;
        let mode = if self.crop.mode.current == 1 {
            AutocropMode::Film
        } else {
            AutocropMode::Image
        };
        let offset = self.crop.offset.value.round() as i64;
        let rebate = self.crop.rebate_trim.value / 100.0;
        let crop_to_valid = !self.straighten.crop_empty;
        let changed = geometry.crop_from_auto != auto_crop
            || (geometry.crop_rect.is_some() != draw)
            || geometry.autocrop_mode != mode
            || geometry.autocrop_square != self.crop.square_first
            || geometry.autocrop_offset != offset
            || (geometry.autocrop_rebate_trim - rebate).abs() > 1e-9
            || (geometry.fine_rotation - self.straighten.fine_rotation.value).abs() > 1e-9
            || geometry.crop_to_valid != crop_to_valid;
        if auto_crop != geometry.crop_from_auto {
            edit.config = with_auto_crop(edit.config.clone(), auto_crop);
        } else if !auto_crop && draw != geometry.crop_rect.is_some() {
            geometry.crop_rect = if draw {
                Some(vec![0.0, 0.0, 1.0, 1.0])
            } else {
                None
            };
        }
        let geometry = &mut edit.config.geometry;
        geometry.autocrop_mode = mode;
        geometry.autocrop_square = self.crop.square_first;
        geometry.autocrop_offset = offset;
        geometry.autocrop_rebate_trim = rebate;
        geometry.fine_rotation = self.straighten.fine_rotation.value;
        geometry.crop_to_valid = crop_to_valid;
        changed
    }

    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut changed = TabSummary::new(2).show(ui) != Default::default();
        ui.add_space(f32::from(Theme::default().space_sm));
        changed |= self.show_crop(ui);
        ui.add_space(f32::from(Theme::default().space_sm));
        changed |= self.show_straighten(ui);
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_crop(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let columns = RowColumns::new();
        let mut section = CollapsibleSection::new("Crop").with_guide();
        section.modified = usize::from(self.crop.auto_crop || self.crop.draw);
        section.expanded = self.crop_open;
        let mut changed = false;
        section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                ui.add_space(f32::from(theme.space_md));
                let (row, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), TOGGLE_CHIP_HEIGHT),
                    Sense::hover(),
                );
                let half = (row.width() - f32::from(theme.space_md)) / 2.0;
                let draw = Rect::from_min_size(row.min, Vec2::new(half, row.height()));
                let auto_crop = Rect::from_min_size(
                    egui::pos2(draw.right() + f32::from(theme.space_md), row.top()),
                    Vec2::new(half, row.height()),
                );
                for (rect, label, on) in [
                    (draw, "Draw", self.crop.draw),
                    (auto_crop, "Auto Crop", self.crop.auto_crop),
                ] {
                    ui.painter().rect_filled(
                        rect,
                        4.0,
                        if on { theme.bg_checked } else { theme.bg_input },
                    );
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        Stroke::new(
                            1.0_f32,
                            if on {
                                theme.border_checked
                            } else {
                                theme.border_primary
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center() - Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(f32::from(theme.font_size_base)),
                        if on {
                            theme.text_primary
                        } else {
                            theme.text_secondary
                        },
                    );
                }
                if ui
                    .interact(draw, ui.id().with("crop-draw"), Sense::click())
                    .clicked()
                {
                    self.crop.draw = !self.crop.draw;
                    changed = true;
                }
                if ui
                    .interact(auto_crop, ui.id().with("crop-auto"), Sense::click())
                    .clicked()
                {
                    self.crop.auto_crop = !self.crop.auto_crop;
                    changed = true;
                }
                ui.add_space(f32::from(theme.space_md));
                field_row(ui, columns, "Ratio", |ui| {
                    changed |= self.crop.ratio.show(ui);
                });
                field_row(ui, columns, "Guide", |ui| {
                    changed |= self.crop.guide.show(ui);
                });
                self.crop.settings.group_header(ui, "Auto Crop Settings");
                if self.crop.settings.open {
                    let _ = field_row(ui, columns, "Mode", |ui| {
                        changed |= self.crop.mode.show(ui);
                    });
                    let (_, offset) = self.crop.offset.show(ui, "Crop Offset");
                    changed |= offset;
                    let (_, rebate) = self.crop.rebate_trim.show(ui, "Rebate Trim");
                    changed |= rebate;
                }
                ui.horizontal(|ui| {
                    changed |=
                        labeled_toggle(ui, "Square First", &mut self.crop.square_first).clicked();
                    let _ = labeled_action(ui, "Crop All Frames", ActionRank::Normal);
                });
            });
        });
        self.crop_open = section.expanded;
        changed
    }

    fn show_straighten(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Straighten").with_guide();
        section.expanded = self.straighten_open;
        section.modified = usize::from(
            !self.straighten.fine_rotation.is_neutral()
                || !self.straighten.tilt.is_neutral()
                || !self.straighten.swing.is_neutral(),
        );
        let mut changed = false;
        section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                let _ = section_subheader(ui, "ROTATE AND FLIP");
                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() - 4.0 * (DEFAULT_BUTTON_HEIGHT + 6.0));
                    for (index, icon) in [
                        Icon::RotateLeft,
                        Icon::RotateRight,
                        Icon::FlipHorizontal,
                        Icon::FlipVertical,
                    ]
                    .iter()
                    .enumerate()
                    {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::splat(DEFAULT_BUTTON_HEIGHT),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 4.0, theme.bg_input);
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            Stroke::new(1.0_f32, theme.border_primary),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            icon.glyph(),
                            FontId::proportional(f32::from(theme.font_size_base)),
                            if index == 3 {
                                theme.text_primary
                            } else {
                                theme.text_secondary
                            },
                        );
                    }
                });
                ui.horizontal(|ui| {
                    changed |= labeled_toggle(ui, "Level", &mut self.straighten.level).clicked();
                    let _ = labeled_action(ui, "Square to Film", ActionRank::Normal);
                    changed |=
                        labeled_toggle(ui, "Draw Lines", &mut self.straighten.draw_lines).clicked();
                });
                let (_, fine) = self.straighten.fine_rotation.show(ui, "Fine Rotation");
                changed |= fine;
                let (_, tilt) = self.straighten.tilt.show(ui, "Tilt");
                changed |= tilt;
                let (_, swing) = self.straighten.swing.show(ui, "Swing");
                changed |= swing;
                ui.add_space(f32::from(theme.space_sm));
                changed |=
                    labeled_toggle(ui, "Crop Empty Corners", &mut self.straighten.crop_empty)
                        .clicked();
            });
        });
        self.straighten_open = section.expanded;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_geometry_page_names_its_two_cards() {
        let state = GeometryState::default();
        assert_eq!(state.crop.ratio.current_label(), "Original");
        assert_eq!(state.crop.guide.current_label(), "Thirds");
        assert_eq!(state.crop.mode.current_label(), "Image only");
        assert!(state.straighten.crop_empty);
    }

    #[test]
    fn the_page_keeps_the_open_state_it_loads() {
        let ctx = egui::Context::default();
        let mut state = GeometryState::default();
        state.crop_open = false;
        state.straighten_open = false;
        state.crop.settings.open = false;
        let mut edit = FrameEdit::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, &mut edit);
            });
        });
        assert!(!state.crop_open);
        assert!(!state.straighten_open);
        assert!(!state.crop.settings.open);
    }

    #[test]
    fn the_geometry_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = GeometryState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_geometry_page_and_stores_it() {
        let mut edit = FrameEdit::default();
        edit.config.geometry.fine_rotation = 1.5;
        edit.config.geometry.crop_from_auto = true;
        edit.config.geometry.autocrop_mode = AutocropMode::Film;
        edit.config.geometry.autocrop_square = true;
        edit.config.geometry.autocrop_offset = 7;
        edit.config.geometry.crop_to_valid = true;
        let mut state = GeometryState::default();
        state.load(&edit);
        assert!(state.crop.auto_crop);
        assert_eq!(state.crop.mode.current_label(), "Film edge");
        assert!(state.crop.square_first);
        assert_eq!(state.crop.offset.value, 7.0);
        assert!(!state.straighten.crop_empty);
        assert!((state.straighten.fine_rotation.value - 1.5).abs() < 1e-9);
        assert!(!state.store(&mut edit));
    }
}
