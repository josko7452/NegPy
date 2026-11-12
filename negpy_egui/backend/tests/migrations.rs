//! Ports of the `migrate_flat_config` assertions behind tests/test_slide_normalize_migration.py,
//! test_config_deserialization.py, test_altprocess_config.py and test_capture_unmix.py.

use serde_json::{Value, json};

use negpy_backend::domain::migrations::{FlatConfig, migrate_export_fmt, migrate_flat_config};

fn run(fields: &[(&str, Value)]) -> FlatConfig {
    let mut data: FlatConfig = fields
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect();
    migrate_flat_config(&mut data);
    data
}

fn value<'a>(data: &'a FlatConfig, key: &str) -> Option<&'a Value> {
    data.get(key)
}

#[test]
fn a_renamed_key_carries_its_value() {
    assert_eq!(
        value(&run(&[("export_border_size", json!(5))]), "border_size"),
        Some(&json!(5))
    );
    assert_eq!(
        value(&run(&[("drange_clip", json!(0.3))]), "luma_range_clip"),
        Some(&json!(0.3))
    );
    assert_eq!(
        value(
            &run(&[("use_colour_average", json!(true))]),
            "use_color_average"
        ),
        Some(&json!(true))
    );
    assert_eq!(
        value(
            &run(&[("manual_crop_rect", json!([1, 2, 3, 4]))]),
            "crop_rect"
        ),
        Some(&json!([1, 2, 3, 4]))
    );
    assert_eq!(
        value(&run(&[("k1", json!(0.1))]), "distortion_k1"),
        Some(&json!(0.1))
    );
    // The bool -> strength coercion is left to ExposureConfig's post-init.
    assert_eq!(
        value(
            &run(&[("cast_removal", json!(true))]),
            "cast_removal_strength"
        ),
        Some(&json!(true))
    );
}

#[test]
fn a_dropped_key_is_removed() {
    let data = run(&[
        ("flare", json!(1)),
        ("surround", json!(2)),
        ("vibrance", json!(3)),
    ]);
    assert!(data.is_empty(), "{data:?}");
}

#[test]
fn a_retired_export_format_maps_to_its_replacement() {
    assert_eq!(migrate_export_fmt("DNG"), "TIFF");
    assert_eq!(migrate_export_fmt("JPEG"), "JPEG");
}

#[test]
fn a_normalized_slide_meters_on_the_transfer_curve() {
    let data = run(&[
        ("process_mode", json!("Transparency")),
        ("e6_normalize", json!(true)),
        ("auto_exposure", json!(false)),
    ]);
    assert_eq!(value(&data, "auto_exposure"), Some(&json!(true)));
    assert_eq!(value(&data, "auto_normalize_contrast"), Some(&json!(true)));
}

#[test]
fn a_raw_slide_keeps_its_inert_autos_off() {
    let data = run(&[
        ("process_mode", json!("Transparency")),
        ("e6_normalize", json!(false)),
        ("auto_exposure", json!(true)),
    ]);
    assert_eq!(value(&data, "auto_exposure"), Some(&json!(false)));
    assert_eq!(value(&data, "auto_normalize_contrast"), Some(&json!(false)));
}

#[test]
fn a_positive_frame_keeps_its_autos() {
    let data = run(&[
        ("process_mode", json!("Transparency")),
        ("e6_normalize", json!(false)),
        ("positive_source", json!(true)),
        ("auto_exposure", json!(true)),
    ]);
    assert_eq!(value(&data, "auto_exposure"), Some(&json!(true)));
}

#[test]
fn a_negative_is_untouched_by_the_slide_migration() {
    let data = run(&[
        ("process_mode", json!("Color Negative")),
        ("e6_normalize", json!(false)),
        ("auto_exposure", json!(true)),
    ]);
    assert_eq!(value(&data, "auto_exposure"), Some(&json!(true)));
    // e6_normalize is still dropped, but it changes nothing on a negative.
    assert!(!data.contains_key("e6_normalize"));
}

#[test]
fn retired_lith_bool_becomes_the_alt_process_enum() {
    assert_eq!(
        value(&run(&[("lith_enabled", json!(true))]), "alt_process"),
        Some(&json!("lith"))
    );
    assert_eq!(
        value(&run(&[("lith_enabled", json!(false))]), "alt_process"),
        Some(&json!("none"))
    );
    // The three lith_* sliders keep their names.
    let data = run(&[("lith_enabled", json!(true)), ("lith_snatch", json!(0.8))]);
    assert_eq!(value(&data, "lith_snatch"), Some(&json!(0.8)));
}

