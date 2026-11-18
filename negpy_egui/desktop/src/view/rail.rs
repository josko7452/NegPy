//! The right rail: the main tab strip and the cards under it.
//!
//! The rail keeps its own state while the backend owns the edit. A card reads the
//! config it is given and reports a change; it never keeps a second copy.

use eframe::egui::{
    self, Align2, Color32, FontId, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2,
};

use crate::app::{Tab, ViewState};
use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{ActionRank, labeled_action, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::sub_tabs::{SUB_TAB_HEIGHT, SubTab, SubTabState};
use crate::view::widgets::tab_header::TabSummary;

pub mod frame_color;
pub mod frame_geometry;
pub mod frame_tone;
pub mod library;

/// The rail's fixed width at the golden window.
pub const RAIL_WIDTH: f32 = 514.0;
/// The search field's height under the tab strip.
pub const SEARCH_HEIGHT: f32 = 36.0;

/// The tab state the rail shows.
#[derive(Clone, Debug, PartialEq)]
pub struct RailState {
    pub tab: Tab,
    pub frame_subtab: SubTabState,
    pub analysis: CollapsibleSection,
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
        let mut analysis = CollapsibleSection::new("Analysis").with_guide();
        analysis.expanded = view.analysis_open;
        let mut setup = CollapsibleSection::new("Setup").with_guide();
        setup.expanded = true;
        Self {
            tab: view.tab,
            frame_subtab,
            analysis,
            setup,
            process_mode: SegmentedChoice {
                labels: &["Color", "B&W", "Slide"],
                current: 0,
            },
            rig: SegmentedChoice {
                labels: &["Camera", "Film Scanner"],
                current: 0,
            },
            light: SegmentedChoice {
                labels: &["White Light", "Narrowband Light"],
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
        }
    }

    /// Paint the rail into `rect` and return whether the user chose another tab.
    pub fn show(&mut self, ui: &mut Ui, rect: Rect) -> Option<Tab> {
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
        inner.add_space(f32::from(theme.space_sm));
        paint_search(&mut inner, &theme);
        inner.add_space(f32::from(theme.space_sm));
        let body = Rect::from_min_max(
            egui::pos2(rect.left(), inner.cursor().top()),
            egui::pos2(rect.right(), rect.bottom()),
        );
        let mut body_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        body_ui.set_width(body.width());
        ScrollArea::vertical()
            .id_salt(("rail", self.tab))
            .auto_shrink([false, false])
            .show(&mut body_ui, |ui| {
                ui.add_space(f32::from(theme.space_sm));
                self.show_body(ui);
                ui.add_space(f32::from(theme.space_xl));
            });
        switched
    }

    fn show_body(&mut self, ui: &mut Ui) {
        match self.tab {
            Tab::Library => {
                self.library.show(ui);
            }
            Tab::Roll => self.show_roll(ui),
            Tab::Frame => self.show_frame(ui),
            Tab::Metadata => self.show_generic(ui, "Metadata"),
            Tab::Export => self.show_generic(ui, "Export"),
            Tab::Scan => self.show_generic(ui, "Scan"),
        }
    }

    fn show_generic(&mut self, ui: &mut Ui, title: &str) {
        let mut section = CollapsibleSection::new(title).with_guide();
        section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                ui.label(
                    RichText::new("No cards on this tab yet.")
                        .size(f32::from(Theme::default().font_size_small))
                        .color(Theme::default().text_muted),
                );
            });
        });
    }

    fn show_roll(&mut self, ui: &mut Ui) {
        self.show_analysis(ui);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_setup(ui);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_collapsed(ui, "Raw Decode", 0);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_collapsed(ui, "Optics", 0);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_collapsed(ui, "Calibration", 3);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_collapsed(ui, "Metering", 0);
        ui.add_space(f32::from(Theme::default().space_sm));
        self.show_collapsed(ui, "Roll Analysis", 3);
    }

    fn show_frame(&mut self, ui: &mut Ui) {
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
                self.geometry.show(ui);
            }
            "tone" => {
                self.exposure.show(ui);
            }
            "color" => {
                self.look.show(ui);
            }
            key => {
                let cards = frame_cards(key);
                if !cards.is_empty() {
                    let _ = TabSummary::new(cards.len()).show(ui);
                }
                ui.add_space(f32::from(Theme::default().space_sm));
                for title in cards {
                    self.show_collapsed(ui, title, 0);
                    ui.add_space(f32::from(Theme::default().space_sm));
                }
            }
        }
    }

    fn show_analysis(&mut self, ui: &mut Ui) {
        let theme = Theme::default();
        self.analysis.show(ui, |ui| {
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
                    ui.painter().rect_filled(log, 4.0, theme.bg_checked);
                    ui.painter().rect_stroke(
                        log,
                        4.0,
                        Stroke::new(1.0_f32, theme.border_checked),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        log.center(),
                        Align2::CENTER_CENTER,
                        "Log",
                        FontId::proportional(13.0),
                        theme.text_primary,
                    );
                });
                ui.add_space(f32::from(theme.space_md));
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
                    let points: Vec<egui::Pos2> = (0..33)
                        .map(|index| {
                            let t = index as f32 / 32.0;
                            egui::pos2(
                                chart.left() + 10.0 + t * (chart.width() - 20.0),
                                bottom - t.powf(0.45) * (chart.height() - 44.0),
                            )
                        })
                        .collect();
                    ui.painter().add(egui::Shape::line(
                        points,
                        Stroke::new(1.5_f32, theme.text_primary),
                    ));
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
                readouts.expanded = false;
                readouts.show(ui, |ui| {
                    let _ = hint(ui, "Double-click to reset");
                });
            });
        });
    }

    fn show_setup(&mut self, ui: &mut Ui) {
        let theme = Theme::default();
        self.setup.show(ui, |ui| {
            let _ = card(ui, |ui| {
                let _ = self.process_mode.show(ui);
                ui.add_space(f32::from(theme.space_sm));
                let _ = self.rig.show(ui);
                ui.add_space(f32::from(theme.space_sm));
                let _ = self.light.show(ui);
                ui.add_space(f32::from(theme.space_sm));
                let _ = hint(
                    ui,
                    "A lightbox or LED panel with white light.\nDecodes the RAW with the camera white balance.\nShows the Raw Decode and Optics cards.\nPrints on Digital (Adobe RGB).",
                );
            });
        });
    }

    fn show_collapsed(&mut self, ui: &mut Ui, title: &str, modified: usize) {
        ui.add_space(f32::from(Theme::default().space_sm));
        let mut section = CollapsibleSection::new(title).with_guide();
        section.modified = modified;
        if title == "Calibration" {
            section.expanded = true;
            section.show(ui, |ui| self.show_calibration_body(ui));
        } else if title == "Roll Analysis" {
            section.expanded = true;
            section.show(ui, |ui| self.show_roll_analysis_body(ui));
        } else {
            section.expanded = false;
            section.show(ui, |ui| {
                let _ = card(ui, |ui| {
                    let _ = hint(ui, "This card has no controls yet.");
                });
            });
        }
    }

    fn show_roll_analysis_body(&mut self, ui: &mut Ui) {
        let theme = Theme::default();
        let _ = card(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = section_subheader(ui, "ROLLS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = labeled_action(ui, "Use Frame", ActionRank::Normal);
                    ui.add_space(f32::from(theme.space_sm));
                    let _ = labeled_action(ui, "Reanalyze", ActionRank::Normal);
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
                "✓ w9",
                FontId::proportional(f32::from(theme.font_size_base)),
                theme.text_primary,
            );
            ui.advance_cursor_after_rect(field);
            ui.add_space(f32::from(theme.space_sm));
            ui.label(
                RichText::new("⚠ Baseline from an older analysis: run Reanalyze")
                    .size(f32::from(theme.font_size_small))
                    .color(theme.warn_amber),
            );
            ui.add_space(f32::from(theme.space_md));
            ui.horizontal(|ui| {
                let _ = section_subheader(ui, "SCENES");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = labeled_action(ui, "Detect", ActionRank::Normal);
                });
            });
            let _ = hint(
                ui,
                "Select frames in the Film Strip, then right-click › Scene › Group as Scene...",
            );
            ui.add_space(f32::from(theme.space_sm));
            let _ = section_subheader(ui, "USE AVERAGE");
            ui.label(
                RichText::new("⚠ Baseline: Roll “w9”, from an older analysis")
                    .size(f32::from(theme.font_size_small))
                    .color(theme.warn_amber),
            );
            ui.add_space(f32::from(theme.space_sm));
            ui.horizontal(|ui| {
                let _ = labeled_action(ui, "Luma", ActionRank::Normal);
                ui.add_space(f32::from(theme.space_sm));
                let _ = labeled_action(ui, "Color", ActionRank::Normal);
                ui.add_space(f32::from(theme.space_sm));
                let _ = labeled_action(ui, "Cast", ActionRank::Normal);
            });
        });
    }

    fn show_calibration_body(&mut self, ui: &mut Ui) {
        let theme = Theme::default();
        let _ = card(ui, |ui| {
            let _ = section_subheader(ui, "CROSSTALK");
            ui.add_space(f32::from(theme.space_sm));
            ui.horizontal(|ui| {
                let _ = ui.label(
                    RichText::new("Matrix")
                        .size(f32::from(theme.font_size_base))
                        .color(theme.text_secondary),
                );
                ui.add_space(f32::from(theme.space_md));
                let combo = Rect::from_min_size(
                    egui::pos2(ui.cursor().left(), ui.cursor().top()),
                    Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
                );
                ui.painter().rect_filled(combo, 4.0, theme.bg_input);
                ui.painter().rect_stroke(
                    combo,
                    4.0,
                    Stroke::new(1.0_f32, theme.border_primary),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    combo.left_center() + Vec2::new(f32::from(theme.space_xl), 0.0),
                    Align2::LEFT_CENTER,
                    "Kodak Gold 200 (approx)",
                    FontId::proportional(f32::from(theme.font_size_base)),
                    theme.text_primary,
                );
                ui.painter().text(
                    combo.right_center() - Vec2::new(f32::from(theme.space_xl), 0.0),
                    Align2::RIGHT_CENTER,
                    Icon::ChevronRight.glyph(),
                    FontId::proportional(f32::from(theme.font_size_small)),
                    theme.text_secondary,
                );
                ui.advance_cursor_after_rect(combo);
            });
            ui.add_space(f32::from(theme.space_sm));
            let mut strength =
                CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.49));
            strength.set_value(0.49);
            let _ = strength.show(ui, "Strength");
        });
    }
}

fn paint_search(ui: &mut Ui, theme: &Theme) {
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
        "favourites" => &["Presets"],
        "history" => &[],
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
        let mut view = ViewState::from_capture("frame", "frame", "tone", "canvas", 0.0);
        view.tab = Tab::Frame;
        let state = RailState::for_view(&view);
        assert_eq!(state.frame_subtab.current_key(), "tone");
    }
}
