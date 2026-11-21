//! The right rail: the main tab strip and the cards under it.
//!
//! The rail keeps its own state while the backend owns the edit. A card reads the
//! config it is given and reports a change; it never keeps a second copy.

use eframe::egui::{
    self, Align2, Color32, FontId, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2,
};

use crate::app::{Tab, ViewState};
use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{
    ActionRank, action_with_icon, labeled_action, labeled_toggle, section_subheader,
};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::sub_tabs::{SUB_TAB_HEIGHT, SubTab, SubTabState};
use negpy_backend::domain::models::process::{ProcessMode, scan_setup_values, with_process_mode};
use negpy_backend::session::proof::ProofSettings;

pub mod analysis;
pub mod calibration;
pub mod export;
pub mod favourites;
pub mod frame_color;
pub mod frame_finish;
pub mod frame_geometry;
pub mod frame_tone;
pub mod history;
pub mod library;
pub mod metadata;
pub mod metering;
pub mod optics;
pub mod raw_decode;
pub mod roll_analysis;
pub mod scan;

/// The rail's fixed width at the golden window.
pub const RAIL_WIDTH: f32 = 514.0;
/// The search field's height in the top bar.
pub const SEARCH_HEIGHT: f32 = 36.0;

/// A count and its singular or plural noun.
#[must_use]
pub fn count_of(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// The tab state the rail shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportStatus {
    pub message: String,
    pub failed: bool,
    pub running: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RailState {
    pub tab: Tab,
    pub frame_subtab: SubTabState,
    pub setup: CollapsibleSection,
    pub process_mode: SegmentedChoice<'static>,
    pub rig: SegmentedChoice<'static>,
    pub light: SegmentedChoice<'static>,
    pub density: CompactSlider,
    pub grade: CompactSlider,
    pub shadow_density: CompactSlider,
    pub highlight_density: CompactSlider,
    pub library: library::LibraryState,
    pub geometry: frame_geometry::GeometryState,
    pub exposure: frame_tone::ExposureState,
    pub look: frame_color::LookState,
    pub finish: frame_finish::FinishState,
    pub metadata: metadata::MetadataState,
    pub export: export::ExportState,
    pub scan: scan::ScanState,
    pub history: history::HistoryState,
    /// The strip frame names, for the Roll Analysis scene list.
    pub strip_names: Vec<String>,
    /// The Favorites card's stored slider list.
    pub favourites: favourites::FavouritesState,
    pub analysis_open: bool,
    pub analysis_log: bool,
    /// Which Analysis chart paints: 0 curve, 1 waveform, 2 parade, 3 vectorscope.
    pub analysis_view: usize,
    pub scopes: crate::render_worker::ScopeBins,
    pub readouts_open: bool,
    pub collapsed_sections: std::collections::BTreeMap<String, bool>,
    pub metrics: analysis::AnalysisMetrics,
    /// The display histogram `(4, 256)` of the frame on the canvas.
    pub histogram: Option<Vec<f64>>,
    /// The `(4, 120)` density histogram of the normalized frame.
    pub density_histogram: Option<Vec<f64>>,
    pub export_status: ExportStatus,
    pub roll_analysis: roll_analysis::RollAnalysisState,
    pub raw_decode: raw_decode::RawDecodeState,
    pub optics: optics::OpticsState,
    pub metering: metering::MeteringState,
    pub calibration: calibration::CalibrationState,
}

impl RailState {
    /// The rail in the state a capture asks for.
    #[must_use]
    pub fn for_view(view: &ViewState) -> Self {
        let subtabs = frame_subtabs();
        let frame_subtab = SubTabState::new(
            &subtabs,
            (!view.subtab.is_empty()).then_some(view.subtab.as_str()),
        );
        let mut setup = CollapsibleSection::new("Setup").with_guide();
        setup.expanded = true;
        let mut metadata = metadata::MetadataState::default();
        let mut export = export::ExportState::default();
        if view.tab == Tab::Metadata {
            metadata.show_subtab(&view.subtab);
        }
        if view.tab == Tab::Export {
            export.show_subtab(&view.subtab);
        }
        Self {
            tab: view.tab,
            frame_subtab,
            setup,
            process_mode: SegmentedChoice {
                labels: &["Color", "B&W", "Slide"],
                icons: &[Some(Icon::Folder), None, Some(Icon::Film)],
                current: 0,
            },
            rig: SegmentedChoice {
                labels: &["Camera", "Film Scanner"],
                icons: &[Some(Icon::Image), Some(Icon::Scan)],
                current: 0,
            },
            light: SegmentedChoice {
                labels: &["White Light", "Narrowband Light"],
                icons: &[None, Some(Icon::Circle)],
                current: 0,
            },
            density: CompactSlider::new(SliderSpec::new(0.0, 2.0, 0.01, 2).with_default(1.08)),
            grade: CompactSlider::new(
                SliderSpec::new(-0.5, 5.0, 0.01, 2)
                    .with_default(1.83)
                    .with_unit(" R"),
            ),
            shadow_density: CompactSlider::new(
                SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            highlight_density: CompactSlider::new(
                SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            library: library::LibraryState::default(),
            geometry: frame_geometry::GeometryState::default(),
            exposure: frame_tone::ExposureState::default(),
            look: frame_color::LookState::default(),
            finish: frame_finish::FinishState::default(),
            metadata,
            export,
            scan: scan::ScanState::default(),
            history: history::HistoryState::default(),
            strip_names: Vec::new(),
            favourites: favourites::FavouritesState::default(),
            analysis_open: view.analysis_open,
            analysis_log: view.analysis_log,
            analysis_view: 0,
            scopes: crate::render_worker::ScopeBins::default(),
            readouts_open: false,
            collapsed_sections: std::collections::BTreeMap::new(),
            metrics: analysis::AnalysisMetrics::default(),
            histogram: None,
            density_histogram: None,
            export_status: ExportStatus::default(),
            roll_analysis: roll_analysis::RollAnalysisState::default(),
            raw_decode: raw_decode::RawDecodeState::default(),
            optics: optics::OpticsState::default(),
            metering: metering::MeteringState::default(),
            calibration: calibration::CalibrationState::default(),
        }
    }

    /// Read the crosstalk gallery for the open profile and frame.
    pub fn set_profile(
        &mut self,
        profile: Option<&std::path::Path>,
        config: &negpy_backend::domain::models::WorkspaceConfig,
    ) {
        self.calibration.set_profile(profile, config);
    }

    /// Show the metrics the newest render measured.
    pub fn set_metrics(&mut self, metrics: analysis::AnalysisMetrics) {
        self.metrics = metrics;
    }

    /// Show the display histogram of the frame on the canvas.
    pub fn set_histogram(&mut self, histogram: Option<Vec<f64>>) {
        self.histogram = histogram;
    }

    /// Show the scope bins of the frame on the canvas.
    pub fn set_scopes(&mut self, scopes: crate::render_worker::ScopeBins) {
        self.scopes = scopes;
    }

    /// Show the density histogram of the normalized frame.
    pub fn set_density_histogram(&mut self, bins: Option<Vec<f64>>) {
        self.density_histogram = bins;
    }

    /// Select the tab and page a capture or a launch asks for, keeping every card's state.
    pub fn select_view(&mut self, view: &ViewState) {
        self.tab = view.tab;
        match view.tab {
            Tab::Frame => {
                self.frame_subtab = SubTabState::new(
                    &frame_subtabs(),
                    (!view.subtab.is_empty()).then_some(view.subtab.as_str()),
                );
            }
            Tab::Metadata => self.metadata.show_subtab(&view.subtab),
            Tab::Export => self.export.show_subtab(&view.subtab),
            _ => {}
        }
    }

    /// Paint the rail into `rect` and return whether the user chose another tab.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        edit: &mut FrameEdit,
        proof: &mut ProofSettings,
    ) -> Option<Tab> {
        let theme = Theme::default();
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        inner.set_width(rect.width());
        let tabs = main_tabs();
        let current = Tab::ALL
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or(1);
        let mut switched = None;
        if let Some(index) = panel_tab_strip(&mut inner, &tabs, current, 58.0) {
            self.tab = Tab::ALL[index];
            switched = Some(self.tab);
        }
        let body = Rect::from_min_max(
            egui::pos2(rect.left(), inner.cursor().top()),
            egui::pos2(rect.right(), rect.bottom()),
        );
        let footer_height = if self.tab == Tab::Export { 52.0 } else { 0.0 };
        let scroll_body = Rect::from_min_max(
            body.min,
            egui::pos2(body.right(), body.bottom() - footer_height),
        );
        let mut body_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(scroll_body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        body_ui.set_width(scroll_body.width());
        let footer_y = scroll_body.bottom() - f32::from(theme.space_lg);
        ScrollArea::vertical()
            .id_salt(("rail", self.tab))
            .auto_shrink([false, false])
            .show(&mut body_ui, |ui| {
                ui.add_space(f32::from(theme.space_sm));
                self.show_body(ui, footer_y, edit, proof);
                ui.add_space(f32::from(theme.space_xl));
            });
        if footer_height > 0.0 {
            let footer = Rect::from_min_max(
                egui::pos2(body.left(), body.bottom() - footer_height),
                body.max,
            );
            let mut footer_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(footer.shrink2(Vec2::new(
                        f32::from(theme.space_xl),
                        f32::from(theme.space_sm),
                    )))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            footer_ui.set_width(footer.width() - 2.0 * f32::from(theme.space_xl));
            let action = crate::view::widgets::split_action::split_action(
                &mut footer_ui,
                Icon::Export,
                "Export Roll",
                crate::styles::templates::ActionRank::Primary,
            );
            if action.main.clicked() {
                self.export.requested = true;
            }
            if !self.export_status.message.is_empty() {
                let message = if self.export_status.failed {
                    format!("Export failed: {}", self.export_status.message)
                } else {
                    self.export_status.message.clone()
                };
                footer_ui.add_space(f32::from(theme.space_sm));
                let _ = hint(&mut footer_ui, &message);
            }
        }
        switched
    }

    /// Run the profile action the Optics card asked for. Returns whether the edit changed.
    pub fn apply_optics_action(&mut self, edit: &mut FrameEdit) -> bool {
        match self.optics.take_action() {
            Some(crate::view::rail::optics::FlatFieldAction::Add) => {
                let Some(reference) = rfd::FileDialog::new()
                    .set_title("Add Flat Field Profile")
                    .add_filter(
                        "Images",
                        &[
                            "tif", "tiff", "dng", "orf", "nef", "fff", "cr2", "arw", "raf", "jpg",
                            "jpeg", "png",
                        ],
                    )
                    .pick_file()
                else {
                    return false;
                };
                let Some(id) = self
                    .optics
                    .add_profile(&reference, edit.config.geometry.distortion_k1)
                else {
                    return false;
                };
                edit.config.flatfield.profile_id = id;
                edit.config.flatfield.apply = true;
                edit.mark_dirty();
                true
            }
            Some(crate::view::rail::optics::FlatFieldAction::Remove) => {
                if !self.optics.remove_profile() {
                    return false;
                }
                edit.config.flatfield.profile_id.clear();
                edit.config.flatfield.apply = false;
                edit.mark_dirty();
                true
            }
            None => false,
        }
    }

    /// Run the selected History action against the frame's store.
    pub fn apply_history_action(&mut self, edit: &mut FrameEdit) -> bool {
        if self.history.take_save_request() {
            let name = format!("Work print {}", self.history.work_prints.len() + 1);
            let _ = edit.save_work_print(&name);
            if let Some((profile, path)) = edit.source() {
                let (rows, prints) = crate::frame_source::stored_history(Some(&profile), &path);
                self.history.load_session(&rows, &prints);
                return true;
            }
        }
        match self.history.take_restore() {
            Some(history::RestoreTarget::Step(index)) => {
                if let Some(stored) = edit.history_step(index) {
                    edit.restore(stored);
                    return true;
                }
            }
            Some(history::RestoreTarget::WorkPrint(name)) => {
                if let Some(stored) = edit.work_print(&name) {
                    edit.restore(stored);
                    return true;
                }
            }
            None => {}
        }
        false
    }

    fn show_body(
        &mut self,
        ui: &mut Ui,
        footer_y: f32,
        edit: &mut FrameEdit,
        proof: &mut ProofSettings,
    ) {
        match self.tab {
            Tab::Library => {
                self.library.show(ui, Some(footer_y));
            }
            Tab::Roll => self.show_roll(ui, edit),
            Tab::Frame => self.show_frame(ui, edit),
            Tab::Metadata => {
                if self.metadata.show(ui, edit) {
                    edit.mark_dirty();
                }
            }
            Tab::Export => {
                self.export.proof = proof.clone();
                if self.export.show(ui, edit) {
                    edit.mark_dirty();
                }
                *proof = self.export.proof.clone();
            }
            Tab::Scan => {
                self.scan.show(ui);
            }
        }
    }

    fn show_roll(&mut self, ui: &mut Ui, edit: &mut FrameEdit) {
        self.show_analysis(ui);
        ui.add_space(f32::from(Theme::default().space_lg));
        self.show_setup(ui, edit);
        ui.add_space(f32::from(Theme::default().space_lg));
        if self.raw_decode.show(ui, edit) {
            edit.mark_dirty();
        }
        ui.add_space(f32::from(Theme::default().space_lg));
        if self.optics.show(ui, edit) {
            edit.mark_dirty();
        }
        ui.add_space(f32::from(Theme::default().space_lg));
        if self.metering.show(ui, edit) {
            edit.mark_dirty();
        }
        ui.add_space(f32::from(Theme::default().space_lg));
        if self.calibration.show(ui, edit) {
            edit.mark_dirty();
        }
        ui.add_space(f32::from(Theme::default().space_lg));
        self.show_roll_analysis(ui, edit);
    }

    fn show_frame(&mut self, ui: &mut Ui, edit: &mut FrameEdit) {
        self.show_analysis(ui);
        ui.add_space(f32::from(Theme::default().space_sm));
        let subtabs = frame_subtabs();
        let labels: Vec<PanelTab> = subtabs.iter().map(|tab| tab.tab.clone()).collect();
        if let Some(picked) =
            panel_tab_strip(ui, &labels, self.frame_subtab.current, SUB_TAB_HEIGHT)
        {
            self.frame_subtab.current = picked;
        }
        ui.add_space(f32::from(Theme::default().space_sm));
        match self.frame_subtab.current_key() {
            "geometry" => {
                if self.geometry.show(ui, edit) {
                    edit.mark_dirty();
                }
            }
            "tone" => {
                let histogram = self.histogram.clone();
                if self.exposure.show(ui, edit, histogram.as_deref()) {
                    edit.mark_dirty();
                }
            }
            "color" => {
                if self.look.show(ui, edit) {
                    edit.mark_dirty();
                }
            }
            "finish" => {
                if self.finish.show(ui, edit) {
                    edit.mark_dirty();
                }
            }
            "favourites" => {
                if self.favourites.show(ui, edit) {
                    edit.mark_dirty();
                }
            }
            "history" => {
                if self.history.show(ui) {
                    edit.mark_dirty();
                }
            }
            _ => {}
        }
    }

    fn show_analysis(&mut self, ui: &mut Ui) {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Analysis").with_guide();
        section.expanded = *self
            .collapsed_sections
            .entry("Analysis".to_string())
            .or_insert(self.analysis_open);
        let (header, _) = section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                ui.horizontal(|ui| {
                    let tab_height = DEFAULT_BUTTON_HEIGHT;
                    let (tools, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), tab_height),
                        Sense::hover(),
                    );
                    for (index, icon) in
                        [Icon::ChevronLeft, Icon::Export, Icon::Stack, Icon::Circle]
                            .iter()
                            .enumerate()
                    {
                        let button = Rect::from_min_size(
                            egui::pos2(tools.left() + index as f32 * 38.0, tools.top()),
                            Vec2::new(34.0, tab_height),
                        );
                        ui.painter().rect_filled(button, 4.0, theme.bg_input);
                        ui.painter().rect_stroke(
                            button,
                            4.0,
                            Stroke::new(1.0_f32, theme.border_primary),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            button.center(),
                            Align2::CENTER_CENTER,
                            icon.glyph(),
                            FontId::proportional(13.0),
                            theme.text_secondary,
                        );
                    }
                    let log = Rect::from_min_size(
                        egui::pos2(tools.right() - 44.0, tools.top()),
                        Vec2::new(44.0, tab_height),
                    );
                    let response = ui.interact(log, ui.id().with("analysis-log"), Sense::click());
                    let on = self.analysis_log;
                    ui.painter().rect_filled(
                        log,
                        4.0,
                        if on { theme.bg_checked } else { theme.bg_input },
                    );
                    ui.painter().rect_stroke(
                        log,
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
                        log.center(),
                        Align2::CENTER_CENTER,
                        "Log",
                        FontId::proportional(13.0),
                        if on {
                            theme.text_primary
                        } else {
                            theme.text_secondary
                        },
                    );
                    if response.clicked() {
                        self.analysis_log = !self.analysis_log;
                    }
                });
                ui.add_space(f32::from(theme.space_md));
                let mut view = SegmentedChoice {
                    labels: &["Curve", "Waveform", "Parade", "Vectorscope"],
                    icons: &[],
                    current: self.analysis_view,
                };
                if let Some(index) = view.show(ui) {
                    self.analysis_view = index;
                }
                ui.add_space(f32::from(theme.space_sm));
                ui.horizontal(|ui| {
                    let height = 150.0;
                    let (chart, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), height),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(chart, 4.0, theme.bg_input);
                    ui.painter().rect_stroke(
                        chart,
                        4.0,
                        Stroke::new(1.0_f32, theme.border_primary),
                        egui::StrokeKind::Inside,
                    );
                    let bottom = chart.bottom() - 34.0;
                    let band = Rect::from_min_max(
                        egui::pos2(chart.left() + 4.0, chart.top() + 4.0),
                        egui::pos2(chart.right() - 4.0, bottom),
                    );
                    match self.analysis_view {
                        1 => crate::view::widgets::scopes::paint_waveform(
                            ui,
                            band,
                            self.scopes.waveform.as_deref(),
                            self.analysis_log,
                        ),
                        2 => crate::view::widgets::scopes::paint_parade(
                            ui,
                            band,
                            self.scopes.parade.as_deref(),
                            self.analysis_log,
                        ),
                        3 => crate::view::widgets::scopes::paint_vectorscope(
                            ui,
                            band,
                            self.scopes.vectorscope.as_deref(),
                            self.analysis_log,
                        ),
                        _ => {
                            if let Some(density) = &self.density_histogram {
                                crate::view::widgets::histogram::paint_density(
                                    ui,
                                    band,
                                    density,
                                    self.analysis_log,
                                );
                            }
                            if let Some(bins) = &self.histogram {
                                crate::view::widgets::histogram::paint_strip(
                                    ui,
                                    band,
                                    bins,
                                    self.analysis_log,
                                    false,
                                );
                                crate::view::widgets::histogram::paint_clip_marks(ui, band, bins);
                            }
                            let points: Vec<egui::Pos2> = self
                                .metrics
                                .curve
                                .iter()
                                .map(|[x, y]| {
                                    let t = ((*x + 0.1) / 1.2).clamp(0.0, 1.0) as f32;
                                    egui::pos2(
                                        chart.left() + 10.0 + t * (chart.width() - 20.0),
                                        bottom - *y as f32 * (chart.height() - 44.0),
                                    )
                                })
                                .collect();
                            if points.len() >= 2 {
                                ui.painter().add(egui::Shape::line(
                                    points,
                                    Stroke::new(1.5_f32, theme.text_primary),
                                ));
                            }
                        }
                    }
                    let wedge = Rect::from_min_size(
                        egui::pos2(chart.left() + 2.0, bottom + 2.0),
                        Vec2::new(chart.width() - 4.0, 22.0),
                    );
                    for step in 0..10 {
                        let cell = Rect::from_min_size(
                            egui::pos2(
                                wedge.left() + step as f32 * wedge.width() / 10.0,
                                wedge.top(),
                            ),
                            Vec2::new(wedge.width() / 10.0, wedge.height()),
                        );
                        let value = (step * 255 / 9) as u8;
                        ui.painter()
                            .rect_filled(cell, 0.0, Color32::from_gray(value));
                        ui.painter().text(
                            cell.center(),
                            Align2::CENTER_CENTER,
                            format!("{step}"),
                            FontId::proportional(f32::from(theme.font_size_micro)),
                            if value > 128 {
                                theme.bg_dark
                            } else {
                                theme.text_primary
                            },
                        );
                    }
                });
                ui.add_space(f32::from(theme.space_md));
                let mut readouts = CollapsibleSection::new("Readouts");
                readouts.expanded = self.readouts_open;
                let _ = readouts.show(ui, |ui| {
                    if self.metrics.rows.is_empty() {
                        let _ = hint(ui, "Double-click to reset");
                        return;
                    }
                    for row in &self.metrics.rows {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(row.name.as_str())
                                    .size(f32::from(theme.font_size_small))
                                    .color(theme.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(&row.value)
                                            .size(f32::from(theme.font_size_small))
                                            .color(if row.warn {
                                                theme.warn_amber
                                            } else {
                                                theme.text_primary
                                            }),
                                    );
                                },
                            );
                        });
                    }
                });
                self.readouts_open = readouts.expanded;
            });
        });
        let _ = header;
        self.collapsed_sections
            .insert("Analysis".to_string(), section.expanded);
    }

    fn show_setup(&mut self, ui: &mut Ui, edit: &mut FrameEdit) {
        let theme = Theme::default();
        self.process_mode.current = process_mode_index(edit.config.process.process_mode);
        self.rig.current = usize::from(edit.config.process.linear_raw);
        self.setup.show(ui, |ui| {
            let _ = card(ui, |ui| {
                let mut changed = false;
                if let Some(index) = self.process_mode.show(ui) {
                    changed = true;
                    let mode = [ProcessMode::C41, ProcessMode::BW, ProcessMode::E6]
                        [index.min(2)];
                    edit.config = with_process_mode(edit.config.clone(), mode);
                }
                ui.add_space(f32::from(theme.space_sm));
                if let Some(index) = self.rig.show(ui) {
                    changed = true;
                    edit.config.process.linear_raw = index == 1;
                }
                ui.add_space(f32::from(theme.space_sm));
                if let Some(index) = self.light.show(ui) {
                    changed = true;
                    let (linear_raw, paper) = scan_setup_values(
                        if self.rig.current == 1 { "scanner" } else { "camera" },
                        if index == 1 { "narrowband" } else { "white" },
                    );
                    edit.config.process.linear_raw = linear_raw;
                    edit.config.exposure.paper_profile = paper.to_string();
                }
                if changed {
                    edit.mark_dirty();
                }
                ui.add_space(f32::from(theme.space_sm));
                let _ = hint(
                    ui,
                    "A lightbox or LED panel with white light.\nDecodes the RAW with the camera white balance.\nShows the Raw Decode and Optics cards.\nPrints on Digital (Adobe RGB).",
                );
            });
        });
    }

    fn show_roll_analysis(&mut self, ui: &mut Ui, edit: &mut FrameEdit) {
        let mut section = CollapsibleSection::new("Roll Analysis")
            .with_guide()
            .with_icon(Icon::Stack);
        section.modified = 3;
        section.expanded = self.roll_analysis.open;
        let _ = section.show(ui, |ui| self.show_roll_analysis_body(ui, edit));
        self.roll_analysis.open = section.expanded;
    }

    fn show_roll_analysis_body(&mut self, ui: &mut Ui, edit: &mut FrameEdit) {
        let theme = Theme::default();
        let _ = card(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = section_subheader(ui, "ROLLS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if action_with_icon(
                        ui,
                        Icon::PickFrames,
                        "Use Frame",
                        ActionRank::Normal,
                        "Use this frame as the roll's analysis reference",
                    )
                    .clicked()
                    {
                        self.roll_analysis.use_frame_requested = true;
                    }
                    ui.add_space(f32::from(theme.space_sm));
                    if action_with_icon(
                        ui,
                        Icon::Circle,
                        "Reanalyze",
                        ActionRank::Normal,
                        "Measure the roll again from its frames",
                    )
                    .clicked()
                    {
                        self.roll_analysis.reanalyze_requested = true;
                    }
                });
            });
            ui.add_space(f32::from(theme.space_sm));
            let _ = hint(
                ui,
                "Roll Analysis first crops the frames that wait for Auto Crop.",
            );
            let field = Rect::from_min_size(
                egui::pos2(ui.cursor().left(), ui.cursor().top()),
                Vec2::new(ui.available_width(), 36.0),
            );
            ui.painter().rect_filled(field, 4.0, theme.bg_input);
            ui.painter().rect_stroke(
                field,
                4.0,
                Stroke::new(1.0_f32, theme.border_primary),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                field.left_center() + Vec2::new(f32::from(theme.space_xl), 0.0),
                Align2::LEFT_CENTER,
                Icon::Check.glyph(),
                FontId::proportional(f32::from(theme.font_size_base)),
                theme.text_hint,
            );
            ui.painter().text(
                field.left_center() + Vec2::new(f32::from(theme.space_xl) + 22.0, 0.0),
                Align2::LEFT_CENTER,
                self.roll_analysis.roll_line(),
                FontId::proportional(f32::from(theme.font_size_base)),
                theme.text_primary,
            );
            ui.advance_cursor_after_rect(field);
            ui.add_space(f32::from(theme.space_sm));
            if self.roll_analysis.stale {
                ui.label(
                    RichText::new("⚠ Baseline from an older analysis: run Reanalyze")
                        .size(f32::from(theme.font_size_small))
                        .color(theme.warn_amber),
                );
            }
            ui.add_space(f32::from(theme.space_md));
            ui.horizontal(|ui| {
                let _ = section_subheader(ui, "SCENES");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if action_with_icon(
                        ui,
                        Icon::Circle,
                        "Detect",
                        ActionRank::Normal,
                        "Find the scenes the selected frames belong to",
                    )
                    .clicked()
                    {
                        self.roll_analysis.detect_requested = true;
                    }
                });
            });
            if self.roll_analysis.found.is_empty() {
                let _ = hint(
                    ui,
                    "Select frames in the Film Strip, then right-click › Scene › Group as Scene...",
                );
            } else {
                let names: Vec<String> = self
                    .roll_analysis
                    .found
                    .iter()
                    .map(|group| {
                        let frames: Vec<String> = group
                            .iter()
                            .filter_map(|index| self.strip_names.get(*index).cloned())
                            .collect();
                        format!("{}: {}", count_of(group.len(), "frame"), frames.join(", "))
                    })
                    .collect();
                for name in names {
                    let _ = hint(ui, &name);
                }
                ui.horizontal(|ui| {
                    if action_with_icon(
                        ui,
                        Icon::Check,
                        "Accept All",
                        ActionRank::Primary,
                        "Make every group a scene with its own baseline",
                    )
                    .clicked()
                    {
                        self.roll_analysis.accept_requested = true;
                    }
                    ui.add_space(f32::from(theme.space_sm));
                    if labeled_action(ui, "Discard", ActionRank::Normal).clicked() {
                        self.roll_analysis.discard_requested = true;
                    }
                });
            }
            ui.add_space(f32::from(theme.space_sm));
            let _ = section_subheader(ui, "USE AVERAGE");
            if let Some(warning) = self.roll_analysis.baseline_warning() {
                ui.label(
                    RichText::new(warning)
                        .size(f32::from(theme.font_size_small))
                        .color(theme.warn_amber),
                );
            }
            ui.add_space(f32::from(theme.space_sm));
            ui.horizontal(|ui| {
                let mut use_luma = edit.config.process.use_luma_average;
                let mut use_color = edit.config.process.use_color_average;
                let mut use_cast = edit.config.process.use_cast_average;
                let luma = labeled_toggle(ui, "Luma", &mut use_luma).clicked();
                ui.add_space(f32::from(theme.space_sm));
                let color = labeled_toggle(ui, "Color", &mut use_color).clicked();
                ui.add_space(f32::from(theme.space_sm));
                let cast = labeled_toggle(ui, "Cast", &mut use_cast).clicked();
                if luma || color || cast {
                    edit.config.process.use_luma_average = use_luma;
                    edit.config.process.use_color_average = use_color;
                    edit.config.process.use_cast_average = use_cast;
                    edit.mark_dirty();
                }
            });
        });
    }
}

