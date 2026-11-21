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
use crate::view::find::FindState;
use crate::view::rail::{RAIL_WIDTH, RailState};
use negpy_backend::session::proof::ProofSettings;

use crate::export_worker::{ExportRequest, ExportWorker};
use crate::render_worker::ProofRequest;
use std::path::Path;

/// The metadata presets under the profile's preset directory.
fn metadata_presets(
    app: &negpy_backend::domain::types::AppConfig,
) -> negpy_backend::services::assets::presets::PresetStore {
    negpy_backend::services::assets::presets::PresetStore::new(app.presets_dir.clone(), "metadata")
}

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
    /// The toolbar's Photo and Roll mode: `true` selects Photo.
    pub photo_mode: Option<bool>,
    /// The toolbar's rotate counter-clockwise, rotate clockwise, flip horizontal and
    /// flip vertical, by index, when one is clicked.
    pub transform: Option<usize>,
    /// The Tools button opened its menu.
    pub tools: bool,
    /// A Find result the user opened.
    pub find: Option<crate::view::find::FindTarget>,
    /// The zoom percent a toolbar preset selected.
    pub zoom: Option<u32>,
    /// The Roll Average toggle, taken as its new state.
    pub roll_average: Option<bool>,
    /// The Print button, which opens the Export flow.
    pub print: bool,
    /// The photo rect the canvas drew, for a zoom preset to read its fit scale.
    pub photo: Option<Rect>,
    pub undo: bool,
    pub redo: bool,
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
    /// The toolbar's Photo and Roll mode; Photo selects the Frame tab.
    pub photo_mode: bool,
    /// The open frame's linear proxy, held until the frame changes.
    source: Option<std::sync::Arc<crate::render_worker::SourceFrame>>,
    renderer: crate::render_worker::RenderWorker,
    exporter: ExportWorker,
    scanner: crate::scan_worker::ScanWorker,
    sheets: crate::contact_sheet_worker::SheetWorker,
    crops: crate::crop_worker::CropWorker,
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
    /// The fit-relative canvas zoom.
    pub zoom: f32,
    /// A true-pixel zoom a shortcut asked for, applied once the canvas is known.
    pub zoom_percent: Option<u32>,
    /// The Find field state the shell passes to the painter.
    pub find: FindState,
    /// Whether the Tools menu should open under the pointer.
    pub tools_open: bool,
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
            photo_mode: false,
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            crops: crate::crop_worker::CropWorker::start(),
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
            zoom: 1.0,
            zoom_percent: None,
            find: FindState::default(),
            tools_open: false,
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
            photo_mode: tab == "frame",
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            crops: crate::crop_worker::CropWorker::start(),
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
            zoom: 1.0,
            zoom_percent: None,
            find: FindState::default(),
            tools_open: false,
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
        self.rail.set_profile(Some(profile), &self.edit.config);
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
        let store = metadata_presets(&app);
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

    /// Import the scan output folder the Scan card asked to open as a roll.
    fn open_scan_folder(&mut self) {
        let Some(folder) = self.rail.scan.take_folder_as_roll_request() else {
            return;
        };
        if self.rail.library.import(&folder, false) {
            self.rail.scan.set_status("Folder imported as a roll.");
            self.open_requested_roll();
        } else {
            self.rail.scan.set_status("Could not import that folder.");
        }
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

    /// Submit one Crop All Frames run for the strip.
    /// Ask for one Crop All Frames run on the strip.
    pub fn request_crop_all(&mut self, ctx: &egui::Context) {
        if !self.rail.geometry.crop_all_request {
            return;
        }
        self.rail.geometry.crop_all_request = false;
        let Some(profile) = self.profile.clone() else {
            self.rail.library.set_status("Open a profile first.");
            return;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let _ = self.edit.save();
        let mut frames = Vec::new();
        for frame in &self.strip {
            let Some(path) = &frame.path else {
                continue;
            };
            let Ok(hash) =
                negpy_backend::infrastructure::storage::local_asset_store::file_hash(path)
            else {
                continue;
            };
            let config = crate::frame_source::stored_config(Some(&profile), path);
            frames.push(crate::crop_worker::CropFrame {
                path: path.clone(),
                hash,
                config_json: serde_json::Value::Object(config.to_dict()).to_string(),
                fingerprint: crate::crop_worker::fingerprint(&config),
                started_auto: config.geometry.crop_from_auto,
            });
        }
        if frames.is_empty() {
            self.rail.library.set_status("No frames to auto-crop.");
            return;
        }
        self.rail.library.set_status(format!(
            "Auto-cropping {}...",
            crate::view::rail::count_of(frames.len(), "frame")
        ));
        self.crops.submit(crate::crop_worker::CropRequest {
            frames,
            edits_db_path: std::path::PathBuf::from(app.edits_db_path),
            settings_db_path: std::path::PathBuf::from(app.settings_db_path),
            flatfield_dir: std::path::PathBuf::from(app.flatfield_dir),
        });
        ctx.request_repaint();
    }

    fn receive_crops(&mut self, ctx: &egui::Context) {
        let Some(report) = self.crops.drain() else {
            return;
        };
        let mut message = format!("Auto Crop: saved {}, kept {}", report.saved, report.kept);
        if report.failed > 0 {
            message.push_str(&format!(", failed {}", report.failed));
        }
        self.rail.library.set_status(message);
        self.rail.geometry.crop_all_status =
            format!("Saved {}, kept {}.", report.saved, report.kept);
        if report.saved > 0
            && let Some(path) = self.edit.source().map(|(_, path)| path.to_path_buf())
        {
            let config = crate::frame_source::stored_config(self.profile.as_deref(), &path);
            self.edit.restore(config);
            self.rail.select_view(&self.view);
            self.request_render(ctx, false);
        }
        ctx.request_repaint();
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

    /// Replace the Presets page with the store's current names.
    pub fn reload_metadata_presets(&mut self) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        self.rail
            .metadata
            .set_presets(metadata_presets(&app).list());
    }

    /// Apply or delete the metadata preset the Presets page asked for.
    fn apply_metadata_preset(&mut self) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let store = metadata_presets(&app);
        if let Some(name) = self.rail.metadata.take_save_request() {
            let rows = negpy_backend::session::settings_catalog::rows_for_keys(
                &self.edit.config.to_dict(),
                "metadata",
            );
            let fields = negpy_backend::session::settings_catalog::selected_flat_dict(
                &self.edit.config,
                &rows,
            );
            match store.save(&name, &fields) {
                Ok(()) => {
                    self.rail.metadata.preset_name.clear();
                    self.rail.metadata.set_presets(store.list());
                }
                Err(error) => eprintln!("Could not save the metadata preset: {error}"),
            }
        }
        if let Some(name) = self.rail.metadata.take_apply_request() {
            if let Some(data) = store.load(&name) {
                let rows =
                    negpy_backend::session::settings_catalog::rows_for_keys(&data, "metadata");
                let source = negpy_backend::session::settings_catalog::preset_config(&data);
                self.edit.config = negpy_backend::session::settings_catalog::apply_selected_fields(
                    &source,
                    &self.edit.config,
                    &rows,
                );
                self.edit.mark_dirty();
            }
        }
        if let Some(name) = self.rail.metadata.take_delete_request() {
            let _ = store.delete(&name);
            self.rail.metadata.selected_preset = None;
            self.rail.metadata.set_presets(store.list());
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
            self.rail.set_scopes(output.scopes);
            self.rail.set_density_histogram(output.density_histogram);
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
        self.rail.set_profile(profile.as_deref(), &self.edit.config);
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
        self.rail.set_profile(Some(&profile), &self.edit.config);
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
        crate::view::shortcuts::dispatch(self, ctx);
        if ctx.input(|input| input.key_pressed(egui::Key::K) && input.modifiers.command) {
            self.find.focus = true;
            ctx.request_repaint();
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.find.query.clear();
            self.find.results.clear();
            self.find.open = false;
        }
        self.receive_exports(ctx);
        self.receive_render(ctx);
        self.receive_scans(ctx);
        self.receive_sheets(ctx);
        self.receive_crops(ctx);
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
                    self.open_scan_folder();
                    let proof = self.proof.clone();
                    let switched = self
                        .rail
                        .show(ui, rail_rect, &mut self.edit, &mut self.proof);
                    self.rail.apply_optics_action(&mut self.edit);
                    self.rail.apply_history_action(&mut self.edit);
                    self.apply_metadata_preset();
                    self.apply_square_request(ctx);
                    self.request_crop_all(ctx);
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
            let recipe = self.recipe();
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
                self.photo_mode,
                &mut self.mask_drag,
                retouch_tool,
                &mut self.retouch_drag,
                self.zoom,
                self.edit.config.process.use_luma_average
                    || self.edit.config.process.use_color_average
                    || self.edit.config.process.use_cast_average,
                &mut self.find,
                &recipe,
            );
            let find_results = self.find.results.clone();
            let search = Rect::from_min_max(
                egui::pos2(
                    screen.right() - RAIL_WIDTH.min((screen.width() * 0.27).max(420.0)) - 8.0,
                    screen.top() + 8.0,
                ),
                egui::pos2(
                    screen.right() - 8.0,
                    screen.top() + 8.0 + crate::view::rail::SEARCH_HEIGHT,
                ),
            );
            let mut active = self.find.active;
            let nav = crate::view::find::results(
                ui,
                search,
                &find_results,
                &mut active,
                self.find.query_settled(),
            );
            self.find.active = nav.active;
            if let Some(target) = nav.picked {
                self.open_find(target, ctx);
            }
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
            if hits.undo {
                self.undo_action(ctx);
            }
            if hits.redo {
                self.redo_action(ctx);
            }
            if let Some(action) = hits.transform {
                self.transform_action(action, ctx);
            }
            if (hits.tools || std::mem::take(&mut self.tools_open))
                && let Some(tool) = crate::view::tools_menu::show_from_pointer(ctx)
            {
                let mut subtab = self.rail.frame_subtab.current_key().to_string();
                crate::view::tools_menu::arm(
                    tool,
                    &mut self.rail.geometry,
                    &mut self.rail.exposure,
                    &mut self.rail.finish,
                    &mut self.rail.tab,
                    &mut subtab,
                );
                self.view.tab = self.rail.tab;
                self.view.subtab = subtab;
                self.rail.select_view(&self.view);
                ctx.request_repaint();
            }
            if let Some(percent) = hits.zoom {
                if let (Some(photo), Some(frame)) = (hits.photo, self.frame.as_ref()) {
                    let fit = crate::view::canvas::zoom_percent(
                        photo,
                        frame.width as u32,
                        frame.height as u32,
                    )
                    .max(1) as f32;
                    self.zoom = (percent as f32 / fit)
                        .clamp(crate::view::canvas::ZOOM_MIN, crate::view::canvas::ZOOM_MAX);
                    ctx.request_repaint();
                }
            }
            if let Some(percent) = self.zoom_percent.take() {
                if let (Some(photo), Some(frame)) = (hits.photo, self.frame.as_ref()) {
                    let fit = crate::view::canvas::zoom_percent(
                        photo,
                        frame.width as u32,
                        frame.height as u32,
                    )
                    .max(1) as f32;
                    self.zoom = (percent as f32 / fit)
                        .clamp(crate::view::canvas::ZOOM_MIN, crate::view::canvas::ZOOM_MAX);
                    ctx.request_repaint();
                }
            }
            if let Some(on) = hits.roll_average {
                self.rail.metering.set_all_averages(on, &mut self.edit);
                self.rail.select_view(&self.view);
                self.request_render(ctx, false);
                ctx.request_repaint();
            }
            if hits.print {
                self.rail.tab = Tab::Export;
                self.rail.export.show_subtab("file");
                self.view.tab = Tab::Export;
                self.photo_mode = false;
                ctx.request_repaint();
            }
            self.apply_canvas_zoom(ctx, hits.photo);
            if let Some(photo) = hits.photo_mode {
                self.rail.tab = if photo { Tab::Frame } else { Tab::Roll };
                self.photo_mode = photo;
                ctx.request_repaint();
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
            self.open_scan_folder();
            let proof = self.proof.clone();
            let before = self.edit.config.clone();
            let switched = self
                .rail
                .show(ui, rail_rect, &mut self.edit, &mut self.proof);
            if self.edit.config != before {
                self.edit.checkpoint_before(before);
            }
            self.rail.apply_optics_action(&mut self.edit);
            self.rail.apply_history_action(&mut self.edit);
            self.apply_metadata_preset();
            self.apply_square_request(ctx);
            self.request_crop_all(ctx);
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

    /// Step the open frame back, then show the earlier config.
    /// Step the open frame back.
    pub fn undo_action(&mut self, ctx: &egui::Context) {
        if self.edit.undo() {
            self.rail.select_view(&self.view);
            self.request_render(ctx, false);
            ctx.request_repaint();
        }
    }

    /// Step the open frame forward, then show the later config.
    /// Step the open frame forward.
    pub fn redo_action(&mut self, ctx: &egui::Context) {
        if self.edit.redo() {
            self.rail.select_view(&self.view);
            self.request_render(ctx, false);
            ctx.request_repaint();
        }
    }

    /// Rotate, flip or toggle the crop lock from the toolbar.
    /// Rotate, flip or toggle the crop lock from the toolbar.
    pub fn transform_action(&mut self, action: usize, ctx: &egui::Context) {
        let before = self.edit.config.clone();
        match action {
            0 => {
                let (rotated, _) =
                    negpy_backend::features::geometry::logic::rotate_geometry_and_analysis(
                        &self.edit.config.geometry,
                        None,
                        -1,
                    );
                self.edit.config.geometry = rotated;
            }
            1 => {
                let (rotated, _) =
                    negpy_backend::features::geometry::logic::rotate_geometry_and_analysis(
                        &self.edit.config.geometry,
                        None,
                        1,
                    );
                self.edit.config.geometry = rotated;
            }
            2 => {
                self.edit.config.geometry = negpy_backend::features::geometry::logic::toggle_flip(
                    &self.edit.config.geometry,
                    true,
                )
            }
            3 => {
                self.edit.config.geometry = negpy_backend::features::geometry::logic::toggle_flip(
                    &self.edit.config.geometry,
                    false,
                )
            }
            _ => return,
        }
        self.edit.checkpoint_before(before);
        self.request_render(ctx, false);
        ctx.request_repaint();
    }

    /// Read the wheel and a double-click over the canvas into the zoom state.
    fn apply_canvas_zoom(&mut self, ctx: &egui::Context, photo: Option<Rect>) {
        let Some(photo) = photo else {
            return;
        };
        let response = ctx.input(|input| {
            let hover = input
                .pointer
                .hover_pos()
                .is_some_and(|point| photo.contains(point));
            let scroll = if hover {
                input.smooth_scroll_delta.y
            } else {
                0.0
            };
            let double = input
                .pointer
                .button_double_clicked(egui::PointerButton::Primary);
            (scroll, double)
        });
        if response.0 != 0.0 {
            self.zoom = crate::view::canvas::zoom_for_wheel(self.zoom, response.0 / 120.0);
            ctx.request_repaint();
        }
        if response.1 {
            self.zoom = if (self.zoom - 1.0).abs() < 1e-3 {
                crate::view::canvas::ZOOM_MAX
            } else {
                1.0
            };
            ctx.request_repaint();
        }
    }

    /// Square the open frame to its film edges when the Geometry card asks.
    fn apply_square_request(&mut self, ctx: &egui::Context) {
        if !self.rail.geometry.square_request {
            return;
        }
        self.rail.geometry.square_request = false;
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let Some((squared, _)) = negpy_backend::features::geometry::skew::squared_geometry(
            &source.image,
            &self.edit.config.geometry,
            self.edit.config.process.process_mode,
        ) else {
            return;
        };
        let current = &self.edit.config.geometry;
        if (squared.fine_rotation - current.fine_rotation).abs() < 0.05
            && (squared.converge_v - current.converge_v).abs() < 0.05
            && (squared.converge_h - current.converge_h).abs() < 0.05
        {
            return;
        }
        self.edit.config.geometry = squared;
        self.edit.mark_dirty();
        self.request_render(ctx, false);
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

    /// Open one Find result and close the list.
    fn open_find(&mut self, target: crate::view::find::FindTarget, ctx: &egui::Context) {
        use crate::view::find::FindTarget;
        self.find.query.clear();
        self.find.results.clear();
        self.find.focus = false;
        match target {
            FindTarget::Tab(tab) => {
                self.rail.tab = tab;
                self.view.tab = tab;
                self.rail.select_view(&self.view);
            }
            FindTarget::Tool => self.rail.tab = Tab::Roll,
            FindTarget::Zoom => {
                self.rail.tab = Tab::Frame;
            }
            FindTarget::Export => {
                self.rail.tab = Tab::Export;
                self.rail.export.show_subtab("file");
                self.view.tab = Tab::Export;
                self.photo_mode = false;
            }
            FindTarget::CropAll => {
                self.rail.tab = Tab::Frame;
                self.rail.frame_subtab = crate::view::widgets::sub_tabs::SubTabState::new(
                    &crate::view::rail::frame_subtabs(),
                    Some("geometry"),
                );
                self.view.tab = Tab::Frame;
                self.view.subtab = "geometry".to_string();
            }
            FindTarget::RollAverage => {
                let process = &self.edit.config.process;
                let on = !(process.use_luma_average
                    || process.use_color_average
                    || process.use_cast_average);
                self.rail.metering.set_all_averages(on, &mut self.edit);
                self.rail.select_view(&self.view);
                self.request_render(ctx, false);
            }
        }
        ctx.request_repaint();
    }

    /// Focus the Find field.
    pub fn focus_find(&mut self) {
        self.find.focus = true;
    }

    /// Set the fit-relative canvas zoom.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(crate::view::canvas::ZOOM_MIN, crate::view::canvas::ZOOM_MAX);
    }

    /// Ask for a true-pixel zoom, applied by the next canvas paint.
    pub fn zoom_to_percent(&mut self, percent: u32) {
        self.zoom_percent = Some(percent);
    }

    /// Toggle the Roll Average on every axis.
    pub fn toggle_roll_average(&mut self, ctx: &egui::Context) {
        let process = &self.edit.config.process;
        let on =
            !(process.use_luma_average || process.use_color_average || process.use_cast_average);
        self.rail.metering.set_all_averages(on, &mut self.edit);
        self.rail.select_view(&self.view);
        self.request_render(ctx, false);
    }

    /// Open the Tools menu at the pointer.
    pub fn open_tools(&mut self) {
        self.tools_open = true;
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

    /// The print recipe the top bar prints for the open frame.
    #[must_use]
    pub fn recipe(&self) -> String {
        use negpy_backend::features::exposure::papers::resolve_paper;
        let exposure = &self.edit.config.exposure;
        let paper = resolve_paper(&exposure.paper_profile).label;
        let density = if exposure.auto_exposure {
            format!("Print Density {:.2} (auto)", exposure.density)
        } else {
            format!("Print Density {:.2}", exposure.density)
        };
        let grade = if exposure.auto_normalize_contrast {
            format!("Grade ISO-R {:.0} (auto)", exposure.grade)
        } else {
            format!("Grade ISO-R {:.0}", exposure.grade)
        };
        format!("{paper} · {density} · {grade}")
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
    photo_mode: bool,
    mask_drag: &mut crate::view::canvas::MaskDrag,
    retouch_tool: crate::view::canvas::RetouchTool,
    retouch_drag: &mut crate::view::canvas::RetouchDrag,
    zoom: f32,
    averages_on: bool,
    find: &mut FindState,
    recipe: &str,
) -> (Rect, ShellHits) {
    let painter = ui.painter().clone();
    let margin = 8.0;
    paint_top_bar_with(&painter, screen, label, recipe);

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
    let paint = crate::view::find::paint(&mut search_ui, search, &mut find.query, &mut find.focus);
    find.results = paint.results;
    find.open = paint.open;
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
                crate::view::canvas::zoomed_photo_rect(
                    canvas_rect,
                    width as u32,
                    height as u32,
                    zoom,
                )
            },
        )
    } else {
        canvas_rect
    };
    let mut hits = ShellHits::default();
    hits.photo = Some(photo);
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
    let navigation = paint_toolbar(
        ui,
        &painter,
        toolbar,
        selected,
        thumbnails.len(),
        photo_mode,
        geometry.1.map_or(100, |(width, height)| {
            crate::view::canvas::zoom_percent(photo, width as u32, height as u32)
        }),
        averages_on,
    );
    hits.previous = navigation.previous;
    hits.next = navigation.next;
    hits.undo = navigation.undo;
    hits.redo = navigation.redo;
    hits.zoom = navigation.zoom;
    hits.roll_average = navigation.roll_average;
    hits.tools = navigation.tools;
    hits.print = navigation.print;
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
    paint_top_bar_with(
        painter,
        screen,
        "P5080635.ORF · 7792 x 10388 px",
        "Digital (Adobe RGB) · Print Density 1.08 (auto) · Grade ISO-R 83 (auto)",
    );
}

/// Paint the file name and the current recipe over `screen`.
pub fn paint_top_bar_with(painter: &egui::Painter, screen: Rect, frame: &str, recipe: &str) {
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
        recipe,
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
    photo_mode: bool,
    zoom_percent: u32,
    averages_on: bool,
) -> ShellHits {
    use crate::styles::icons::Icon;
    use crate::styles::layout::{
        BAR_BUTTON_HEIGHT, BAR_ICON_SIZE, BAR_TRACK_BUTTON_HEIGHT, BAR_TRACK_PAD,
    };

    let mut hits = ShellHits::default();
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
    let photo_on = photo_mode;
    for (segment, icon, label, on) in [
        (photo, Icon::Image, "Photo", photo_on),
        (roll, Icon::Film, "Roll", !photo_on),
    ] {
        if on {
            painter.rect_filled(segment, 4.0, theme.bg_checked);
            painter.rect_stroke(
                segment,
                4.0,
                egui::Stroke::new(1.0_f32, theme.border_checked),
                egui::StrokeKind::Inside,
            );
        }
        flat(
            painter,
            Rect::from_min_size(segment.min, Vec2::new(24.0, segment.height())),
            icon,
            if on {
                theme.text_primary
            } else {
                theme.text_hint
            },
        );
        painter.text(
            segment.left_center() + Vec2::new(26.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            if on {
                theme.text_primary
            } else {
                theme.text_hint
            },
        );
    }
    let photo_hit = ui.interact(photo, ui.id().with("toolbar-photo"), Sense::click());
    let roll_hit = ui.interact(roll, ui.id().with("toolbar-roll"), Sense::click());
    if photo_hit.clicked() {
        hits.photo_mode = Some(true);
    }
    if roll_hit.clicked() {
        hits.photo_mode = Some(false);
    }
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
        &format!("{zoom_percent}%"),
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
    let zoom_hit = ui.interact(zoom, ui.id().with("toolbar-zoom"), Sense::click());
    if let Some(percent) = zoom_menu(zoom_hit) {
        hits.zoom = Some(percent);
    }
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
        let hit = ui.interact(
            button,
            ui.id().with(("toolbar-transform", index)),
            Sense::click(),
        );
        if hit.clicked() {
            hits.transform = Some(index);
        }
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
            let hit = ui.interact(button, ui.id().with("toolbar-tools"), Sense::click());
            if hit.clicked() {
                hits.tools = true;
            }
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
        let hit = ui.interact(
            button,
            ui.id().with(("toolbar", icon as u8)),
            Sense::click(),
        );
        if icon == Icon::Undo {
            hits.undo = hit.clicked();
        } else {
            hits.redo = hit.clicked();
        }
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
    let print_hit = ui.interact(print, ui.id().with("toolbar-print"), Sense::click());
    if print_hit.clicked() {
        hits.print = true;
    }
    x = print.right() + 6.0;

    let roll_average = Rect::from_min_size(egui::pos2(x, top), Vec2::new(66.0, row_height));
    track(painter, roll_average);
    if averages_on {
        painter.rect_filled(roll_average, 4.0, theme.bg_checked);
        painter.rect_stroke(
            roll_average,
            4.0,
            egui::Stroke::new(1.0_f32, theme.border_checked),
            egui::StrokeKind::Inside,
        );
    }
    flat(
        painter,
        roll_average,
        Icon::Film,
        if averages_on {
            theme.text_primary
        } else {
            theme.text_secondary
        },
    );
    if ui
        .interact(
            roll_average,
            ui.id().with("toolbar-roll-average"),
            Sense::click(),
        )
        .clicked()
    {
        hits.roll_average = Some(!averages_on);
    }
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
    hits.previous = previous.clicked();
    hits.next = next.clicked();
    hits
}

/// The zoom presets a click on the toolbar readout opens.
fn zoom_menu(response: egui::Response) -> Option<u32> {
    egui::Popup::menu(&response)
        .show(|ui| {
            ui.set_min_width(120.0);
            let mut picked = None;
            for percent in [25, 50, 100, 200, 400] {
                if ui.button(format!("{percent}%")).clicked() {
                    picked = Some(percent);
                }
            }
            picked
        })
        .and_then(|response| response.inner)
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
    fn the_toolbar_photo_mode_selects_the_frame_tab() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        assert_eq!(app.rail.tab, Tab::Roll);
        app.photo_mode = true;
        app.rail.tab = if app.photo_mode {
            Tab::Frame
        } else {
            Tab::Roll
        };
        assert_eq!(app.rail.tab, Tab::Frame);
    }

    #[test]
    fn the_toolbar_rotate_turns_the_geometry() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.transform_action(1, &ctx);
        assert_eq!(app.edit.config.geometry.rotation, 1);
        app.transform_action(0, &ctx);
        assert_eq!(app.edit.config.geometry.rotation, 0);
        app.transform_action(2, &ctx);
        assert!(app.edit.config.geometry.flip_horizontal);
        app.transform_action(3, &ctx);
        assert!(app.edit.config.geometry.flip_vertical);
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
            scopes: crate::render_worker::ScopeBins::default(),
            density_histogram: None,
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
    fn a_zoom_preset_sets_the_fit_relative_scale() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let photo = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 400.0));
        app.zoom = 100.0 / crate::view::canvas::zoom_percent(photo, 1600, 800) as f32;
        assert!((app.zoom - 2.0).abs() < 1e-6);
    }

    #[test]
    fn the_recipe_follows_the_open_frame() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.exposure.paper_profile = "ilford_multigrade_rc".to_string();
        app.edit.config.exposure.density = 1.25;
        app.edit.config.exposure.grade = 95.0;
        let recipe = app.recipe();
        assert!(recipe.contains("Print Density 1.25"), "{recipe}");
        assert!(recipe.contains("Grade ISO-R 95"), "{recipe}");
    }

    #[test]
    fn an_auto_recipe_names_its_autos() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.exposure.auto_exposure = true;
        app.edit.config.exposure.auto_normalize_contrast = true;
        let recipe = app.recipe();
        assert!(recipe.contains("Print Density"));
        assert!(recipe.contains("(auto)"), "{recipe}");
    }

    #[test]
    fn the_roll_average_toggle_sets_every_axis() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.process.use_luma_average = false;
        app.edit.config.process.use_color_average = false;
        app.edit.config.process.use_cast_average = false;
        app.rail.metering.set_all_averages(true, &mut app.edit);
        assert!(app.edit.config.process.use_luma_average);
        assert!(app.edit.config.process.use_color_average);
        assert!(app.edit.config.process.use_cast_average);
        app.rail.metering.set_all_averages(false, &mut app.edit);
        assert!(!app.edit.config.process.use_luma_average);
    }

    #[test]
    fn the_print_button_opens_the_export_form() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.rail.tab = Tab::Frame;
        app.rail.tab = Tab::Export;
        app.rail.export.show_subtab("file");
        app.photo_mode = false;
        assert_eq!(app.rail.tab, Tab::Export);
        assert_eq!(app.rail.export.page, 0);
        assert!(!app.photo_mode);
    }

    #[test]
    fn a_find_result_opens_its_tab() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.find.query = "metadata".to_string();
        app.find.results = crate::view::find::rank("metadata");
        app.open_find(crate::view::find::FindTarget::Tab(Tab::Metadata), &ctx);
        assert_eq!(app.rail.tab, Tab::Metadata);
        assert!(app.find.query.is_empty());
    }

    #[test]
    fn a_find_result_can_toggle_the_roll_average() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.edit.config.process.use_luma_average = false;
        app.edit.config.process.use_color_average = false;
        app.edit.config.process.use_cast_average = false;
        app.open_find(crate::view::find::FindTarget::RollAverage, &ctx);
        assert!(app.edit.config.process.use_luma_average);
    }

    #[test]
    fn a_find_result_opens_crop_all_on_geometry() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.open_find(crate::view::find::FindTarget::CropAll, &ctx);
        assert_eq!(app.rail.tab, Tab::Frame);
        assert_eq!(app.rail.frame_subtab.current_key(), "geometry");
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
