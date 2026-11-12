//! Export and metadata DTO defaults and coercion, checked against negpy/domain/models.py
//! and negpy/features/metadata/models.py.

use negpy_backend::domain::models::export::{
    EXPORT_COLOR_SPACES, ExportConfig, ExportFormat, ExportPreset, ExportPresetOutputMode,
    ExportResolutionMode, JXL_TAGGABLE_SPACES, TiffCompression, export_blocked, migrate_export_fmt,
    preset_display_name,
};
use negpy_backend::domain::models::metadata::{
    DEFAULT_DESCRIPTION_FIELDS, FORMAT_OPTIONS, FORMAT_UNSET, MetadataConfig, format_label,
    format_value, normalize_description_fields,
};

#[test]
fn the_export_defaults_match_the_python_dataclass() {
    let config = ExportConfig::default();
    assert_eq!(config.export_fmt, ExportFormat::Jpeg);
    assert_eq!(config.export_bit_depth, 16);
    assert_eq!(config.jpeg_quality, 90);
    assert_eq!(config.tiff_compression, TiffCompression::Zip);
    assert!(config.jxl_lossless);
    assert_eq!(config.export_color_space, "sRGB");
    assert_eq!(config.paper_aspect_ratio, "Original");
    assert_eq!(config.export_print_size, 30.0);
    assert_eq!(config.export_dpi, 300);
    assert_eq!(
        config.export_resolution_mode,
        ExportResolutionMode::Original
    );
    assert_eq!(config.export_target_long_edge_px, 2000);
    assert_eq!(config.filename_pattern, "{{ original_name }}");
    assert_eq!(
        config.output_mode,
        ExportPresetOutputMode::SubfolderOfSource
    );
    assert_eq!(config.output_subfolder, "export");
}

#[test]
fn the_user_dir_key_keeps_its_camel_case_spelling() {
    // The Python flat namespace stores the field name `userDir` verbatim.
    let config: ExportConfig = serde_json::from_str(r#"{"userDir":"/home/user/NegPy"}"#).unwrap();
    assert_eq!(config.user_dir, "/home/user/NegPy");
    let text = serde_json::to_string(&config).unwrap();
    assert!(text.contains("\"userDir\":\"/home/user/NegPy\""));
}

#[test]
fn a_retired_export_format_migrates_to_tiff() {
    assert_eq!(migrate_export_fmt("DNG"), "TIFF");
    assert_eq!(migrate_export_fmt("JPEG"), "JPEG");
    assert_eq!(ExportFormat::coerce("XYZ"), ExportFormat::Jpeg);
}

#[test]
fn the_export_block_rule_matches_the_python_helper() {
    assert!(export_blocked(ExportFormat::Jxl, "Adobe RGB"));
    assert!(export_blocked(ExportFormat::Jxl, "Same as Source"));
    assert!(!export_blocked(ExportFormat::Jxl, "sRGB"));
    assert!(!export_blocked(ExportFormat::Jpeg, "Adobe RGB"));
    assert_eq!(EXPORT_COLOR_SPACES.len(), 7);
    assert!(EXPORT_COLOR_SPACES.contains(&"Greyscale"));
    assert!(!EXPORT_COLOR_SPACES.contains(&"ACES"));
    assert_eq!(JXL_TAGGABLE_SPACES.len(), 4);
}

#[test]
fn the_preset_defaults_match_and_flag_flat_masters() {
    let preset = ExportPreset::default();
    assert_eq!(preset.name, "Untitled Preset");
    assert!(preset.enabled);
    assert_eq!(preset.render_intent, "print");
    assert_eq!(preset.output_mode, ExportPresetOutputMode::SameAsSource);
    assert!(!preset.id.is_empty());
    assert_eq!(preset_display_name(&preset), "Untitled Preset");
    let flat = ExportPreset {
        name: "Master".into(),
        render_intent: "flat".into(),
        ..ExportPreset::default()
    };
    assert_eq!(preset_display_name(&flat), "Master (flat)");
}

#[test]
fn the_metadata_defaults_match() {
    let config = MetadataConfig::default();
    assert_eq!(config.push_pull, 0);
    assert_eq!(config.description_fields, None);
    assert!(!config.protect_original_metadata);
    assert_eq!(FORMAT_UNSET, "—");
    assert_eq!(FORMAT_OPTIONS.len(), 7);
    assert_eq!(format_label("35mm"), "35mm");
    assert_eq!(format_label("bogus"), FORMAT_UNSET);
    assert_eq!(format_value(FORMAT_UNSET), "");
    assert_eq!(format_value("120"), "120");
}

#[test]
fn description_fields_normalize_to_canonical_order() {
    let ordered = normalize_description_fields(Some(&[
        "scanning".to_string(),
        "camera".to_string(),
        "iso".to_string(),
        "bogus".to_string(),
    ]));
    assert_eq!(ordered, vec!["camera", "iso", "scanning"]);
    let defaults: Vec<String> = DEFAULT_DESCRIPTION_FIELDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(normalize_description_fields(None), defaults);
    // A set list normalizes on build; None stays None to inherit the sticky choice.
    let config = MetadataConfig {
        description_fields: Some(vec!["scanning".to_string(), "camera".to_string()]),
        ..MetadataConfig::default()
    }
    .normalize();
    assert_eq!(
        config.description_fields,
        Some(vec!["camera".to_string(), "scanning".to_string()])
    );
    assert_eq!(
        MetadataConfig::default().normalize().description_fields,
        None
    );
}
