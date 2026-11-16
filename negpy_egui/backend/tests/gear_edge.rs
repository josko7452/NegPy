//! The film-edge block of a gear stock, as `gear/film_stocks.json` stores it.

use negpy_backend::features::metadata::gear::{
    Camera, DevelopmentProcess, FilmStock, GearLibrary, Lens, ScanSetup,
};
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

#[test]
fn a_saved_library_round_trips_every_user_record() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    fs::write(bundled.path().join("film_stocks.json"), "[]").unwrap();
    let library = GearLibrary {
        cameras: vec![Camera {
            id: "mine".into(),
            make: "Olympus".into(),
            model: "PEN F".into(),
            serial_number: "123".into(),
            notes: "Half frame".into(),
            ..Camera::default()
        }],
        lenses: vec![Lens {
            id: "lens".into(),
            lens_model: "Zuiko 38".into(),
            focal_length_mm: Some(38.0),
            max_aperture: Some(1.8),
            ..Lens::default()
        }],
        film_stocks: vec![FilmStock {
            id: "stock".into(),
            manufacturer: "Ilford".into(),
            stock_name: "HP5 Plus".into(),
            iso: 400,
            format: "35mm".into(),
            color_type: "B&W Negative".into(),
            ..FilmStock::default()
        }],
        processes: vec![DevelopmentProcess {
            id: "process".into(),
            display_name: "D-76 stock".into(),
            developer: "D-76".into(),
            push_pull: 1,
            time_seconds: Some(660),
            temperature_c: Some(20.0),
            ..DevelopmentProcess::default()
        }],
        scan_setups: vec![ScanSetup {
            id: "setup".into(),
            display_name: "Camera scan".into(),
            scanning: "Nikon Z6".into(),
            ..ScanSetup::default()
        }],
    };
    library.save(user.path()).unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(user.path().join("cameras.json")).unwrap())
            .unwrap();
    assert_eq!(saved[0]["serialNumber"], "123");
    assert_eq!(saved[0]["notes"], "Half frame");
    assert!(saved[0].get("isBundled").is_none());

    let reloaded = GearLibrary::load(user.path(), bundled.path());
    assert_eq!(reloaded.cameras[0], library.cameras[0]);
    assert_eq!(reloaded.lenses[0], library.lenses[0]);
    assert_eq!(reloaded.film_stocks[0], library.film_stocks[0]);
    assert_eq!(reloaded.processes[0], library.processes[0]);
    assert_eq!(reloaded.scan_setups[0], library.scan_setups[0]);
}

#[test]
fn saving_drops_bundled_records() {
    let user = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let stock = json!([{"id": "hp5", "manufacturer": "Ilford", "stockName": "HP5 Plus"}]);
    fs::write(
        bundled.path().join("film_stocks.json"),
        serde_json::to_string(&stock).unwrap(),
    )
    .unwrap();
    let library = GearLibrary::load(user.path(), bundled.path());
    library.save(user.path()).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(user.path().join("film_stocks.json")).unwrap())
            .unwrap();
    assert_eq!(saved, json!([]));
}

#[test]
fn a_source_xmp_rating_is_read_as_written() {
    use negpy_backend::features::metadata::exif_read::{
        ExifTags, ExifValue, SourceExif, XMP_PACKET_TAG, source_xmp_rating,
    };

    let packet = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="4"/></rdf:RDF></x:xmpmeta>"#;
    let mut zeroth = ExifTags::new();
    zeroth.insert(XMP_PACKET_TAG, ExifValue::Bytes(packet.to_vec()));
    let source = SourceExif {
        zeroth,
        ..SourceExif::default()
    };
    assert_eq!(source_xmp_rating(Some(&source)), "4");

    let mut zeroth = ExifTags::new();
    zeroth.insert(
        XMP_PACKET_TAG,
        ExifValue::Bytes(
            br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/"><xmp:Rating>2</xmp:Rating></rdf:Description></rdf:RDF></x:xmpmeta>"#.to_vec(),
        ),
    );
    let source = SourceExif {
        zeroth,
        ..SourceExif::default()
    };
    assert_eq!(source_xmp_rating(Some(&source)), "2");
}
