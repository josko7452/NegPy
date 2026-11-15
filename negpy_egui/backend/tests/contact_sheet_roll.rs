//! Ports the pure ordering, format and label cases in tests/test_contact_sheet_roll.py.

use negpy_backend::domain::models::{GeometryConfig, ProcessConfig, ProcessMode, WorkspaceConfig};
use negpy_backend::features::metadata::gear::{FilmStock, GearLibrary};
use negpy_backend::services::export::contact_sheet_roll::{
    FrameFacts, SheetFrame, asset_paths, creation_order, cropped_aspect, film_color_type,
    film_name, frame_order, infer_format, name_order, parse_frame_120, print_aspect,
    roll_label_text, scan_exposure_offsets, turns_for, upright_aspect,
};
use negpy_backend::services::film_edge::geometry::{SheetFormat, film_geometry};

fn frame(name: &str, size: Option<(i64, i64)>) -> SheetFrame {
    SheetFrame {
        name: name.to_string(),
        path: format!("/roll/{name}"),
        half: 0,
        config: WorkspaceConfig::default(),
        facts: FrameFacts {
            scan_size: size,
            ..FrameFacts::default()
        },
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn exif_time_orders_when_every_frame_has_one() {
    let mut b = frame("b.jpg", None);
    b.facts.capture_time = "2026-01-01 10:00:02".into();
    b.facts.birth_time = 1.0;
    let mut a = frame("a.jpg", None);
    a.facts.capture_time = "2026-01-01 10:00:01".into();
    a.facts.birth_time = 2.0;
    assert_eq!(creation_order(&[b, a]), vec![1, 0]);
}

#[test]
fn creation_time_orders_when_one_frame_lacks_exif() {
    let mut a = frame("a.jpg", None);
    a.facts.capture_time = "2026-01-01 10:00:01".into();
    a.facts.birth_time = 30.0;
    let mut b = frame("b.jpg", None);
    b.facts.birth_time = 10.0;
    let mut c = frame("c.jpg", None);
    c.facts.capture_time = "2026-01-01 10:00:00".into();
    c.facts.birth_time = 20.0;
    assert_eq!(creation_order(&[a, b, c]), vec![1, 2, 0]);
}

#[test]
fn ties_break_by_name_then_half() {
    let mut x2 = frame("x.tif", None);
    x2.half = 2;
    x2.facts.birth_time = 5.0;
    let mut x1 = frame("x.tif", None);
    x1.half = 1;
    x1.facts.birth_time = 5.0;
    let mut a = frame("a.tif", None);
    a.facts.birth_time = 5.0;
    assert_eq!(creation_order(&[x2, x1, a]), vec![2, 1, 0]);
}

#[test]
fn frame_order_is_roll_then_frame_number_then_name() {
    let mut a = frame("a.tif", None);
    a.config.metadata.capture_roll = "02".into();
    a.config.metadata.capture_frame = Some(1);
    let mut b = frame("b.tif", None);
    b.config.metadata.capture_roll = "01".into();
    b.config.metadata.capture_frame = Some(10);
    let mut c = frame("c.tif", None);
    c.config.metadata.capture_roll = "01".into();
    c.config.metadata.capture_frame = Some(2);
    let mut e = frame("e.tif", None);
    e.config.metadata.capture_roll = "01".into();
    e.config.metadata.capture_frame = Some(2);
    assert_eq!(frame_order(&[a, b, c, e]), vec![2, 3, 1, 0]);
}

#[test]
fn name_order_ignores_case_and_time() {
    let b = frame("b.tif", None);
    let mut x2 = frame("x.tif", None);
    x2.half = 2;
    let a = frame("A.tif", None);
    let mut x1 = frame("x.tif", None);
    x1.half = 1;
    let frames = vec![b, x2, a, x1];
    assert_eq!(
        name_order(&frames)
            .iter()
            .map(|index| (frames[*index].display_name(), frames[*index].half))
            .collect::<Vec<_>>(),
        vec![
            ("A.tif".into(), 0),
            ("b.tif".into(), 0),
            ("x.tif".into(), 1),
            ("x.tif".into(), 2)
        ]
    );
}

#[test]
fn metadata_majority_picks_the_format() {
    let mut first = frame("a", None);
    first.config.metadata.format = "35mm".into();
    let mut second = frame("b", None);
    second.config.metadata.format = "35mm".into();
    let mut third = frame("c", None);
    third.config.metadata.format = "120".into();
    assert_eq!(
        infer_format(&[first, second, third], false),
        (SheetFormat::FullFrame, "6×6".into())
    );
}

#[test]
fn other_can_name_a_120_size() {
    let mut frame = frame("a", None);
    frame.config.metadata.format = "Other".into();
    frame.config.metadata.format_other = "6x7 Mamiya".into();
    assert_eq!(
        infer_format(&[frame.clone(), frame.clone(), frame], false),
        (SheetFormat::Medium, "6×7".into())
    );
}

#[test]
fn the_crop_shape_can_pick_a_120_size() {
    let frame = frame("a", Some((5600, 4480)));
    let mut configured = frame.clone();
    configured.config.metadata.format = "120".into();
    assert_eq!(
        infer_format(
            &[
                configured.clone(),
                configured.clone(),
                configured.clone(),
                configured
            ],
            false
        ),
        (SheetFormat::Medium, "6×7".into())
    );
}

#[test]
fn half_frames_force_the_half_format() {
    let mut split = frame("a", None);
    split.half = 1;
    split.config.metadata.format = "35mm".into();
    assert_eq!(infer_format(&[split], false).0, SheetFormat::HalfFrame);
    let mut roll = frame("b", None);
    roll.config.metadata.format = "35mm".into();
    assert_eq!(infer_format(&[roll], true).0, SheetFormat::HalfFrame);
}

#[test]
fn no_metadata_reads_the_frame_shape() {
    let square = vec![frame("a", Some((4000, 4000))); 3];
    let landscape = vec![frame("b", Some((3000, 2000))); 3];
    let six_by_nine = vec![frame("c", Some((8400, 5600))); 3];
    assert_eq!(
        infer_format(&square, false),
        (SheetFormat::Medium, "6×6".into())
    );
    assert_eq!(infer_format(&landscape, false).0, SheetFormat::FullFrame);
    assert_eq!(infer_format(&six_by_nine, false).0, SheetFormat::FullFrame);
}

#[test]
fn the_120_parser_accepts_the_named_forms() {
    assert_eq!(parse_frame_120("6×4,5"), "6×4.5");
    assert_eq!(parse_frame_120("645"), "6×4.5");
    assert_eq!(parse_frame_120("6x12 pano"), "6×12");
    assert_eq!(parse_frame_120("4×5"), "");
}

#[test]
fn upright_aspect_swaps_for_a_quarter_turn() {
    let mut turned = frame("a", Some((3000, 2000)));
    assert!(close(upright_aspect(&turned, None).unwrap(), 1.5));
    turned.config.geometry = GeometryConfig {
        rotation: 1,
        ..GeometryConfig::default()
    };
    assert!(close(upright_aspect(&turned, None).unwrap(), 2.0 / 3.0));
    assert_eq!(
        turns_for(&turned, &film_geometry(SheetFormat::FullFrame, "6×6"), None),
        3
    );
}

#[test]
fn a_portrait_crop_stays_upright() {
    let mut cropped = frame("a", Some((3000, 2000)));
    cropped.config.geometry.crop_rect = Some(vec![0.3, 0.0, 0.7, 1.0]);
    assert!(close(cropped_aspect(&cropped).unwrap(), 1.0 / 0.6));
    assert_eq!(
        turns_for(
            &cropped,
            &film_geometry(SheetFormat::FullFrame, "6×6"),
            None
        ),
        0
    );
}

#[test]
fn a_tile_shape_stands_in_for_an_unknown_scan_size() {
    let blank = frame("s", None);
    assert!(upright_aspect(&blank, None).is_none());
    assert!(close(
        upright_aspect(&blank, Some((200, 300))).unwrap(),
        1.5
    ));
}

#[test]
fn the_print_aspect_uses_the_tile_shape() {
    let blank = frame("s", None);
    assert!(close(print_aspect(&blank, Some((200, 300))).unwrap(), 1.5));
}

#[test]
fn film_name_prefers_the_gear_display_name() {
    let mut selection = frame("a", None);
    selection.config.metadata.film_stock_id = "s1".into();
    selection.config.metadata.film = "Fallback".into();
    let library = GearLibrary {
        film_stocks: vec![FilmStock {
            id: "s1".into(),
            manufacturer: "Kodak".into(),
            stock_name: "Portra 400".into(),
            display_name: "Portra".into(),
            color_type: "ColorNegative".into(),
            ..FilmStock::default()
        }],
        ..GearLibrary::default()
    };
    assert_eq!(film_name(&selection, Some(&library)), "Portra");
    selection.config.metadata.film_stock_id.clear();
    assert_eq!(film_name(&selection, Some(&library)), "Fallback");
}

#[test]
fn a_gear_name_avoids_a_doubled_maker() {
    let mut selection = frame("a", None);
    selection.config.metadata.film_stock_id = "s1".into();
    let library = GearLibrary {
        film_stocks: vec![FilmStock {
            id: "s1".into(),
            manufacturer: "Kentmere".into(),
            stock_name: "Kentmere 400".into(),
            color_type: "B&W Negative".into(),
            ..FilmStock::default()
        }],
        ..GearLibrary::default()
    };
    assert_eq!(film_name(&selection, Some(&library)), "Kentmere 400");
}

#[test]
fn film_color_type_falls_back_to_process_mode() {
    let mut color = frame("a", None);
    color.config.process = ProcessConfig {
        process_mode: ProcessMode::C41,
        ..ProcessConfig::default()
    };
    assert_eq!(film_color_type(&color, None), Some("ColorNegative".into()));
    color.config.process.process_mode = ProcessMode::BW;
    assert_eq!(film_color_type(&color, None), Some("B&W Negative".into()));
    color.config.process.process_mode = ProcessMode::E6;
    color.config.process.slide_bw = true;
    assert_eq!(film_color_type(&color, None), Some("B&W Slide".into()));
    color.config.process.positive_source = true;
    assert_eq!(film_color_type(&color, None), None);
}

#[test]
fn the_roll_label_joins_its_parts() {
    let mut frame = frame("a", None);
    frame.config.metadata.film = "Ilford HP5 Plus".into();
    frame.config.metadata.film_iso = Some(400);
    frame.config.metadata.push_pull = 1;
    frame.config.metadata.developer = "ID-11".into();
    frame.config.metadata.process_dilution = "1+1".into();
    frame.config.metadata.camera_make = "Nikon".into();
    frame.config.metadata.camera_model = "Nikon F3".into();
    frame.config.metadata.capture_date = "2026-08-11 14:00".into();
    assert_eq!(
        roll_label_text("Roll 12", &[frame.clone(), frame], None),
        "Roll 12 · Ilford HP5 Plus @ EI 800 · ID-11 1+1 · Nikon F3 · 2026-08-11"
    );
    let empty = SheetFrame::new("a", "/a", WorkspaceConfig::default());
    assert_eq!(roll_label_text("", &[empty], None), "");
}

#[test]
fn paths_of_a_composite_are_its_parts() {
    assert_eq!(
        asset_paths("/a", &["/a".into(), "/b".into()], &[]),
        vec!["/a", "/b"]
    );
    assert_eq!(asset_paths("/a", &[], &[]), vec!["/a"]);
}

#[test]
fn scan_exposure_offsets_center_on_the_median() {
    let mut a = frame("a", None);
    a.facts.scan_ev = Some(1.0);
    let mut b = frame("b", None);
    b.facts.scan_ev = Some(2.0);
    let mut c = frame("c", None);
    c.facts.scan_ev = Some(3.0);
    let d = frame("d", None);
    let offsets = scan_exposure_offsets(&[a, b, c, d]);
    assert!(close(offsets[0], -2.0f64.log10()));
    assert!(close(offsets[1], 0.0));
    assert!(close(offsets[2], 2.0f64.log10()));
    assert_eq!(offsets[3], 0.0);
}
