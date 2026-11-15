//! Ports tests/test_maps.py without the network: cache and parsing helpers only.

use negpy_backend::kernel::system::version::{app_version, is_newer, parse_version};
use negpy_backend::services::maps::{
    accept_language, fetch_tile, place_fields, result_coords, tile_cache_path, user_agent,
};
use serde_json::json;

#[test]
fn the_place_language_uses_the_locale_then_the_environment() {
    assert_eq!(accept_language(Some("pl_PL"), None), "pl-PL,en");
    assert_eq!(accept_language(Some("pl_PL.UTF-8"), None), "pl-PL,en");
    assert_eq!(accept_language(None, Some("ja_JP.UTF-8")), "ja-JP,en");
    assert_eq!(accept_language(None, None), "en");
    assert_eq!(accept_language(Some(""), Some("")), "en");
}

#[test]
fn the_identifying_user_agent_carries_the_version() {
    assert!(user_agent().starts_with(&format!("NegPy/{}", app_version())));
    assert!(user_agent().contains("github.com/marcinz606/NegPy"));
}

#[test]
fn place_fields_fall_through_the_settlement_keys() {
    let city = |address: serde_json::Value| place_fields(Some(&json!({"address": address}))).0;
    assert_eq!(city(json!({"city": "Tokyo"})), "Tokyo");
    assert_eq!(city(json!({"village": "Ōgimi"})), "Ōgimi");
    assert_eq!(city(json!({"town": "Hakone", "city": "Tokyo"})), "Tokyo");
    assert_eq!(city(json!({})), "");
    assert_eq!(
        place_fields(Some(&json!({"lat": "35"}))),
        ("".into(), "".into(), "".into())
    );
    assert_eq!(place_fields(None), ("".into(), "".into(), "".into()));
}

#[test]
fn place_fields_read_state_and_country() {
    let result = json!({"address": {"state": "Kanto", "country": "Japan"}});
    assert_eq!(
        place_fields(Some(&result)),
        ("".into(), "Kanto".into(), "Japan".into())
    );
}

#[test]
fn result_coordinates_are_optional_and_parse_floats() {
    assert_eq!(
        result_coords(Some(&json!({"lat": "35.0", "lon": "139.0"}))),
        Some((35.0, 139.0))
    );
    assert_eq!(result_coords(Some(&json!({"lat": "35"}))), None);
    assert_eq!(result_coords(None), None);
}

#[test]
fn the_tile_path_is_the_cache_mirror() {
    assert_eq!(
        tile_cache_path("/tmp/cache", 9, 1, 2),
        std::path::Path::new("/tmp/cache/map_tiles/9/1/2.png")
    );
}

#[test]
fn a_disk_tile_is_used_before_the_network() {
    let dir = tempfile::tempdir().unwrap();
    let path = tile_cache_path(&dir.path().display().to_string(), 9, 1, 2);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"cached").unwrap();
    assert_eq!(
        fetch_tile(&dir.path().display().to_string(), 9, 1, 2),
        Some(b"cached".to_vec())
    );
}

#[test]
fn the_app_version_and_order_match_python() {
    assert!(!app_version().is_empty());
    assert_eq!(parse_version("0.68.0"), vec![0, 68, 0]);
    assert_eq!(parse_version("bad"), Vec::<i64>::new());
    assert!(is_newer("0.69.0", "0.68.0"));
    assert!(!is_newer("0.68.0", "0.68.0"));
    assert!(!is_newer("bad", "0.68.0"));
    assert!(!is_newer("0.69.0", "unknown"));
}