fn process_mode_index(mode: ProcessMode) -> usize {
    match mode {
        ProcessMode::C41 => 0,
        ProcessMode::BW => 1,
        ProcessMode::E6 => 2,
    }
}

/// Paint the Find field the top bar puts over the rail.
pub fn paint_search(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), SEARCH_HEIGHT),
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
        rect.left_center() + Vec2::new(f32::from(theme.space_xl), 0.0),
        Align2::LEFT_CENTER,
        Icon::Search.glyph(),
        FontId::proportional(f32::from(theme.font_size_header)),
        theme.text_hint,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(32.0, 0.0),
        Align2::LEFT_CENTER,
        "Find a control, card or action...",
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_hint,
    );
    ui.painter().text(
        rect.right_center() - Vec2::new(f32::from(theme.space_xl), 0.0),
        Align2::RIGHT_CENTER,
        "Ctrl+K",
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
}

/// The rail's six main tabs.
#[must_use]
pub fn main_tabs() -> [PanelTab; 6] {
    [
        PanelTab::stacked(Icon::Grid, "Library"),
        PanelTab::stacked(Icon::Film, "Roll"),
        PanelTab::stacked(Icon::Image, "Frame"),
        PanelTab::stacked(Icon::Metadata, "Metadata"),
        PanelTab::stacked(Icon::Export, "Export"),
        PanelTab::stacked(Icon::Scan, "Scan"),
    ]
}

