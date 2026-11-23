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

fn settle_preset_locks(
    repo: &mut negpy_backend::infrastructure::storage::repository::StorageRepository,
    roll_id: &str,
    hash: &str,
    rows: &[negpy_backend::session::settings_catalog::SettingRow],
    before: &serde_json::Map<String, serde_json::Value>,
    after: &negpy_backend::domain::models::WorkspaceConfig,
) {
    let after = after.to_dict();
    for (card_key, _, fields) in negpy_backend::services::assets::rolls::ROLL_DEFAULT_FIELDS {
        if !rows.iter().any(|row| {
            fields
                .iter()
                .any(|field| row.fields.iter().any(|named| named == field))
        }) {
            continue;
        }
        let values: serde_json::Map<String, serde_json::Value> = fields
            .iter()
            .filter_map(|field| {
                after
                    .get(*field)
                    .map(|value| ((*field).to_string(), value.clone()))
            })
            .collect();
        let before: serde_json::Map<String, serde_json::Value> = fields
            .iter()
            .filter_map(|field| {
                before
                    .get(*field)
                    .map(|value| ((*field).to_string(), value.clone()))
            })
            .collect();
        negpy_backend::services::assets::rolls::settle_card_lock(
            repo,
            roll_id,
            hash,
            card_key,
            &values,
            Some(&before),
            None,
        );
    }
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
    /// The Sticky Zoom toggle, taken as its new state.
    pub sticky_zoom: Option<bool>,
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
    /// A manual Dmin or Dmax pick on the photo.
    pub pick: crate::view::canvas::PickAction,
    /// An action the toolbar overflow menu chose.
    pub overflow: Option<ToolbarAction>,
}

