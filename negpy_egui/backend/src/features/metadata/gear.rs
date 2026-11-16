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
    pub serial_number: String,
    pub notes: String,
    pub is_bundled: bool,
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
    pub serial_number: String,
    pub notes: String,
    pub is_bundled: bool,
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
    pub notes: String,
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
    pub notes: String,
    pub is_bundled: bool,
}

/// A digitizing setup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanSetup {
    pub id: String,
    pub display_name: String,
    pub scanning: String,
    pub notes: String,
    pub is_bundled: bool,
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
            cameras: cameras
                .iter()
                .map(|(item, bundled)| Camera {
                    is_bundled: *bundled,
                    ..camera_from(item)
                })
                .collect(),
            lenses: lenses
                .iter()
                .map(|(item, bundled)| Lens {
                    is_bundled: *bundled,
                    ..lens_from(item)
                })
                .collect(),
            film_stocks: film_stocks
                .iter()
                .map(|(item, bundled)| FilmStock {
                    is_bundled: *bundled,
                    ..film_stock_from(item)
                })
                .collect(),
            processes: processes
                .iter()
                .map(|(item, bundled)| DevelopmentProcess {
                    is_bundled: *bundled,
                    ..process_from(item)
                })
                .collect(),
            scan_setups: scan_setups
                .iter()
                .map(|(item, bundled)| ScanSetup {
                    is_bundled: *bundled,
                    ..scan_setup_from(item)
                })
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
        serial_number: text(data, &["serialNumber", "serial_number"]),
        notes: text(data, &["notes"]),
        is_bundled: false,
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
        serial_number: text(data, &["serialNumber", "serial_number"]),
        notes: text(data, &["notes"]),
        is_bundled: false,
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
        notes: text(data, &["notes"]),
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
        notes: text(data, &["notes"]),
        is_bundled: false,
    }
}

fn scan_setup_from(data: &Map<String, Value>) -> ScanSetup {
    ScanSetup {
        id: text(data, &["id"]),
        display_name: text(data, &["displayName", "display_name"]),
        scanning: text(data, &["scanning"]),
        notes: text(data, &["notes"]),
        is_bundled: false,
    }
}

fn put_text(value: &mut Map<String, Value>, key: &str, text: &str) {
    value.insert(key.to_string(), Value::String(text.to_string()));
}

fn put_optional_text(value: &mut Map<String, Value>, key: &str, text: &str) {
    let text = if text.is_empty() {
        Value::Null
    } else {
        Value::String(text.to_string())
    };
    value.insert(key.to_string(), text);
}

fn camera_to_value(camera: &Camera) -> Value {
    let mut value = Map::new();
    put_text(&mut value, "id", &camera.id);
    put_text(&mut value, "make", &camera.make);
    put_text(&mut value, "model", &camera.model);
    put_text(&mut value, "displayName", &camera.display_name);
    put_optional_text(&mut value, "serialNumber", &camera.serial_number);
    put_optional_text(&mut value, "notes", &camera.notes);
    Value::Object(value)
}

fn lens_to_value(lens: &Lens) -> Value {
    let mut value = Map::new();
    put_text(&mut value, "id", &lens.id);
    put_text(&mut value, "lensModel", &lens.lens_model);
    put_optional_text(&mut value, "make", &lens.make);
    put_text(&mut value, "displayName", &lens.display_name);
    put_optional_text(&mut value, "serialNumber", &lens.serial_number);
    put_optional_text(&mut value, "notes", &lens.notes);
    if let Some(focal) = lens.focal_length_mm {
        value.insert("focalLength".to_string(), Value::from(focal));
    }
    if let Some(aperture) = lens.max_aperture {
        value.insert("maxAperture".to_string(), Value::from(aperture));
    }
    Value::Object(value)
}

