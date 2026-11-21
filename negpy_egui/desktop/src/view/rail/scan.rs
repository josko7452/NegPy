//! The Scan tab: the scanner choice over its device, quality, framing and output cards.

use eframe::egui::{self, Ui, Vec2};
use negpy_backend::services::scanning::params::ScanParams;
use negpy_backend::services::scanning::registry;

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
    pub camera_open: bool,
    pub preset_open: bool,
    pub camera: StepCombo,
    pub light: SegmentedChoice<'static>,
    pub output_format: StepCombo,
    pub output_folder: String,
    pub filename: String,
    pub device_open: bool,
    pub quality_open: bool,
    pub output_open: bool,
    pub status: String,
    pub scan_requested: bool,
    /// The output folder the user asked to import as a roll.
    pub folder_as_roll_requested: bool,
    pub devices: Vec<negpy_backend::services::scanning::transport::ScannerDevice>,
}

impl Default for ScanState {
    fn default() -> Self {
        Self {
            source: SegmentedChoice {
                labels: &["Film Scanner", "Camera"],
                icons: &[Some(Icon::CameraRotate), Some(Icon::Camera)],
                current: 1,
            },
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
            camera_open: true,
            preset_open: false,
            camera: StepCombo::new(vec![Choice::new("No camera"), Choice::new("OM-3")]),
            light: SegmentedChoice::new(&["White Light", "Narrowband Light"]),
            output_format: StepCombo::new(vec![Choice::new("DNG"), Choice::new("TIFF")]),
            output_folder: String::new(),
            filename: "{{ date }}_{{ seq }}".to_string(),
            device_open: false,
            quality_open: false,
            output_open: true,
            status: String::new(),
            scan_requested: false,
            folder_as_roll_requested: false,
            devices: Vec::new(),
        }
    }
}

impl ScanState {
    /// Reload the device list for the selected backend.
    pub fn refresh_devices(&mut self) -> bool {
        let backend = self.backend.current_value();
        match registry::list_devices(backend) {
            Ok(devices) => {
                self.devices = devices;
                self.status.clear();
                let choices = self.devices.iter().map(device_choice).collect();
                let selected = self.device.current_value().to_string();
                self.device = StepCombo::new(choices);
                if self.devices.len() > 1 {
                    self.device.pick(&selected);
                }
                let selected = self.selected_device().cloned();
                if let Some(device) = selected {
                    self.set_capabilities(&device);
                }
                true
            }
            Err(error) => {
                self.devices.clear();
                self.status = error;
                false
            }
        }
    }

    fn set_capabilities(
        &mut self,
        device: &negpy_backend::services::scanning::transport::ScannerDevice,
    ) {
        let caps = &device.capabilities;
        if !caps.supported_dpi.is_empty() {
            let selected = self.resolution.current_value().to_string();
            self.resolution = StepCombo::new(
                caps.supported_dpi
                    .iter()
                    .map(|dpi| Choice::with_id(format!("{dpi} dpi"), dpi.to_string()))
                    .collect(),
            );
            self.resolution.pick(&selected);
        }
        if !caps.supported_depths.is_empty() {
            let selected = self.bit_depth.current_value().to_string();
            self.bit_depth = StepCombo::new(
                caps.supported_depths
                    .iter()
                    .map(|depth| Choice::with_id(format!("{depth} bit"), depth.to_string()))
                    .collect(),
            );
            self.bit_depth.pick(&selected);
        }
        if !caps.film_formats.is_empty() {
            let selected = self.film_format.current_value().to_string();
            self.film_format = StepCombo::new(
                caps.film_formats
                    .iter()
                    .map(|format| Choice::with_id(format, format))
                    .collect(),
            );
            self.film_format.pick(&selected);
        }
        self.ir = self.ir && caps.ir_channel;
        if !caps.ir_channel {
            self.ir = false;
        }
    }

    /// Whether the selected device offers the choice the scan needs.
    #[must_use]
    pub fn selected_device(
        &self,
    ) -> Option<&negpy_backend::services::scanning::transport::ScannerDevice> {
        let id = self.device.current_value();
        self.devices.iter().find(|device| device.id == id)
    }

    /// The one scan request the Scan action asked for.
    pub fn take_scan_request(&mut self) -> bool {
        std::mem::take(&mut self.scan_requested)
    }

