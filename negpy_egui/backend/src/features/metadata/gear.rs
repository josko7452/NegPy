//! Gear records and the JSON files that hold them.
//!
//! Mirrors `negpy/features/metadata/gear_models.py`.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

const CAMERAS_FILE: &str = "cameras.json";
const LENSES_FILE: &str = "lenses.json";
const FILM_STOCKS_FILE: &str = "film_stocks.json";
const PROCESSES_FILE: &str = "processes.json";
const SCAN_SETUPS_FILE: &str = "scan_setups.json";

/// A camera body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Camera {
    pub id: String,
    pub make: String,
    pub model: String,
    pub display_name: String,
}

/// A lens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lens {
    pub id: String,
    pub lens_model: String,
    pub make: String,
    pub display_name: String,
    pub focal_length_mm: Option<f64>,
    pub max_aperture: Option<f64>,
}

/// A film stock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilmStock {
    pub id: String,
    pub manufacturer: String,
    pub stock_name: String,
    pub display_name: String,
    pub iso: i64,
    pub format: String,
    pub color_type: String,
}

/// A development recipe.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DevelopmentProcess {
    pub id: String,
    pub display_name: String,
    pub developer: String,
    pub dilution: String,
    pub push_pull: i64,
    pub time_seconds: Option<i64>,
    pub temperature_c: Option<f64>,
}

/// A digitizing setup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanSetup {
    pub id: String,
    pub display_name: String,
    pub scanning: String,
}

/// Every gear collection, merged from bundled and user files.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearLibrary {
    pub cameras: Vec<Camera>,
    pub lenses: Vec<Lens>,
    pub film_stocks: Vec<FilmStock>,
    pub processes: Vec<DevelopmentProcess>,
    pub scan_setups: Vec<ScanSetup>,
}

impl GearLibrary {
    #[must_use]
    pub fn camera(&self, id: &str) -> Option<&Camera> {
        self.cameras.iter().find(|item| item.id == id)
    }

    #[must_use]
    pub fn lens(&self, id: &str) -> Option<&Lens> {
        self.lenses.iter().find(|item| item.id == id)
    }

    #[must_use]
    pub fn film_stock(&self, id: &str) -> Option<&FilmStock> {
        self.film_stocks.iter().find(|item| item.id == id)
    }

    /// Read the bundled and user gear files into one library.
    #[must_use]
    pub fn load(gear_dir: &Path, bundled_dir: &Path) -> Self {
        let cameras = load_merged(CAMERAS_FILE, gear_dir, bundled_dir);
        let lenses = load_merged(LENSES_FILE, gear_dir, bundled_dir);
        let film_stocks = load_merged(FILM_STOCKS_FILE, gear_dir, bundled_dir);
        let processes = load_merged(PROCESSES_FILE, gear_dir, bundled_dir);
        let scan_setups = load_merged(SCAN_SETUPS_FILE, gear_dir, bundled_dir);
        Self {
            cameras: cameras.iter().map(camera_from).collect(),
            lenses: lenses.iter().map(lens_from).collect(),
            film_stocks: film_stocks.iter().map(film_stock_from).collect(),
            processes: processes.iter().map(process_from).collect(),
            scan_setups: scan_setups.iter().map(scan_setup_from).collect(),
        }
    }
}

impl Camera {
    #[must_use]
    pub fn resolved_display_name(&self) -> String {
        let display = self.display_name.trim();
        if display.is_empty() {
            format!("{} {}", self.make, self.model).trim().to_string()
        } else {
            display.to_string()
        }
    }
}

impl Lens {
    #[must_use]
    pub fn resolved_display_name(&self) -> String {
        let display = self.display_name.trim();
        if !display.is_empty() {
            display.to_string()
        } else if !self.lens_model.trim().is_empty() {
            self.lens_model.trim().to_string()
        } else {
            "Unnamed lens".to_string()
        }
    }
}

impl FilmStock {
    #[must_use]
    pub fn full_film_label(&self) -> String {
        if self.manufacturer.trim().is_empty() {
            self.stock_name.trim().to_string()
        } else {
            format!("{} {}", self.manufacturer.trim(), self.stock_name.trim())
                .trim()
                .to_string()
        }
    }
}

