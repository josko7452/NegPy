//! The Library tab: the Roll tree over its filter and toolbar.
//!
//! A Roll is the Library's only unit; a folder is an import source. The panel shows the
//! saved rolls for the open profile, newest state first.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, FontId, Sense, Stroke, Ui, Vec2};
use negpy_backend::domain::types::AppConfig;
use negpy_backend::features::metadata::gear::GearLibrary;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::kernel::system::paths::resource_path;
use negpy_backend::kernel::system::text::{count_of, short_date};
use negpy_backend::services::assets::composites::{frame_count, saved_composites};
use negpy_backend::services::assets::library::folder_image_paths;
use negpy_backend::services::assets::rolls;

use crate::styles::icons::Icon;
use crate::styles::layout::TOOLBAR_BUTTON_HEIGHT;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::flat_rows::{FlatRow, paint_pill};

const ROLL_ROW: f32 = 46.0;

/// One Roll row, as the saved store reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollRow {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub mode: Option<egui::Color32>,
    pub open: bool,
}

impl RollRow {
    /// A row with the count and date the Library prints under the name.
    #[must_use]
    pub fn new(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            name: name.into(),
            detail: detail.into(),
            mode: None,
            open: false,
        }
    }

    /// Give the row the roll id the store reports.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Mark the roll's film mode with the dot the Library prints.
    #[must_use]
    pub fn with_mode(mut self, color: egui::Color32) -> Self {
        self.mode = Some(color);
        self
    }
}

/// The Library tab's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryState {
    pub rolls: Vec<RollRow>,
    pub filter: String,
    profile: Option<PathBuf>,
    status: String,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            rolls: Vec::new(),
            filter: String::new(),
            profile: None,
            status: String::new(),
        }
    }
}

impl LibraryState {
    /// The saved rolls of `profile`, in name order.
    #[must_use]
    pub fn load(profile: Option<&Path>) -> Self {
        let Some(profile) = profile else {
            return Self::default();
        };
        let app = AppConfig::for_user_dir(profile);
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        let gear = GearLibrary::load(Path::new(&app.gear_dir), &resource_path("gear"));
        let composites = saved_composites(&mut repo);
        let rows = rolls::all_rolls_sorted(&repo)
            .into_iter()
            .map(|(roll_id, entry)| {
                let frames = roll_frame_count(&entry, &composites);
                RollRow::new(roll_name(&entry), roll_detail(&entry, frames, &gear)).with_id(roll_id)
            })
            .collect();
        Self {
            rolls: rows,
            filter: String::new(),
            profile: Some(profile.to_path_buf()),
            status: String::new(),
        }
    }

    /// The rolls the reference profile shows.
    #[must_use]
    pub fn reference() -> Self {
        let mut rows = Vec::new();
        for (name, detail, mode) in [
            ("w1", "37 photos · 12 Nov", None),
            ("w10", "13 photos · 12 Nov", None),
            ("w2", "37 photos · 12 Nov", None),
            ("w3", "37 photos · 12 Nov", None),
            ("w4", "39 photos · 12 Nov", None),
            ("w5", "37 photos · 12 Nov", None),
            ("w6", "13 photos · 11 Nov", None),
            ("w7", "13 photos · 12 Nov", None),
            ("w8", "13 photos · 12 Nov", None),
            (
                "w9",
                "13 photos · Color · 12 Nov",
                Some(Theme::default().mode_c41),
            ),
        ] {
            let mut row = RollRow::new(name, detail);
            row.mode = mode;
            rows.push(row);
        }
        if let Some(open) = rows.last_mut() {
            open.open = true;
        }
        Self {
            rolls: rows,
            filter: String::new(),
            profile: None,
            status: String::new(),
        }
    }

    /// The message the last import left.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Import one folder as a roll, or its subfolders as separate rolls.
    pub fn import(&mut self, parent: &Path, subfolders: bool) -> bool {
        let Some(profile) = self.profile.clone() else {
            return false;
        };
        let app = AppConfig::for_user_dir(&profile);
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        if repo.initialize().is_err() {
            self.status = "Could not open the library.".to_string();
            return false;
        }
        let parent = parent.display().to_string();
        let ids = if subfolders {
            rolls::import_subfolders_as_rolls(&mut repo, &parent, false)
        } else {
            vec![rolls::recognize_folder(&mut repo, &parent, None)]
        };
        if ids.is_empty() {
            self.status = "No folders with images found.".to_string();
            return false;
        }
        let mut settings = serde_json::Map::new();
        settings.insert("last_open_folder".to_string(), serde_json::json!(parent));
        let _ = repo.save_global_settings(&settings);
        let count =
            negpy_backend::kernel::system::text::count_of(ids.len() as i64, "roll", "rolls");
        let filter = std::mem::take(&mut self.filter);
        *self = Self {
            filter,
            status: format!("Imported {count}."),
            ..Self::load(Some(&profile))
        };
        true
    }

