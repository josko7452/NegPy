//! The NegFe window shell.
//!
//! The shell is intentionally small during the theme port: it paints the fixed regions
//! the golden comparator scores, so every capture has the same geometry as NegPy.

use eframe::egui::{
    self, Color32, Frame, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2,
};

use crate::edit::FrameEdit;
use crate::fixture::ReferenceFixture;
use crate::frame_source::FrameImage;
use crate::styles::icons::Icon;
use crate::styles::layout::TOP_ROW_HEIGHT;
use crate::styles::theme::Theme;
use crate::view::contact_sheet;
use crate::view::rail::{RAIL_WIDTH, RailState};
use negpy_backend::session::proof::ProofSettings;

use crate::export_worker::{ExportRequest, ExportWorker};
use crate::render_worker::ProofRequest;
use std::path::Path;

/// The visible tab of the right rail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    Library,
    #[default]
    Roll,
    Frame,
    Metadata,
    Export,
    Scan,
}

impl Tab {
    /// The tab labels in the golden tab-strip order.
    pub const ALL: [Self; 6] = [
        Self::Library,
        Self::Roll,
        Self::Frame,
        Self::Metadata,
        Self::Export,
        Self::Scan,
    ];

    /// The label under the tab icon.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Roll => "Roll",
            Self::Frame => "Frame",
            Self::Metadata => "Metadata",
            Self::Export => "Export",
            Self::Scan => "Scan",
        }
    }

    /// The icon above the tab label.
    #[must_use]
    pub fn icon(self) -> Icon {
        match self {
            Self::Library => Icon::Grid,
            Self::Roll => Icon::Film,
            Self::Frame => Icon::Image,
            Self::Metadata => Icon::Metadata,
            Self::Export => Icon::Export,
            Self::Scan => Icon::Scan,
        }
    }

    /// Select a tab by its manifest key.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "library" => Self::Library,
            "frame" => Self::Frame,
            "metadata" => Self::Metadata,
            "export" => Self::Export,
            "scan" => Self::Scan,
            _ => Self::Roll,
        }
    }
}

/// The controls a painted shell reported clicked this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellHits {
    pub import: bool,
    pub previous: bool,
    pub next: bool,
    pub tile: Option<usize>,
    pub crop: crate::view::canvas::CropAction,
    pub mask: Option<crate::view::canvas::DrawnMask>,
    pub retouch: crate::view::canvas::RetouchAction,
    pub mask_action: crate::view::canvas::MaskAction,
}

/// The whole-window view a manifest entry asks to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub tab: Tab,
    pub subtab: String,
    pub group: String,
    pub view: String,
    pub scroll: u32,
    pub analysis_open: bool,
    pub analysis_log: bool,
    pub strip_open: bool,
    pub texture_size: Option<Vec2>,
    pub contact_sheet: bool,
}

impl ViewState {
    /// The frame view in the reference fixture.
    #[must_use]
    pub fn from_fixture(fixture: &ReferenceFixture) -> Self {
        Self {
            tab: Tab::from_key(&fixture.fixture.tab),
            subtab: String::new(),
            group: "roll".to_string(),
            view: "canvas".to_string(),
            scroll: 0,
            analysis_open: fixture.fixture.analysis.open,
            analysis_log: fixture.fixture.analysis.log,
            strip_open: fixture.fixture.strip.open,
            texture_size: None,
            contact_sheet: false,
        }
    }

    /// Set the state named by one manifest entry.
    pub fn from_capture(group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) -> Self {
        let (tab, subtab) = if group == "frame" {
            (Tab::Frame, tab.to_string())
        } else {
            let tab = if tab.is_empty() {
                Tab::from_key(group)
            } else {
                Tab::from_key(tab)
            };
            (tab, subtab.to_string())
        };
        Self {
            tab,
            subtab,
            group: group.to_string(),
            view: view.to_string(),
            scroll: (scroll.max(0.0) * 1000.0).round() as u32,
            analysis_open: true,
            analysis_log: true,
            strip_open: true,
            texture_size: None,
            contact_sheet: view == "contact_sheet_dialog",
        }
    }
}

/// The desktop application state.
pub struct NegFeApp {
    pub view: ViewState,
    pub rail: RailState,
    pub edit: FrameEdit,
    pub proof: ProofSettings,
    pub profile: Option<std::path::PathBuf>,
    settings: Option<negpy_backend::infrastructure::storage::repository::StorageRepository>,
    pub frame: Option<FrameImage>,
    pub strip: Vec<FrameImage>,
    /// The open frame's index in the strip.
    pub selected: usize,
    /// The open frame's linear proxy, held until the frame changes.
    source: Option<std::sync::Arc<crate::render_worker::SourceFrame>>,
    renderer: crate::render_worker::RenderWorker,
    exporter: ExportWorker,
    scanner: crate::scan_worker::ScanWorker,
    sheets: crate::contact_sheet_worker::SheetWorker,
    sheet: crate::view::contact_sheet_dialog::ContactSheetDialog,
    sheet_request: u64,
    /// The request hash in flight, and the config hash that painted the canvas.
    requested: Option<String>,
    rendered: Option<String>,
    generation: u64,
    texture: Option<TextureHandle>,
    strip_textures: Vec<TextureHandle>,
    /// The crop drag on the canvas.
    crop_drag: Option<crate::view::canvas::CropDrag>,
    /// The local mask lasso, between clicks.
    mask_drag: crate::view::canvas::MaskDrag,
    /// The retouch stroke in progress.
    retouch_drag: crate::view::canvas::RetouchDrag,
}

impl NegFeApp {
    /// An app showing the reference fixture.
    #[must_use]
    pub fn new(fixture: &ReferenceFixture) -> Self {
        let view = ViewState::from_fixture(fixture);
        Self {
            rail: RailState::for_view(&view),
            view,
            edit: FrameEdit::default(),
            proof: ProofSettings::default(),
            profile: None,
            settings: None,
            frame: None,
            strip: Vec::new(),
            selected: 0,
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            sheet: crate::view::contact_sheet_dialog::ContactSheetDialog::default(),
            sheet_request: 0,
            requested: None,
            rendered: None,
            generation: 0,
            texture: None,
            strip_textures: Vec::new(),
            crop_drag: None,
            mask_drag: crate::view::canvas::MaskDrag::default(),
            retouch_drag: crate::view::canvas::RetouchDrag::default(),
        }
    }

    /// An app showing one manifest capture.
    #[must_use]
    pub fn for_capture(group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) -> Self {
        let view = ViewState::from_capture(group, tab, subtab, view, scroll);
        Self {
            rail: RailState::for_view(&view),
            view,
            edit: FrameEdit::default(),
            proof: ProofSettings::default(),
            profile: None,
            settings: None,
            frame: None,
            strip: Vec::new(),
            selected: 0,
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            sheet: crate::view::contact_sheet_dialog::ContactSheetDialog::default(),
            sheet_request: 0,
            requested: None,
            rendered: None,
            generation: 0,
            texture: None,
            strip_textures: Vec::new(),
            crop_drag: None,
            mask_drag: crate::view::canvas::MaskDrag::default(),
            retouch_drag: crate::view::canvas::RetouchDrag::default(),
        }
    }