fn text(data: &Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| data.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn optional_number(data: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        data.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse().ok(),
            _ => None,
        })
    })
}

fn optional_int(data: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        data.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse().ok(),
            _ => None,
        })
    })
}

fn record(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

fn read_list(path: &Path) -> Vec<Map<String, Value>> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    items.iter().filter_map(record).cloned().collect()
}

fn merge(
    bundled: Vec<Map<String, Value>>,
    user: Vec<Map<String, Value>>,
) -> Vec<Map<String, Value>> {
    let ids: Vec<String> = bundled
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut merged = bundled;
    merged.extend(user.into_iter().filter(|item| {
        item.get("id")
            .and_then(Value::as_str)
            .is_none_or(|id| !ids.iter().any(|known| known == id))
    }));
    merged
}

fn load_merged(filename: &str, gear_dir: &Path, bundled_dir: &Path) -> Vec<Map<String, Value>> {
    merge(
        read_list(&bundled_dir.join(filename)),
        read_list(&gear_dir.join(filename)),
    )
}

fn camera_from(data: &Map<String, Value>) -> Camera {
    Camera {
        id: text(data, &["id"]),
        make: text(data, &["make"]),
        model: text(data, &["model"]),
        display_name: text(data, &["displayName", "display_name"]),
    }
}

fn lens_from(data: &Map<String, Value>) -> Lens {
    Lens {
        id: text(data, &["id"]),
        lens_model: text(data, &["lensModel", "lens_model"]),
        make: text(data, &["make"]),
        display_name: text(data, &["displayName", "display_name"]),
        focal_length_mm: optional_number(data, &["focalLength", "focal_length_mm"]),
        max_aperture: optional_number(data, &["maxAperture", "max_aperture"]),
    }
}

/// The legacy gear aliases `FilmFormat.from_storage` accepts.
fn format_from_storage(value: &str) -> String {
    match value {
        "Format35mm" => "35mm",
        "Format120" => "120",
        "Format110" => "110",
        "LargeFormat" => "4×5",
        "" => "35mm",
        known if ["35mm", "120", "110", "4×5", "8×10", "Other"].contains(&known) => known,
        _ => "Other",
    }
    .to_string()
}

/// The legacy gear aliases `FilmColorType.from_storage` accepts.
fn color_type_from_storage(value: &str) -> String {
    match value {
        "BlackAndWhiteNegative" => "B&W Negative",
        "BlackAndWhiteSlide" => "B&W Slide",
        "" => "ColorNegative",
        known
            if [
                "ColorNegative",
                "B&W Negative",
                "ColorSlide",
                "B&W Slide",
                "Other",
            ]
            .contains(&known) =>
        {
            known
        }
        _ => "Other",
    }
    .to_string()
}

fn film_stock_from(data: &Map<String, Value>) -> FilmStock {
    let format = text(data, &["format"]);
    let color_type = text(data, &["colorType", "color_type"]);
    FilmStock {
        id: text(data, &["id"]),
        manufacturer: text(data, &["manufacturer"]),
        stock_name: text(data, &["stockName", "stock_name"]),
        display_name: text(data, &["displayName", "display_name"]),
        iso: optional_int(data, &["iso"]).unwrap_or(100),
        format: format_from_storage(&format),
        color_type: color_type_from_storage(&color_type),
    }
}

fn process_from(data: &Map<String, Value>) -> DevelopmentProcess {
    DevelopmentProcess {
        id: text(data, &["id"]),
        display_name: text(data, &["displayName", "display_name"]),
        developer: text(data, &["developer"]),
        dilution: text(data, &["dilution"]),
        push_pull: optional_int(data, &["pushPull", "push_pull"]).unwrap_or(0),
        time_seconds: optional_int(data, &["timeSeconds", "time_seconds"]),
        temperature_c: optional_number(data, &["temperatureC", "temperature_c"]),
    }
}

fn scan_setup_from(data: &Map<String, Value>) -> ScanSetup {
    ScanSetup {
        id: text(data, &["id"]),
        display_name: text(data, &["displayName", "display_name"]),
        scanning: text(data, &["scanning"]),
    }
}
