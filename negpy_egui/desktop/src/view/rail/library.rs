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
use crate::view::widgets::sort_button::{SortButton, SortChange, SortOrder};

const ROLL_ROW: f32 = 46.0;

/// One Roll row, as the saved store reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollRow {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub sort_date: i64,
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
            sort_date: 0,
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
    pub sort: SortButton,
    pub open: bool,
    profile: Option<PathBuf>,
    status: String,
    imported: Option<String>,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            rolls: Vec::new(),
            filter: String::new(),
            sort: library_sort("name", false),
            open: true,
            profile: None,
            status: String::new(),
            imported: None,
        }
    }
}

impl LibraryState {
    /// Run the folder picker and import the chosen folder. Returns whether a roll was
    /// imported; the caller then opens the roll it names.
    pub fn import_with_picker(&mut self, subfolders: bool) -> bool {
        let Some(parent) = rfd::FileDialog::new()
            .set_title(if subfolders {
                "Import Subfolders as Rolls"
            } else {
                "Import Folder as a Roll"
            })
            .pick_folder()
        else {
            return false;
        };
        self.import(&parent, subfolders)
    }

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
        let mut rows: Vec<RollRow> = rolls::all_rolls_sorted(&repo)
            .into_iter()
            .map(|(roll_id, entry)| {
                let frames = roll_frame_count(&entry, &composites);
                let mut row = RollRow::new(roll_name(&entry), roll_detail(&entry, frames, &gear))
                    .with_id(roll_id);
                row.sort_date = entry
                    .get("created_at")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as i64;
                row
            })
            .collect();
        let (order, descending) = saved_sort(&repo);
        sort_rows(&mut rows, order, descending);
        Self {
            rolls: rows,
            filter: String::new(),
            sort: library_sort(order, descending),
            open: true,
            profile: Some(profile.to_path_buf()),
            status: String::new(),
            imported: None,
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
            sort: library_sort("name", false),
            open: true,
            profile: None,
            status: String::new(),
            imported: None,
        }
    }

    /// The message the last import left.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Order the roll list and save the choice, as Python's Library sort does.
    pub fn set_sort(&mut self, order: &'static str, descending: bool) {
        self.sort.show_order(order, descending);
        sort_rows(&mut self.rolls, order, descending);
        let Some(profile) = &self.profile else {
            return;
        };
        let app = AppConfig::for_user_dir(profile);
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        if repo.initialize().is_err() {
            return;
        }
        let mut settings = serde_json::Map::new();
        settings.insert("library_sort_order".to_string(), serde_json::json!(order));
        settings.insert(
            "library_sort_descending".to_string(),
            serde_json::json!(descending),
        );
        let _ = repo.save_global_settings(&settings);
    }

    /// Import one folder as a roll, or its subfolders as separate rolls.
    pub fn import(&mut self, parent: &Path, subfolders: bool) -> bool {
        let Some(profile) = self.profile.clone() else {
            self.status = "Open a profile first.".to_string();
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
        let imported = ids.first().cloned();
        *self = Self {
            filter,
            status: format!("Imported {count}."),
            imported,
            ..Self::load(Some(&profile))
        };
        true
    }

    /// Take the roll a click or an import asked to open.
    pub fn take_open_request(&mut self) -> Option<String> {
        self.imported.take()
    }

    /// Rediscover rolls under every import source and re-read the row counts.
    pub fn refresh(&mut self) -> bool {
        let Some(profile) = self.profile.clone() else {
            return false;
        };
        let app = AppConfig::for_user_dir(&profile);
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        if repo.initialize().is_err() {
            self.status = "Could not open the library.".to_string();
            return false;
        }
        let dropped = rolls::prune_filtered_rolls(&mut repo);
        let before = rolls::all_rolls_sorted(&repo).len();
        for source in rolls::import_sources(&repo) {
            rolls::import_subfolders_as_rolls(&mut repo, &source, true);
        }
        let found = rolls::all_rolls_sorted(&repo).len().saturating_sub(before);
        let status = match (found, dropped) {
            (0, 0) => String::new(),
            (found, 0) => format!("Found {}.", count_of(found as i64, "new roll", "new rolls")),
            (0, dropped) => format!(
                "Removed {}.",
                count_of(dropped as i64, "filtered roll", "filtered rolls")
            ),
            (found, dropped) => format!(
                "Found {}, removed {}.",
                count_of(found as i64, "new roll", "new rolls"),
                count_of(dropped as i64, "filtered roll", "filtered rolls")
            ),
        };
        let filter = std::mem::take(&mut self.filter);
        *self = Self {
            filter,
            status,
            imported: None,
            ..Self::load(Some(&profile))
        };
        found > 0 || dropped > 0
    }

    /// Paint the tab and return the id of the Roll the user opened or imported.
    pub fn show(&mut self, ui: &mut Ui, footer_y: Option<f32>) -> Option<String> {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Library").with_guide();
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| {
            card(ui, |ui| {
                let mut import = None;
                let mut more = None;
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
                    if let Some(change) = self.sort.show(ui, "Sort the roll list") {
                        let (order, descending) = match change {
                            SortChange::Order(order) => (order, self.sort.descending),
                            SortChange::Direction(descending) => (self.sort.order, descending),
                        };
                        self.set_sort(order, descending);
                    }
                    ui.add_space(f32::from(theme.space_sm));
                    import = Some(small_button(ui, "Import", 76.0, &theme));
                    ui.add_space(f32::from(theme.space_sm));
                    more = Some(small_button(
                        ui,
                        Icon::Menu.glyph(),
                        TOOLBAR_BUTTON_HEIGHT,
                        &theme,
                    ));
                });
                let picked = import.as_ref().and_then(import_menu);
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
                if more.as_ref().is_some_and(action_menu) {
                    self.refresh();
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
                    if response.response.clicked() && !row.id.is_empty() {
                        opened = Some(row.id.clone());
                    }
                }
                opened
            })
        });
        self.open = section.expanded;
        let opened = result.flatten();
        if opened.is_some() {
            self.imported = opened.clone();
        }
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

