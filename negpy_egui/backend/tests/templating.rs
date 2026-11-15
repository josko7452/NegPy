//! Ports tests/test_templating.py.

use negpy_backend::domain::models::{
    ExportConfig, ExportFormat, ExportResolutionMode, MetadataConfig,
};
use negpy_backend::services::export::templating::{parse_capture_stem, render_export_filename};

fn render(pattern: &str, path: &str, metadata: Option<&MetadataConfig>) -> String {
    let config = ExportConfig {
        filename_pattern: pattern.to_string(),
        ..ExportConfig::default()
    };
    render_export_filename(path, &config, 0.0, 0, metadata, "")
}

#[test]
fn the_basic_template_cleans_structural_separators() {
    let config = ExportConfig {
        filename_pattern: "test_{{ original_name }}_{{ colorspace }}".to_string(),
        export_color_space: "Adobe RGB".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("/path/to/image.orf", &config, 0.0, 0, None, ""),
        "test_image_Adobe_RGB"
    );
}

#[test]
fn the_date_template_uses_today() {
    let now = chrono::Local::now().format("%Y%m%d").to_string();
    assert_eq!(
        render("{{ date }}_{{ original_name }}", "my_scan.tiff", None),
        format!("{now}_my_scan")
    );
}

#[test]
fn size_and_dpi_render_in_print_mode_only() {
    let print = ExportConfig {
        export_resolution_mode: ExportResolutionMode::Print,
        export_print_size: 30.0,
        export_dpi: 300,
        filename_pattern: "{{ original_name }}_{{ size }}_{{ dpi }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("shot.jpg", &print, 0.0, 0, None, ""),
        "shot_30cm_300dpi"
    );
    let original = ExportConfig {
        export_resolution_mode: ExportResolutionMode::Original,
        export_print_size: 30.0,
        export_dpi: 300,
        filename_pattern: "{{ original_name }}_{{ size }}_{{ dpi }}_end".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("shot.jpg", &original, 0.0, 0, None, ""),
        "shot_end"
    );
}

#[test]
fn target_pixels_render_only_in_target_mode() {
    let target = ExportConfig {
        export_resolution_mode: ExportResolutionMode::TargetPx,
        export_target_long_edge_px: 2048,
        filename_pattern: "{{ original_name }}_{{ target_px }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("shot.jpg", &target, 0.0, 0, None, ""),
        "shot_2048px"
    );
    let print = ExportConfig {
        export_resolution_mode: ExportResolutionMode::Print,
        filename_pattern: "{{ original_name }}_{{ target_px }}_end".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("shot.jpg", &print, 0.0, 0, None, ""),
        "shot_end"
    );
}

#[test]
fn a_border_adds_its_word_only_when_present() {
    let config = ExportConfig {
        filename_pattern: "{{ original_name }}_{{ border }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("img.jpg", &config, 1.5, 0, None, ""),
        "img_border"
    );
    assert_eq!(
        render_export_filename("img.jpg", &config, 0.0, 0, None, ""),
        "img"
    );
}

#[test]
fn a_name_keeps_its_spaces_dashes_and_underscores() {
    let config = ExportConfig {
        filename_pattern: "print_{{ original_name }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("/shots/IMG-0001.orf", &config, 0.0, 0, None, ""),
        "print_IMG-0001"
    );
    assert_eq!(
        render_export_filename("scan_001-A.orf", &config, 0.0, 0, None, ""),
        "print_scan_001-A"
    );
    assert_eq!(
        render_export_filename("IMG__0001.orf", &config, 0.0, 0, None, ""),
        "print_IMG__0001"
    );
    assert_eq!(
        render_export_filename("_scan.orf", &config, 0.0, 0, None, ""),
        "print__scan"
    );
    assert_eq!(
        render_export_filename("my scan.jpg", &config, 0.0, 0, None, ""),
        "print_my scan"
    );
}

#[test]
fn an_empty_or_invalid_template_falls_back_to_the_name() {
    assert_eq!(render("", "IMG-0001.orf", None), "IMG-0001");
    assert_eq!(
        render("{{ invalid_var }}", "IMG-0001.orf", None),
        "IMG-0001"
    );
}

#[test]
fn format_and_paper_ratio_are_distinct() {
    let config = ExportConfig {
        export_fmt: ExportFormat::Tiff,
        paper_aspect_ratio: "3:2".to_string(),
        filename_pattern: "{{ original_name }}_{{ format }}_{{ paper_ratio }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("img.jpg", &config, 0.0, 0, None, ""),
        "img_TIFF_3:2"
    );
}

