//! The pure shutter-label half of the capture calibration.

use negpy_backend::services::capture::calibration::{
    REFERENCE_LEVELS, REFERENCE_SHUTTER, SHUTTER_CANDIDATES, aperture_fnumber, ladder_stops,
    nearest_shutter, normalize_start_point, scan_ladder, shutter_at_least, shutter_display,
    shutter_displays, shutter_seconds, true_seconds, usable_ladder,
};

const THIRDS: [&str; 14] = [
    "1", "8/10", "6/10", "5/10", "4/10", "1/3", "1/4", "1/5", "1/6", "1/8", "1/10", "1/13", "1/15",
    "1/20",
];
const HALVES: [&str; 11] = [
    "1", "1/1.5", "1/2", "1/3", "1/4", "1/6", "1/8", "1/11", "1/15", "1/22", "1/30",
];

fn close(actual: f64, expected: f64, relative: f64) -> bool {
    (actual - expected).abs() <= relative * expected.abs().max(1e-12)
}

#[test]
fn a_label_parses_to_nominal_seconds() {
    assert_eq!(shutter_seconds("1/100"), Ok(0.01));
    assert_eq!(shutter_seconds("0.4"), Ok(0.4));
    assert_eq!(shutter_seconds("1"), Ok(1.0));
    assert!(shutter_seconds("1/0").is_err());
    assert!(shutter_seconds("Bulb").is_err());
}

#[test]
fn a_unit_suffix_is_ignored() {
    assert!(close(shutter_seconds("0.4000s").unwrap(), 0.4, 1e-9));
    assert!(close(shutter_seconds("0.0666s").unwrap(), 0.0666, 1e-9));
    assert!(close(shutter_seconds("2 sec").unwrap(), 2.0, 1e-9));
    assert!(close(shutter_seconds("0.4").unwrap(), 0.4, 1e-9));
}

#[test]
fn true_seconds_undoes_display_rounding() {
    assert!(close(true_seconds("1", &THIRDS), 1.0, 1e-3));
    assert!(close(true_seconds("1/4", &THIRDS), 0.25, 1e-3));
    assert!(close(true_seconds("1/8", &THIRDS), 0.125, 1e-3));
    assert!(close(
        true_seconds("1/3", &THIRDS),
        2f64.powf(-5.0 / 3.0),
        0.01
    ));
    assert!(close(
        true_seconds("1/6", &THIRDS),
        2f64.powf(-8.0 / 3.0),
        0.01
    ));
    assert!(close(
        true_seconds("4/10", &THIRDS),
        2f64.powf(-4.0 / 3.0),
        0.01
    ));
    assert!(close(shutter_seconds("1/3").unwrap(), 1.0 / 3.0, 1e-12));
}

#[test]
fn ladder_spacing_is_read_from_the_body() {
    assert!(close(ladder_stops(&THIRDS), 1.0 / 3.0, 1e-6));
    assert!(close(ladder_stops(&HALVES), 0.5, 1e-6));
    assert!(close(true_seconds("1/3", &HALVES), 2f64.powf(-1.5), 0.02));
    assert!(close(ladder_stops(&["1/4", "1/8"]), 1.0 / 3.0, 1e-12));
}

#[test]
fn a_label_off_the_ladder_is_left_alone() {
    assert!(close(true_seconds("0.7", &THIRDS), 0.7, 1e-6));
    for label in THIRDS {
        assert!(close(
            true_seconds(label, &THIRDS),
            shutter_seconds(label).unwrap(),
            0.08
        ));
    }
}

#[test]
fn the_ladder_cleaner_drops_junk_and_ascends() {
    let raw = ["1/250", "1/0", "Bulb", "", "1/60", "1/4", "2", "30"];
    assert_eq!(usable_ladder(&raw), ["1/250", "1/60", "1/4", "2", "30"]);
    assert!(ladder_stops(&raw) > 0.0);
    assert_eq!(shutter_at_least(0.5, &usable_ladder(&raw)), "2");
}