    /// Take the one Folder as Roll request, with its folder.
    #[must_use]
    pub fn take_folder_as_roll_request(&mut self) -> Option<std::path::PathBuf> {
        if !std::mem::take(&mut self.folder_as_roll_requested) {
            return None;
        }
        let folder = self.output_folder.trim();
        (!folder.is_empty()).then(|| std::path::PathBuf::from(folder))
    }

    /// Show the result of the last scan.
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    /// Build the scan parameters from the visible controls.
    #[must_use]
    pub fn params(&self) -> ScanParams {
        let dpi = self
            .resolution
            .current_value()
            .split_whitespace()
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(3600);
        let depth = self
            .bit_depth
            .current_value()
            .split_whitespace()
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(16);
        ScanParams {
            dpi,
            depth,
            capture_ir: self.ir,
            ..ScanParams::default()
        }
    }

    /// Paint the tab. Returns whether a control changed the setup.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        let mut summary = CollapsibleSection::new("Scanner")
            .with_guide()
            .with_icon(Icon::Exchange);
        summary.expanded = true;
        let (_, result) = summary.show(ui, |ui| {
            card(ui, |ui| {
                let changed = self.source.show(ui).is_some();
                ui.add_space(f32::from(theme.space_sm));
                changed
            })
        });
        changed |= result.unwrap_or(false);
        let camera_active = self.source.current == 1;
        if !camera_active {
            let (device_open, device_changed) =
                scan_card(ui, "Device", Icon::Plug, self.device_open, |ui| {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Backend");
                        if self.backend.show(ui) {
                            changed = true;
                            self.refresh_devices();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Device");
                        changed |= self.device.show(ui);
                        if labeled_action(ui, "Refresh", ActionRank::Normal).clicked() {
                            changed = true;
                            self.refresh_devices();
                        }
                    });
                    if !self.status.is_empty() {
                        let _ = hint(ui, &self.status);
                    }
                    let _ = hint(ui, "Writes nkscan.log with Trace for a bug report.");
                    changed
                });
            self.device_open = device_open;
            changed |= device_changed;
            let (quality_open, quality_changed) = scan_card(
                ui,
                "Film & Quality",
                Icon::Sliders,
                self.quality_open,
                |ui| {
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
                },
            );
            self.quality_open = quality_open;
            changed |= quality_changed;
            let (framing_open, framing_changed) =
                scan_card(ui, "Framing", Icon::Crop, self.framing_open, |ui| {
                    let _ = hint(
                        ui,
                        "Prescan needs a scanner worker that is not yet ported to Rust.",
                    );
                    let _ = ui.add_enabled(
                        false,
                        egui::Button::new("Prescan...")
                            .min_size(Vec2::new(0.0, DEFAULT_BUTTON_HEIGHT)),
                    );
                    false
                });
            self.framing_open = framing_open;
            changed |= framing_changed;
        }
        if camera_active {
            let (camera_open, camera_changed) = scan_card(
                ui,
                "Camera",
                Icon::Camera,
                self.camera_open,
                |ui| {
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
                },
            );
            self.camera_open = camera_open;
            changed |= camera_changed;
        }
        let (preset_open, light_changed) =
            scan_card(ui, "Preset", Icon::Lightbulb, self.preset_open, |ui| {
                self.light.show(ui).is_some()
            });
        self.preset_open = preset_open;
        changed |= light_changed;
        let (output_open, output_changed) =
            scan_card(ui, "Output", Icon::FolderOpen, self.output_open, |ui| {
                self.show_output(ui)
            });
        self.output_open = output_open;
        changed |= output_changed;
        ui.add_space(f32::from(theme.space_md));
        if split_action(ui, Icon::Scan, "Scan", ActionRank::Primary)
            .main
            .clicked()
        {
            self.scan_requested = true;
            changed = true;
        }
        changed
    }

    fn show_output(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Folder");
            changed |= ui
                .add_sized(
                    Vec2::new(ui.available_width() - 36.0, DEFAULT_BUTTON_HEIGHT),
                    egui::TextEdit::singleline(&mut self.output_folder)
                        .hint_text("Output folder..."),
                )
                .changed();
            if labeled_action(ui, "Folder", ActionRank::Normal).clicked()
                && let Some(folder) = rfd::FileDialog::new()
                    .set_title("Scan Output Folder")
                    .pick_folder()
            {
                self.output_folder = folder.display().to_string();
                changed = true;
            }
        });
        let _ = card(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = ui.add_enabled(
                    false,
                    egui::Button::new("Scan as Roll")
                        .min_size(Vec2::new(0.0, DEFAULT_BUTTON_HEIGHT)),
                );
                if labeled_action(ui, "Folder as Roll", ActionRank::Normal).clicked() {
                    if self.output_folder.trim().is_empty() {
                        self.status = "Choose the output folder first.".to_string();
                    } else {
                        self.folder_as_roll_requested = true;
                    }
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Roll");
                changed |= ui.text_edit_singleline(&mut self.filename).changed();
                let _ = ui.add_enabled(
                    false,
                    egui::Button::new("+").min_size(Vec2::new(0.0, DEFAULT_BUTTON_HEIGHT)),
                );
            });
            let _ = hint(
                ui,
                "Folder as Roll imports the output folder. Scan as Roll needs a ported strip worker.",
            );
        });
        changed
    }
}