    /// Paint the tab and return the Roll the user opened.
    pub fn show(&mut self, ui: &mut Ui, footer_y: Option<f32>) -> Option<String> {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Library").with_guide();
        section.expanded = true;
        let (_, result) = section.show(ui, |ui| {
            card(ui, |ui| {
                let mut import = None;
                ui.horizontal(|ui| {
                    let height = TOOLBAR_BUTTON_HEIGHT;
                    let width = (ui.available_width() - 210.0).max(100.0);
                    ui.add_sized(
                        Vec2::new(width, height),
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter rolls...")
                            .margin(Vec2::new(f32::from(theme.space_xl), 8.0)),
                    );
                    ui.add_space(f32::from(theme.space_sm));
                    paint_small_button(ui, "Name", 66.0, &theme);
                    ui.add_space(f32::from(theme.space_sm));
                    import = Some(small_button(ui, "Import", 76.0, &theme));
                    ui.add_space(f32::from(theme.space_sm));
                    paint_small_button(ui, Icon::Menu.glyph(), TOOLBAR_BUTTON_HEIGHT, &theme);
                });
                let picked = import.and_then(|import| import_menu(&import));
                if let Some(subfolders) = picked
                    && let Some(parent) = rfd::FileDialog::new()
                        .set_title(if subfolders {
                            "Import Subfolders as Rolls"
                        } else {
                            "Import Folder as a Roll"
                        })
                        .pick_folder()
                {
                    self.import(&parent, subfolders);
                }
                ui.add_space(f32::from(theme.space_sm));
                let needle = self.filter.trim().to_lowercase();
                let mut opened = None;
                for row in &self.rolls {
                    let visible = needle.is_empty()
                        || row.name.to_lowercase().contains(&needle)
                        || row.detail.to_lowercase().contains(&needle);
                    if !visible {
                        continue;
                    }
                    let mut flat =
                        FlatRow::new(row.name.clone(), row.detail.clone()).with_icon(Icon::Folder);
                    if let Some(mode) = row.mode {
                        flat = flat.with_dot(mode);
                    }
                    flat.current = row.open;
                    let response = flat.show(ui, ROLL_ROW);
                    if row.open {
                        let rect = response.response.rect;
                        let right = rect.right() - f32::from(theme.space_xl);
                        paint_pill(ui.painter(), right, rect.center().y, "Open", true);
                    }
                    if response.response.clicked() {
                        opened = Some(row.name.clone());
                    }
                }
                opened
            })
        });
        let opened = result.flatten();
        let needle = self.filter.trim().to_lowercase();
        let shown: Vec<&RollRow> = self
            .rolls
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.detail.to_lowercase().contains(&needle)
            })
            .collect();
        let total_frames: i64 = self.rolls.iter().map(|row| roll_photos(&row.detail)).sum();
        let shown_frames: i64 = shown.iter().map(|row| roll_photos(&row.detail)).sum();
        let tally = if needle.is_empty() {
            format!(
                "{} · {}",
                count_of(self.rolls.len() as i64, "roll", "rolls"),
                count_of(total_frames, "photo", "photos")
            )
        } else {
            format!(
                "{} of {} · {}",
                shown.len(),
                count_of(self.rolls.len() as i64, "roll", "rolls"),
                count_of(shown_frames, "photo", "photos")
            )
        };
        let hint_y = footer_y.map_or(ui.cursor().top() + f32::from(theme.space_xl), |y| y - 8.0);
        ui.painter().text(
            egui::pos2(ui.cursor().left(), hint_y),
            Align2::LEFT_CENTER,
            tally,
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        if !self.status.is_empty() {
            ui.painter().text(
                egui::pos2(ui.cursor().left(), hint_y + 18.0),
                Align2::LEFT_CENTER,
                &self.status,
                FontId::proportional(f32::from(theme.font_size_small)),
                theme.text_hint,
            );
        }
        opened
    }
}

fn roll_photos(detail: &str) -> i64 {
    detail
        .split_whitespace()
        .next()
        .and_then(|first| first.parse().ok())
        .unwrap_or(0)
}