#[test]
fn gear_variables_render_and_empty_ones_collapse() {
    let meta = MetadataConfig {
        camera_make: "Mamiya".into(),
        camera_model: "7".into(),
        lens_model: "80mm f/4".into(),
        film: "Portra 400".into(),
        film_iso: Some(400),
        film_manufacturer: "Kodak".into(),
        format: "35mm".into(),
        developer: "D-76 1+1".into(),
        push_pull: 1,
        scanning: "DSLR copy-stand".into(),
        exposure_override: "1/125s f/2.8".into(),
        focal_length_mm: Some(80.0),
        ..MetadataConfig::default()
    };
    let config = ExportConfig {
        filename_pattern: "{{ film }}_{{ film_iso }}_{{ camera }}_{{ lens }}_{{ focal_length }}_{{ film_format }}_{{ developer }}_{{ push_pull }}_{{ scanning }}_{{ exposure }}_{{ original_name }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("DSC0123.orf", &config, 0.0, 0, Some(&meta), ""),
        "Portra_400_400_Mamiya_7_80mm_f4_80_35mm_D_76_1+1_1_DSLR_copy_stand_1125s_f2.8_DSC0123"
    );
    assert_eq!(
        render(
            "{{ film }}_{{ camera }}_{{ original_name }}_end",
            "shot.jpg",
            Some(&MetadataConfig::default())
        ),
        "shot_end"
    );
}

#[test]
fn the_film_format_can_be_other() {
    let meta = MetadataConfig {
        format: "Other".into(),
        format_other: "6×7".into(),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ film_format }}_{{ original_name }}",
            "img.jpg",
            Some(&meta)
        ),
        "6×7_img"
    );
}

#[test]
fn roll_and_frame_parse_from_the_stem() {
    assert_eq!(
        parse_capture_stem("Roll001_Frame012"),
        ("Roll001".into(), Some(12))
    );
    assert_eq!(
        parse_capture_stem("roll001_frame7"),
        ("roll001".into(), Some(7))
    );
    assert_eq!(parse_capture_stem("DSC0123"), (String::new(), None));
    let meta = MetadataConfig {
        film: "Portra".into(),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ roll }}_Frame{{ frame }}_ {{ film }}",
            "/rolls/Roll001_Frame012.tif",
            Some(&meta)
        ),
        "Roll001_Frame12_Portra"
    );
}

#[test]
fn metadata_roll_wins_over_the_stem() {
    let meta = MetadataConfig {
        capture_roll: "ManualRoll".into(),
        capture_frame: Some(3),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ roll }}_{{ frame }}",
            "Roll001_Frame012.tif",
            Some(&meta)
        ),
        "ManualRoll_3"
    );
}

#[test]
fn path_unsafe_characters_are_removed() {
    let meta = MetadataConfig {
        camera_model: "Foo/Bar:Baz*?\"<>|".into(),
        film: "A\\B".into(),
        ..MetadataConfig::default()
    };
    let value = render(
        "{{ camera_model }}_{{ film }}_{{ original_name }}",
        "shot.jpg",
        Some(&meta),
    );
    assert_eq!(value, "FooBarBaz_AB_shot");
    assert!(!value.contains('/') && !value.contains(':') && !value.contains('*'));
}

#[test]
fn a_half_appends_its_index() {
    let meta = MetadataConfig {
        film: "HP5".into(),
        capture_roll: "R1".into(),
        capture_frame: Some(1),
        ..MetadataConfig::default()
    };
    let config = ExportConfig {
        filename_pattern: "{{ roll }}_{{ film }}_{{ original_name }}".to_string(),
        ..ExportConfig::default()
    };
    assert_eq!(
        render_export_filename("/x/IMG420.tif", &config, 0.0, 2, Some(&meta), ""),
        "R1_HP5_IMG420_2"
    );
}

#[test]
fn frame_padded_and_pad_filter_handle_an_unset_frame() {
    let meta = MetadataConfig {
        capture_frame: Some(12),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ frame_padded }}_{{ original_name }}",
            "shot.tif",
            Some(&meta)
        ),
        "012_shot"
    );
    let meta = MetadataConfig {
        capture_roll: "Summer24".into(),
        film: "Portra".into(),
        film_iso: Some(400),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ roll }}_Frame{{ frame|pad(3) }}_{{ film }}_{{ film_iso }}_{{ original_name }}",
            "HighDef2 (3).tif",
            Some(&meta)
        ),
        "Summer24_Frame_Portra_400_HighDef2 (3)"
    );
}

#[test]
fn capture_date_variables_are_compact_or_empty() {
    let meta = MetadataConfig {
        capture_date: "1998-07".into(),
        ..MetadataConfig::default()
    };
    assert_eq!(
        render(
            "{{ capture_year }}_{{ capture_date }}_{{ original_name }}",
            "shot.tif",
            Some(&meta)
        ),
        "1998_19980701_shot"
    );
    assert_eq!(
        render(
            "{{ capture_year }}_{{ original_name }}",
            "shot.tif",
            Some(&MetadataConfig::default())
        ),
        "shot"
    );
}