#[test]
fn the_scan_ladder_keeps_a_body_inside_the_built_in_span() {
    let labels = ["1/8000", "Bulb", " 1/60 ", "1/250", "30", "2", "1/0"].map(str::to_string);
    assert_eq!(scan_ladder(&labels), ["1/250", "1/60", "2"]);
}

#[test]
fn an_aperture_label_parses_or_reads_as_manual() {
    assert_eq!(aperture_fnumber("f/8"), Some(8.0));
    assert_eq!(aperture_fnumber("F5.6"), Some(5.6));
    assert_eq!(aperture_fnumber("11"), Some(11.0));
    assert_eq!(aperture_fnumber(""), None);
    assert_eq!(aperture_fnumber("—"), None);
}

#[test]
fn the_start_point_scales_by_iso_and_aperture() {
    let (levels, shutter) = normalize_start_point("100", "f/8", &SHUTTER_CANDIDATES);
    assert_eq!(levels, REFERENCE_LEVELS);
    assert_eq!(shutter, REFERENCE_SHUTTER);
    assert_eq!(
        normalize_start_point("400", "f/8", &SHUTTER_CANDIDATES).1,
        "1/10"
    );
    assert_eq!(
        normalize_start_point("100", "f/11", &SHUTTER_CANDIDATES).1,
        "0.8"
    );
    assert_eq!(
        normalize_start_point("100", "f/16", &SHUTTER_CANDIDATES).1,
        "1.6"
    );
    assert_eq!(
        normalize_start_point("200", "", &SHUTTER_CANDIDATES).1,
        "1/5"
    );
}

#[test]
fn nearest_shutter_snaps_onto_the_body_ladder() {
    assert_eq!(nearest_shutter("0.4", &["1/4", "1/3", "1/2"]), "1/3");
    assert_eq!(nearest_shutter("1/5", &["1/5", "1/4"]), "1/5");
}

const NIKON_THIRDS: [&str; 18] = [
    "0.0010s", "0.0013s", "0.0016s", "0.0020s", "0.0040s", "0.0080s", "0.0167s", "0.0333s",
    "0.0667s", "0.0769s", "0.1250s", "0.2500s", "0.3000s", "0.4000s", "1.0000s", "1.3000s",
    "2.5000s", "30.0000s",
];
const NIKON_HALVES: [&str; 13] = [
    "0.0010s", "0.0013s", "0.0020s", "0.0029s", "0.0040s", "0.0056s", "0.0080s", "0.0111s",
    "0.0167s", "0.0222s", "0.0333s", "0.7000s", "1.5000s",
];

#[test]
fn display_reads_a_decimal_ladder_as_the_camera_marks_it() {
    assert_eq!(
        shutter_displays(&NIKON_THIRDS),
        [
            "1/1000", "1/800", "1/640", "1/500", "1/250", "1/125", "1/60", "1/30", "1/15", "1/13",
            "1/8", "1/4", "0.3\"", "0.4\"", "1\"", "1.3\"", "2.5\"", "30\""
        ]
    );
    assert_eq!(
        shutter_displays(&NIKON_HALVES),
        [
            "1/1000", "1/750", "1/500", "1/350", "1/250", "1/180", "1/125", "1/90", "1/60", "1/45",
            "1/30", "0.7\"", "1.5\""
        ]
    );
}

#[test]
fn display_of_one_label() {
    let cases = [
        ("1/30", "1/30"),
        ("1/2", "1/2"),
        ("4/10", "0.4\""),
        ("0.4", "0.4\""),
        ("1", "1\""),
        ("0.0013s", "1/800"),
        ("Bulb", "Bulb"),
        ("1/0", "1/0"),
        ("", ""),
    ];
    for (label, shown) in cases {
        assert_eq!(shutter_display(label, &[]), shown, "{label}");
    }
}