    /// Load the Library from the profile the app reads and saves.
    pub fn set_profile(&mut self, profile: Option<&Path>) {
        self.rail.library = crate::view::rail::library::LibraryState::load(profile);
        let Some(profile) = profile else {
            return;
        };
        self.profile = Some(profile.to_path_buf());
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
        let gear = negpy_backend::features::metadata::gear::GearLibrary::load(
            Path::new(&app.gear_dir),
            &negpy_backend::kernel::system::paths::resource_path("gear"),
        );
        self.rail.metadata.set_gear(gear);
        self.rail
            .optics
            .set_profile(Some(profile), &self.edit.config.flatfield.profile_id);
        let repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path.clone(),
            app.settings_db_path.clone(),
        );
        self.proof = ProofSettings::load(&repo);
        self.settings = Some(repo.clone());
        let presets = repo.load_export_presets();
        self.rail.export.set_presets(presets);
        let profiles = negpy_backend::infrastructure::display::color_mgmt::available_profiles(
            std::path::Path::new(&app.user_icc_dir),
        );
        self.rail.export.set_icc_profiles(&profiles);
        let store = negpy_backend::services::assets::presets::PresetStore::new(
            app.presets_dir.clone(),
            "metadata",
        );
        self.rail.metadata.set_presets(store.list());
    }

    /// Show a decoded frame on the canvas.
    pub fn set_frame(&mut self, frame: FrameImage) {
        self.view.texture_size = Some(Vec2::new(frame.width as f32, frame.height as f32));
        self.texture = None;
        self.frame = Some(frame);
    }

    /// Set the edit the rail reads and writes.
    pub fn set_edit(&mut self, edit: FrameEdit) {
        self.edit = edit;
    }

    /// Keep the linear proxy of the open frame, so an edit re-renders without a decode.
    pub fn set_source(&mut self, source: Option<crate::render_worker::SourceFrame>) {
        self.source = source.map(std::sync::Arc::new);
        self.requested = None;
        self.rendered = None;
    }

    /// Submit one render when the config differs from the frame on the canvas.
    fn request_render(&mut self, ctx: &egui::Context, interactive: bool) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let request = crate::render_worker::RenderRequest {
            generation: self.generation + 1,
            source: source.clone(),
            config: self.edit.config.clone(),
            proof: self.proof_request(),
        };
        let hash = request.config_hash();
        if self.rendered.as_deref() == Some(hash.as_str())
            || self.requested.as_deref() == Some(hash.as_str())
        {
            return;
        }
        self.generation += 1;
        self.requested = Some(hash);
        self.renderer.submit(request);
        ctx.request_repaint();
        let _ = interactive;
    }

    /// Show the newest finished preview.
    fn receive_exports(&mut self, ctx: &egui::Context) {
        for report in self.exporter.drain() {
            self.rail.export_status.running = false;
            if report.failed.is_empty() {
                self.rail.export_status.failed = false;
                self.rail.export_status.message = format!(
                    "Wrote {}.",
                    crate::view::rail::count_of(report.written.len(), "file")
                );
            } else {
                self.rail.export_status.failed = true;
                self.rail.export_status.message = report.failed.join(" ");
            }
            ctx.request_repaint();
        }
    }

    fn request_scan(&mut self, ctx: &egui::Context) {
        if !self.rail.scan.take_scan_request() {
            return;
        }
        let Some(device) = self.rail.scan.selected_device().cloned() else {
            self.rail.scan.set_status("Select a scanner first.");
            return;
        };
        let folder = if self.rail.scan.output_folder.trim().is_empty() {
            let Some(profile) = self.profile.clone() else {
                self.rail.scan.set_status("Open a profile first.");
                return;
            };
            negpy_backend::domain::types::AppConfig::for_user_dir(&profile).default_export_dir
        } else {
            self.rail.scan.output_folder.clone()
        };
        self.rail.scan.set_status("Scanning...");
        self.scanner.submit(crate::scan_worker::ScanRequest {
            backend: self.rail.scan.backend.current_value().to_string(),
            device: device.id,
            params: self.rail.scan.params(),
            output_folder: std::path::PathBuf::from(&folder),
            filename_pattern: self.rail.scan.filename.clone(),
            output_format: self.rail.scan.output_format.current_value().to_string(),
        });
        ctx.request_repaint();
    }

    fn receive_scans(&mut self, ctx: &egui::Context) {
        for report in self.scanner.drain() {
            if let Some(error) = report.error {
                self.rail.scan.set_status(error);
            } else if let Some(path) = report.path {
                self.rail.scan.set_status(format!(
                    "Wrote {}.",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("the scan")
                ));
                if let Some(parent) = path.parent()
                    && self.rail.library.import(parent, false)
                {
                    self.open_requested_roll();
                }
            }
            ctx.request_repaint();
        }
    }

    fn sheet_entries(
        &self,
    ) -> Vec<negpy_backend::services::export::contact_sheet_page::SheetEntry> {
        let profile = self.profile.clone();
        self.strip
            .iter()
            .filter_map(|frame| {
                let path = frame.path.clone()?;
                let tile = crate::frame_source::load_thumbnail(&path, profile.as_deref(), 480)?;
                Some(
                    negpy_backend::services::export::contact_sheet_page::SheetEntry {
                        name: frame.name.clone(),
                        path: path.display().to_string(),
                        config: crate::frame_source::stored_config(profile.as_deref(), &path),
                        tile: negpy_backend::services::export::contact_sheet::Tile::new(
                            tile.width,
                            tile.height,
                            tile.pixels,
                        ),
                    },
                )
            })
            .collect()
    }

    fn request_sheet(&mut self, ctx: &egui::Context, export: bool) {
        self.sheet_request += 1;
        let entries = self.sheet_entries();
        let library = self.profile.as_ref().map(|profile| {
            let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
            negpy_backend::features::metadata::gear::GearLibrary::load(
                Path::new(&app.gear_dir),
                &negpy_backend::kernel::system::paths::resource_path("gear"),
            )
        });
        let output = export.then(|| {
            let folder = if self.rail.export.sidecar_path.trim().is_empty() {
                self.profile.as_ref().map_or_else(
                    || std::path::PathBuf::from("."),
                    |profile| {
                        std::path::PathBuf::from(
                            negpy_backend::domain::types::AppConfig::for_user_dir(profile)
                                .default_export_dir,
                        )
                    },
                )
            } else {
                std::path::PathBuf::from(self.rail.export.sidecar_path.trim())
            };
            folder.join(format!("contact_sheet_{}.png", self.sheet.page + 1))
        });
        let roll_name = self
            .source
            .as_ref()
            .map_or_else(|| "Roll".to_string(), |source| source.name.clone());
        self.sheets
            .submit(crate::contact_sheet_worker::SheetRequest {
                entries,
                settings: self.sheet.settings.clone(),
                roll_name,
                library,
                page: self.sheet.page,
                output,
                request_id: self.sheet_request,
            });
        ctx.request_repaint();
    }

    fn receive_sheets(&mut self, ctx: &egui::Context) {
        for report in self.sheets.drain() {
            if report.request_id != self.sheet_request {
                continue;
            }
            if let Some(error) = report.error {
                self.sheet.status = error;
            } else if let Some(path) = report.written {
                self.sheet.status = format!(
                    "Wrote {}.",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("the sheet")
                );
            } else {
                self.sheet
                    .set_preview(ctx, report.width, report.height, &report.pixels);
            }
            ctx.request_repaint();
        }
    }

    fn request_export(&mut self, ctx: &egui::Context) {
        if !self.rail.export.take_request() {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            self.rail.export_status.failed = true;
            self.rail.export_status.message = "Open a profile first.".to_string();
            return;
        };
        let mut paths = Vec::new();
        let mut configs = Vec::new();
        for frame in &self.strip {
            let Some(path) = &frame.path else {
                continue;
            };
            paths.push(path.clone());
            configs.push(crate::frame_source::stored_config(Some(&profile), path));
        }
        if paths.is_empty() {
            self.rail.export_status.failed = true;
            self.rail.export_status.message = "No frames to export.".to_string();
            return;
        }
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        self.rail.export_status.running = true;
        self.rail.export_status.failed = false;
        self.rail.export_status.message = format!(
            "Exporting {}...",
            crate::view::rail::count_of(paths.len(), "frame")
        );
        self.exporter.submit(ExportRequest {
            paths,
            configs,
            color_space: self.edit.config.export.export_color_space.clone(),
            user_icc_dir: std::path::PathBuf::from(app.user_icc_dir),
        });
        ctx.request_repaint();
    }

    fn save_export_preset(&mut self) {
        if !self.rail.export.take_save_preset_request() {
            return;
        }
        let Some(repo) = &mut self.settings else {
            self.rail.export.set_status("Open a profile first.");
            return;
        };
        let name = format!("Preset {}", self.rail.export.presets.len() + 1);
        let mut presets = self.rail.export.presets.clone();
        presets.push(self.rail.export.preset_named(&name));
        match repo.save_export_presets(&presets) {
            Ok(()) => {
                self.rail.export.set_presets(presets);
                self.rail.export.set_status(format!("Saved {name}."));
            }
            Err(error) => self.rail.export.set_status(error.to_string()),
        }
    }

    /// Whether a finished render is the one the canvas waits for.
    fn accepts_render(&self, output: &crate::render_worker::RenderOutput) -> bool {
        output.generation == self.generation
            && self.requested.as_deref() == Some(output.request_hash.as_str())
    }

    fn receive_render(&mut self, ctx: &egui::Context) -> bool {
        let mut shown = false;
        for output in self.renderer.drain() {
            if !self.accepts_render(&output) {
                continue;
            }
            self.rendered = Some(output.config_hash);
            self.freeze_autocrop(&output.metrics);
            self.store_measured_bounds(&output.metrics);
            self.rail
                .set_metrics(crate::view::rail::analysis::AnalysisMetrics::from_values(
                    &self.edit.config,
                    &output.metrics,
                ));
            self.rail.set_histogram(output.histogram);
            self.view.texture_size = Some(Vec2::new(
                output.frame.width as f32,
                output.frame.height as f32,
            ));
            self.frame = Some(FrameImage::from_display(self.source_name(), output.frame));
            self.texture = None;
            shown = true;
        }
        if shown {
            ctx.request_repaint();
        }
        shown
    }

    /// Store the per-frame bounds the render measured, unless the frame rides the roll.
    fn store_measured_bounds(
        &mut self,
        metrics: &std::collections::BTreeMap<String, serde_json::Value>,
    ) {
        if self.edit.config.process.lock_bounds
            || (self.edit.config.process.use_luma_average
                && self.edit.config.process.use_color_average)
        {
            return;
        }
        let Some(bounds) = metrics.get("final_bounds").and_then(|value| {
            serde_json::from_value::<
                negpy_backend::features::exposure::normalization::LogNegativeBounds,
            >(value.clone())
            .ok()
        }) else {
            return;
        };
        let process = &mut self.edit.config.process;
        if process.local_floors == bounds.floors && process.local_ceils == bounds.ceils {
            return;
        }
        process.local_floors = bounds.floors;
        process.local_ceils = bounds.ceils;
        self.edit.mark_dirty();
    }

    /// Store the crop a render found, unless the frame moved to another crop meanwhile.
    fn freeze_autocrop(&mut self, metrics: &std::collections::BTreeMap<String, serde_json::Value>) {
        let Some(resolved) = metrics.get("autocrop_resolved").and_then(|value| {
            serde_json::from_value::<negpy_backend::services::rendering::auto_geometry::ArmedCrop>(
                value.clone(),
            )
            .ok()
        }) else {
            return;
        };
        let Some(geometry) = freeze_autocrop_geometry(&self.edit.config.geometry, &resolved) else {
            return;
        };
        self.edit.config.geometry = geometry;
        self.edit.mark_dirty();
        self.rail.roll_analysis.stale = true;
    }

    /// Keep a confirmed crop once, and leave the Draw tool.
    fn confirm_crop(&mut self, rect: [f64; 4]) {
        self.store_crop(rect);
        self.rail.geometry.crop.draw = false;
        self.rail.geometry.crop.has_crop = true;
    }

    /// Write a crop rect and clear the Auto Crop state it replaces.
    fn store_crop(&mut self, rect: [f64; 4]) {
        let geometry = &mut self.edit.config.geometry;
        geometry.crop_rect = Some(rect.to_vec());
        geometry.crop_from_auto = false;
        geometry.crop_detect_key.clear();
        geometry.autocrop_squared = false;
        self.edit.mark_dirty();
    }

    /// The retouch tool the canvas accepts.
    fn retouch_tool(&self) -> crate::view::canvas::RetouchTool {
        match self.rail.finish.tool {
            crate::view::rail::frame_finish::RetouchTool::Heal => {
                crate::view::canvas::RetouchTool::Heal
            }
            crate::view::rail::frame_finish::RetouchTool::Scratch => {
                crate::view::canvas::RetouchTool::Scratch
            }
            crate::view::rail::frame_finish::RetouchTool::Line => {
                crate::view::canvas::RetouchTool::Line
            }
            crate::view::rail::frame_finish::RetouchTool::Clone => {
                crate::view::canvas::RetouchTool::Clone
            }
            crate::view::rail::frame_finish::RetouchTool::Off => {
                crate::view::canvas::RetouchTool::Off
            }
        }
    }

    /// Store one finished retouch gesture.
    fn apply_retouch(&mut self, action: crate::view::canvas::RetouchAction) {
        let retouch = &mut self.edit.config.retouch;
        let size = retouch.manual_dust_size as f64;
        match action {
            crate::view::canvas::RetouchAction::None => return,
            crate::view::canvas::RetouchAction::Heal(points) => {
                retouch
                    .manual_heal_strokes
                    .push(serde_json::json!([points, size, 0.0, 0.0]));
            }
            crate::view::canvas::RetouchAction::Line(line) => {
                retouch.scratch_lines.push(serde_json::json!([
                    line[0], line[1], line[2], line[3], line[4]
                ]));
            }
            crate::view::canvas::RetouchAction::Clone(points) => {
                let Some(source) = self.retouch_drag.clone_source_normalized else {
                    return;
                };
                let origin = points.first().copied().unwrap_or([0.0, 0.0]);
                let dx = source[0] - origin[0];
                let dy = source[1] - origin[1];
                retouch.clone_strokes.push(serde_json::json!([
                    points,
                    size,
                    dx,
                    dy,
                    retouch.clone_strength,
                    retouch.clone_feather,
                    retouch.clone_match_tone
                ]));
            }
        }
        self.edit.mark_dirty();
    }

    fn proof_request(&self) -> ProofRequest {
        ProofRequest {
            enabled: self.proof.enabled,
            icc_path: self.proof.icc_path.clone(),
            intent: self.proof.intent.clone(),
            black_point: self.proof.black_point,
            paper_white: self.proof.paper_white,
            ink_black: self.proof.ink_black,
            gamut_warning: self.proof.gamut_warning,
        }
    }

    fn source_name(&self) -> String {
        self.source
            .as_ref()
            .map_or_else(|| "frame".to_string(), |source| source.name.clone())
    }

    /// Move to the tab and page a capture entry asks for, without rebuilding the rail.
    pub fn select_view(&mut self, group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) {
        let texture_size = self.view.texture_size;
        self.view = ViewState::from_capture(group, tab, subtab, view, scroll);
        self.view.texture_size = texture_size;
        self.rail.select_view(&self.view);
    }

    /// Show the roll's thumbnails in the Film Strip.
    pub fn set_strip(&mut self, strip: Vec<FrameImage>) {
        self.strip_textures.clear();
        self.strip = strip;
        self.selected = 0;
    }

    /// Open the strip frame at `index` and bind its edit. Saves the old edit first.
    pub fn select_frame(&mut self, index: usize) -> bool {
        let Some(path) = self.strip.get(index).and_then(|frame| frame.path.clone()) else {
            return false;
        };
        let profile = self.profile.clone();
        let _ = self.edit.save();
        let Ok(frame) = crate::frame_source::load_display_frame(&path, profile.as_deref(), 1600)
        else {
            return false;
        };
        let edit = FrameEdit::load_in_roll(
            profile.as_deref(),
            &path,
            self.rail.roll_analysis.roll_id.as_deref(),
        );
        let config = edit.config.clone();
        if let Ok((source, _)) =
            crate::frame_source::load_linear_source(&path, profile.as_deref(), 1600)
        {
            self.set_source(Some(source));
        }
        self.set_frame(frame);
        self.set_edit(edit);
        self.rail.metadata.set_folder_name(
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        );
        self.rail
            .optics
            .set_profile(profile.as_deref(), &self.edit.config.flatfield.profile_id);
        self.selected = index;
        if let Some(profile) = profile.as_deref() {
            let (rows, prints) = crate::frame_source::stored_history(Some(profile), &path);
            self.set_history(&rows, &prints);
            self.set_roll_analysis(crate::view::rail::roll_analysis::RollAnalysisState::load(
                Some(profile),
                &path,
                Some(&config),
            ));
        }
        true
    }

    /// Move to the previous strip frame.
    pub fn previous_frame(&mut self) -> bool {
        if self.selected == 0 {
            return false;
        }
        self.select_frame(self.selected - 1)
    }

    /// Move to the next strip frame.
    pub fn next_frame(&mut self) -> bool {
        if self.selected + 1 >= self.strip.len() {
            return false;
        }
        self.select_frame(self.selected + 1)
    }

    /// Open a saved roll: load its first decodable frame, fill the strip, and bind the
    /// frame's edit. A roll with no decodable frame leaves the canvas empty.
    pub fn open_roll(&mut self, roll_id: &str) -> bool {
        let Some(profile) = self.profile.clone() else {
            self.mark_roll_open(roll_id);
            return false;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let paths: Vec<std::path::PathBuf> =
            negpy_backend::services::assets::rolls::roll_frame_paths(&repo, roll_id)
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect();
        self.mark_roll_open(roll_id);
        let Some((frame, strip)) =
            crate::frame_source::load_first_frame(&paths, Some(&profile), 1600, 12)
        else {
            return false;
        };
        let open_path = frame.path.clone();
        let edit = open_path
            .as_deref()
            .map_or_else(FrameEdit::default, |path| {
                FrameEdit::load_in_roll(Some(&profile), path, Some(roll_id))
            });
        let edit_config = edit.config.clone();
        if let Some(path) = open_path.as_deref()
            && let Ok((source, _)) =
                crate::frame_source::load_linear_source(path, Some(&profile), 1600)
        {
            self.set_source(Some(source));
        }
        self.set_edit(edit);
        self.rail
            .optics
            .set_profile(Some(&profile), &self.edit.config.flatfield.profile_id);
        self.set_frame(frame);
        self.set_strip(strip);
        self.selected = 0;
        if let Some(path) = open_path {
            self.rail.metadata.set_folder_name(
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
            );
            let (rows, prints) = crate::frame_source::stored_history(Some(&profile), &path);
            self.set_history(&rows, &prints);
            self.set_roll_analysis(crate::view::rail::roll_analysis::RollAnalysisState::load(
                Some(&profile),
                &path,
                Some(&edit_config),
            ));
        }
        true
    }

    fn mark_roll_open(&mut self, roll_id: &str) {
        for row in &mut self.rail.library.rolls {
            row.open = row.id == roll_id;
        }
    }

    /// Show the frame's stored History rows and work-print names.
    pub fn set_history(
        &mut self,
        rows: &[negpy_backend::session::workspace::HistoryRow],
        work_prints: &[String],
    ) {
        self.rail.history.load_session(rows, work_prints);
    }

    /// Show the roll and baseline labels the stored edit names.
    pub fn set_roll_analysis(
        &mut self,
        state: crate::view::rail::roll_analysis::RollAnalysisState,
    ) {
        self.rail.roll_analysis = state;
    }

    /// Paint the shell regions for this frame.
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.receive_exports(ctx);
        self.receive_render(ctx);
        self.receive_scans(ctx);
        self.receive_sheets(ctx);
        let theme = Theme::default();
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = theme.bg_panel;
        style.visuals.window_fill = theme.bg_panel;
        style.visuals.extreme_bg_color = theme.bg_input;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border_primary);
        ctx.set_style(style);
        let texture = self.texture_for(ctx).map(|texture| texture.id());
        self.ensure_strip_textures(ctx);
        let strip_textures: Vec<(egui::TextureId, Vec2)> = self
            .strip_textures
            .iter()
            .zip(self.strip.iter())
            .map(|(texture, frame)| {
                (
                    texture.id(),
                    Vec2::new(frame.width as f32, frame.height as f32),
                )
            })
            .collect();
        let screen = ctx.screen_rect();
        if self.view.view == "light_table" {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme.bg_panel))
                .show(ctx, |ui| {
                    if let Some(index) =
                        crate::view::light_table::paint(ui, screen, &strip_textures, self.selected)
                    {
                        self.select_frame(index);
                    }
                    let rail_rect = rail_rect(screen);
                    self.request_export(ctx);
                    self.request_scan(ctx);
                    let proof = self.proof.clone();
                    let switched = self
                        .rail
                        .show(ui, rail_rect, &mut self.edit, &mut self.proof);
                    self.rail.apply_optics_action(&mut self.edit);
                    self.rail.apply_history_action(&mut self.edit);
                    self.apply_roll_analysis(ctx);
                    self.open_requested_roll();
                    self.view.tab = self.rail.tab;
                    if let Some(tab) = switched {
                        self.view.tab = tab;
                    }
                    let _ = self.edit.save();
                    if self.proof != proof {
                        self.save_proof();
                    }
                });
            return;
        }
        if self.view.contact_sheet {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme.bg_panel))
                .show(ctx, |ui| {
                    if self.view.view == "contact_sheet_dialog" {
                        let actions = self.sheet.show(ui, screen);
                        if actions.refresh {
                            self.request_sheet(ctx, false);
                        }
                        if actions.export {
                            self.request_sheet(ctx, true);
                        }
                        if actions.close {
                            self.view.contact_sheet = false;
                        }
                    } else {
                        ui.painter().rect_filled(screen, 0.0, theme.bg_panel);
                        contact_sheet::paint(ui.painter(), screen, texture);
                    }
                });
            return;
        }
        let root = Frame::NONE.fill(surface());
        egui::CentralPanel::default().frame(root).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, surface());
            let label = self.frame_label();
            let retouch_tool = self.retouch_tool();
            let (rail_rect, hits) = paint_shell(
                ui,
                screen,
                &self.view,
                texture,
                &strip_textures,
                self.has_frame(),
                &label,
                self.selected,
                (
                    &self.edit.config.geometry,
                    self.frame.as_ref().map(|frame| (frame.width, frame.height)),
                ),
                self.rail.geometry.crop.draw,
                &mut self.crop_drag,
                (
                    &self.edit.config.local.masks,
                    self.rail.exposure.local.draw,
                    self.rail.exposure.local.oval,
                    self.rail.exposure.local.card,
                    self.rail.exposure.mask_index,
                ),
                &mut self.mask_drag,
                retouch_tool,
                &mut self.retouch_drag,
            );
            if hits.import {
                self.rail.library.import_with_picker(false);
            }
            if let Some(index) = hits.tile {
                self.select_frame(index);
            }
            if hits.previous {
                self.previous_frame();
            }
            if hits.next {
                self.next_frame();
            }
            match hits.crop {
                crate::view::canvas::CropAction::Pending(rect) => self.store_crop(rect),
                crate::view::canvas::CropAction::Confirm(rect) => {
                    self.confirm_crop(rect);
                    self.request_render(ctx, false);
                }
                crate::view::canvas::CropAction::None => {}
            }
            self.apply_retouch(hits.retouch);
            match hits.mask_action {
                crate::view::canvas::MaskAction::None => {}
                crate::view::canvas::MaskAction::Select(index) => {
                    self.rail.exposure.mask_index = index;
                    ctx.request_repaint();
                }
                crate::view::canvas::MaskAction::MoveVertex(index, vertex, point) => {
                    if let Some(mask) = self.edit.config.local.masks.get_mut(index)
                        && let Some(vertex) = mask.vertices.get_mut(vertex)
                    {
                        *vertex = point;
                        self.edit.mark_dirty();
                        ctx.request_repaint();
                    }
                }
            }
            if let Some((shape, vertices)) = hits.mask {
                self.edit
                    .config
                    .local
                    .masks
                    .push(negpy_backend::domain::models::LocalMask {
                        vertices,
                        shape,
                        ..Default::default()
                    });
                self.rail.exposure.local.draw = false;
                self.rail.exposure.local.oval = false;
                self.rail.exposure.local.card = false;
                self.rail.exposure.mask_index =
                    self.edit.config.local.masks.len().saturating_sub(1);
                self.edit.mark_dirty();
                ctx.request_repaint();
            }
            self.request_render(ctx, false);
            self.request_export(ctx);
            self.save_export_preset();
            self.request_scan(ctx);
            let proof = self.proof.clone();
            let switched = self
                .rail
                .show(ui, rail_rect, &mut self.edit, &mut self.proof);
            self.rail.apply_optics_action(&mut self.edit);
            self.rail.apply_history_action(&mut self.edit);
            self.apply_roll_analysis(ctx);
            if self.rail.export.take_contact_sheet_request() {
                self.view.contact_sheet = true;
                self.view.view = "contact_sheet_dialog".to_string();
                self.sheet.page = 0;
                self.request_sheet(ctx, false);
            }
            self.open_requested_roll();
            self.view.tab = self.rail.tab;
            if let Some(tab) = switched {
                self.view.tab = tab;
            }
            let _ = self.edit.save();
            if self.proof != proof {
                self.save_proof();
            }
        });
    }

    fn save_proof(&mut self) {
        let Some(repo) = &mut self.settings else {
            return;
        };
        let _ = self.proof.save(repo);
    }

    fn apply_roll_analysis(&mut self, ctx: &egui::Context) {
        if self.rail.roll_analysis.take_use_frame() {
            self.use_frame_as_baseline(ctx);
        }
        if self.rail.roll_analysis.take_reanalyze() {
            self.rail.roll_analysis.stale = false;
            self.requested = None;
            self.rendered = None;
            ctx.request_repaint();
        }
    }

    /// Save the open frame's own measurement as the roll baseline, then load it back.
    fn use_frame_as_baseline(&mut self, ctx: &egui::Context) {
        let Some(profile) = self.edit.source().map(|(profile, _)| profile.to_path_buf()) else {
            return;
        };
        let Some(roll_id) = self
            .rail
            .roll_analysis
            .roll_id
            .clone()
            .or_else(|| self.rail.roll_analysis.roll_name.clone())
        else {
            return;
        };
        let process = &self.edit.config.process;
        if !process.is_local_initialized() {
            return;
        }
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let floors = process.local_floors;
        let ceils = process.local_ceils;
        negpy_backend::services::assets::rolls::set_roll_normalization(
            &mut repo,
            &roll_id,
            &floors,
            &ceils,
            &[0.0; 3],
            &[],
            None,
            negpy_backend::domain::models::workspace::BOUNDS_VERSION,
            None,
            None,
        );
        self.edit.config.process.baseline_source = format!("roll:{roll_id}");
        self.edit.config.process.locked_floors = floors;
        self.edit.config.process.locked_ceils = ceils;
        self.edit.mark_dirty();
        self.rail.roll_analysis.stale = false;
        self.rail.roll_analysis.baseline =
            negpy_backend::services::assets::rolls::baseline_label_without_repo(
                &self.edit.config.process,
            );
        self.requested = None;
        self.rendered = None;
        ctx.request_repaint();
    }

    fn open_requested_roll(&mut self) {
        if let Some(roll_id) = self.rail.library.take_open_request() {
            self.open_roll(&roll_id);
        }
    }

    /// Whether a decoded frame is on the canvas.
    #[must_use]
    pub fn has_frame(&self) -> bool {
        self.frame.as_ref().is_some_and(FrameImage::is_rgb)
    }

    /// The file name and size the top bar prints.
    #[must_use]
    pub fn frame_label(&self) -> String {
        match &self.frame {
            Some(frame) if frame.is_rgb() => {
                format!("{} · {} x {} px", frame.name, frame.width, frame.height)
            }
            _ => "No frame open".to_string(),
        }
    }

    fn texture_for(&mut self, ctx: &egui::Context) -> Option<&TextureHandle> {
        if self.texture.is_none()
            && let Some(frame) = &self.frame
            && frame.is_rgb()
        {
            let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.pixels);
            self.texture = Some(ctx.load_texture("fixture-frame", image, TextureOptions::LINEAR));
        }
        self.texture.as_ref()
    }

    fn ensure_strip_textures(&mut self, ctx: &egui::Context) {
        if self.strip_textures.len() == self.strip.len() {
            return;
        }
        self.strip_textures = self
            .strip
            .iter()
            .filter(|frame| frame.is_rgb())
            .map(|frame| {
                let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.pixels);
                ctx.load_texture(frame.name.clone(), image, TextureOptions::LINEAR)
            })
            .collect();
    }
}