fn library_sort(order: &'static str, descending: bool) -> SortButton {
    let mut sort = SortButton::new(vec![
        SortOrder::new("name", "Name"),
        SortOrder::new("date", "Date"),
    ]);
    sort.show_order(order, descending);
    sort
}

fn saved_sort(repo: &StorageRepository) -> (&'static str, bool) {
    let value = |key: &str| repo.global_setting(key).ok().flatten();
    let order = match value("library_sort_order")
        .and_then(|value| value.as_str().map(str::to_string))
        .as_deref()
    {
        Some("date") => "date",
        Some(_) => "name",
        None => match value("file_sort_order")
            .and_then(|value| value.as_str().map(str::to_string))
            .as_deref()
        {
            Some("date") => "date",
            _ => "name",
        },
    };
    let descending = value("library_sort_descending")
        .or_else(|| value("file_sort_descending"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    (order, descending)
}

fn sort_rows(rows: &mut [RollRow], order: &str, descending: bool) {
    if order == "date" {
        rows.sort_by_key(|row| row.sort_date);
    } else {
        rows.sort_by_key(|row| row.name.to_lowercase());
    }
    if descending {
        rows.reverse();
    }
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

fn action_menu(more: &egui::Response) -> bool {
    egui::Popup::menu(more)
        .show(|ui| {
            ui.set_min_width(180.0);
            ui.button("Refresh").clicked()
        })
        .is_some_and(|response| response.inner)
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
    fn an_import_without_a_profile_reports_the_missing_store() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = LibraryState::load(None);
        assert!(!state.import(dir.path(), false));
        assert_eq!(state.status(), "Open a profile first.");
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
    fn the_name_sort_orders_rows_and_saves_the_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = LibraryState::load(Some(dir.path()));
        state.rolls = vec![
            RollRow::new("w10", "1 photo"),
            RollRow::new("w2", "1 photo"),
        ];
        state.set_sort("name", false);
        assert_eq!(state.rolls[0].name, "w10");
        let app = AppConfig::for_user_dir(dir.path());
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        assert_eq!(
            repo.global_setting("library_sort_order").unwrap(),
            Some(serde_json::json!("name"))
        );
        assert_eq!(
            repo.global_setting("library_sort_descending").unwrap(),
            Some(serde_json::json!(false))
        );
    }

    #[test]
    fn the_date_sort_puts_the_newest_roll_first_when_descending() {
        let mut state = LibraryState::reference();
        state.rolls[0].sort_date = 10;
        state.rolls[1].sort_date = 20;
        state.set_sort("date", true);
        assert_eq!(state.rolls[0].sort_date, 20);
        assert_eq!(state.rolls[1].sort_date, 10);
    }

    #[test]
    fn a_refresh_finds_a_roll_added_under_an_import_source() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("photos");
        let first = parent.join("w1");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::write(first.join("frame.tif"), b"image").unwrap();
        let mut state = LibraryState::load(Some(dir.path()));
        assert!(state.import(&first, false));
        let second = parent.join("w2");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(second.join("frame.tif"), b"image").unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        let mut sources = serde_json::Map::new();
        sources.insert(
            "roll_import_sources".to_string(),
            serde_json::json!([parent.display().to_string()]),
        );
        repo.save_global_settings(&sources).unwrap();
        assert!(state.refresh());
        assert_eq!(state.rolls.len(), 2);
        assert_eq!(state.status(), "Found 1 new roll.");
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