fn device_choice(device: &negpy_backend::services::scanning::transport::ScannerDevice) -> Choice {
    Choice::with_id(
        format!("{} {}", device.vendor, device.model),
        device.id.clone(),
    )
}

fn scan_card(
    ui: &mut Ui,
    title: &str,
    icon: Icon,
    expanded: bool,
    body: impl FnOnce(&mut Ui) -> bool,
) -> (bool, bool) {
    let mut section = CollapsibleSection::new(title).with_guide().with_icon(icon);
    section.expanded = expanded;
    let (_, result) = section.show(ui, |ui| card(ui, body));
    ui.add_space(f32::from(Theme::default().space_sm));
    (section.expanded, result.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_source_keeps_the_reference_order() {
        let state = ScanState::default();
        assert_eq!(state.source.labels, ["Film Scanner", "Camera"]);
        assert_eq!(state.source.icons[0], Some(Icon::CameraRotate));
        assert_eq!(state.source.icons[1], Some(Icon::Camera));
    }

    #[test]
    fn the_scan_tab_opens_on_the_camera() {
        let state = ScanState::default();
        assert_eq!(state.source.current_label(), "Camera");
        assert_eq!(state.source.current, 1);
        assert_eq!(state.backend.current_label(), "Auto");
        assert_eq!(state.scan_mode.current_label(), "Single pass");
        assert!(!state.device_open);
        assert!(!state.quality_open);
        assert!(state.output_open);
    }

    #[test]
    fn the_camera_card_keeps_its_state_across_the_source_switch() {
        let mut state = ScanState::default();
        state.camera_open = false;
        state.source.current = 0;
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui);
            });
        });
        assert!(!state.camera_open);
        state.source.current = 1;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui);
            });
        });
        assert!(!state.camera_open);
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

    #[test]
    fn a_device_limits_the_resolution_and_depth_choices() {
        let mut state = ScanState::default();
        state.set_capabilities(&negpy_backend::services::scanning::simulated::devices()[0]);
        let labels: Vec<&str> = state
            .resolution
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect();
        assert_eq!(labels, ["1000 dpi", "2000 dpi", "4000 dpi"]);
        assert_eq!(state.bit_depth.current_label(), "8 bit");
    }

    #[test]
    fn the_scan_action_uses_the_visible_quality_controls() {
        let mut state = ScanState::default();
        state.resolution = StepCombo::new(vec![Choice::with_id("2400 dpi", "2400")]);
        state.bit_depth = StepCombo::new(vec![Choice::with_id("8 bit", "8")]);
        state.ir = true;
        let params = state.params();
        assert_eq!(params.dpi, 2400);
        assert_eq!(params.depth, 8);
        assert!(params.capture_ir);
    }

    #[test]
    fn an_unavailable_backend_reports_its_hint() {
        let mut state = ScanState::default();
        state.backend = StepCombo::new(vec![Choice::with_id("PIEUSB", "pieusb")]);
        assert!(!state.refresh_devices());
        assert!(state.status.contains("PIEUSB"));
    }

    #[test]
    fn folder_as_roll_leaves_one_request_with_its_folder() {
        let mut state = ScanState::default();
        state.output_folder = "/tmp/scans".to_string();
        state.folder_as_roll_requested = true;
        assert_eq!(
            state.take_folder_as_roll_request(),
            Some(std::path::PathBuf::from("/tmp/scans"))
        );
        assert!(state.take_folder_as_roll_request().is_none());
    }

    #[test]
    fn the_scan_action_leaves_one_request() {
        let mut state = ScanState::default();
        state.scan_requested = true;
        assert!(state.take_scan_request());
        assert!(!state.take_scan_request());
    }
}