/// The right rail's bounds for a window size.
#[must_use]
pub fn rail_rect(screen: Rect) -> Rect {
    let margin = 8.0;
    let body_top = screen.top() + TOP_ROW_HEIGHT + 8.0;
    let rail_width = if screen.width() >= 1800.0 {
        RAIL_WIDTH
    } else {
        RAIL_WIDTH.min((screen.width() * 0.27).max(420.0))
    };
    Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    )
}

impl eframe::App for NegFeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

fn surface() -> Color32 {
    Theme::default().bg_panel
}

/// The geometry that freezes a resolved Auto Crop, or `None` when the arm changed.
fn freeze_autocrop_geometry(
    current: &negpy_backend::domain::models::GeometryConfig,
    resolved: &negpy_backend::services::rendering::auto_geometry::ArmedCrop,
) -> Option<negpy_backend::domain::models::GeometryConfig> {
    use negpy_backend::features::geometry::logic::{autocrop_detection_key, squares_on_arm};
    let mut geometry = current.clone();
    if !geometry.crop_from_auto {
        return None;
    }
    if resolved.squared {
        if !squares_on_arm(&geometry) {
            return None;
        }
        if let Some([fine, converge_v, converge_h]) = resolved.skew {
            geometry.fine_rotation = fine;
            geometry.converge_v = converge_v;
            geometry.converge_h = converge_h;
        }
        geometry.autocrop_squared = true;
    }
    if autocrop_detection_key(&geometry) != resolved.key {
        return None;
    }
    if let Some(rect) = resolved.rect {
        geometry.crop_rect = Some(rect.to_vec());
        geometry.crop_detect_key = resolved.key.clone();
    }
    (geometry != *current).then_some(geometry)
}

