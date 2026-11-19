//! The Scan tab: the scanner choice over its device, quality, framing and output cards.

use eframe::egui::{self, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::split_action::split_action;
use crate::view::widgets::step_combo::{Choice, StepCombo};

/// The Scan tab's state.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanState {
    pub source: SegmentedChoice<'static>,
    pub backend: StepCombo,
    pub device: StepCombo,
    pub film: StepCombo,
    pub film_format: StepCombo,
    pub resolution: StepCombo,
    pub bit_depth: StepCombo,
    pub scan_mode: StepCombo,
    pub ir: bool,
    pub dust_removal: SegmentedChoice<'static>,
    pub framing_open: bool,
    pub camera: StepCombo,
    pub light: SegmentedChoice<'static>,
    pub output_format: StepCombo,
    pub output_folder: String,
    pub filename: String,
    pub device_open: bool,
    pub quality_open: bool,
    pub output_open: bool,
}

impl Default for ScanState {
    fn default() -> Self {
        Self {
            source: SegmentedChoice::new(&["Camera", "Film Scanner"]),
            backend: StepCombo::new(vec![
                Choice::new("Auto"),
                Choice::new("SANE"),
                Choice::new("NKS"),
            ]),
            device: StepCombo::new(vec![
                Choice::new("Detecting scanners..."),
                Choice::new("Plustek OpticFilm 8200i"),
            ]),
            film: StepCombo::new(vec![
                Choice::new("Color negative"),
                Choice::new("Black and white negative"),
                Choice::new("Slide"),
            ]),
            film_format: StepCombo::new(vec![
                Choice::new("35 mm"),
                Choice::new("120"),
                Choice::new("Auto"),
            ]),
            resolution: StepCombo::new(vec![Choice::new("3600 dpi"), Choice::new("7200 dpi")]),
            bit_depth: StepCombo::new(vec![Choice::new("16 bit"), Choice::new("8 bit")]),
            scan_mode: StepCombo::new(vec![Choice::new("Single pass"), Choice::new("Multi pass")]),
            ir: false,
            dust_removal: SegmentedChoice::new(&["Off", "Auto"]),
            framing_open: false,
            camera: StepCombo::new(vec![Choice::new("No camera"), Choice::new("OM-3")]),
            light: SegmentedChoice::new(&["White Light", "Narrowband Light"]),
            output_format: StepCombo::new(vec![Choice::new("DNG"), Choice::new("TIFF")]),
            output_folder: String::new(),
            filename: "{{ date }}_{{ seq }}".to_string(),
            device_open: false,
            quality_open: false,
            output_open: true,
        }
    }
}

impl ScanState {
    /// Paint the tab. Returns whether a control changed the setup.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        let mut summary = CollapsibleSection::new("Scanner").with_guide();
        summary.expanded = true;
        let (_, result) = summary.show(ui, |ui| {
            card(ui, |ui| {
                let changed = self.source.show(ui).is_some();
                ui.add_space(f32::from(theme.space_sm));
                changed
            })
        });
        changed |= result.unwrap_or(false);
        let camera_active = self.source.current == 0;
        if !camera_active {
            let (device_open, device_changed) = scan_card(ui, "Device", self.device_open, |ui| {
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Backend");
                    changed |= self.backend.show(ui);
                });
                ui.horizontal(|ui| {
                    ui.label("Device");
                    changed |= self.device.show(ui);
                    let _ = labeled_action(ui, "Refresh", ActionRank::Normal);
                });
                let _ = hint(ui, "Writes nkscan.log with Trace for a bug report.");
                changed
            });
            self.device_open = device_open;
            changed |= device_changed;
            let (quality_open, quality_changed) =
                scan_card(ui, "Film & Quality", self.quality_open, |ui| {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Film");
                        changed |= self.film.show(ui);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Film format");
                        changed |= self.film_format.show(ui);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Resolution");
                        changed |= self.resolution.show(ui);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Bit depth");
                        changed |= self.bit_depth.show(ui);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scan mode");
                        changed |= self.scan_mode.show(ui);
                    });
                    changed |= labeled_toggle(ui, "IR", &mut self.ir).clicked();
                    changed
                });
            self.quality_open = quality_open;
            changed |= quality_changed;
            let (framing_open, framing_changed) =
                scan_card(ui, "Framing", self.framing_open, |ui| {
                    let _ = hint(ui, "Prescan sets a crop for the next scan.");
                    let _ = labeled_action(ui, "Prescan...", ActionRank::Normal);
                    false
                });
            self.framing_open = framing_open;
            changed |= framing_changed;
        }
        let (_, camera_changed) = scan_card(ui, "Camera", camera_active, |ui| {
            let mut changed = false;
            let _ = hint(
                ui,
                "Camera scanning needs python-gphoto2 (`pip install gphoto2`), which has no Windows build. See docs/CAMERA_SCANNING.md.",
            );
            let _ = hint(
                ui,
                "Connect the camera by USB, in PC Remote mode. NegPy finds it by itself.",
            );
            ui.horizontal(|ui| {
                ui.label("● Camera");
                ui.add_space(f32::from(theme.space_lg));
                ui.label("● Light");
            });
            ui.horizontal(|ui| {
                ui.label("Camera");
                changed |= self.camera.show(ui);
            });
            changed
        });
        changed |= camera_changed;
        let (_, light_changed) = scan_card(ui, "Preset", false, |ui| self.light.show(ui).is_some());
        changed |= light_changed;
        let (output_open, output_changed) =
            scan_card(ui, "Output", self.output_open, |ui| self.show_output(ui));
        self.output_open = output_open;
        changed |= output_changed;
        ui.add_space(f32::from(theme.space_md));
        let _ = split_action(ui, Icon::Scan, "Scan", ActionRank::Primary);
        changed
    }

    fn show_output(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Folder");
            let _ = ui.add_sized(
                Vec2::new(ui.available_width() - 36.0, DEFAULT_BUTTON_HEIGHT),
                egui::TextEdit::singleline(&mut self.output_folder).hint_text("Output folder..."),
            );
            let _ = labeled_action(ui, "Folder", ActionRank::Normal);
        });
        let _ = card(ui, |ui| {
            ui.horizontal(|ui| {
                changed |= labeled_action(ui, "Scan as Roll", ActionRank::Normal).clicked();
                changed |= labeled_action(ui, "Folder as Roll", ActionRank::Normal).clicked();
            });
            ui.horizontal(|ui| {
                ui.label("Roll");
                let _ = ui.text_edit_singleline(&mut self.filename);
                let _ = labeled_action(ui, "+", ActionRank::Normal);
            });
        });
        changed
    }
}

fn scan_card(
    ui: &mut Ui,
    title: &str,
    expanded: bool,
    body: impl FnOnce(&mut Ui) -> bool,
) -> (bool, bool) {
    let mut section = CollapsibleSection::new(title).with_guide();
    section.expanded = expanded;
    let (_, result) = section.show(ui, |ui| card(ui, body));
    ui.add_space(f32::from(Theme::default().space_sm));
    (section.expanded, result.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_tab_opens_on_the_camera() {
        let state = ScanState::default();
        assert_eq!(state.source.current_label(), "Camera");
        assert_eq!(state.backend.current_label(), "Auto");
        assert_eq!(state.scan_mode.current_label(), "Single pass");
        assert!(!state.device_open);
        assert!(!state.quality_open);
        assert!(state.output_open);
    }

    #[test]
    fn the_scan_tab_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ScanState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
