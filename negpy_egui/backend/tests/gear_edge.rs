//! The film-edge block of a gear stock, as `gear/film_stocks.json` stores it.

use negpy_backend::features::metadata::gear::GearLibrary;
use serde_json::json;
use std::fs;

#[test]
fn a_stock_keeps_its_edge_print_and_color_lines() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let stock = json!([{
        "id": "vision3-500t",
        "manufacturer": "Kodak",
        "stockName": "Vision3 500T",
        "colorType": "ColorNegative",
        "edge": {
            "style": "cine",
            "maker": "EASTMAN",
            "text": "5219",
            "code": "EJ",
            "dx": "95-13",
            "printsAs": "5219",
            "source": "photo",
            "colorLines": [
                {"at": 1.2, "color": "green"},
                {"at": 2.0, "color": "red", "thickness": 0.4, "dashed": true}
            ]
        }
    }]);
    fs::write(
        bundled.path().join("film_stocks.json"),
        serde_json::to_string(&stock).unwrap(),
    )
    .unwrap();

    let library = GearLibrary::load(user.path(), bundled.path());
    let stock = library.film_stock("vision3-500t").expect("the stock");
    let edge = stock.edge.as_ref().expect("an edge block");
    assert_eq!(edge.style, "cine");
    assert_eq!(edge.maker, "EASTMAN");
    assert_eq!(edge.text, "5219");
    assert_eq!(edge.code, "EJ");
    assert_eq!(edge.dx, "95-13");
    assert_eq!(edge.prints_as, "5219");
    assert_eq!(edge.source, "photo");
    assert_eq!(edge.color_lines.len(), 2);
    assert_eq!(edge.color_lines[1].thickness, Some(0.4));
    assert!(edge.color_lines[1].dashed);
    assert!(stock.is_bundled);
}

#[test]
fn a_stock_without_an_edge_block_prints_the_name_guess() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let stock = json!([{"id": "hp5", "manufacturer": "Ilford", "stockName": "HP5 Plus"}]);
    fs::write(
        bundled.path().join("film_stocks.json"),
        serde_json::to_string(&stock).unwrap(),
    )
    .unwrap();
    let library = GearLibrary::load(user.path(), bundled.path());
    assert!(library.film_stock("hp5").unwrap().edge.is_none());
}

#[test]
fn a_user_stock_is_not_bundled_and_a_bundled_duplicate_wins() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let stock = json!([{"id": "hp5", "manufacturer": "Ilford", "stockName": "HP5 Plus"}]);
    fs::write(
        bundled.path().join("film_stocks.json"),
        serde_json::to_string(&stock).unwrap(),
    )
    .unwrap();
    let user_stock = json!([
        {"id": "hp5", "manufacturer": "Ilford", "stockName": "HP5 Plus (mine)"},
        {"id": "mine", "manufacturer": "Foma", "stockName": "Fomapan 400"}
    ]);
    fs::write(
        user.path().join("film_stocks.json"),
        serde_json::to_string(&user_stock).unwrap(),
    )
    .unwrap();
    let library = GearLibrary::load(user.path(), bundled.path());
    let hp5 = library.film_stock("hp5").unwrap();
    assert_eq!(hp5.stock_name, "HP5 Plus");
    assert!(hp5.is_bundled);
    assert!(!library.film_stock("mine").unwrap().is_bundled);
}

#[test]
fn prints_as_finds_a_stock_by_its_catalog_name() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let stock = json!([
        {"id": "k5219", "manufacturer": "Kodak", "stockName": "Vision3 500T"},
        {"id": "cinestill", "manufacturer": "CineStill", "stockName": "800T"}
    ]);
    fs::write(
        bundled.path().join("film_stocks.json"),
        serde_json::to_string(&stock).unwrap(),
    )
    .unwrap();
    let library = GearLibrary::load(user.path(), bundled.path());
    assert_eq!(library.get_film_stock("Vision3 500T").unwrap().id, "k5219");
    assert_eq!(
        library.get_film_stock("Kodak Vision3 500T").unwrap().id,
        "k5219"
    );
    assert!(library.get_film_stock("5219").is_none());
}