fn canvas() -> Color32 {
    Theme::default().canvas_bg_mid_grey
}

fn line() -> Stroke {
    Stroke::new(1.0_f32, Theme::default().border_primary)
}

fn paint_shell(
    ui: &mut egui::Ui,
    screen: Rect,
    view: &ViewState,
    frame: Option<egui::TextureId>,
    thumbnails: &[(egui::TextureId, Vec2)],
    has_frame: bool,
    label: &str,
    selected: usize,
    geometry: (
        &negpy_backend::domain::models::GeometryConfig,
        Option<(usize, usize)>,
    ),
    crop_draw: bool,
    crop_drag: &mut Option<crate::view::canvas::CropDrag>,
    local: (
        &[negpy_backend::domain::models::LocalMask],
        bool,
        bool,
        bool,
        usize,
    ),
    mask_drag: &mut crate::view::canvas::MaskDrag,
    retouch_tool: crate::view::canvas::RetouchTool,
    retouch_drag: &mut crate::view::canvas::RetouchDrag,
) -> (Rect, ShellHits) {
    let painter = ui.painter().clone();
    let margin = 8.0;
    paint_top_bar_with(&painter, screen, label);

    let body_top = screen.top() + crate::styles::layout::TOP_ROW_HEIGHT + 8.0;
    let rail_width = RAIL_WIDTH.min((screen.width() * 0.27).max(420.0));
    let rail_width = if screen.width() >= 1800.0 {
        RAIL_WIDTH
    } else {
        rail_width
    };
    let search = Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, screen.top() + margin),
        egui::pos2(
            screen.right() - margin,
            screen.top() + margin + crate::view::rail::SEARCH_HEIGHT,
        ),
    );
    let mut search_ui = egui::Ui::new(
        painter.ctx().clone(),
        "top-bar-search".into(),
        egui::UiBuilder::new()
            .max_rect(search)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    search_ui.set_width(search.width());
    crate::view::rail::paint_search(&mut search_ui, &Theme::default());
    let rail = Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    );
    let strip_height = if view.strip_open && has_frame {
        239.0
    } else {
        0.0
    };
    let toolbar_height = 52.0;
    let canvas_width = rail.left() - margin - (screen.left() + margin);
    let canvas_bottom = screen.bottom() - margin - toolbar_height - strip_height;
    let canvas_rect = Rect::from_min_size(
        egui::pos2(screen.left() + margin, body_top),
        Vec2::new(canvas_width, canvas_bottom - body_top),
    );
    painter.rect_filled(canvas_rect, 0.0, canvas());

    let photo = if has_frame {
        geometry.1.map_or_else(
            || fitted_rect(canvas_rect, Vec2::new(1208.0, 1181.0)),
            |(width, height)| {
                crate::view::canvas::photo_rect(canvas_rect, width as u32, height as u32)
            },
        )
    } else {
        canvas_rect
    };
    let mut hits = ShellHits::default();
    if let Some(texture) = frame.filter(|_| has_frame) {
        painter.image(
            texture,
            photo,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let crop = crate::view::canvas::crop_tool(
            ui,
            photo,
            geometry.0.crop_rect.as_deref(),
            crop_draw,
            geometry.0.crop_rect.is_some() && !geometry.0.crop_from_auto,
            crop_drag,
        );
        hits.crop = crop;
        let tool = if local.1 {
            crate::view::canvas::MaskTool::Lasso
        } else if local.2 {
            crate::view::canvas::MaskTool::Oval
        } else if local.3 {
            crate::view::canvas::MaskTool::Card
        } else {
            crate::view::canvas::MaskTool::Off
        };
        let (mask, action) =
            crate::view::canvas::local_tool(ui, photo, local.0, local.4, tool, mask_drag);
        hits.mask = mask;
        hits.mask_action = action;
        hits.retouch = crate::view::canvas::retouch_tool(ui, photo, retouch_tool, retouch_drag);
    } else {
        hits.import = paint_empty_canvas(ui, photo);
    }

    if strip_height > 0.0 {
        let strip = Rect::from_min_size(
            egui::pos2(screen.left() + margin, canvas_bottom),
            Vec2::new(canvas_width, strip_height),
        );
        let theme = Theme::default();
        painter.rect_filled(strip, 0.0, Color32::from_rgb(0x6f, 0x6f, 0x6f));
        let scrollbar = 12.0;
        let tiles = Rect::from_min_max(
            strip.min + Vec2::splat(0.0),
            egui::pos2(strip.right(), strip.bottom() - scrollbar),
        );
        let tile_width = 210.0;
        let gap = 4.0;
        let inset = 4.0;
        let count = thumbnails
            .len()
            .max(((tiles.width() - inset) / (tile_width + gap)).ceil() as usize);
        for index in 0..count {
            let left = tiles.left() + inset + index as f32 * (tile_width + gap);
            let tile = Rect::from_min_size(
                egui::pos2(left, tiles.top() + inset),
                Vec2::new(tile_width, tiles.height() - 2.0 * inset),
            );
            painter.rect_filled(tile, 0.0, Color32::from_rgb(0x17, 0x15, 0x16));
            if let Some((texture, size)) = thumbnails.get(index) {
                let content = cover_rect(tile, *size);
                painter.with_clip_rect(tile).image(
                    *texture,
                    content,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            if index < thumbnails.len() {
                let response = ui.interact(
                    tile,
                    ui.id().with(("film-strip-tile", index)),
                    Sense::click(),
                );
                if response.clicked() {
                    hits.tile = Some(index);
                }
            }
            let border = if index == selected && index < thumbnails.len() {
                theme.border_checked
            } else if index == selected {
                theme.error
            } else {
                theme.border_color
            };
            painter.rect_stroke(
                tile,
                0.0,
                Stroke::new(if index == selected { 3.0_f32 } else { 1.0_f32 }, border),
                egui::StrokeKind::Inside,
            );
            let dot = egui::pos2(tile.right() - 7.0, tile.top() + 7.0);
            painter.circle_filled(
                dot,
                4.0,
                if index < 2 {
                    theme.warn_amber
                } else {
                    Color32::from_rgb(0x8b, 0x8b, 0x8b)
                },
            );
            painter.circle_stroke(dot, 4.0, Stroke::new(1.0_f32, theme.mark_photo));
        }
        let bar = Rect::from_min_max(
            egui::pos2(strip.left() + inset, tiles.bottom() + 2.0),
            egui::pos2(strip.left() + strip.width() * 0.5, strip.bottom() - 2.0),
        );
        painter.rect_filled(bar, bar.height() / 2.0, theme.border_color);
    }

    let toolbar = Rect::from_min_size(
        egui::pos2(
            screen.left() + margin,
            screen.bottom() - margin - toolbar_height,
        ),
        Vec2::new(canvas_width, toolbar_height),
    );
    painter.rect_filled(toolbar, 0.0, surface());
    painter.line_segment([toolbar.left_top(), toolbar.right_top()], line());

    painter.rect_filled(rail, 0.0, surface());
    painter.rect_stroke(rail, 0.0, line(), egui::StrokeKind::Outside);
    let navigation = paint_toolbar(ui, &painter, toolbar, selected, thumbnails.len());
    hits.previous = navigation.previous;
    hits.next = navigation.next;
    (rail, hits)
}

/// The top bar's bounds, from the window edge to the rail.
#[must_use]
pub fn top_bar_rect(screen: Rect) -> Rect {
    let margin = 8.0;
    Rect::from_min_max(
        screen.min + Vec2::new(margin, margin),
        egui::pos2(
            screen.right() - margin,
            screen.top() + TOP_ROW_HEIGHT + margin,
        ),
    )
}

/// Paint the file name and the current recipe over `screen`.
pub fn paint_top_bar(painter: &egui::Painter, screen: Rect) {
    paint_top_bar_with(painter, screen, "P5080635.ORF · 7792 x 10388 px");
}

/// Paint the file name and the current recipe over `screen`.
pub fn paint_top_bar_with(painter: &egui::Painter, screen: Rect, frame: &str) {
    let top = top_bar_rect(screen);
    painter.rect_filled(top, 0.0, surface());
    painter.text(
        top.left_center() + Vec2::new(f32::from(Theme::default().space_xl), 0.0),
        egui::Align2::LEFT_CENTER,
        frame,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );
    painter.text(
        egui::pos2(screen.right() - RAIL_WIDTH - 26.0, top.center().y),
        egui::Align2::RIGHT_CENTER,
        "Digital (Adobe RGB) · Print Density 1.08 (auto) · Grade ISO-R 83 (auto)",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );
}

fn cover_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).max(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn fitted_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).min(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

/// Paint the first-run state: one line that says what to do and the Import action.
/// Returns whether the user asked to open a folder as a roll.
fn paint_empty_canvas(ui: &mut egui::Ui, area: Rect) -> bool {
    let theme = Theme::default();
    ui.painter().text(
        area.center() - Vec2::new(0.0, 40.0),
        egui::Align2::CENTER_CENTER,
        "No roll is open",
        egui::FontId::proportional(f32::from(theme.font_size_title)),
        theme.text_primary,
    );
    ui.painter().text(
        area.center() - Vec2::new(0.0, 10.0),
        egui::Align2::CENTER_CENTER,
        "Open a folder as a roll from the Library tab.",
        egui::FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_secondary,
    );
    let action = Rect::from_center_size(
        area.center() + Vec2::new(0.0, 40.0),
        Vec2::new(220.0, crate::styles::layout::DEFAULT_BUTTON_HEIGHT),
    );
    let mut action_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(action)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    action_ui.set_width(action.width());
    crate::styles::templates::action_with_icon(
        &mut action_ui,
        Icon::Folder,
        "Open folder as a roll…",
        crate::styles::templates::ActionRank::Normal,
        "Open a folder as a roll",
    )
    .clicked()
}

fn paint_toolbar(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    toolbar: Rect,
    selected: usize,
    frames: usize,
) -> ShellHits {
    use crate::styles::icons::Icon;
    use crate::styles::layout::{
        BAR_BUTTON_HEIGHT, BAR_ICON_SIZE, BAR_TRACK_BUTTON_HEIGHT, BAR_TRACK_PAD,
    };

    let theme = Theme::default();
    painter.rect_filled(toolbar, 0.0, surface());
    let row_height = BAR_BUTTON_HEIGHT;
    let top = toolbar.top() + (toolbar.height() - row_height) / 2.0;
    let mut x = toolbar.left() + 8.0;

    let flat = |painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32| {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon.glyph(),
            egui::FontId::proportional(BAR_ICON_SIZE),
            color,
        );
    };
    let track = |painter: &egui::Painter, rect: Rect| {
        painter.rect_filled(rect, 4.0, theme.bg_input);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0_f32, theme.border_primary),
            egui::StrokeKind::Inside,
        );
    };

    let mode = Rect::from_min_size(egui::pos2(x, top), Vec2::new(146.0, row_height));
    track(painter, mode);
    let segment = mode.shrink2(Vec2::new(BAR_TRACK_PAD, BAR_TRACK_PAD));
    let photo = Rect::from_min_size(
        segment.min,
        Vec2::new(segment.width() / 2.0, BAR_TRACK_BUTTON_HEIGHT),
    );
    let roll = Rect::from_min_size(
        egui::pos2(photo.right(), photo.top()),
        Vec2::new(segment.width() / 2.0, BAR_TRACK_BUTTON_HEIGHT),
    );
    painter.rect_filled(photo, 4.0, theme.bg_checked);
    painter.rect_stroke(
        photo,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_checked),
        egui::StrokeKind::Inside,
    );
    flat(
        painter,
        Rect::from_min_size(photo.min, Vec2::new(24.0, photo.height())),
        Icon::Image,
        theme.text_primary,
    );
    painter.text(
        photo.left_center() + Vec2::new(26.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Photo",
        egui::FontId::proportional(13.0),
        theme.text_primary,
    );
    flat(
        painter,
        Rect::from_min_size(roll.min, Vec2::new(24.0, roll.height())),
        Icon::Film,
        theme.text_hint,
    );
    painter.text(
        roll.left_center() + Vec2::new(26.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Roll",
        egui::FontId::proportional(13.0),
        theme.text_hint,
    );
    x = mode.right() + 8.0;

    let navigation = Rect::from_min_size(egui::pos2(x, top), Vec2::new(172.0, row_height));
    track(painter, navigation);
    let inner = navigation.shrink2(Vec2::new(BAR_TRACK_PAD, BAR_TRACK_PAD));
    let previous = Rect::from_min_size(inner.min, Vec2::splat(BAR_TRACK_BUTTON_HEIGHT));
    let next = Rect::from_min_size(
        egui::pos2(inner.right() - BAR_TRACK_BUTTON_HEIGHT, inner.top()),
        Vec2::splat(BAR_TRACK_BUTTON_HEIGHT),
    );
    flat(painter, previous, Icon::ChevronLeft, theme.text_secondary);
    let previous = ui.interact(previous, ui.id().with("previous-frame"), Sense::click());
    painter.text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        &format!("{} / {}", selected + 1, frames.max(selected + 1)),
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(painter, next, Icon::ChevronRight, theme.text_secondary);
    let next = ui.interact(next, ui.id().with("next-frame"), Sense::click());
    x = navigation.right() + 8.0;

    let zoom = Rect::from_min_size(egui::pos2(x, top), Vec2::new(76.0, row_height));
    track(painter, zoom);
    painter.text(
        zoom.center() - Vec2::new(6.0, 0.0),
        egui::Align2::CENTER_CENTER,
        "16%",
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(zoom.right() - 26.0, zoom.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_secondary,
    );
    x = zoom.right() + 8.0;

    let geometry = Rect::from_min_size(egui::pos2(x, top), Vec2::new(160.0, row_height));
    track(painter, geometry);
    for (index, icon) in [
        Icon::RotateLeft,
        Icon::RotateRight,
        Icon::LockSimple,
        Icon::LockOpen,
    ]
    .iter()
    .enumerate()
    {
        let button = Rect::from_min_size(
            egui::pos2(
                geometry.left()
                    + BAR_TRACK_PAD
                    + index as f32 * (geometry.width() - 2.0 * BAR_TRACK_PAD) / 4.0,
                geometry.top() + BAR_TRACK_PAD,
            ),
            Vec2::new(
                (geometry.width() - 2.0 * BAR_TRACK_PAD) / 4.0,
                BAR_TRACK_BUTTON_HEIGHT,
            ),
        );
        if index == 3 {
            painter.rect_filled(button, 4.0, theme.bg_checked);
        }
        flat(
            painter,
            button,
            *icon,
            if index == 3 {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
    }
    x = geometry.right() + 8.0;

    for (icon, width) in [(Icon::Wrench, 94.0), (Icon::Undo, 36.0), (Icon::Redo, 36.0)] {
        let button = Rect::from_min_size(egui::pos2(x, top), Vec2::new(width, row_height));
        track(painter, button);
        if icon == Icon::Wrench {
            flat(
                painter,
                Rect::from_min_size(button.min, Vec2::new(30.0, row_height)),
                icon,
                theme.text_primary,
            );
            painter.text(
                button.left_center() + Vec2::new(34.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "Tools",
                egui::FontId::proportional(13.0),
                theme.text_primary,
            );
            flat(
                painter,
                Rect::from_min_size(
                    egui::pos2(button.right() - 26.0, button.top()),
                    Vec2::new(24.0, row_height),
                ),
                Icon::ChevronDown,
                theme.text_secondary,
            );
            x = button.right() + 8.0;
            continue;
        }
        flat(
            painter,
            button,
            icon,
            if icon == Icon::Undo {
                theme.text_secondary
            } else {
                theme.text_muted
            },
        );
        x = button.right() + 6.0;
    }

    let print = Rect::from_min_size(egui::pos2(x, top), Vec2::new(112.0, row_height));
    track(painter, print);
    flat(
        painter,
        Rect::from_min_size(print.min, Vec2::new(34.0, row_height)),
        Icon::Print,
        theme.text_secondary,
    );
    painter.text(
        print.left_center() + Vec2::new(38.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Print",
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(print.right() - 26.0, print.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_secondary,
    );
    x = print.right() + 6.0;

    let roll_average = Rect::from_min_size(egui::pos2(x, top), Vec2::new(66.0, row_height));
    track(painter, roll_average);
    flat(painter, roll_average, Icon::Film, theme.text_secondary);
    x = roll_average.right() + 6.0;

    let trailing = Rect::from_min_size(
        egui::pos2(toolbar.right() - 8.0 - 108.0, top),
        Vec2::new(108.0, row_height),
    );
    let primary = Rect::from_min_size(egui::pos2(x, top), Vec2::new(154.0, row_height));
    painter.rect_filled(primary, 4.0, theme.bg_primary);
    painter.rect_stroke(
        primary,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_checked_hover),
        egui::StrokeKind::Inside,
    );
    flat(
        painter,
        Rect::from_min_size(primary.min, Vec2::new(36.0, row_height)),
        Icon::Export,
        theme.text_on_accent,
    );
    painter.text(
        primary.left_center() + Vec2::new(40.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Roll",
        egui::FontId::proportional(13.0),
        theme.text_on_accent,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(primary.right() - 26.0, primary.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_on_accent,
    );

    track(painter, trailing);
    for (index, icon) in [
        Icon::Grid,
        Icon::ListDashes,
        Icon::Sliders,
        Icon::ChevronDown,
    ]
    .iter()
    .enumerate()
    {
        let width = 26.0;
        let button = Rect::from_min_size(
            egui::pos2(
                trailing.left() + BAR_TRACK_PAD + index as f32 * width,
                trailing.top() + BAR_TRACK_PAD,
            ),
            Vec2::new(width, BAR_TRACK_BUTTON_HEIGHT),
        );
        flat(
            painter,
            button,
            *icon,
            if index == 0 {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
        if index == 1 {
            painter.circle_filled(
                egui::pos2(button.center().x, button.center().y - 6.0),
                2.5,
                theme.status_success,
            );
        }
    }
    ShellHits {
        previous: previous.clicked(),
        next: next.clicked(),
        ..ShellHits::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_key_selects_its_tab() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_key(&tab.label().to_lowercase()), tab);
        }
        assert_eq!(Tab::from_key("unknown"), Tab::Roll);
    }

    #[test]
    fn an_empty_shell_has_no_frame_label() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        assert!(!app.has_frame());
        assert_eq!(app.frame_label(), "No frame open");
        app.set_frame(crate::frame_source::FrameImage::from_display(
            "frame.orf",
            negpy_backend::services::rendering::display_frame::DisplayFrame::from_rgb(
                4,
                2,
                vec![0x20; 24],
            ),
        ));
        assert!(app.has_frame());
        assert_eq!(app.frame_label(), "frame.orf · 4 x 2 px");
    }

    #[test]
    fn every_tab_has_a_distinct_icon_role() {
        let icons: Vec<_> = Tab::ALL.iter().map(|tab| tab.icon()).collect();
        for (index, icon) in icons.iter().enumerate() {
            assert!(!icons[..index].contains(icon));
        }
    }

    #[test]
    fn a_capture_keeps_its_scroll_in_thousandths() {
        let view = ViewState::from_capture("roll", "frame", "tone", "canvas", 0.25);
        assert_eq!(view.tab, Tab::Frame);
        assert_eq!(view.subtab, "tone");
        assert_eq!(view.scroll, 250);
    }

    #[test]
    fn selecting_a_capture_keeps_the_rail_state() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.rail.library.filter = "kept".to_string();
        app.select_view("frame", "tone", "", "canvas", 0.0);
        assert_eq!(app.view.tab, Tab::Frame);
        assert_eq!(app.rail.frame_subtab.current_key(), "tone");
        assert_eq!(app.rail.library.filter, "kept");
    }

    #[test]
    fn frame_navigation_stops_at_the_strip_ends() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        assert!(!app.previous_frame());
        assert!(!app.next_frame());
        app.strip = vec![
            crate::frame_source::FrameImage::empty(),
            crate::frame_source::FrameImage::empty(),
        ];
        assert!(!app.next_frame());
        assert!(!app.select_frame(2));
    }

    #[test]
    fn a_capture_without_a_tab_key_reads_the_group() {
        let view = ViewState::from_capture("metadata", "", "shot", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Metadata);
        let view = ViewState::from_capture("scan", "", "", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Scan);
    }

    #[test]
    fn a_frame_capture_opens_the_page_in_its_tab_key() {
        let view = ViewState::from_capture("frame", "tone", "", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Frame);
        assert_eq!(view.subtab, "tone");
        assert_eq!(
            crate::view::rail::RailState::for_view(&view)
                .frame_subtab
                .current_key(),
            "tone"
        );
    }

    #[test]
    fn a_resolved_crop_lands_once_the_arm_still_matches() {
        use negpy_backend::services::rendering::auto_geometry::ArmedCrop;
        let mut geometry = negpy_backend::domain::models::GeometryConfig::default();
        geometry.crop_from_auto = true;
        let resolved = ArmedCrop {
            rect: Some([0.1, 0.1, 0.9, 0.9]),
            key: negpy_backend::features::geometry::logic::autocrop_detection_key(&geometry),
            skew: None,
            squared: false,
        };
        let frozen = freeze_autocrop_geometry(&geometry, &resolved).expect("a crop freezes");
        assert_eq!(frozen.crop_rect, Some(vec![0.1, 0.1, 0.9, 0.9]));
    }

    #[test]
    fn a_resolved_crop_is_dropped_after_the_arm_changed() {
        use negpy_backend::services::rendering::auto_geometry::ArmedCrop;
        let mut geometry = negpy_backend::domain::models::GeometryConfig::default();
        geometry.crop_from_auto = true;
        let resolved = ArmedCrop {
            rect: Some([0.1, 0.1, 0.9, 0.9]),
            key: "another geometry".to_string(),
            skew: None,
            squared: false,
        };
        assert!(freeze_autocrop_geometry(&geometry, &resolved).is_none());
    }

    #[test]
    fn an_auto_crop_freezes_before_its_texture_is_shown() {
        use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
        use negpy_backend::kernel::caching::logic::calculate_config_hash;

        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let mut pixels = Vec::new();
        for y in 0..12 {
            for x in 0..16 {
                let value = 0.05 + 0.8 * (y as f32 / 12.0);
                pixels.extend_from_slice(&[value, value * 0.9, value * 0.8 + x as f32 * 1e-4]);
            }
        }
        let source = crate::render_worker::SourceFrame {
            name: "frame.orf".to_string(),
            hash: "source".to_string(),
            image: ImageBuffer::from_rgb(16, 12, pixels),
            ir: None,
            ir_width: 0,
            ir_height: 0,
        };
        app.set_source(Some(source));
        app.edit.config.geometry.crop_from_auto = true;
        app.edit.config.process.lock_bounds = true;
        let ctx = egui::Context::default();
        app.request_render(&ctx, false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !app.receive_render(&ctx) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let geometry = &app.edit.config.geometry;
        assert!(geometry.crop_rect.is_some());
        assert!(!geometry.crop_detect_key.is_empty());
        let painted = app.rendered.as_deref().expect("a painted hash");
        assert!(painted.starts_with(&calculate_config_hash(&app.edit.config)));
        let generation = app.generation;
        app.request_render(&ctx, false);
        assert_eq!(app.generation, generation);
    }

    #[test]
    fn a_render_is_accepted_only_for_the_request_in_flight() {
        use crate::render_worker::RenderOutput;
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.generation = 4;
        app.requested = Some("confirmed".to_string());
        let output = |generation: u64, request_hash: &str| RenderOutput {
            generation,
            request_hash: request_hash.to_string(),
            config_hash: "painted".to_string(),
            frame: negpy_backend::services::rendering::display_frame::DisplayFrame::from_rgb(
                1,
                1,
                vec![0; 3],
            ),
            histogram: None,
            metrics: std::collections::BTreeMap::new(),
        };
        assert!(app.accepts_render(&output(4, "confirmed")));
        assert!(!app.accepts_render(&output(3, "confirmed")));
        assert!(!app.accepts_render(&output(4, "stale")));
    }

    #[test]
    fn a_heal_gesture_stores_one_stroke() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.retouch.manual_dust_size = 9;
        app.apply_retouch(crate::view::canvas::RetouchAction::Heal(vec![
            [0.4, 0.4],
            [0.5, 0.5],
        ]));
        let stroke = &app.edit.config.retouch.manual_heal_strokes[0];
        assert_eq!(stroke[0], serde_json::json!([[0.4, 0.4], [0.5, 0.5]]));
        assert_eq!(stroke[1], serde_json::json!(9.0));
        assert!(app.edit.is_dirty());
    }

    #[test]
    fn a_clone_gesture_uses_the_picked_source() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.retouch_drag.clone_source_normalized = Some([0.7, 0.6]);
        app.apply_retouch(crate::view::canvas::RetouchAction::Clone(vec![
            [0.4, 0.4],
            [0.5, 0.5],
        ]));
        let stroke = &app.edit.config.retouch.clone_strokes[0];
        assert!((stroke[2].as_f64().unwrap() - 0.3).abs() < 1e-9);
        assert!((stroke[3].as_f64().unwrap() - 0.2).abs() < 1e-9);
        assert!(app.edit.is_dirty());
    }

    #[test]
    fn a_confirmed_crop_writes_once_and_leaves_draw() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.geometry.crop_from_auto = true;
        app.edit.config.geometry.crop_detect_key = "stale".to_string();
        app.rail.geometry.crop.draw = true;
        app.confirm_crop([0.1, 0.2, 0.8, 0.9]);
        assert_eq!(
            app.edit.config.geometry.crop_rect.as_deref(),
            Some(&[0.1, 0.2, 0.8, 0.9][..])
        );
        assert!(!app.edit.config.geometry.crop_from_auto);
        assert!(app.edit.config.geometry.crop_detect_key.is_empty());
        assert!(!app.rail.geometry.crop.draw);
        assert!(app.rail.geometry.crop.has_crop);
        assert!(app.edit.is_dirty());
    }

    #[test]
    fn a_measured_bounds_pair_reaches_the_frame_process() {
        use negpy_backend::features::exposure::normalization::LogNegativeBounds;
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.process.local_floors = [0.0; 3];
        app.edit.config.process.local_ceils = [0.0; 3];
        let bounds = LogNegativeBounds::new([0.1, 0.2, 0.3], [0.7, 0.8, 0.9]);
        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "final_bounds".to_string(),
            serde_json::to_value(bounds).unwrap(),
        );
        app.store_measured_bounds(&metrics);
        assert_eq!(app.edit.config.process.local_floors, [0.1, 0.2, 0.3]);
        assert!(app.edit.is_dirty());
    }

    #[test]
    fn a_frame_locked_to_its_own_bounds_keeps_them() {
        use negpy_backend::features::exposure::normalization::LogNegativeBounds;
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.process.lock_bounds = true;
        app.edit.config.process.local_floors = [0.4; 3];
        let bounds = LogNegativeBounds::new([0.1, 0.2, 0.3], [0.7, 0.8, 0.9]);
        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "final_bounds".to_string(),
            serde_json::to_value(bounds).unwrap(),
        );
        app.store_measured_bounds(&metrics);
        assert_eq!(app.edit.config.process.local_floors, [0.4; 3]);
    }

    #[test]
    fn using_a_frame_saves_its_bounds_on_the_roll() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path();
        let photo = profile.join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            negpy_backend::domain::types::AppConfig::for_user_dir(profile).edits_db_path,
            negpy_backend::domain::types::AppConfig::for_user_dir(profile).settings_db_path,
        );
        repo.initialize().unwrap();
        let roll_id = negpy_backend::services::assets::rolls::create_virtual_roll(
            &mut repo,
            "Roll",
            &[photo.display().to_string()],
        );
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.profile = Some(profile.to_path_buf());
        app.set_edit(FrameEdit::load(Some(profile), &photo));
        app.edit.config.process.local_floors = [0.1, 0.2, 0.3];
        app.edit.config.process.local_ceils = [0.7, 0.8, 0.9];
        app.rail.roll_analysis.roll_id = Some(roll_id.clone());
        let ctx = egui::Context::default();
        app.use_frame_as_baseline(&ctx);
        let saved = negpy_backend::services::assets::rolls::roll_normalization(&repo, &roll_id)
            .expect("a saved baseline");
        assert_eq!(saved["floors"], serde_json::json!([0.1, 0.2, 0.3]));
        assert_eq!(
            app.edit.config.process.baseline_source,
            format!("roll:{roll_id}")
        );
        assert_eq!(app.edit.config.process.locked_floors, [0.1, 0.2, 0.3]);
    }
}