fn roll_frame_count(
    entry: &serde_json::Value,
    composites: &serde_json::Map<String, serde_json::Value>,
) -> usize {
    let paths: Vec<String> =
        if entry.get("kind").and_then(serde_json::Value::as_str) == Some("folder") {
            let excluded: Vec<String> = entry
                .get("excluded_paths")
                .and_then(serde_json::Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let folder = entry
                .get("folder_path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            folder_image_paths(Path::new(folder))
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .filter(|path| !excluded.iter().any(|excluded| excluded == path))
                .collect()
        } else {
            entry
                .get("member_paths")
                .and_then(serde_json::Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
    frame_count(&paths, composites, &serde_json::Map::new())
}

fn roll_detail(entry: &serde_json::Value, frames: usize, gear: &GearLibrary) -> String {
    let defaults = entry.get("defaults").and_then(serde_json::Value::as_object);
    let mut parts = vec![count_of(frames as i64, "photo", "photos")];
    if let Some(mode) = defaults
        .and_then(|defaults| defaults.get("process_mode"))
        .and_then(serde_json::Value::as_str)
    {
        parts.push(match mode {
            "B&W Negative" | "B&W" => "B&W".to_string(),
            "Transparency" | "E-6" => "Slide".to_string(),
            _ => "Color".to_string(),
        });
    }
    if let Some(stock) = defaults
        .and_then(|defaults| defaults.get("film_stock_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.is_empty())
        .and_then(|id| gear.film_stock(id))
    {
        parts.push(stock.full_film_label());
    }
    if let Some(scenes) = entry.get("scenes").and_then(serde_json::Value::as_array)
        && !scenes.is_empty()
    {
        parts.push(count_of(scenes.len() as i64, "scene", "scenes"));
    }
    if let Some(created) = entry.get("created_at").and_then(serde_json::Value::as_f64) {
        parts.push(short_date(created));
    }
    parts.join(" · ")
}

fn roll_name(entry: &serde_json::Value) -> String {
    entry
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn small_button(ui: &mut Ui, label: &str, width: f32, theme: &Theme) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, TOOLBAR_BUTTON_HEIGHT), Sense::click());
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
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
    response
}

fn paint_small_button(ui: &mut Ui, label: &str, width: f32, theme: &Theme) {
    let _ = small_button(ui, label, width, theme);
}

fn import_menu(import: &egui::Response) -> Option<bool> {
    egui::Popup::menu(import)
        .show(|ui| {
            ui.set_min_width(230.0);
            let mut picked = None;
            if ui.button("Import Folder as a Roll...").clicked() {
                picked = Some(false);
            }
            if ui.button("Import Subfolders as Rolls...").clicked() {
                picked = Some(true);
            }
            picked
        })
        .map(|response| response.inner)
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_library_opens_one_roll() {
        let state = LibraryState::reference();
        assert_eq!(state.rolls.len(), 10);
        assert_eq!(state.rolls.iter().filter(|row| row.open).count(), 1);
        assert_eq!(state.rolls.last().unwrap().name, "w9");
    }

    #[test]
    fn the_library_reads_the_saved_rolls() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos/w1");
        std::fs::create_dir_all(&photos).unwrap();
        std::fs::write(photos.join("frame.tif"), b"image").unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().unwrap();
        let roll = rolls::recognize_folder(&mut repo, &photos.display().to_string(), Some("w1"));
        let mut store = serde_json::Map::new();
        store.insert(
            rolls::ROLLS_KEY.to_string(),
            serde_json::json!({
                roll.clone(): {
                    "kind": "folder",
                    "name": "w1",
                    "folder_path": photos.display().to_string(),
                    "created_at": 1_700_000_000.0,
                    "defaults": {"process_mode": "B&W Negative"}
                }
            }),
        );
        repo.save_global_settings(&store).unwrap();
        let state = LibraryState::load(Some(dir.path()));
        assert_eq!(state.rolls.len(), 1);
        assert_eq!(state.rolls[0].name, "w1");
        assert_eq!(state.rolls[0].id, roll);
        assert_eq!(
            state.rolls[0].detail,
            format!("1 photo · B&W · {}", short_date(1_700_000_000.0))
        );
    }

    #[test]
    fn a_profile_without_rolls_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(LibraryState::load(Some(dir.path())).rolls.is_empty());
        assert!(LibraryState::load(None).rolls.is_empty());
    }

    #[test]
    fn the_filter_keeps_only_matching_rows() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("w1");
        let second = dir.path().join("w2");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().unwrap();
        rolls::recognize_folder(&mut repo, &first.display().to_string(), Some("w1"));
        rolls::recognize_folder(&mut repo, &second.display().to_string(), Some("w2"));
        let mut state = LibraryState::load(Some(dir.path()));
        state.filter = "w2".to_string();
        let needle = state.filter.to_lowercase();
        let shown: Vec<&RollRow> = state
            .rolls
            .iter()
            .filter(|row| row.name.to_lowercase().contains(&needle))
            .collect();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].name, "w2");
    }

    #[test]
    fn an_imported_folder_becomes_a_roll_and_reloads_the_rows() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos/w1");
        std::fs::create_dir_all(&photos).unwrap();
        std::fs::write(photos.join("frame.tif"), b"image").unwrap();
        let mut state = LibraryState::load(Some(dir.path()));
        assert!(state.rolls.is_empty());
        assert!(state.import(&photos, false));
        assert_eq!(state.rolls.len(), 1);
        assert_eq!(state.rolls[0].name, "w1");
        assert_eq!(state.status(), "Imported 1 roll.");
    }

    #[test]
    fn a_subfolder_import_makes_a_roll_for_each_folder() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["w1", "w2"] {
            let photos = dir.path().join("photos").join(name);
            std::fs::create_dir_all(&photos).unwrap();
            std::fs::write(photos.join("frame.tif"), b"image").unwrap();
        }
        let mut state = LibraryState::load(Some(dir.path()));
        assert!(state.import(&dir.path().join("photos"), true));
        assert_eq!(state.rolls.len(), 2);
        assert_eq!(state.status(), "Imported 2 rolls.");
    }

    #[test]
    fn the_library_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = LibraryState::reference();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(state.show(ui, None), None);
            });
        });
    }
}
