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

/// A colored line printed on the film edge, `at` mm in from the edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeColorLine {
    pub at: f64,
    pub color: String,
    /// The line's height in mm; `None` takes the shared default.
    pub thickness: Option<f64>,
    pub dashed: bool,
}

/// What a stock prints on its rebate; an empty field falls back to the stock's name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilmEdge {
    /// A film edge layout key.
    pub style: String,
    pub maker: String,
    pub text: String,
    pub code: String,
    /// `part1-part2` of the DX number.
    pub dx: String,
    /// The stock whose edge print this film carries, a respooled cine stock.
    pub prints_as: String,
    pub source: String,
    pub color_lines: Vec<EdgeColorLine>,
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
    pub edge: Option<FilmEdge>,
    pub is_bundled: bool,
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

    /// A stock by the name its maker uses, as `prints_as` names one.
    #[must_use]
    pub fn get_film_stock(&self, name: &str) -> Option<&FilmStock> {
        let wanted = name.trim().to_lowercase();
        self.film_stocks.iter().find(|stock| {
            stock.stock_name.trim().to_lowercase() == wanted
                || stock.full_film_label().trim().to_lowercase() == wanted
        })
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
            cameras: cameras.iter().map(|(item, _)| camera_from(item)).collect(),
            lenses: lenses.iter().map(|(item, _)| lens_from(item)).collect(),
            film_stocks: film_stocks
                .iter()
                .map(|(item, bundled)| FilmStock {
                    is_bundled: *bundled,
                    ..film_stock_from(item)
                })
                .collect(),
            processes: processes
                .iter()
                .map(|(item, _)| process_from(item))
                .collect(),
            scan_setups: scan_setups
                .iter()
                .map(|(item, _)| scan_setup_from(item))
                .collect(),
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

/// A merged record and whether it came from the bundled file.
fn load_merged(
    filename: &str,
    gear_dir: &Path,
    bundled_dir: &Path,
) -> Vec<(Map<String, Value>, bool)> {
    let bundled = read_list(&bundled_dir.join(filename));
    let ids: Vec<String> = bundled
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let user = read_list(&gear_dir.join(filename))
        .into_iter()
        .filter(|item| {
            item.get("id")
                .and_then(Value::as_str)
                .is_none_or(|id| !ids.iter().any(|known| known == id))
        })
        .map(|item| (item, false));
    bundled
        .into_iter()
        .map(|item| (item, true))
        .chain(user)
        .collect()
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

/// `FilmEdge.from_dict`: an absent or non-object edge prints nothing.
#[must_use]
pub fn film_edge_from(value: Option<&Value>) -> Option<FilmEdge> {
    let data = value?.as_object()?;
    let color_lines = data
        .get("colorLines")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_object)
                .filter(|line| line.contains_key("at") && line.contains_key("color"))
                .map(|line| EdgeColorLine {
                    at: line.get("at").and_then(Value::as_f64).unwrap_or(0.0),
                    color: line
                        .get("color")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    thickness: line.get("thickness").and_then(Value::as_f64),
                    dashed: line.get("dashed").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(FilmEdge {
        style: text(data, &["style"]),
        maker: text(data, &["maker"]),
        text: text(data, &["text"]),
        code: text(data, &["code"]),
        dx: text(data, &["dx"]),
        prints_as: text(data, &["printsAs", "prints_as"]),
        source: text(data, &["source"]),
        color_lines,
    })
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
        edge: film_edge_from(data.get("edge")),
        is_bundled: false,
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
