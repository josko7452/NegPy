//! Mirrors `negpy/services/maps.py`: OpenStreetMap tiles and Nominatim lookups.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const TILE_SIZE: u32 = 256;
pub const MIN_ZOOM: u32 = 2;
pub const MAX_ZOOM: u32 = 18;

const TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const NOMINATIM: &str = "https://nominatim.openstreetmap.org";
const TIMEOUT_SECONDS: u64 = 3;

const CITY_KEYS: [&str; 6] = [
    "city",
    "town",
    "village",
    "hamlet",
    "municipality",
    "suburb",
];
const STATE_KEYS: [&str; 4] = ["state", "region", "province", "county"];

/// The identifying User-Agent the OSM tile policy requires.
#[must_use]
pub fn user_agent() -> String {
    format!(
        "NegPy/{} (+https://github.com/marcinz606/NegPy)",
        crate::kernel::system::version::app_version()
    )
}

/// The place-name language request, with an English fallback.
#[must_use]
pub fn accept_language(locale: Option<&str>, lang: Option<&str>) -> String {
    let raw = locale
        .filter(|value| !value.is_empty())
        .or(lang)
        .unwrap_or_default();
    let tag = raw.split('.').next().unwrap_or_default().replace('_', "-");
    if tag.is_empty() {
        "en".to_string()
    } else {
        format!("{tag},en")
    }
}

/// The on-disk path of one cached tile.
#[must_use]
pub fn tile_cache_path(cache_dir: &str, z: u32, x: u32, y: u32) -> PathBuf {
    Path::new(cache_dir)
        .join("map_tiles")
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png"))
}

/// Percent-encode one query component as `urllib.parse.urlencode` does for ASCII input.
fn encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn query(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// One HTTP GET body, or `None` when the request fails or returns a non-200 status.
fn get(url: &str, timeout_seconds: u64) -> Option<Vec<u8>> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(timeout_seconds)))
        .build()
        .new_agent();
    let response = agent
        .get(url)
        .header("User-Agent", &user_agent())
        .call()
        .ok()?;
    let mut body = Vec::new();
    let mut reader = response.into_body().into_reader().take(32 * 1024 * 1024);
    reader.read_to_end(&mut body).ok()?;
    Some(body)
}

/// One map tile, from the disk cache when present, otherwise from OSM.
#[must_use]
pub fn fetch_tile(cache_dir: &str, z: u32, x: u32, y: u32) -> Option<Vec<u8>> {
    let path = tile_cache_path(cache_dir, z, x, y);
    if let Ok(data) = std::fs::read(&path) {
        return Some(data);
    }
    let url = TILE_URL
        .replace("{z}", &z.to_string())
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string());
    let data = get(&url, TIMEOUT_SECONDS)?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temporary = path.with_extension("png.tmp");
    if std::fs::write(&temporary, &data).is_ok() {
        let _ = std::fs::rename(&temporary, &path);
    }
    Some(data)
}

fn json(url: &str) -> Option<Value> {
    let bytes = get(url, TIMEOUT_SECONDS)?;
    serde_json::from_slice(&bytes).ok()
}

/// Nominatim hits for a place name; empty when offline or nothing matches.
#[must_use]
pub fn search_places(query_text: &str, limit: usize, language: &str) -> Vec<Value> {
    let text = query_text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let params = query(&[
        ("q", text.to_string()),
        ("format", "json".to_string()),
        ("addressdetails", "1".to_string()),
        ("limit", limit.to_string()),
        ("accept-language", language.to_string()),
    ]);
    let Some(payload) = json(&format!("{NOMINATIM}/search?{params}")) else {
        return Vec::new();
    };
    payload
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// The Nominatim result for a position, or `None` when it cannot be reached.
#[must_use]
pub fn reverse_place(lat: f64, lon: f64, language: &str) -> Option<Value> {
    let params = query(&[
        ("lat", format!("{lat:.6}")),
        ("lon", format!("{lon:.6}")),
        ("format", "json".to_string()),
        ("addressdetails", "1".to_string()),
        ("zoom", "10".to_string()),
        ("accept-language", language.to_string()),
    ]);
    let payload = json(&format!("{NOMINATIM}/reverse?{params}"))?;
    (payload.is_object() && payload.get("error").is_none()).then_some(payload)
}

fn first_address(address: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = address.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

/// City, state and country from a Nominatim result.
#[must_use]
pub fn place_fields(result: Option<&Value>) -> (String, String, String) {
    let Some(address) = result
        .and_then(|value| value.get("address"))
        .and_then(Value::as_object)
    else {
        return (String::new(), String::new(), String::new());
    };
    (
        first_address(address, &CITY_KEYS),
        first_address(address, &STATE_KEYS),
        first_address(address, &["country"]),
    )
}

/// The coordinates of a Nominatim result, or `None` when they are missing.
#[must_use]
pub fn result_coords(result: Option<&Value>) -> Option<(f64, f64)> {
    let result = result?;
    let lat = result.get("lat")?.as_str()?.parse::<f64>().ok()?;
    let lon = result.get("lon")?.as_str()?.parse::<f64>().ok()?;
    Some((lat, lon))
}
