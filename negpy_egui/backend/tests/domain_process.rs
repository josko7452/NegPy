//! Ports of the `ProcessConfig`/`ExposureConfig` assertions behind
//! tests/test_config_deserialization.py, tests/test_altprocess_config.py and
//! tests/test_cast_removal_migration.py.

use negpy_backend::domain::models::exposure::{BwFilter, ExposureConfig, RenderIntent};
use negpy_backend::domain::models::process::{
    DEFAULT_CROSSTALK_MATRIX, DemosaicMode, ProcessConfig, ProcessMode, SensorUnmix, SourceEncoding,
};

fn from_map<T: serde::de::DeserializeOwned>(fields: &[(&str, serde_json::Value)]) -> T {
    let map: serde_json::Map<String, serde_json::Value> = fields
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect();
    serde_json::from_value(serde_json::Value::Object(map)).expect("deserializes")
}

#[test]
fn the_process_defaults_match_the_python_dataclass() {
    let config = ProcessConfig::default();
    assert_eq!(config.process_mode, ProcessMode::C41);
    assert_eq!(config.source_color, None);
    assert_eq!(config.source_encoding, SourceEncoding::Auto);
    assert_eq!(config.demosaic_preview, DemosaicMode::Auto);
    assert_eq!(config.highlight_reconstruction, 0);
    assert_eq!(config.analysis_buffer, 0.05);
    assert_eq!(config.color_range_clip, 1.0);
    assert_eq!(config.locked_floors, [0.0, 0.0, 0.0]);
    assert_eq!(config.crosstalk_profile, "Generic C41");
    assert_eq!(config.crosstalk_process, "Color Negative");
    assert_eq!(config.sensor_profile, "None");
    assert_eq!(config.sensor_unmix, SensorUnmix::TwoScale);
    assert_eq!(config.fade_ratio_r, 1.0);
    assert_eq!(config.baseline_source, "");
}

#[test]
fn a_retired_process_mode_reads_as_a_color_negative() {
    // The Python `_missing_` maps legacy chemistry codes and anything stale to C41.
    assert_eq!(ProcessMode::coerce("Color Negative"), ProcessMode::C41);
    assert_eq!(ProcessMode::coerce("C41"), ProcessMode::C41);
    assert_eq!(ProcessMode::coerce("B&W"), ProcessMode::BW);
    assert_eq!(ProcessMode::coerce("E-6"), ProcessMode::E6);
    assert_eq!(ProcessMode::coerce("nonsense"), ProcessMode::C41);
}

#[test]
fn a_retired_demosaic_mode_reads_as_auto() {
    assert_eq!(DemosaicMode::coerce("VNG"), DemosaicMode::Vng);
    assert_eq!(
        DemosaicMode::coerce("this build cannot run it"),
        DemosaicMode::Auto
    );
}

#[test]
fn the_crosstalk_matrix_default_is_the_built_in_fallback() {
    assert_eq!(
        DEFAULT_CROSSTALK_MATRIX,
        [1.0, -0.05, -0.02, -0.04, 1.0, -0.08, -0.01, -0.1, 1.0]
    );
}

#[test]
fn the_exposure_defaults_match_the_python_dataclass() {
    let config = ExposureConfig::default();
    assert_eq!(config.density, 1.0);
    assert_eq!(config.grade, 115.0);
    assert_eq!(config.toe_width, 2.5);
    assert_eq!(config.cast_removal_strength, 0.5);
    assert!(config.auto_exposure);
    assert!(config.auto_normalize_contrast);
    assert_eq!(config.render_intent, RenderIntent::Print);
    assert_eq!(config.paper_profile, "neutral");
    assert_eq!(config.dye_separation, 1.0);
    assert_eq!(config.bw_filter, BwFilter::None);
}

#[test]
fn a_legacy_paper_grade_converts_on_load() {
    // R = 150 - 20*G for the legacy 0-5 ladder; ISO R starts at 50.
    let config: ExposureConfig = from_map(&[("grade", serde_json::json!(3.0))]);
    assert_eq!(config.grade, 90.0);
    // An ISO-R value is left alone.
    let config: ExposureConfig = from_map(&[("grade", serde_json::json!(130.0))]);
    assert_eq!(config.grade, 130.0);
}

#[test]
fn a_legacy_cast_removal_bool_coerces_to_a_strength() {
    let config: ExposureConfig = from_map(&[("cast_removal_strength", serde_json::json!(true))]);
    assert_eq!(config.cast_removal_strength, 1.0);
    let config: ExposureConfig = from_map(&[("cast_removal_strength", serde_json::json!(false))]);
    assert_eq!(config.cast_removal_strength, 0.0);
    let config: ExposureConfig = from_map(&[("cast_removal_strength", serde_json::json!(0.3))]);
    assert_eq!(config.cast_removal_strength, 0.3);
}

#[test]
fn a_programmatic_build_normalizes_the_legacy_grade() {
    let config = ExposureConfig {
        grade: 4.0,
        ..ExposureConfig::default()
    }
    .normalize();
    assert_eq!(config.grade, 70.0);
}

#[test]
fn a_round_trip_keeps_the_canonical_spellings() {
    let config = ProcessConfig {
        process_mode: ProcessMode::BW,
        source_encoding: SourceEncoding::Gamma22,
        ..ProcessConfig::default()
    };
    let text = serde_json::to_string(&config).unwrap();
    assert!(text.contains("\"process_mode\":\"B&W Negative\""));
    assert!(text.contains("\"source_encoding\":\"gamma22\""));
    let back: ProcessConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(back, config);
}