fn edge_to_value(edge: &FilmEdge) -> Value {
    let mut value = Map::new();
    put_optional_text(&mut value, "style", &edge.style);
    put_optional_text(&mut value, "maker", &edge.maker);
    put_optional_text(&mut value, "text", &edge.text);
    put_optional_text(&mut value, "code", &edge.code);
    put_optional_text(&mut value, "dx", &edge.dx);
    put_optional_text(&mut value, "printsAs", &edge.prints_as);
    put_optional_text(&mut value, "source", &edge.source);
    if !edge.color_lines.is_empty() {
        let lines = edge
            .color_lines
            .iter()
            .map(|line| {
                let mut entry = Map::new();
                entry.insert("at".to_string(), Value::from(line.at));
                put_text(&mut entry, "color", &line.color);
                if let Some(thickness) = line.thickness {
                    entry.insert("thickness".to_string(), Value::from(thickness));
                }
                if line.dashed {
                    entry.insert("dashed".to_string(), Value::Bool(true));
                }
                Value::Object(entry)
            })
            .collect();
        value.insert("colorLines".to_string(), Value::Array(lines));
    }
    Value::Object(value)
}

fn film_stock_to_value(stock: &FilmStock) -> Value {
    let mut value = Map::new();
    put_text(&mut value, "id", &stock.id);
    put_text(&mut value, "displayName", &stock.display_name);
    put_text(&mut value, "manufacturer", &stock.manufacturer);
    put_text(&mut value, "stockName", &stock.stock_name);
    value.insert("iso".to_string(), Value::from(stock.iso));
    put_text(&mut value, "format", &stock.format);
    put_text(&mut value, "colorType", &stock.color_type);
    put_optional_text(&mut value, "notes", &stock.notes);
    if let Some(edge) = &stock.edge {
        value.insert("edge".to_string(), edge_to_value(edge));
    }
    Value::Object(value)
}

fn process_to_value(process: &DevelopmentProcess) -> Value {
    let mut value = Map::new();
    put_text(&mut value, "id", &process.id);
    put_text(&mut value, "displayName", &process.display_name);
    put_text(&mut value, "developer", &process.developer);
    put_text(&mut value, "dilution", &process.dilution);
    value.insert("pushPull".to_string(), Value::from(process.push_pull));
    if let Some(seconds) = process.time_seconds {
        value.insert("timeSeconds".to_string(), Value::from(seconds));
    }
    if let Some(temperature) = process.temperature_c {
        value.insert("temperatureC".to_string(), Value::from(temperature));
    }
    put_optional_text(&mut value, "notes", &process.notes);
    Value::Object(value)
}

fn scan_setup_to_value(setup: &ScanSetup) -> Value {
    let mut value = Map::new();
    put_text(&mut value, "id", &setup.id);
    put_text(&mut value, "displayName", &setup.display_name);
    put_text(&mut value, "scanning", &setup.scanning);
    put_optional_text(&mut value, "notes", &setup.notes);
    Value::Object(value)
}

fn write_list(path: &Path, items: Vec<Value>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&items).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&temporary, text + "\n").map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

impl GearLibrary {
    /// Write the user-owned records, leaving bundled records to their files.
    pub fn save(&self, gear_dir: &Path) -> Result<(), String> {
        let cameras = self
            .cameras
            .iter()
            .filter(|item| !item.is_bundled)
            .map(camera_to_value)
            .collect();
        let lenses = self
            .lenses
            .iter()
            .filter(|item| !item.is_bundled)
            .map(lens_to_value)
            .collect();
        let film_stocks = self
            .film_stocks
            .iter()
            .filter(|item| !item.is_bundled)
            .map(film_stock_to_value)
            .collect();
        let processes = self
            .processes
            .iter()
            .filter(|item| !item.is_bundled)
            .map(process_to_value)
            .collect();
        let scan_setups = self
            .scan_setups
            .iter()
            .filter(|item| !item.is_bundled)
            .map(scan_setup_to_value)
            .collect();
        write_list(&gear_dir.join(CAMERAS_FILE), cameras)?;
        write_list(&gear_dir.join(LENSES_FILE), lenses)?;
        write_list(&gear_dir.join(FILM_STOCKS_FILE), film_stocks)?;
        write_list(&gear_dir.join(PROCESSES_FILE), processes)?;
        write_list(&gear_dir.join(SCAN_SETUPS_FILE), scan_setups)
    }
}