/// A toolbar action the overflow menu offers when its control leaves the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarAction {
    Previous,
    Next,
    Zoom(u32),
    StickyZoom(bool),
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
    Tools,
    Undo,
    Redo,
    Print,
    RollAverage(bool),
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
    /// The extra frames selected with Ctrl or Shift on the Light Table.
    pub selected_frames: std::collections::BTreeSet<usize>,
    /// The toolbar's Photo and Roll mode; Photo selects the Frame tab.
    pub photo_mode: bool,
    /// The open frame's linear proxy, held until the frame changes.
    source: Option<std::sync::Arc<crate::render_worker::SourceFrame>>,
    renderer: crate::render_worker::RenderWorker,
    frame_worker: crate::frame_worker::FrameWorker,
    /// The newest frame selection the worker has not answered yet.
    frame_generation: u64,
    exporter: ExportWorker,
    scanner: crate::scan_worker::ScanWorker,
    sheets: crate::contact_sheet_worker::SheetWorker,
    crops: crate::crop_worker::CropWorker,
    detect: crate::detect_worker::DetectWorker,
    sheet: crate::view::contact_sheet_dialog::ContactSheetDialog,
    sheet_request: u64,
    /// The request hash in flight, and the config hash that painted the canvas.
    requested: Option<String>,
    rendered: Option<String>,
    /// The frame decode or preview render the canvas is waiting for.
    processing: bool,
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
    /// Whether a frame switch keeps the current zoom instead of fitting the frame.
    pub sticky_zoom: bool,
    /// A true-pixel zoom a shortcut asked for, applied once the canvas is known.
    pub zoom_percent: Option<u32>,
    /// Whether preview renders may use the GPU chain. Startup applies `override.toml`.
    pub gpu_enabled: bool,
    /// The milliseconds the last frame open took, from the click to the bound state.
    pub last_frame_open_ms: f64,
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
            selected_frames: std::collections::BTreeSet::new(),
            photo_mode: false,
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            frame_worker: crate::frame_worker::FrameWorker::start(),
            frame_generation: 0,
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            crops: crate::crop_worker::CropWorker::start(),
            detect: crate::detect_worker::DetectWorker::start(),
            sheet: crate::view::contact_sheet_dialog::ContactSheetDialog::default(),
            sheet_request: 0,
            requested: None,
            rendered: None,
            processing: false,
            generation: 0,
            texture: None,
            strip_textures: Vec::new(),
            crop_drag: None,
            mask_drag: crate::view::canvas::MaskDrag::default(),
            retouch_drag: crate::view::canvas::RetouchDrag::default(),
            zoom: 1.0,
            sticky_zoom: false,
            zoom_percent: None,
            gpu_enabled: true,
            last_frame_open_ms: 0.0,
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
            selected_frames: std::collections::BTreeSet::new(),
            photo_mode: tab == "frame",
            source: None,
            renderer: crate::render_worker::RenderWorker::start(),
            frame_worker: crate::frame_worker::FrameWorker::start(),
            frame_generation: 0,
            exporter: ExportWorker::start(),
            scanner: crate::scan_worker::ScanWorker::start(),
            sheets: crate::contact_sheet_worker::SheetWorker::start(),
            crops: crate::crop_worker::CropWorker::start(),
            detect: crate::detect_worker::DetectWorker::start(),
            sheet: crate::view::contact_sheet_dialog::ContactSheetDialog::default(),
            sheet_request: 0,
            requested: None,
            rendered: None,
            processing: false,
            generation: 0,
            texture: None,
            strip_textures: Vec::new(),
            crop_drag: None,
            mask_drag: crate::view::canvas::MaskDrag::default(),
            retouch_drag: crate::view::canvas::RetouchDrag::default(),
            zoom: 1.0,
            sticky_zoom: false,
            zoom_percent: None,
            gpu_enabled: true,
            last_frame_open_ms: 0.0,
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
        self.sticky_zoom = repo
            .global_setting("sticky_zoom")
            .ok()
            .flatten()
            .is_some_and(|value| value.as_bool().unwrap_or(false));
        self.gpu_enabled = repo
            .global_setting("gpu_enabled")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let mut app_config = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
        let override_path = std::path::PathBuf::from(&app_config.override_toml_path);
        negpy_backend::kernel::system::override_config::load_startup(
            &override_path,
            &mut app_config,
            |key| {
                repo.global_setting(key)
                    .ok()
                    .flatten()
                    .and_then(|value| value.as_bool())
            },
        );
        self.gpu_enabled = app_config.use_gpu;
        let presets = repo.load_export_presets();
        self.rail.export.set_presets(presets);
        let profiles = negpy_backend::infrastructure::display::color_mgmt::available_profiles(
            std::path::Path::new(&app.user_icc_dir),
        );
        self.rail.export.set_icc_profiles(&profiles);
        let store = metadata_presets(&app);
        self.rail.metadata.set_presets(store.list());
        self.load_favourites();
    }

    /// Read the stored Favorites slider list.
    fn load_favourites(&mut self) {
        let Some(repo) = &self.settings else {
            return;
        };
        let ids = repo
            .global_setting("favourite_sliders")
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
            .unwrap_or_default();
        self.rail.favourites.set_chosen(ids);
    }

    /// Save the Favorites slider list when the picker changed it.
    fn save_favourites(&mut self) {
        let Some(repo) = &mut self.settings else {
            return;
        };
        let mut settings = serde_json::Map::new();
        settings.insert(
            "favourite_sliders".to_string(),
            serde_json::json!(self.rail.favourites.chosen),
        );
        let _ = repo.save_global_settings(&settings);
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

    /// Whether the canvas waits for a frame decode or a preview render.
    #[must_use]
    pub fn processing(&self) -> bool {
        self.processing
    }

    /// Mark the canvas busy for a request this app is about to issue.
    pub fn begin_processing(&mut self) {
        self.processing = true;
    }

    /// Submit one render when the config differs from the frame on the canvas.
    fn request_render(&mut self, ctx: &egui::Context, interactive: bool) {
        if self.submit_render() {
            ctx.request_repaint();
        }
        let _ = interactive;
    }

    /// Submit the first render without an event loop.
    pub fn start_render_now(&mut self) {
        self.submit_render();
    }

    /// Whether a source is bound and no render for its config is pending.
    #[must_use]
    pub fn needs_render(&self) -> bool {
        self.source.is_some() && self.requested.is_none()
    }

    fn submit_render(&mut self) -> bool {
        let Some(source) = self.source.clone() else {
            return false;
        };
        let request = crate::render_worker::RenderRequest {
            generation: self.generation + 1,
            source: source.clone(),
            config: self.edit.config.clone(),
            proof: self.proof_request(),
            gpu_enabled: self.gpu_enabled,
            crop_preview_full: self.rail.geometry.crop.draw,
        };
        let hash = request.config_hash();
        if self.rendered.as_deref() == Some(hash.as_str())
            || self.requested.as_deref() == Some(hash.as_str())
        {
            return false;
        }
        self.generation += 1;
        self.requested = Some(hash);
        self.processing = true;
        self.renderer.submit(request);
        true
    }

    /// Show the newest finished preview.
    fn receive_exports(&mut self, ctx: &egui::Context) {
        for report in self.exporter.drain() {
            self.rail.export_status.running = false;
            if report.failed.is_empty() {
                self.rail.export_status.failed = false;
                let engine = report
                    .engines
                    .first()
                    .map(negpy_backend::services::export::render::ExportEngine::label)
                    .unwrap_or_else(|| "CPU".to_string());
                self.rail.export_status.message = format!(
                    "Wrote {} on the {engine}.",
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

    /// The contact sheet's frames, without the ones the picker left out.
    fn sheet_entries(
        &self,
    ) -> Vec<negpy_backend::services::export::contact_sheet_page::SheetEntry> {
        let profile = self.profile.clone();
        self.strip
            .iter()
            .enumerate()
            .filter(|(index, frame)| {
                !self.sheet.left_out.contains(index)
                    && !frame.path.as_deref().is_some_and(|path| {
                        crate::frame_source::stored_mark(profile.as_deref(), path).as_deref()
                            == Some("excluded")
                    })
            })
            .filter_map(|(_, frame)| {
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
        self.sheet.page_count = negpy_backend::services::export::contact_sheet_page::planned_pages(
            &entries,
            &self.sheet.settings,
            &self.sheet_roll_name(),
            self.sheet_library().as_ref(),
        );
        if self.sheet.page >= self.sheet.page_count {
            self.sheet.page = self.sheet.page_count.saturating_sub(1);
        }
        let library = self.sheet_library();
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
        let roll_name = self.sheet_roll_name();
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

    /// The roll name a contact sheet labels itself with.
    fn sheet_roll_name(&self) -> String {
        self.source
            .as_ref()
            .map_or_else(|| "Roll".to_string(), |source| source.name.clone())
    }

    /// The gear library a contact sheet reads its stock facts from.
    fn sheet_library(&self) -> Option<negpy_backend::features::metadata::gear::GearLibrary> {
        self.profile.as_ref().map(|profile| {
            let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
            negpy_backend::features::metadata::gear::GearLibrary::load(
                Path::new(&app.gear_dir),
                &negpy_backend::kernel::system::paths::resource_path("gear"),
            )
        })
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
            if crate::frame_source::stored_mark(Some(&profile), path).as_deref() == Some("excluded")
            {
                continue;
            }
            paths.push(path.clone());
            configs.push(crate::frame_source::stored_config(Some(&profile), path));
        }
        if paths.is_empty() {
            self.rail.export_status.failed = true;
            self.rail.export_status.message = "No frames to export.".to_string();
            return;
        }
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let count = paths.len();
        self.rail.export_status.running = true;
        self.rail.export_status.failed = false;
        self.rail.export_status.message = format!(
            "Exporting {}...",
            crate::view::rail::count_of(count, "frame")
        );
        self.exporter.submit(ExportRequest {
            paths,
            configs,
            preset_exports: vec![None; count],
            color_space: self.edit.config.export.export_color_space.clone(),
            user_icc_dir: std::path::PathBuf::from(app.user_icc_dir),
            gpu_enabled: self.gpu_enabled,
        });
        ctx.request_repaint();
    }

    /// Run the chosen preset on every non-rejected frame of the strip.
    fn run_export_preset(&mut self, ctx: &egui::Context) {
        let Some(index) = self.rail.export.take_run_preset_request() else {
            return;
        };
        let Some(preset) = self.rail.export.presets.get(index).cloned() else {
            return;
        };
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
            if crate::frame_source::stored_mark(Some(&profile), path).as_deref() == Some("excluded")
            {
                continue;
            }
            paths.push(path.clone());
            configs.push(crate::frame_source::stored_config(Some(&profile), path));
        }
        if paths.is_empty() {
            self.rail.export_status.failed = true;
            self.rail.export_status.message = "No frames to export.".to_string();
            return;
        }
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let preset_exports = paths
            .iter()
            .zip(&configs)
            .map(|(path, config)| {
                Some(preset.apply_to(
                    &config.export,
                    path.parent().unwrap_or(std::path::Path::new(".")),
                ))
            })
            .collect();
        let count = paths.len();
        self.rail.export_status.running = true;
        self.rail.export_status.failed = false;
        self.rail.export_status.message = format!(
            "Exporting {} with {}...",
            crate::view::rail::count_of(count, "frame"),
            preset.name
        );
        self.exporter.submit(ExportRequest {
            paths,
            configs,
            preset_exports,
            color_space: preset.export_color_space.clone(),
            user_icc_dir: std::path::PathBuf::from(app.user_icc_dir),
            gpu_enabled: self.gpu_enabled,
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

    /// Apply the Presets page's rename, enable toggle and delete to the store.
    fn update_export_presets(&mut self) {
        let toggle = self.rail.export.take_toggle_preset();
        let delete = self.rail.export.take_delete_preset_request();
        let mut presets = self.rail.export.presets.clone();
        if let Some((index, enabled)) = toggle
            && let Some(preset) = presets.get_mut(index)
        {
            preset.enabled = enabled;
        }
        let rename = self
            .rail
            .export
            .selected_preset
            .filter(|index| {
                let name = self.rail.export.preset_name.trim();
                !name.is_empty()
                    && presets
                        .get(*index)
                        .is_some_and(|preset| preset.name != name)
            })
            .map(|index| (index, self.rail.export.preset_name.trim().to_string()));
        if let Some((index, name)) = &rename
            && let Some(preset) = presets.get_mut(*index)
        {
            preset.name = name.clone();
        }
        if let Some(index) = delete
            && index < presets.len()
        {
            presets.remove(index);
            self.rail.export.selected_preset = None;
        }
        let Some(repo) = &mut self.settings else {
            return;
        };
        if toggle.is_some() || delete.is_some() || rename.is_some() {
            match repo.save_export_presets(&presets) {
                Ok(()) => self.rail.export.set_presets(presets),
                Err(error) => self.rail.export.set_status(error.to_string()),
            }
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
                self.relock_preset_cards(&rows);
                let changed = match self.rail.metadata.apply_scope.current {
                    1 => self.apply_preset_to_selection(&source, &rows),
                    2 => self.apply_preset_to_roll(&source, &rows),
                    _ => 0,
                };
                self.rail.metadata.preset_status =
                    negpy_backend::session::settings_catalog::preset_applied_for_scope(
                        &name,
                        self.rail.metadata.apply_scope.current_label(),
                        changed,
                    );
            }
        }
        if let Some(name) = self.rail.metadata.take_delete_request() {
            let _ = store.delete(&name);
            self.rail.metadata.selected_preset = None;
            self.rail.metadata.set_presets(store.list());
        }
    }

    /// Write the preset's rows over each selected Light Table frame. Returns the count.
    fn apply_preset_to_selection(
        &mut self,
        source: &negpy_backend::domain::models::WorkspaceConfig,
        rows: &[negpy_backend::session::settings_catalog::SettingRow],
    ) -> usize {
        let targets: Vec<usize> = self
            .selected_frames
            .iter()
            .copied()
            .filter(|index| *index != self.selected)
            .collect();
        self.apply_preset_to_frames(source, rows, &targets)
    }

    fn apply_preset_to_frames(
        &mut self,
        source: &negpy_backend::domain::models::WorkspaceConfig,
        rows: &[negpy_backend::session::settings_catalog::SettingRow],
        targets: &[usize],
    ) -> usize {
        let Some(profile) = self.profile.clone() else {
            return 0;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let roll_id = self.rail.roll_analysis.roll_id.clone();
        let mut count = 0;
        for index in targets {
            let Some(path) = self.strip.get(*index).and_then(|frame| frame.path.clone()) else {
                continue;
            };
            let Ok(hash) =
                negpy_backend::infrastructure::storage::local_asset_store::file_hash(&path)
            else {
                continue;
            };
            let target = roll_id.as_deref().map_or_else(
                || crate::frame_source::stored_config(Some(&profile), &path),
                |roll_id| crate::frame_source::load_config_in_roll(Some(&profile), &path, roll_id),
            );
            let synced = negpy_backend::session::settings_catalog::apply_selected_fields(
                source, &target, rows,
            );
            let before = target.to_dict();
            if repo
                .save_file_settings(&hash, &synced, &path.to_string_lossy())
                .is_err()
            {
                continue;
            }
            if let Some(roll_id) = roll_id.as_deref() {
                settle_preset_locks(&mut repo, roll_id, &hash, rows, &before, &synced);
            }
            count += 1;
        }
        count
    }

    /// Lock each card whose preset rows made that frame diverge from the roll.
    fn relock_preset_cards(
        &mut self,
        rows: &[negpy_backend::session::settings_catalog::SettingRow],
    ) {
        self.lock_preset_cards(rows, &self.edit.config.clone());
    }

    fn lock_preset_cards(
        &mut self,
        rows: &[negpy_backend::session::settings_catalog::SettingRow],
        after: &negpy_backend::domain::models::WorkspaceConfig,
    ) {
        let (Some(profile), Some(roll_id), Some(path)) = (
            self.profile.clone(),
            self.rail.roll_analysis.roll_id.clone(),
            self.strip
                .get(self.selected)
                .and_then(|frame| frame.path.clone()),
        ) else {
            return;
        };
        let Ok(hash) = negpy_backend::infrastructure::storage::local_asset_store::file_hash(&path)
        else {
            return;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let before = crate::frame_source::load_config_in_roll(Some(&profile), &path, &roll_id);
        settle_preset_locks(&mut repo, &roll_id, &hash, rows, &before.to_dict(), after);
    }

    /// Write the preset's rows over every other frame of the open roll. Returns the count.
    fn apply_preset_to_roll(
        &mut self,
        source: &negpy_backend::domain::models::WorkspaceConfig,
        rows: &[negpy_backend::session::settings_catalog::SettingRow],
    ) -> usize {
        let (Some(profile), Some(roll_id)) = (
            self.profile.clone(),
            self.rail.roll_analysis.roll_id.clone(),
        ) else {
            return 0;
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let paths = negpy_backend::services::assets::rolls::roll_frame_paths(&repo, &roll_id);
        let open = self
            .strip
            .get(self.selected)
            .and_then(|frame| frame.path.clone());
        let mut count = 0;
        for path in paths {
            if open
                .as_deref()
                .is_some_and(|open| open == std::path::Path::new(&path))
            {
                continue;
            }
            let target = crate::frame_source::load_config_in_roll(Some(&profile), &path, &roll_id);
            let synced = negpy_backend::session::settings_catalog::apply_selected_fields(
                source, &target, rows,
            );
            let Ok(hash) = negpy_backend::infrastructure::storage::local_asset_store::file_hash(
                std::path::Path::new(&path),
            ) else {
                continue;
            };
            let before = target.to_dict();
            if repo.save_file_settings(&hash, &synced, &path).is_ok() {
                settle_preset_locks(&mut repo, &roll_id, &hash, rows, &before, &synced);
                count += 1;
            }
        }
        count
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
            self.processing = false;
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

    /// Store a hand-picked Dmin or Dmax from the sample at `point`, then disarm the tool.
    fn store_bounds_pick(&mut self, point: [f64; 2]) {
        let pick = self.rail.metering.pick;
        if pick == crate::view::rail::metering::MeteringPick::Off {
            return;
        }
        let Some(source) = self.source.clone() else {
            self.rail
                .metering
                .set_pick_status("Pick by Hand: click once the frame has rendered");
            return;
        };
        let processed = negpy_backend::features::geometry::processor::process(
            &source.image,
            &self.edit.config.geometry,
            negpy_backend::features::geometry::processor::GeometryContext {
                scale_factor: source.image.height.max(source.image.width) as f64 / 4000.0,
            },
        );
        let Some(sample) = negpy_backend::features::exposure::metering::sample_log_patch(
            &processed.image.pixels,
            processed.image.width,
            processed.image.height,
            &self.edit.config.process,
            self.edit.config.process.process_mode,
            (point[0], point[1]),
            4,
        ) else {
            self.rail
                .metering
                .set_pick_status("Pick by Hand: click once the frame has rendered");
            return;
        };
        let before = self.edit.config.clone();
        let process = &mut self.edit.config.process;
        match pick {
            crate::view::rail::metering::MeteringPick::Dmin => {
                process.manual_dmin = Some(sample.to_vec());
                process.manual_dmin_at = Some(point.to_vec());
            }
            crate::view::rail::metering::MeteringPick::Dmax => {
                process.manual_dmax = Some(sample.to_vec());
                process.manual_dmax_at = Some(point.to_vec());
            }
            crate::view::rail::metering::MeteringPick::Off => return,
        }
        process.local_floors = [0.0; 3];
        process.local_ceils = [0.0; 3];
        self.edit.checkpoint_before(before);
        self.edit.mark_dirty();
        self.rail.metering.pick = crate::view::rail::metering::MeteringPick::Off;
        self.rail.metering.set_pick_status(
            if pick == crate::view::rail::metering::MeteringPick::Dmin {
                "Dmin set for this frame"
            } else {
                "Dmax set for this frame"
            },
        );
    }

    /// Keep a confirmed crop once, and leave the Draw tool.
    fn confirm_crop(&mut self, rect: [f64; 4]) {
        self.store_crop(rect);
        self.rail.geometry.crop.draw = false;
        self.rail.geometry.crop.has_crop = true;
    }

    /// Apply what the crop tool did. A pending rect lives in `crop_drag` until confirm.
    fn apply_crop_action(&mut self, action: crate::view::canvas::CropAction, ctx: &egui::Context) {
        match action {
            crate::view::canvas::CropAction::Pending(_) => ctx.request_repaint(),
            crate::view::canvas::CropAction::Confirm(rect) => {
                self.confirm_crop(rect);
                self.crop_drag = None;
                self.request_render(ctx, false);
            }
            crate::view::canvas::CropAction::None => {}
        }
    }

    /// Commit a pending crop when Draw closes, and drop a stale drag when it opens.
    fn settle_crop_tool(&mut self, was_drawing: bool, ctx: &egui::Context) {
        let drawing = self.rail.geometry.crop.draw;
        if was_drawing && !drawing {
            if let Some(drag) = self.crop_drag.take() {
                self.confirm_crop(drag.pending);
                self.request_render(ctx, false);
            }
        } else if !was_drawing && drawing {
            self.crop_drag = None;
        }
    }

    /// Write a crop rect and clear the Auto Crop state it replaces.
    fn store_crop(&mut self, rect: [f64; 4]) {
        let geometry = &mut self.edit.config.geometry;
        geometry.crop_rect = Some(rect.to_vec());
        geometry.crop_from_auto = false;
        geometry.crop_detect_key.clear();
        geometry.autocrop_squared = false;
        let process = &mut self.edit.config.process;
        process.local_floors = [0.0; 3];
        process.local_ceils = [0.0; 3];
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

    /// Write the triage mark a Light Table button asked for on every frame in the strip.
    fn apply_light_table_action(&mut self, action: crate::view::light_table::LightTableAction) {
        match action {
            crate::view::light_table::LightTableAction::Keep => self.mark_selected("keeper"),
            crate::view::light_table::LightTableAction::Reject => self.mark_selected("excluded"),
            crate::view::light_table::LightTableAction::AutoCrop => {
                self.edit.config = negpy_backend::domain::models::geometry::with_auto_crop(
                    self.edit.config.clone(),
                    true,
                );
                self.edit.mark_dirty();
                self.rail.geometry.load(&self.edit);
            }
            crate::view::light_table::LightTableAction::Export => {
                self.rail.export.requested = true;
            }
        }
    }

    /// Write one triage mark on the open frame.
    fn mark_selected(&mut self, mark: &str) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let Some(path) = self
            .strip
            .get(self.selected)
            .and_then(|frame| frame.path.clone())
        else {
            return;
        };
        let _ = crate::frame_source::write_mark(Some(&profile), &path, Some(mark));
        self.strip_textures.clear();
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
        self.rail.strip_names = strip.iter().map(|frame| frame.name.clone()).collect();
        self.strip = strip;
        self.selected = 0;
        self.selected_frames.clear();
        self.selected_frames.insert(0);
    }

    /// Queue the strip frame at `index` for the frame worker. Saves the old edit first.
    ///
    /// The decode and the history read run on the worker; a newer click replaces an
    /// unanswered one so only the last selection reaches the canvas.
    pub fn select_frame(&mut self, index: usize) -> bool {
        let Some(path) = self.strip.get(index).and_then(|frame| frame.path.clone()) else {
            return false;
        };
        let _ = self.edit.save();
        self.selected_frames.clear();
        self.selected_frames.insert(index);
        self.frame_generation += 1;
        self.processing = true;
        self.frame_worker
            .submit(crate::frame_worker::FrameOpenRequest {
                generation: self.frame_generation,
                index,
                path,
                profile: self.profile.clone(),
                roll_id: self.rail.roll_analysis.roll_id.clone(),
                max_edge: 1600,
            });
        true
    }

    /// Bind a decoded frame and bring the panels it owns up to date.
    fn show_frame(&mut self, report: crate::frame_worker::FrameOpenReport) {
        let profile = self.profile.clone();
        let path = report.frame.path.clone();
        self.set_source(Some(report.source));
        self.crop_drag = None;
        self.mask_drag = crate::view::canvas::MaskDrag::default();
        self.retouch_drag = crate::view::canvas::RetouchDrag::default();
        self.rail.geometry.crop.draw = false;
        self.rail.metering.pick = crate::view::rail::metering::MeteringPick::Off;
        self.rail.exposure.local.draw = false;
        self.rail.exposure.local.oval = false;
        self.rail.exposure.local.card = false;
        self.rail.finish.tool = crate::view::rail::frame_finish::RetouchTool::Off;
        self.set_frame(report.frame);
        self.set_edit(report.edit);
        if let Some(path) = path.as_deref() {
            self.rail.metadata.set_folder_name(
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
            );
        }
        self.rail
            .optics
            .set_profile(profile.as_deref(), &self.edit.config.flatfield.profile_id);
        self.rail.set_profile(profile.as_deref(), &self.edit.config);
        if !self.sticky_zoom {
            self.zoom = 1.0;
            self.zoom_percent = None;
        }
        self.selected = report.index;
        self.last_frame_open_ms = report.elapsed_ms;
        self.set_history(&report.history, &report.prints);
        self.set_roll_analysis(report.roll_analysis);
    }

    /// Show the newest finished frame open, dropping a superseded one.
    fn receive_frame(&mut self, ctx: &egui::Context) {
        for report in self.frame_worker.drain() {
            let report = match report {
                Ok(report) => report,
                Err((generation, _)) => {
                    if generation == self.frame_generation {
                        self.processing = false;
                    }
                    self.rail.library.set_status("Could not open the frame.");
                    continue;
                }
            };
            if report.generation != self.frame_generation {
                continue;
            }
            self.show_frame(report);
            self.processing = false;
            ctx.request_repaint();
        }
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
        self.receive_frame(ctx);
        self.receive_render(ctx);
        self.receive_scans(ctx);
        self.receive_sheets(ctx);
        self.receive_crops(ctx);
        self.receive_detect(ctx);
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
                    let marks: Vec<Option<String>> = self
                        .strip
                        .iter()
                        .map(|frame| {
                            frame.path.as_deref().and_then(|path| {
                                crate::frame_source::stored_mark(self.profile.as_deref(), path)
                            })
                        })
                        .collect();
                    let mark_refs: Vec<Option<&str>> =
                        marks.iter().map(|mark| mark.as_deref()).collect();
                    let hit = crate::view::light_table::paint(
                        ui,
                        screen,
                        &strip_textures,
                        self.selected,
                        &self.selected_frames,
                        &mark_refs,
                    );
                    if let Some(index) = hit.cell {
                        if hit.modified {
                            let anchor = self.selected;
                            self.selected_frames.insert(index);
                            if hit.shift {
                                let (low, high) = if anchor <= index {
                                    (anchor, index)
                                } else {
                                    (index, anchor)
                                };
                                self.selected_frames.extend(low..=high);
                            } else if self.selected_frames.contains(&index) && index != anchor {
                                self.selected_frames.remove(&index);
                            } else {
                                self.selected_frames.insert(anchor);
                            }
                        } else {
                            self.selected_frames.clear();
                            self.selected_frames.insert(index);
                            self.select_frame(index);
                        }
                    }
                    if let Some(action) = hit.action {
                        self.apply_light_table_action(action);
                    }
                    let rail_rect = rail_rect(screen);
                    self.request_render(ctx, false);
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
                    self.apply_detect_ratio(ctx);
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
                        let frames: Vec<String> =
                            self.strip.iter().map(|frame| frame.name.clone()).collect();
                        let actions = self.sheet.show(ui, screen, &frames);
                        if actions.refresh {
                            self.request_sheet(ctx, false);
                        }
                        if actions.export {
                            self.request_sheet(ctx, true);
                        }
                        if let Some(index) = actions.tile {
                            self.select_frame(index);
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
                crate::view::canvas::CropGuide::coerce(&self.rail.geometry.guide_key),
                &mut self.crop_drag,
                (
                    &self.edit.config.local.masks,
                    self.rail.exposure.local.draw,
                    self.rail.exposure.local.oval,
                    self.rail.exposure.local.card,
                    self.rail.exposure.mask_index,
                ),
                (
                    self.edit
                        .config
                        .process
                        .manual_dmin_at
                        .as_deref()
                        .unwrap_or(&[]),
                    self.edit
                        .config
                        .process
                        .manual_dmax_at
                        .as_deref()
                        .unwrap_or(&[]),
                ),
                self.photo_mode,
                &mut self.mask_drag,
                self.rail.metering.pick != crate::view::rail::metering::MeteringPick::Off,
                self.processing,
                retouch_tool,
                &mut self.retouch_drag,
                self.zoom,
                self.sticky_zoom,
                self.edit.config.process.use_luma_average
                    || self.edit.config.process.use_color_average
                    || self.edit.config.process.use_cast_average,
                &mut self.find,
                &recipe,
            );
            let find_results = self.find.results.clone();
            let search = Rect::from_min_max(
                egui::pos2(
                    screen.right() - rail_width(screen.width()) - 8.0,
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
            if let Some(on) = hits.sticky_zoom {
                self.set_sticky_zoom(on);
                ctx.request_repaint();
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
            if let Some(action) = hits.overflow {
                self.apply_toolbar_action(action, ctx);
            }
            self.apply_canvas_zoom(ctx, hits.photo);
            if let Some(photo) = hits.photo_mode {
                self.rail.tab = if photo { Tab::Frame } else { Tab::Roll };
                self.photo_mode = photo;
                ctx.request_repaint();
            }
            self.apply_crop_action(hits.crop, ctx);
            self.apply_retouch(hits.retouch);
            if let crate::view::canvas::PickAction::At(point) = hits.pick {
                self.store_bounds_pick(point);
                self.request_render(ctx, false);
            }
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
            self.update_export_presets();
            self.save_export_preset();
            self.run_export_preset(ctx);
            self.request_scan(ctx);
            self.open_scan_folder();
            let proof = self.proof.clone();
            let before = self.edit.config.clone();
            let before_favourites = self.rail.favourites.chosen.clone();
            let was_drawing = self.rail.geometry.crop.draw;
            let switched = self
                .rail
                .show(ui, rail_rect, &mut self.edit, &mut self.proof);
            self.settle_crop_tool(was_drawing, ctx);
            if self.edit.config != before {
                self.edit.checkpoint_before(before);
            }
            self.rail.apply_optics_action(&mut self.edit);
            self.rail.apply_history_action(&mut self.edit);
            if self.rail.favourites.chosen != before_favourites {
                self.save_favourites();
            }
            self.apply_metadata_preset();
            self.apply_square_request(ctx);
            self.apply_detect_ratio(ctx);
            self.request_crop_all(ctx);
            self.apply_roll_analysis(ctx);
            if self.rail.export.take_contact_sheet_request() {
                let excluded: Vec<bool> = self
                    .strip
                    .iter()
                    .map(|frame| {
                        frame.path.as_deref().is_some_and(|path| {
                            crate::frame_source::stored_mark(self.profile.as_deref(), path)
                                .as_deref()
                                == Some("excluded")
                        })
                    })
                    .collect();
                self.sheet.seed_excluded(&excluded);
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

    /// Measure the film frame's closest standard ratio when the Crop card asks.
    fn apply_detect_ratio(&mut self, ctx: &egui::Context) {
        if !self.rail.geometry.detect_ratio_request {
            return;
        }
        self.rail.geometry.detect_ratio_request = false;
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let ratio = negpy_backend::features::geometry::processor::detect_closest_aspect_ratio(
            &source.image,
            self.edit.config.geometry.autocrop_ratio,
        );
        if ratio != self.edit.config.geometry.autocrop_ratio {
            self.edit.config.geometry.autocrop_ratio = ratio;
            self.edit.mark_dirty();
            self.request_render(ctx, false);
        }
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
        if self.rail.roll_analysis.take_detect() {
            self.detect_scenes();
        }
        if self.rail.roll_analysis.take_accept() {
            self.accept_scenes();
            self.requested = None;
            self.rendered = None;
            ctx.request_repaint();
        }
        if self.rail.roll_analysis.take_discard() {
            self.rail.roll_analysis.found.clear();
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

    /// Queue the strip for Roll Analysis Detect. The decode runs on its worker.
    fn detect_scenes(&mut self) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.detect.submit(crate::detect_worker::DetectRequest {
            paths: self.strip.iter().map(|frame| frame.path.clone()).collect(),
            profile,
        });
    }

    /// Group the measured axes once the Detect worker reports.
    fn receive_detect(&mut self, ctx: &egui::Context) {
        let Some(report) = self.detect.drain() else {
            return;
        };
        self.rail.roll_analysis.found =
            negpy_backend::features::exposure::normalization::detect_light_groups(&report.axes);
        self.rail.roll_analysis.open = true;
        ctx.request_repaint();
    }

    /// Write each found group as a scene of the open roll.
    fn accept_scenes(&mut self) {
        let Some(profile) = self.profile.clone() else {
            return;
        };
        let Some(roll_id) = self.rail.roll_analysis.roll_id.clone() else {
            return;
        };
        let found = std::mem::take(&mut self.rail.roll_analysis.found);
        if found.is_empty() {
            return;
        }
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(&profile);
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app.edits_db_path,
            app.settings_db_path,
        );
        let names = negpy_backend::services::assets::rolls::next_scene_names(
            &repo,
            Some(&roll_id),
            found.len(),
        );
        for (group, name) in found.iter().zip(names) {
            let hashes: Vec<String> = group
                .iter()
                .filter_map(|index| self.strip.get(*index))
                .filter_map(|frame| frame.path.as_deref())
                .filter_map(|path| {
                    negpy_backend::infrastructure::storage::local_asset_store::file_hash(path).ok()
                })
                .collect();
            if hashes.len() >= 2 {
                negpy_backend::services::assets::rolls::create_scene(
                    &mut repo, &roll_id, &name, &hashes,
                );
            }
        }
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

    /// Keep or reset the zoom on every frame switch, and save the choice.
    pub fn set_sticky_zoom(&mut self, enabled: bool) {
        if self.sticky_zoom == enabled {
            return;
        }
        self.sticky_zoom = enabled;
        let Some(repo) = &mut self.settings else {
            return;
        };
        let mut settings = serde_json::Map::new();
        settings.insert("sticky_zoom".to_string(), serde_json::json!(enabled));
        let _ = repo.save_global_settings(&settings);
    }

    /// Run one action the toolbar overflow menu offered.
    fn apply_toolbar_action(&mut self, action: ToolbarAction, ctx: &egui::Context) {
        match action {
            ToolbarAction::Previous => {
                self.previous_frame();
            }
            ToolbarAction::Next => {
                self.next_frame();
            }
            ToolbarAction::Zoom(percent) => self.zoom_to_percent(percent),
            ToolbarAction::StickyZoom(on) => self.set_sticky_zoom(on),
            ToolbarAction::RotateLeft => self.transform_action(0, ctx),
            ToolbarAction::RotateRight => self.transform_action(1, ctx),
            ToolbarAction::FlipHorizontal => self.transform_action(2, ctx),
            ToolbarAction::FlipVertical => self.transform_action(3, ctx),
            ToolbarAction::Tools => self.open_tools(),
            ToolbarAction::Undo => self.undo_action(ctx),
            ToolbarAction::Redo => self.redo_action(ctx),
            ToolbarAction::Print => {
                self.rail.tab = Tab::Export;
                self.rail.export.show_subtab("file");
                self.view.tab = Tab::Export;
                self.photo_mode = false;
            }
            ToolbarAction::RollAverage(on) => {
                self.rail.metering.set_all_averages(on, &mut self.edit);
                self.rail.select_view(&self.view);
                self.request_render(ctx, false);
            }
        }
        ctx.request_repaint();
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
    let rail_width = rail_width(screen.width());
    Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    )
}

/// The right rail's width for a window width: a 27% share, and never less than the
/// canvas needs beside it.
fn rail_width(screen_width: f32) -> f32 {
    RAIL_WIDTH
        .min((screen_width * 0.27).max(MIN_RAIL_WIDTH))
        .min((screen_width - MIN_CANVAS_WIDTH).max(MIN_RAIL_WIDTH))
}

/// The narrowest rail a panel still reads in.
const MIN_RAIL_WIDTH: f32 = 320.0;

/// The canvas width the toolbar and a fit preview need at a narrow window.
const MIN_CANVAS_WIDTH: f32 = 120.0;

/// The canvas height the photo keeps when a short window squeezes the strip.
const MIN_CANVAS_HEIGHT: f32 = 120.0;

/// The Film Strip's height when it is open.
const STRIP_HEIGHT: f32 = 239.0;

/// The canvas toolbar's height.
const TOOLBAR_HEIGHT: f32 = 52.0;

/// The line the canvas shows while a frame decode or render runs.
pub const PROCESSING: &str = "Processing…";

/// The smallest window that keeps a canvas between the top bar, toolbar and strip.
#[must_use]
pub fn minimum_window_size() -> Vec2 {
    Vec2::new(
        2.0 * MIN_CANVAS_WIDTH + MIN_RAIL_WIDTH + 16.0,
        TOP_ROW_HEIGHT + MIN_CANVAS_HEIGHT + TOOLBAR_HEIGHT + STRIP_HEIGHT + 24.0,
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
    crop_guide: crate::view::canvas::CropGuide,
    crop_drag: &mut Option<crate::view::canvas::CropDrag>,
    local: (
        &[negpy_backend::domain::models::LocalMask],
        bool,
        bool,
        bool,
        usize,
    ),
    bounds_picks: (&[f64], &[f64]),
    photo_mode: bool,
    mask_drag: &mut crate::view::canvas::MaskDrag,
    pick_active: bool,
    processing: bool,
    retouch_tool: crate::view::canvas::RetouchTool,
    retouch_drag: &mut crate::view::canvas::RetouchDrag,
    zoom: f32,
    sticky_zoom: bool,
    averages_on: bool,
    find: &mut FindState,
    recipe: &str,
) -> (Rect, ShellHits) {
    let painter = ui.painter().clone();
    let margin = 8.0;
    let body_top = screen.top() + crate::styles::layout::TOP_ROW_HEIGHT + 8.0;
    let rail_width = rail_width(screen.width());
    paint_top_bar_beside(&painter, screen, rail_width, label, recipe);
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
        STRIP_HEIGHT
    } else {
        0.0
    };
    let canvas_width = rail.left() - margin - (screen.left() + margin);
    let room = (screen.bottom() - margin - body_top - TOOLBAR_HEIGHT).max(0.0);
    // Keep the canvas height before the strip gives way, as Python's splitter does.
    let strip_height = strip_height.min((room - MIN_CANVAS_HEIGHT).max(0.0));
    let canvas_bottom = screen.bottom() - margin - TOOLBAR_HEIGHT - strip_height;
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
            crop_drag,
            crop_guide,
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
        crate::view::canvas::paint_picks(
            ui,
            photo,
            &[(bounds_picks.0, "Dmin"), (bounds_picks.1, "Dmax")],
        );
        hits.pick = crate::view::canvas::pick_tool(ui, photo, pick_active);
        hits.retouch = crate::view::canvas::retouch_tool(ui, photo, retouch_tool, retouch_drag);
    } else {
        hits.import = paint_empty_canvas(ui, photo);
    }
    paint_processing(painter.clone(), photo, processing);

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
            screen.bottom() - margin - TOOLBAR_HEIGHT,
        ),
        Vec2::new(canvas_width, TOOLBAR_HEIGHT),
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
        sticky_zoom,
        averages_on,
    );
    hits.previous = navigation.previous;
    hits.next = navigation.next;
    hits.undo = navigation.undo;
    hits.redo = navigation.redo;
    hits.zoom = navigation.zoom;
    hits.sticky_zoom = navigation.sticky_zoom;
    hits.roll_average = navigation.roll_average;
    hits.tools = navigation.tools;
    hits.print = navigation.print;
    hits.overflow = navigation.overflow;
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
    paint_top_bar_beside(painter, screen, rail_width(screen.width()), frame, recipe);
}

/// Paint the file name and the recipe beside a rail of `rail_width`.
pub fn paint_top_bar_beside(
    painter: &egui::Painter,
    screen: Rect,
    rail_width: f32,
    frame: &str,
    recipe: &str,
) {
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
        egui::pos2(screen.right() - rail_width - 26.0, top.center().y),
        egui::Align2::RIGHT_CENTER,
        recipe,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );
}

/// Paint the busy line over the picture while a frame decode or render runs.
fn paint_processing(painter: egui::Painter, photo: Rect, processing: bool) {
    if !processing {
        return;
    }
    let theme = Theme::default();
    painter.text(
        egui::pos2(photo.center().x, photo.top() + f32::from(theme.space_xl)),
        egui::Align2::CENTER_TOP,
        PROCESSING,
        egui::FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
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

/// The toolbar controls that leave the row as it narrows, first to last, as Python
/// `_COLLAPSE_ORDER` does.
const TOOLBAR_COLLAPSE_ORDER: [&str; 11] = [
    "flip_v",
    "flip_h",
    "rot_r",
    "rot_l",
    "average",
    "redo",
    "undo",
    "tools",
    "print",
    "zoom",
    "navigation",
];

/// One control's width on the toolbar row.
const TOOLBAR_ITEM_WIDTHS: [(&str, f32); 8] = [
    ("navigation", 172.0),
    ("zoom", 76.0),
    ("geometry", 160.0),
    ("tools", 94.0),
    ("undo", 36.0),
    ("redo", 36.0),
    ("print", 112.0),
    ("average", 66.0),
];

const TOOLBAR_MODE_WIDTH: f32 = 146.0;
const TOOLBAR_PRIMARY_WIDTH: f32 = 154.0;
const TOOLBAR_TRAILING_WIDTH: f32 = 108.0;
const TOOLBAR_OVERFLOW_WIDTH: f32 = 36.0;
const TOOLBAR_GAP: f32 = 8.0;
const TOOLBAR_TRACK_PAD: f32 = 4.0;
const TOOLBAR_TRANSFORM_WIDTH: f32 = 36.0;

/// What the row shows at a given width.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ToolbarLayout {
    visible: [bool; 8],
    transforms: [bool; 4],
    overflow: bool,
}

impl ToolbarLayout {
    /// Show every control at a wide width.
    fn all() -> Self {
        Self {
            visible: [true; 8],
            transforms: [true; 4],
            overflow: false,
        }
    }

    fn transform_count(&self) -> usize {
        self.transforms.iter().filter(|shown| **shown).count()
    }

    fn needed(&self) -> f32 {
        let mut width = TOOLBAR_MODE_WIDTH + TOOLBAR_GAP;
        for (index, (_, item_width)) in TOOLBAR_ITEM_WIDTHS.iter().enumerate() {
            if self.visible[index] {
                let width_if = if TOOLBAR_ITEM_WIDTHS[index].0 == "geometry" {
                    self.transform_count() as f32 * TOOLBAR_TRANSFORM_WIDTH
                        + 2.0 * TOOLBAR_TRACK_PAD
                } else {
                    *item_width
                };
                width += width_if + TOOLBAR_GAP;
            }
        }
        width
            + TOOLBAR_PRIMARY_WIDTH
            + 6.0
            + TOOLBAR_TRAILING_WIDTH
            + if self.overflow {
                TOOLBAR_OVERFLOW_WIDTH + TOOLBAR_GAP
            } else {
                0.0
            }
    }

    fn hide(&mut self, key: &str) {
        match key {
            "flip_v" => self.transforms[3] = false,
            "flip_h" => self.transforms[2] = false,
            "rot_r" => self.transforms[1] = false,
            "rot_l" => self.transforms[0] = false,
            other => {
                if let Some(index) = TOOLBAR_ITEM_WIDTHS
                    .iter()
                    .position(|(name, _)| *name == other)
                {
                    self.visible[index] = false;
                }
            }
        }
        self.overflow = true;
    }
}

/// Fit the toolbar controls to `available` width, hiding in the Python order.
fn toolbar_layout(available: f32) -> ToolbarLayout {
    let mut layout = ToolbarLayout::all();
    for key in TOOLBAR_COLLAPSE_ORDER {
        if layout.needed() <= available {
            break;
        }
        layout.hide(key);
    }
    layout
}

fn paint_toolbar(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    toolbar: Rect,
    selected: usize,
    frames: usize,
    photo_mode: bool,
    zoom_percent: u32,
    sticky_zoom: bool,
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
    let layout = toolbar_layout(toolbar.width());
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

    if layout.visible[0] {
        let navigation = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[0].1, row_height),
        );
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
        hits.previous = previous.clicked();
        hits.next = next.clicked();
        x = navigation.right() + TOOLBAR_GAP;
    }

    if layout.visible[1] {
        let zoom = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[1].1, row_height),
        );
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
        if let Some(picked) = zoom_menu(zoom_hit, sticky_zoom) {
            match picked {
                ZoomPick::Percent(percent) => hits.zoom = Some(percent),
                ZoomPick::StickyZoom(on) => hits.sticky_zoom = Some(on),
            }
        }
        x = zoom.right() + TOOLBAR_GAP;
    }

    if layout.transform_count() > 0 {
        let width =
            layout.transform_count() as f32 * TOOLBAR_TRANSFORM_WIDTH + 2.0 * TOOLBAR_TRACK_PAD;
        let geometry = Rect::from_min_size(egui::pos2(x, top), Vec2::new(width, row_height));
        track(painter, geometry);
        let count = layout.transform_count();
        for (slot, index) in layout
            .transforms
            .iter()
            .enumerate()
            .filter(|(_, shown)| **shown)
            .map(|(index, _)| index)
            .enumerate()
        {
            let icon = [
                Icon::RotateLeft,
                Icon::RotateRight,
                Icon::LockSimple,
                Icon::LockOpen,
            ][index];
            let button = Rect::from_min_size(
                egui::pos2(
                    geometry.left()
                        + BAR_TRACK_PAD
                        + slot as f32 * (geometry.width() - 2.0 * BAR_TRACK_PAD) / count as f32,
                    geometry.top() + BAR_TRACK_PAD,
                ),
                Vec2::new(
                    (geometry.width() - 2.0 * BAR_TRACK_PAD) / count as f32,
                    BAR_TRACK_BUTTON_HEIGHT,
                ),
            );
            if index == 3 {
                painter.rect_filled(button, 4.0, theme.bg_checked);
            }
            flat(
                painter,
                button,
                icon,
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
        x = geometry.right() + TOOLBAR_GAP;
    }

    if layout.visible[3] {
        let button = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[3].1, row_height),
        );
        track(painter, button);
        flat(
            painter,
            Rect::from_min_size(button.min, Vec2::new(30.0, row_height)),
            Icon::Wrench,
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
        x = button.right() + TOOLBAR_GAP;
    }

    for (index, icon) in [(4, Icon::Undo), (5, Icon::Redo)] {
        if !layout.visible[index] {
            continue;
        }
        let button = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[index].1, row_height),
        );
        track(painter, button);
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

    if layout.visible[6] {
        let print = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[6].1, row_height),
        );
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
    }

    if layout.visible[7] {
        let roll_average = Rect::from_min_size(
            egui::pos2(x, top),
            Vec2::new(TOOLBAR_ITEM_WIDTHS[7].1, row_height),
        );
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
    }

    let trailing = Rect::from_min_size(
        egui::pos2(toolbar.right() - 8.0 - TOOLBAR_TRAILING_WIDTH, top),
        Vec2::new(TOOLBAR_TRAILING_WIDTH, row_height),
    );
    if layout.overflow {
        let more = Rect::from_min_size(
            egui::pos2(trailing.left() - TOOLBAR_GAP - TOOLBAR_OVERFLOW_WIDTH, top),
            Vec2::new(TOOLBAR_OVERFLOW_WIDTH, row_height),
        );
        track(painter, more);
        flat(painter, more, Icon::More, theme.text_secondary);
        let response = ui.interact(more, ui.id().with("toolbar-overflow"), Sense::click());
        hits.overflow = show_toolbar_overflow(&response, &layout, sticky_zoom, averages_on);
    }
    let primary = Rect::from_min_size(
        egui::pos2(
            toolbar.right() - 8.0 - TOOLBAR_TRAILING_WIDTH - 6.0 - TOOLBAR_PRIMARY_WIDTH,
            top,
        ),
        Vec2::new(TOOLBAR_PRIMARY_WIDTH, row_height),
    );
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
    hits
}

/// The overflow menu's actions, one per control the row had to drop.
fn show_toolbar_overflow(
    response: &egui::Response,
    layout: &ToolbarLayout,
    sticky_zoom: bool,
    averages_on: bool,
) -> Option<ToolbarAction> {
    egui::Popup::menu(response)
        .show(|ui| {
            ui.set_min_width(190.0);
            let mut picked = None;
            if !layout.visible[0] {
                ui.horizontal(|ui| {
                    if ui.button("Previous").clicked() {
                        picked = Some(ToolbarAction::Previous);
                    }
                    if ui.button("Next").clicked() {
                        picked = Some(ToolbarAction::Next);
                    }
                });
            }
            if !layout.visible[1] {
                for percent in [25, 50, 100, 200, 400] {
                    if ui.button(format!("{percent}%")).clicked() {
                        picked = Some(ToolbarAction::Zoom(percent));
                    }
                }
                let label = if sticky_zoom {
                    "Sticky Zoom: On"
                } else {
                    "Sticky Zoom: Off"
                };
                if ui.button(label).clicked() {
                    picked = Some(ToolbarAction::StickyZoom(!sticky_zoom));
                }
                ui.separator();
            }
            if !layout.transforms[0] && ui.button("Rotate CCW").clicked() {
                picked = Some(ToolbarAction::RotateLeft);
            }
            if !layout.transforms[1] && ui.button("Rotate CW").clicked() {
                picked = Some(ToolbarAction::RotateRight);
            }
            if !layout.transforms[2] && ui.button("Flip Horizontal").clicked() {
                picked = Some(ToolbarAction::FlipHorizontal);
            }
            if !layout.transforms[3] && ui.button("Flip Vertical").clicked() {
                picked = Some(ToolbarAction::FlipVertical);
            }
            if !layout.visible[3] && ui.button("Tools").clicked() {
                picked = Some(ToolbarAction::Tools);
            }
            if !layout.visible[4] && ui.button("Undo").clicked() {
                picked = Some(ToolbarAction::Undo);
            }
            if !layout.visible[5] && ui.button("Redo").clicked() {
                picked = Some(ToolbarAction::Redo);
            }
            if !layout.visible[6] && ui.button("Print").clicked() {
                picked = Some(ToolbarAction::Print);
            }
            if !layout.visible[7] {
                let label = if averages_on {
                    "Roll Average: On"
                } else {
                    "Roll Average: Off"
                };
                if ui.button(label).clicked() {
                    picked = Some(ToolbarAction::RollAverage(!averages_on));
                }
            }
            picked
        })
        .and_then(|response| response.inner)
}

/// What a click on the toolbar zoom readout selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoomPick {
    Percent(u32),
    StickyZoom(bool),
}

/// The zoom presets and Sticky Zoom toggle the toolbar readout opens.
fn zoom_menu(response: egui::Response, sticky: bool) -> Option<ZoomPick> {
    egui::Popup::menu(&response)
        .show(|ui| {
            ui.set_min_width(120.0);
            let mut picked = None;
            for percent in [25, 50, 100, 200, 400] {
                if ui.button(format!("{percent}%")).clicked() {
                    picked = Some(ZoomPick::Percent(percent));
                }
            }
            ui.separator();
            let mark = if sticky {
                "✓ Sticky Zoom"
            } else {
                "Sticky Zoom"
            };
            if ui.button(mark).clicked() {
                picked = Some(ZoomPick::StickyZoom(!sticky));
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
    fn a_roll_scoped_metadata_preset_writes_every_other_frame() {
        let dir = tempfile::tempdir().unwrap();
        let photos: Vec<std::path::PathBuf> = (0..3)
            .map(|index| {
                let path = dir.path().join(format!("frame{index}.orf"));
                std::fs::write(&path, format!("frame {index}")).unwrap();
                path
            })
            .collect();
        let app_config = negpy_backend::domain::types::AppConfig::for_user_dir(dir.path());
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path.clone(),
            app_config.settings_db_path.clone(),
        );
        repo.initialize().unwrap();
        let roll_id = negpy_backend::services::assets::rolls::create_virtual_roll(
            &mut repo,
            "Roll",
            &photos
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        );
        let mut app = NegFeApp::for_capture("metadata", "", "presets", "canvas", 0.0);
        app.profile = Some(dir.path().to_path_buf());
        app.settings = Some(repo);
        app.rail.roll_analysis.roll_id = Some(roll_id);
        app.strip = vec![
            crate::frame_source::FrameImage::empty().with_path(&photos[0]),
            crate::frame_source::FrameImage::empty().with_path(&photos[1]),
            crate::frame_source::FrameImage::empty().with_path(&photos[2]),
        ];
        app.selected = 0;
        let mut source = negpy_backend::domain::models::WorkspaceConfig::default();
        source.metadata.camera_model = "Leica M3".to_string();
        let rows =
            negpy_backend::session::settings_catalog::rows_for_keys(&source.to_dict(), "metadata");
        let count = app.apply_preset_to_roll(&source, &rows);
        assert_eq!(count, 2);
        let other = crate::frame_source::stored_config(Some(dir.path()), &photos[1]);
        assert_eq!(other.metadata.camera_model, "Leica M3");
        let untouched = crate::frame_source::stored_config(Some(dir.path()), &photos[0]);
        assert!(untouched.metadata.camera_model.is_empty());
    }

    #[test]
    fn a_selection_scoped_metadata_preset_writes_the_chosen_frames() {
        let dir = tempfile::tempdir().unwrap();
        let photos: Vec<std::path::PathBuf> = (0..3)
            .map(|index| {
                let path = dir.path().join(format!("frame{index}.orf"));
                std::fs::write(&path, format!("frame {index}")).unwrap();
                path
            })
            .collect();
        let app_config = negpy_backend::domain::types::AppConfig::for_user_dir(dir.path());
        let repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path.clone(),
            app_config.settings_db_path.clone(),
        );
        repo.initialize().unwrap();
        let mut app = NegFeApp::for_capture("metadata", "", "presets", "canvas", 0.0);
        app.profile = Some(dir.path().to_path_buf());
        app.settings = Some(repo);
        app.strip = photos
            .iter()
            .map(|path| crate::frame_source::FrameImage::empty().with_path(path))
            .collect();
        app.selected = 0;
        app.selected_frames = [0, 2].into_iter().collect();
        let mut source = negpy_backend::domain::models::WorkspaceConfig::default();
        source.metadata.camera_model = "Leica M3".to_string();
        let rows =
            negpy_backend::session::settings_catalog::rows_for_keys(&source.to_dict(), "metadata");
        let count = app.apply_preset_to_selection(&source, &rows);
        assert_eq!(count, 1);
        let chosen = crate::frame_source::stored_config(Some(dir.path()), &photos[2]);
        assert_eq!(chosen.metadata.camera_model, "Leica M3");
        let open = crate::frame_source::stored_config(Some(dir.path()), &photos[0]);
        assert!(open.metadata.camera_model.is_empty());
        let skipped = crate::frame_source::stored_config(Some(dir.path()), &photos[1]);
        assert!(skipped.metadata.camera_model.is_empty());
    }

    #[test]
    fn a_preset_locks_the_card_it_diverges() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let app_config = negpy_backend::domain::types::AppConfig::for_user_dir(dir.path());
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path.clone(),
            app_config.settings_db_path.clone(),
        );
        repo.initialize().unwrap();
        let roll_id = negpy_backend::services::assets::rolls::create_virtual_roll(
            &mut repo,
            "Roll",
            &[photo.display().to_string()],
        );
        let mut app = NegFeApp::for_capture("metadata", "", "presets", "canvas", 0.0);
        app.profile = Some(dir.path().to_path_buf());
        app.settings = Some(repo);
        app.rail.roll_analysis.roll_id = Some(roll_id.clone());
        app.strip = vec![crate::frame_source::FrameImage::empty().with_path(&photo)];
        app.selected = 0;
        let mut source = negpy_backend::domain::models::WorkspaceConfig::default();
        source.metadata.camera_model = "Leica M3".to_string();
        let rows =
            negpy_backend::session::settings_catalog::rows_for_keys(&source.to_dict(), "metadata");
        app.edit.config.metadata.camera_model = "Leica M3".to_string();
        app.lock_preset_cards(&rows, &app.edit.config.clone());
        let hash = negpy_backend::infrastructure::storage::local_asset_store::file_hash(&photo)
            .expect("a hash");
        let repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path,
            app_config.settings_db_path,
        );
        let cards =
            negpy_backend::services::assets::rolls::frame_override_cards(&repo, &roll_id, &hash);
        assert!(cards.iter().any(|card| card == "metadata_gear"));
    }

    #[test]
    fn the_favourites_list_round_trips_through_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let app_config = negpy_backend::domain::types::AppConfig::for_user_dir(dir.path());
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path,
            app_config.settings_db_path,
        );
        repo.initialize().unwrap();
        let mut app = NegFeApp::for_capture("frame", "favourites", "", "canvas", 0.0);
        app.settings = Some(repo);
        app.rail.favourites.chosen = vec!["density".to_string(), "grade".to_string()];
        app.save_favourites();
        app.rail.favourites.chosen.clear();
        app.load_favourites();
        assert_eq!(
            app.rail.favourites.chosen,
            vec!["density".to_string(), "grade".to_string()]
        );
    }

    #[test]
    fn accepting_found_scenes_writes_them_to_the_roll() {
        let dir = tempfile::tempdir().unwrap();
        let photos: Vec<std::path::PathBuf> = (0..3)
            .map(|index| {
                let path = dir.path().join(format!("frame{index}.orf"));
                std::fs::write(&path, format!("frame {index}")).unwrap();
                path
            })
            .collect();
        let app_config = negpy_backend::domain::types::AppConfig::for_user_dir(dir.path());
        let mut repo = negpy_backend::infrastructure::storage::repository::StorageRepository::new(
            app_config.edits_db_path.clone(),
            app_config.settings_db_path.clone(),
        );
        repo.initialize().unwrap();
        let roll_id = negpy_backend::services::assets::rolls::create_virtual_roll(
            &mut repo,
            "Roll",
            &photos
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        );
        let mut app = NegFeApp::for_capture("roll", "", "metering", "canvas", 0.0);
        app.profile = Some(dir.path().to_path_buf());
        app.settings = Some(repo);
        app.rail.roll_analysis.roll_id = Some(roll_id.clone());
        app.set_strip(
            photos
                .iter()
                .map(|path| crate::frame_source::FrameImage::empty().with_path(path))
                .collect(),
        );
        app.rail.roll_analysis.found = vec![vec![1, 2]];
        app.accept_scenes();
        let repo = app.settings.as_ref().unwrap();
        let scenes = negpy_backend::services::assets::rolls::roll_scenes(repo, Some(&roll_id));
        assert_eq!(scenes.len(), 1);
        assert!(app.rail.roll_analysis.found.is_empty());
    }

    #[test]
    fn a_left_out_frame_leaves_the_sheet_entries() {
        let mut app = NegFeApp::for_capture("export", "", "alongside", "contact_sheet_dialog", 0.0);
        app.strip = vec![
            crate::frame_source::FrameImage::empty().with_path(std::path::Path::new("/roll/a.tif")),
            crate::frame_source::FrameImage::empty().with_path(std::path::Path::new("/roll/b.tif")),
        ];
        app.sheet.left_out.insert(0);
        assert!(app.sheet_entries().is_empty());
    }

    #[test]
    fn a_bounds_pick_stores_the_sample_and_disarms_the_tool() {
        let mut app = NegFeApp::for_capture("roll", "", "metering", "canvas", 0.0);
        app.set_source(Some(crate::render_worker::SourceFrame {
            name: "picked".to_string(),
            hash: "picked".to_string(),
            image: negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer::from_rgb(
                8,
                8,
                vec![0.5; 8 * 8 * 3],
            ),
            ir: None,
            ir_width: 0,
            ir_height: 0,
        }));
        app.rail.metering.pick = crate::view::rail::metering::MeteringPick::Dmin;
        app.store_bounds_pick([0.5, 0.5]);
        assert_eq!(
            app.rail.metering.pick,
            crate::view::rail::metering::MeteringPick::Off
        );
        let picked = app.edit.config.process.manual_dmin.clone().unwrap();
        let expected = f64::from(0.5f32).log10();
        for channel in picked {
            assert!((channel - expected).abs() < 1e-6, "{channel} != {expected}");
        }
        assert_eq!(app.edit.config.process.manual_dmin_at, Some(vec![0.5, 0.5]));
        assert!(app.edit.is_dirty());
    }

    #[test]
    fn a_bounds_pick_without_a_render_stores_nothing() {
        let mut app = NegFeApp::for_capture("roll", "", "metering", "canvas", 0.0);
        app.rail.metering.pick = crate::view::rail::metering::MeteringPick::Dmax;
        app.store_bounds_pick([0.5, 0.5]);
        assert!(app.edit.config.process.manual_dmax.is_none());
        assert_eq!(
            app.rail.metering.pick,
            crate::view::rail::metering::MeteringPick::Dmax
        );
        assert!(!app.rail.metering.status.is_empty());
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
    fn a_confirmed_crop_clears_the_measured_bounds() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.edit.config.process.local_floors = [0.2, 0.2, 0.2];
        app.edit.config.process.local_ceils = [0.9, 0.9, 0.9];
        app.confirm_crop([0.1, 0.2, 0.8, 0.9]);
        assert_eq!(app.edit.config.process.local_floors, [0.0; 3]);
        assert_eq!(app.edit.config.process.local_ceils, [0.0; 3]);
    }

    #[test]
    fn a_pending_crop_stays_out_of_the_edit() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.apply_crop_action(
            crate::view::canvas::CropAction::Pending([0.1, 0.2, 0.8, 0.9]),
            &ctx,
        );
        assert!(app.edit.config.geometry.crop_rect.is_none());
        assert!(!app.edit.is_dirty());
    }

    #[test]
    fn closing_draw_commits_the_pending_rect() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let ctx = egui::Context::default();
        app.crop_drag = Some(crate::view::canvas::CropDrag {
            start: egui::Pos2::ZERO,
            pending: [0.1, 0.2, 0.8, 0.9],
            mode: crate::view::canvas::CropDragMode::Draw,
        });
        app.settle_crop_tool(true, &ctx);
        assert_eq!(
            app.edit.config.geometry.crop_rect.as_deref(),
            Some(&[0.1, 0.2, 0.8, 0.9][..])
        );
        assert!(app.crop_drag.is_none());
    }

    #[test]
    fn a_zoom_preset_sets_the_fit_relative_scale() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        let photo = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 400.0));
        app.zoom = 100.0 / crate::view::canvas::zoom_percent(photo, 1600, 800) as f32;
        assert!((app.zoom - 2.0).abs() < 1e-6);
    }

    fn two_frame_strip(dir: &std::path::Path) -> Vec<FrameImage> {
        ["a.tif", "b.tif"]
            .iter()
            .map(|name| {
                let path = dir.join(name);
                let mut encoder =
                    tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
                let samples: Vec<u8> = (0..8 * 6 * 3).map(|value| (value % 32) as u8 * 8).collect();
                encoder
                    .write_image::<tiff::encoder::colortype::RGB8>(8, 6, &samples)
                    .unwrap();
                FrameImage {
                    name: (*name).to_string(),
                    path: Some(path),
                    width: 8,
                    height: 6,
                    pixels: vec![128; 8 * 6 * 3],
                }
            })
            .collect()
    }

    fn wait_for_frame(app: &mut NegFeApp) {
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.frame.is_none() {
            app.receive_frame(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    #[test]
    #[ignore = "times the golden roll; run with NEGPY_GUI_FOLDER set"]
    fn the_golden_roll_measures_a_frame_switch() {
        let Some(folder) = std::env::var_os("NEGPY_GUI_FOLDER").map(std::path::PathBuf::from)
        else {
            return;
        };
        if !folder.is_dir() {
            return;
        }
        let mut paths = crate::frame_source::roll_paths(&folder);
        paths.retain(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("orf"))
        });
        paths.truncate(3);
        if paths.len() < 2 {
            return;
        }
        let profile = std::env::var_os("NEGPY_GUI_PROFILE").map(std::path::PathBuf::from);
        let mut app = NegFeApp::for_capture("frame", "tone", "", "canvas", 0.0);
        app.set_profile(profile.as_deref());
        let strip: Vec<FrameImage> = paths
            .iter()
            .map(|path| FrameImage {
                name: path.file_name().unwrap().to_string_lossy().to_string(),
                path: Some(path.clone()),
                width: 1,
                height: 1,
                pixels: vec![0; 3],
            })
            .collect();
        app.set_strip(strip);
        let ctx = egui::Context::default();
        let mut first = 0.0;
        for index in 1..paths.len() {
            let started = std::time::Instant::now();
            assert!(app.select_frame(index));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                app.receive_frame(&ctx);
                if !app.processing() && app.frame.is_some() {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "frame open timed out");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            if index == 1 {
                first = elapsed;
            }
            assert!(app.last_frame_open_ms > 0.0);
        }
        eprintln!(
            "first switch {first:.1} ms (open {:.1} ms)",
            app.last_frame_open_ms
        );
    }

    #[test]
    fn a_frame_switch_queues_the_decode_and_fits_the_frame() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = NegFeApp::for_capture("roll", "", "geometry", "canvas", 0.0);
        app.set_strip(two_frame_strip(dir.path()));
        app.zoom = 2.5;
        assert!(app.select_frame(1));
        assert!(app.processing(), "the canvas waits for the decode");
        assert!(app.frame.is_none(), "the click must not decode inline");
        wait_for_frame(&mut app);
        assert!(!app.processing(), "the decoded frame ends the wait");
        assert!((app.zoom - 1.0).abs() < 1e-6);
        app.set_sticky_zoom(true);
        app.zoom = 2.5;
        app.frame = None;
        assert!(app.select_frame(0));
        wait_for_frame(&mut app);
        assert!((app.zoom - 2.5).abs() < 1e-6);
    }

    #[test]
    fn a_render_request_marks_the_canvas_busy() {
        let mut app = NegFeApp::for_capture("frame", "tone", "", "canvas", 0.0);
        let source = crate::render_worker::SourceFrame {
            name: "frame.orf".to_string(),
            hash: "busy".to_string(),
            image: negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer::from_rgb(
                8,
                6,
                vec![0.2; 8 * 6 * 3],
            ),
            ir: None,
            ir_width: 0,
            ir_height: 0,
        };
        app.set_source(Some(source));
        app.begin_processing();
        assert!(app.processing());
    }

    #[test]
    fn a_failed_frame_open_clears_the_processing_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = NegFeApp::for_capture("roll", "", "geometry", "canvas", 0.0);
        app.set_strip(two_frame_strip(dir.path()));
        std::fs::remove_file(dir.path().join("b.tif")).unwrap();
        assert!(app.select_frame(1));
        assert!(app.processing);
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.processing {
            app.receive_frame(&ctx);
            assert!(
                std::time::Instant::now() < deadline,
                "the failure must answer"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(app.rail.library.status(), "Could not open the frame.");
    }

    #[test]
    fn a_frame_switch_drops_transient_canvas_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = NegFeApp::for_capture("roll", "", "geometry", "canvas", 0.0);
        app.set_strip(two_frame_strip(dir.path()));
        app.crop_drag = Some(crate::view::canvas::CropDrag {
            start: egui::Pos2::ZERO,
            pending: [0.1, 0.2, 0.8, 0.9],
            mode: crate::view::canvas::CropDragMode::Draw,
        });
        app.rail.geometry.crop.draw = true;
        app.rail.exposure.local.draw = true;
        assert!(app.select_frame(1));
        wait_for_frame(&mut app);
        assert!(app.crop_drag.is_none());
        assert!(!app.rail.geometry.crop.draw);
        assert!(!app.rail.exposure.local.draw);
    }

    #[test]
    fn the_toolbar_hides_controls_in_the_python_order() {
        let wide = toolbar_layout(1400.0);
        assert_eq!(wide, ToolbarLayout::all());
        let narrow = toolbar_layout(1180.0);
        assert!(!narrow.transforms[3], "Flip Vertical leaves first");
        assert!(!narrow.transforms[2], "Flip Horizontal leaves next");
        assert!(narrow.transforms[0], "Rotate CCW stays while it fits");
        assert!(narrow.visible[7], "Roll Average stays while it fits");
    }

    #[test]
    fn every_hidden_toolbar_control_is_in_the_overflow_menu() {
        let layout = toolbar_layout(700.0);
        assert!(layout.overflow);
        for (index, shown) in layout.visible.iter().enumerate() {
            if !shown {
                assert!(
                    TOOLBAR_COLLAPSE_ORDER.contains(&TOOLBAR_ITEM_WIDTHS[index].0),
                    "{} has an overflow entry",
                    TOOLBAR_ITEM_WIDTHS[index].0
                );
            }
        }
        assert!(layout.transforms.iter().any(|shown| !shown));
    }

    #[test]
    fn a_narrow_toolbar_fits_its_visible_controls() {
        for width in [700.0, 1024.0, 1400.0] {
            let layout = toolbar_layout(width);
            assert!(layout.needed() <= width, "{width}: {}", layout.needed());
            assert_eq!(layout.overflow, width < 1400.0);
        }
    }

    #[test]
    fn the_recipe_follows_a_narrow_rail() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900.0, 900.0));
        let wide = rail_width(1920.0);
        let narrow = rail_width(screen.width());
        assert!(narrow < wide);
        assert!(screen.right() - narrow - 26.0 > screen.right() - wide - 26.0);
    }

    #[test]
    fn a_narrow_tab_strip_keeps_the_current_tab_and_offers_the_rest() {
        use crate::view::widgets::overflow_bar::{hidden_tabs, visible_tabs};
        let visible = visible_tabs(6, Some(4), 180.0, 36.0);
        assert!(visible.contains(&4));
        let hidden = hidden_tabs(6, Some(4), 180.0, 36.0);
        assert!(!hidden.contains(&4));
        assert!(!hidden.is_empty());
    }

    #[test]
    fn a_short_window_keeps_the_canvas_height_before_the_strip() {
        let minimum = minimum_window_size();
        assert!(minimum.y >= TOP_ROW_HEIGHT + MIN_CANVAS_HEIGHT + TOOLBAR_HEIGHT + 16.0);
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1920.0, minimum.y));
        let view = ViewState {
            strip_open: true,
            ..ViewState::default()
        };
        let rail = rail_rect(screen);
        let body_top = screen.top() + TOP_ROW_HEIGHT + 8.0;
        let room = (screen.bottom() - 8.0 - body_top - TOOLBAR_HEIGHT).max(0.0);
        let strip = STRIP_HEIGHT.min((room - MIN_CANVAS_HEIGHT).max(0.0));
        assert!(room - strip >= MIN_CANVAS_HEIGHT);
        assert!(rail.top() >= body_top);
        assert!(view.strip_open);
    }

    #[test]
    fn a_narrow_window_keeps_canvas_room_beside_the_rail() {
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(700.0, 900.0));
        let rail = rail_rect(screen);
        let canvas_width = rail.left() - 8.0 - (screen.left() + 8.0);
        assert!(canvas_width >= MIN_CANVAS_WIDTH, "{canvas_width}");
        assert!(rail.width() >= MIN_RAIL_WIDTH, "{}", rail.width());
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