/// The Frame tab's sub-tabs, in workflow order.
#[must_use]
pub fn frame_subtabs() -> [SubTab; 6] {
    [
        SubTab::new("geometry", Icon::Image, "Geometry").with_divider(),
        SubTab::new("tone", Icon::Sliders, "Exposure"),
        SubTab::new("color", Icon::Circle, "Look"),
        SubTab::new("finish", Icon::Check, "Finish"),
        SubTab::new("favourites", Icon::Stack, "Favorites").icon_only(),
        SubTab::new("history", Icon::TurnPage, "History").icon_only(),
    ]
}

/// The card titles a Frame sub-tab holds.
#[must_use]
pub fn frame_cards(key: &str) -> &'static [&'static str] {
    match key {
        "geometry" => &["Crop", "Straighten"],
        "tone" => &["Filtration", "Tone", "Paper Response", "Dodge & Burn"],
        "color" => &["Lab", "Alternative Processes", "Toning"],
        "finish" => &["Retouch", "Finishing"],
        "favourites" => &["Favorites"],
        "history" => &["Work Prints", "Edit History"],
        _ => &[],
    }
}

/// The card titles the Roll tab holds.
#[must_use]
pub fn roll_cards() -> &'static [&'static str] {
    &[
        "Setup",
        "Raw Decode",
        "Optics",
        "Calibration",
        "Metering",
        "Roll Analysis",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_strip_keeps_the_six_tab_order() {
        let labels: Vec<&str> = main_tabs().iter().map(|tab| tab.label).collect();
        assert_eq!(
            labels,
            vec!["Library", "Roll", "Frame", "Metadata", "Export", "Scan"]
        );
    }

    #[test]
    fn every_frame_subtab_names_its_cards() {
        for tab in frame_subtabs() {
            let cards = frame_cards(tab.key);
            if tab.key != "history" {
                assert!(!cards.is_empty(), "{} has no cards", tab.key);
            }
        }
    }

    #[test]
    fn the_roll_card_order_starts_with_setup() {
        assert_eq!(roll_cards()[0], "Setup");
    }

    #[test]
    fn the_rail_state_reads_the_capture_subtab() {
        let view = ViewState::from_capture("frame", "tone", "", "canvas", 0.0);
        let state = RailState::for_view(&view);
        assert_eq!(state.frame_subtab.current_key(), "tone");
    }

    #[test]
    fn a_capture_subtab_selects_metadata_and_export_pages() {
        let view = ViewState::from_capture("metadata", "", "process", "canvas", 0.0);
        assert_eq!(RailState::for_view(&view).metadata.page, 1);
        let view = ViewState::from_capture("export", "", "proof", "canvas", 0.0);
        assert_eq!(RailState::for_view(&view).export.page, 1);
    }

    #[test]
    fn a_collapsed_roll_card_keeps_its_state_across_frames() {
        let ctx = egui::Context::default();
        let fixture = crate::fixture::ReferenceFixture::bundled().unwrap();
        let mut app = crate::app::NegFeApp::new(&fixture);
        app.rail
            .collapsed_sections
            .insert("Calibration".to_string(), false);
        app.rail.roll_analysis.open = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = app
                    .rail
                    .show(ui, ui.max_rect(), &mut app.edit, &mut app.proof);
            });
        });
        assert_eq!(app.rail.collapsed_sections.get("Calibration"), Some(&false));
        assert!(!app.rail.roll_analysis.open);
    }

    #[test]
    fn the_analysis_card_keeps_its_collapsed_state() {
        let ctx = egui::Context::default();
        let fixture = crate::fixture::ReferenceFixture::bundled().unwrap();
        let mut app = crate::app::NegFeApp::new(&fixture);
        app.rail.analysis_open = false;
        app.rail
            .collapsed_sections
            .insert("Analysis".to_string(), false);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = app
                    .rail
                    .show(ui, ui.max_rect(), &mut app.edit, &mut app.proof);
            });
        });
        assert_eq!(app.rail.collapsed_sections.get("Analysis"), Some(&false));
    }

    #[test]
    fn the_analysis_log_toggle_keeps_its_state() {
        let fixture = crate::fixture::ReferenceFixture::bundled().unwrap();
        let mut app = crate::app::NegFeApp::new(&fixture);
        app.rail.analysis_log = false;
        assert!(!app.rail.analysis_log);
        app.rail.analysis_log = !app.rail.analysis_log;
        assert!(app.rail.analysis_log);
    }

    #[test]
    fn the_analysis_chart_takes_the_render_histogram() {
        let ctx = egui::Context::default();
        let fixture = crate::fixture::ReferenceFixture::bundled().unwrap();
        let mut app = crate::app::NegFeApp::new(&fixture);
        let mut bins = vec![0.0; 4 * 256];
        bins[3 * 256 + 128] = 10.0;
        app.rail.set_histogram(Some(bins));
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = app
                    .rail
                    .show(ui, ui.max_rect(), &mut app.edit, &mut app.proof);
            });
        });
        assert!(app.rail.histogram.is_some());
    }

    #[test]
    fn the_setup_modes_map_to_their_stored_values() {
        assert_eq!(process_mode_index(ProcessMode::C41), 0);
        assert_eq!(process_mode_index(ProcessMode::BW), 1);
        assert_eq!(process_mode_index(ProcessMode::E6), 2);
        let (linear_raw, paper) = scan_setup_values("camera", "narrowband");
        assert!(linear_raw);
        assert_eq!(paper, "pakon");
    }

    #[test]
    fn the_rail_keeps_the_render_histogram() {
        let mut rail = RailState::for_view(&ViewState::default());
        let mut bins = vec![0.0; 4 * 256];
        bins[3 * 256 + 10] = 5.0;
        rail.set_histogram(Some(bins.clone()));
        assert_eq!(rail.histogram.as_deref(), Some(bins.as_slice()));
        rail.set_histogram(None);
        assert!(rail.histogram.is_none());
    }
}