#[test]
fn a_retired_narrowband_scan_becomes_the_pakon_stock() {
    assert_eq!(
        value(&run(&[("narrowband_scan", json!(false))]), "paper_profile"),
        None
    );
    assert_eq!(
        value(&run(&[("narrowband_scan", json!(true))]), "paper_profile"),
        Some(&json!("pakon"))
    );
    // A non-neutral paper profile keeps its stock.
    let endura = run(&[
        ("narrowband_scan", json!(true)),
        ("paper_profile", json!("kodak_endura")),
    ]);
    assert_eq!(
        value(&endura, "paper_profile"),
        Some(&json!("kodak_endura"))
    );
    let bw = run(&[
        ("narrowband_scan", json!(true)),
        ("process_mode", json!("B&W Negative")),
    ]);
    assert_eq!(value(&bw, "paper_profile"), None);
    let slide = run(&[
        ("narrowband_scan", json!(true)),
        ("process_mode", json!("Transparency")),
    ]);
    assert_eq!(value(&slide, "paper_profile"), None);
}

#[test]
fn a_retired_resolution_or_output_choice_maps() {
    assert_eq!(
        value(
            &run(&[("use_original_res", json!(true))]),
            "export_resolution_mode"
        ),
        Some(&json!("original"))
    );
    assert_eq!(
        value(
            &run(&[("use_original_res", json!(false))]),
            "export_resolution_mode"
        ),
        Some(&json!("print"))
    );
    assert_eq!(
        value(&run(&[("same_as_source", json!(true))]), "output_mode"),
        Some(&json!("same_as_source"))
    );
}

#[test]
fn a_carrier_toggle_off_forces_the_width_to_zero() {
    let data = run(&[
        ("carrier_enabled", json!(false)),
        ("carrier_width", json!(2.0)),
    ]);
    assert_eq!(value(&data, "carrier_width"), Some(&json!(0.0)));
}

#[test]
fn a_vignette_strength_maps_to_stops() {
    assert_eq!(
        value(&run(&[("vignette_strength", json!(0.5))]), "vignette_stops"),
        Some(&json!(-1.0))
    );
}

#[test]
fn a_lab_separation_maps_to_crosstalk_strength() {
    assert_eq!(
        value(
            &run(&[("color_separation", json!(1.5))]),
            "crosstalk_strength"
        ),
        Some(&json!(0.5))
    );
}

#[test]
fn a_contrast_mask_maps_to_the_tone_strength() {
    assert_eq!(
        value(
            &run(&[("contrast_mask", json!(0.25))]),
            "contrast_mask_strength"
        ),
        Some(&json!(0.5))
    );
    assert_eq!(
        value(
            &run(&[("contrast_mask", json!(-0.2))]),
            "contrast_mask_strength"
        ),
        Some(&json!(0.0))
    );
}

#[test]
fn true_black_becomes_paper_black_with_inverted_polarity() {
    assert_eq!(
        value(&run(&[("true_black", json!(true))]), "paper_black"),
        Some(&json!(false))
    );
}

#[test]
fn a_roll_average_toggle_splits_into_luma_and_color() {
    let data = run(&[("use_roll_average", json!(false))]);
    assert_eq!(value(&data, "use_luma_average"), Some(&json!(false)));
    assert_eq!(value(&data, "use_color_average"), Some(&json!(false)));
}

#[test]
fn a_combined_camera_override_splits_into_make_and_model() {
    let both = run(&[("camera_override", json!("Nikon D850"))]);
    assert_eq!(value(&both, "camera_make"), Some(&json!("Nikon")));
    assert_eq!(value(&both, "camera_model"), Some(&json!("D850")));
    let one = run(&[("camera_override", json!("Nikon"))]);
    assert_eq!(value(&one, "camera_model"), Some(&json!("Nikon")));
    // A row that already carries a model keeps it.
    let present = run(&[
        ("camera_override", json!("Nikon")),
        ("camera_model", json!("x")),
    ]);
    assert_eq!(value(&present, "camera_model"), Some(&json!("x")));
}

#[test]
fn a_legacy_lens_override_becomes_the_lens_model() {
    assert_eq!(
        value(&run(&[("lens_override", json!("50mm f1.8"))]), "lens_model"),
        Some(&json!("50mm f1.8"))
    );
}

#[test]
fn the_merged_dye_separation_control_drops_an_old_signed_value() {
    // A dict carrying density_saturation predates the merge; the old value is dropped and
    // the rename then writes the frame-wide key.
    let data = run(&[
        ("density_saturation", json!(0.9)),
        ("dye_separation", json!(0.0)),
    ]);
    assert_eq!(value(&data, "dye_separation"), Some(&json!(0.9)));
    // Anything under the new slider floor can only be the old signed control.
    assert!(run(&[("dye_separation", json!(0.2))]).is_empty());
    assert_eq!(
        value(&run(&[("dye_separation", json!(0.3))]), "dye_separation"),
        Some(&json!(0.3))
    );
}

#[test]
fn a_row_saved_after_a_retirement_is_untouched() {
    let data = run(&[
        ("auto_exposure", json!(true)),
        ("dye_separation", json!(1.0)),
    ]);
    assert_eq!(value(&data, "auto_exposure"), Some(&json!(true)));
    assert_eq!(value(&data, "dye_separation"), Some(&json!(1.0)));
}
