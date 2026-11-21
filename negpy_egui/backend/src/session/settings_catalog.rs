//! Declarative catalog of copyable settings.
//!
//! Mirrors `negpy/desktop/settings_catalog.py`. A row maps one label to one or more
//! fields on a `WorkspaceConfig` section; the fields of a row always travel together.

use serde_json::{Map, Value, json};

use crate::domain::models::process::{SOURCE_ENCODING_LABELS, SourceEncoding};
use crate::domain::models::{ProcessMode, WorkspaceConfig, with_auto_crop_field, with_film_fields};
use crate::features::exposure::papers::effective_paper_profile;
use crate::features::process::logic::VALID_HIGHLIGHT_LEVELS;
use crate::features::process::path::is_mono;
use crate::kernel::system::i18n::{mark, tr};
use crate::services::assets::migrations::gear_presets::GEAR_FIELDS;
use crate::services::assets::presets::preset_fields;
use crate::services::assets::rolls::card_fields;

/// One copyable setting. `fields` are field names on `section`; `channels` gives
/// per-channel letters for grouped numeric trims.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingRow {
    pub label: &'static str,
    pub section: &'static str,
    pub fields: &'static [&'static str],
    pub channels: &'static str,
    pub kind: RowFormat,
    pub sticky: bool,
}

impl SettingRow {
    /// Stable identity for persistence. No field is listed twice, so it is unique.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}.{}", self.section, self.fields[0])
    }
}

/// The formatter a row uses. Most rows show their values; grouped rows name a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowFormat {
    Default,
    Scalar,
    SourceEncoding,
    PrintStock,
    Process,
    Gear,
    FlatField,
    WhiteBalance,
    Highlights,
}

const fn row(
    label: &'static str,
    section: &'static str,
    fields: &'static [&'static str],
) -> SettingRow {
    SettingRow {
        label,
        section,
        fields,
        channels: "",
        kind: RowFormat::Default,
        sticky: false,
    }
}

const fn sticky(mut row: SettingRow) -> SettingRow {
    row.sticky = true;
    row
}

const fn channels(mut row: SettingRow, channels: &'static str) -> SettingRow {
    row.channels = channels;
    row
}

const fn formatted(mut row: SettingRow, kind: RowFormat) -> SettingRow {
    row.kind = kind;
    row
}

fn catalog() -> Vec<(&'static str, Vec<SettingRow>)> {
    vec![
        (
            mark("Setup"),
            vec![
                sticky(row(mark("Film Mode"), "process", &["process_mode"])),
                sticky(row(mark("Positive"), "process", &["positive_source"])),
                sticky(row(mark("B&W Slide"), "process", &["slide_bw"])),
                sticky(formatted(
                    row(mark("Encoding"), "process", &["source_encoding"]),
                    RowFormat::SourceEncoding,
                )),
            ],
        ),
        (
            mark("Metering"),
            vec![
                sticky(row(
                    mark("Analysis Buffer"),
                    "process",
                    &["analysis_buffer"],
                )),
                sticky(row(
                    mark("Luma Range Clip"),
                    "process",
                    &["luma_range_clip"],
                )),
                sticky(row(mark("Color Clip"), "process", &["color_range_clip"])),
                row(mark("White Point"), "process", &["white_point_offset"]),
                channels(
                    row(
                        mark("White Point Trim"),
                        "process",
                        &[
                            "white_point_trim_red",
                            "white_point_trim_green",
                            "white_point_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(mark("Black Point"), "process", &["black_point_offset"]),
                channels(
                    row(
                        mark("Black Point Trim"),
                        "process",
                        &[
                            "black_point_trim_red",
                            "black_point_trim_green",
                            "black_point_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
            ],
        ),
        (
            mark("Roll Analysis"),
            vec![
                row(mark("Use Average Luma"), "process", &["use_luma_average"]),
                row(mark("Use Average Color"), "process", &["use_color_average"]),
                formatted(
                    row(
                        mark("Use Average Cast"),
                        "process",
                        &["use_cast_average", "locked_neutral_axis"],
                    ),
                    RowFormat::Scalar,
                ),
            ],
        ),
        (
            mark("Calibration"),
            vec![
                sticky(formatted(
                    row(
                        mark("Crosstalk"),
                        "process",
                        &[
                            "crosstalk_strength",
                            "crosstalk_profile",
                            "crosstalk_matrix",
                            "crosstalk_process",
                        ],
                    ),
                    RowFormat::Scalar,
                )),
                sticky(formatted(
                    row(
                        mark("Sensor Crosstalk Profile"),
                        "process",
                        &["sensor_profile", "sensor_matrix", "sensor_unmix"],
                    ),
                    RowFormat::Scalar,
                )),
                sticky(row(mark("Hue Trim"), "process", &["hue_trim"])),
            ],
        ),
        (
            mark("Fade Restoration"),
            vec![formatted(
                row(mark("Fade Restoration"), "process", FADE_CARD_FIELDS),
                RowFormat::Scalar,
            )],
        ),
        (
            mark("Raw Decode"),
            vec![
                formatted(
                    row(mark("White Balance"), "process", &["linear_raw"]),
                    RowFormat::WhiteBalance,
                ),
                row(mark("Demosaic Preview"), "process", &["demosaic_preview"]),
                row(mark("Demosaic Export"), "process", &["demosaic_export"]),
                formatted(
                    row(
                        mark("Highlight Recovery"),
                        "process",
                        &["highlight_reconstruction"],
                    ),
                    RowFormat::Highlights,
                ),
            ],
        ),
        (
            mark("Crop"),
            vec![
                sticky(row(mark("Auto Crop"), "geometry", &["crop_from_auto"])),
                sticky(row(mark("Crop Offset"), "geometry", &["autocrop_offset"])),
                sticky(row(
                    mark("Rebate Trim"),
                    "geometry",
                    &["autocrop_rebate_trim"],
                )),
                sticky(row(mark("Crop Ratio"), "geometry", &["autocrop_ratio"])),
                sticky(row(mark("Crop Mode"), "geometry", &["autocrop_mode"])),
                sticky(row(mark("Square First"), "geometry", &["autocrop_square"])),
                formatted(
                    row(mark("Crop"), "geometry", &["crop_rect", "crop_detect_key"]),
                    RowFormat::Scalar,
                ),
            ],
        ),
        (
            mark("Straighten"),
            vec![
                row(mark("Rotation"), "geometry", &["rotation"]),
                row(mark("Fine Rotation"), "geometry", &["fine_rotation"]),
                row(mark("Tilt"), "geometry", &["converge_v"]),
                row(mark("Swing"), "geometry", &["converge_h"]),
                sticky(row(
                    mark("Flip Horizontal"),
                    "geometry",
                    &["flip_horizontal"],
                )),
                sticky(row(mark("Flip Vertical"), "geometry", &["flip_vertical"])),
            ],
        ),
        (
            mark("Optics"),
            vec![
                sticky(row(
                    mark("Distortion Correction"),
                    "geometry",
                    &["distortion_k1", "lens_distortion_from_metadata"],
                )),
                sticky(row(
                    mark("Embedded CA"),
                    "geometry",
                    &["lens_ca_from_metadata"],
                )),
                formatted(
                    row(mark("Flat Field"), "flatfield", &["apply", "profile_id"]),
                    RowFormat::FlatField,
                ),
            ],
        ),
        (
            mark("Tone"),
            vec![
                row(mark("Print Density"), "exposure", &["density"]),
                row(mark("ISO-R Grade"), "exposure", &["grade"]),
                channels(
                    row(
                        mark("Grade Trim"),
                        "exposure",
                        &["grade_trim_red", "grade_trim_green", "grade_trim_blue"],
                    ),
                    "RGB",
                ),
                row(mark("Shadows Density"), "exposure", &["shadow_density"]),
                row(mark("Hold Black"), "exposure", &["hold_black"]),
                row(
                    mark("Highlights Density"),
                    "exposure",
                    &["highlight_density"],
                ),
                row(mark("Shadows Grade"), "exposure", &["shadow_grade"]),
                row(mark("Highlights Grade"), "exposure", &["highlight_grade"]),
                channels(
                    row(
                        mark("Shadows Grade Trim"),
                        "exposure",
                        &[
                            "shadow_grade_trim_red",
                            "shadow_grade_trim_green",
                            "shadow_grade_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                channels(
                    row(
                        mark("Highlights Grade Trim"),
                        "exposure",
                        &[
                            "highlight_grade_trim_red",
                            "highlight_grade_trim_green",
                            "highlight_grade_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(
                    mark("Contrast Mask"),
                    "exposure",
                    &["contrast_mask_strength"],
                ),
                row(mark("Diffusion"), "exposure", &["diffusion"]),
                row(mark("Preflash"), "exposure", &["preflash"]),
                sticky(row(mark("Auto Density"), "exposure", &["auto_exposure"])),
                sticky(row(
                    mark("Auto Grade"),
                    "exposure",
                    &["auto_normalize_contrast"],
                )),
            ],
        ),
        (
            mark("Paper Response"),
            vec![
                sticky(formatted(
                    row(mark("Print Stock"), "exposure", &["paper_profile"]),
                    RowFormat::PrintStock,
                )),
                sticky(row(mark("Paper Black"), "exposure", &["paper_black"])),
                sticky(row(mark("Paper White"), "exposure", &["paper_dmin"])),
                row(mark("Snap"), "exposure", &["midtone_gamma"]),
                channels(
                    row(
                        mark("Snap Trim"),
                        "exposure",
                        &[
                            "midtone_gamma_trim_red",
                            "midtone_gamma_trim_green",
                            "midtone_gamma_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(mark("Toe"), "exposure", &["toe"]),
                row(mark("Toe Width"), "exposure", &["toe_width"]),
                channels(
                    row(
                        mark("Toe Trim"),
                        "exposure",
                        &["toe_trim_red", "toe_trim_green", "toe_trim_blue"],
                    ),
                    "RGB",
                ),
                channels(
                    row(
                        mark("Toe Width Trim"),
                        "exposure",
                        &[
                            "toe_width_trim_red",
                            "toe_width_trim_green",
                            "toe_width_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(mark("Shoulder"), "exposure", &["shoulder"]),
                row(mark("Shoulder Width"), "exposure", &["shoulder_width"]),
                channels(
                    row(
                        mark("Shoulder Trim"),
                        "exposure",
                        &[
                            "shoulder_trim_red",
                            "shoulder_trim_green",
                            "shoulder_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                channels(
                    row(
                        mark("Shoulder Width Trim"),
                        "exposure",
                        &[
                            "shoulder_width_trim_red",
                            "shoulder_width_trim_green",
                            "shoulder_width_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(mark("Dye Separation"), "exposure", &["dye_separation"]),
                channels(
                    row(
                        mark("Dye Separation Trim"),
                        "exposure",
                        &[
                            "dye_separation_trim_red",
                            "dye_separation_trim_green",
                            "dye_separation_trim_blue",
                        ],
                    ),
                    "RGB",
                ),
                row(
                    mark("Separation Damping"),
                    "exposure",
                    &["separation_damping"],
                ),
            ],
        ),
        (
            mark("Filtration"),
            vec![
                row(mark("Cyan"), "exposure", &["wb_cyan"]),
                row(mark("Magenta"), "exposure", &["wb_magenta"]),
                row(mark("Yellow"), "exposure", &["wb_yellow"]),
                channels(
                    row(
                        mark("Shadows CMY"),
                        "exposure",
                        &["shadow_cyan", "shadow_magenta", "shadow_yellow"],
                    ),
                    "CMY",
                ),
                channels(
                    row(
                        mark("Highlights CMY"),
                        "exposure",
                        &["highlight_cyan", "highlight_magenta", "highlight_yellow"],
                    ),
                    "CMY",
                ),
                sticky(row(
                    mark("Cast Removal"),
                    "exposure",
                    &["cast_removal_strength"],
                )),
                row(mark("Filter"), "exposure", &["bw_filter"]),
            ],
        ),
        (
            mark("Lab"),
            vec![
                sticky(row(mark("Chroma"), "lab", &["saturation"])),
                sticky(row(mark("Skin Protection"), "lab", &["skin_protection"])),
                sticky(row(mark("CLAHE"), "lab", &["clahe_strength"])),
                sticky(row(mark("Sharpening"), "lab", &["sharpen"])),
                sticky(row(mark("Sharpen Method"), "lab", &["sharpen_method"])),
                sticky(row(mark("Sharpen Radius"), "lab", &["sharpen_radius"])),
                sticky(row(mark("Sharpen Masking"), "lab", &["sharpen_masking"])),
                sticky(row(mark("Auto Masking"), "lab", &["sharpen_auto_masking"])),
                sticky(row(mark("Chroma Denoise"), "lab", &["chroma_denoise"])),
                sticky(row(mark("Glow"), "lab", &["glow_amount"])),
                sticky(row(mark("Halation"), "lab", &["halation_strength"])),
            ],
        ),
        (
            mark("Alternative Processes"),
            vec![
                row(mark("Process"), "altproc", &["alt_process"]),
                row(mark("Lith Exposure"), "altproc", &["lith_exposure"]),
                row(mark("Snatch Point"), "altproc", &["lith_snatch"]),
                row(mark("Abruptness"), "altproc", &["lith_abruptness"]),
                row(mark("Sensitizer"), "altproc", &["cyano_sensitizer"]),
                row(mark("Cyanotype Exposure"), "altproc", &["cyano_exposure"]),
                row(mark("Exposure Scale"), "altproc", &["cyano_scale"]),
                row(mark("Bleach"), "altproc", &["cyano_bleach"]),
                row(mark("Tannin"), "altproc", &["cyano_tannin"]),
                row(
                    mark("Sabattier Strength"),
                    "altproc",
                    &["sabattier_strength"],
                ),
                row(mark("Re-exposure"), "altproc", &["sabattier_reexposure"]),
                row(mark("Agitation"), "altproc", &["sabattier_agitation"]),
            ],
        ),
        (
            mark("Toning"),
            vec![
                row(mark("Selenium"), "toning", &["selenium_strength"]),
                row(mark("Sepia"), "toning", &["sepia_strength"]),
                row(mark("Gold"), "toning", &["gold_strength"]),
                row(mark("Iron Blue"), "toning", &["blue_strength"]),
                row(mark("Copper"), "toning", &["copper_strength"]),
                row(mark("Vanadium"), "toning", &["vanadium_strength"]),
                row(mark("Shadow Hue"), "toning", &["shadow_tint_hue"]),
                row(mark("Shadow Strength"), "toning", &["shadow_tint_strength"]),
                row(mark("Highlight Hue"), "toning", &["highlight_tint_hue"]),
                row(
                    mark("Highlight Strength"),
                    "toning",
                    &["highlight_tint_strength"],
                ),
            ],
        ),
        (
            mark("Finishing"),
            vec![
                row(mark("Vignette Burn"), "finish", &["vignette_stops"]),
                row(mark("Vignette Size"), "finish", &["vignette_size"]),
                row(
                    mark("Vignette Roundness"),
                    "finish",
                    &["vignette_roundness"],
                ),
                row(mark("Carrier Width"), "finish", &["carrier_width"]),
                row(mark("Carrier Roughness"), "finish", &["carrier_rough"]),
                row(mark("Carrier Flare"), "finish", &["carrier_flare"]),
                row(mark("Carrier Corners"), "finish", &["carrier_corner"]),
                row(mark("Border Width"), "finish", &["border_size"]),
                row(mark("Border Color"), "finish", &["border_color"]),
                row(
                    mark("Border Bottom Weight"),
                    "finish",
                    &["border_bottom_weight"],
                ),
                row(
                    mark("Border Paper White"),
                    "finish",
                    &["border_match_paper"],
                ),
            ],
        ),
        (
            mark("Retouch"),
            vec![
                sticky(row(mark("Optical Removal"), "retouch", &["dust_remove"])),
                row(
                    mark("Optical Spot Threshold"),
                    "retouch",
                    &["dust_threshold"],
                ),
                row(
                    mark("Optical Hair Threshold"),
                    "retouch",
                    &["dust_hair_threshold"],
                ),
                row(mark("Optical Size"), "retouch", &["dust_size"]),
                row(mark("IR Removal"), "retouch", &["ir_dust_remove"]),
                row(mark("IR Threshold"), "retouch", &["ir_threshold"]),
                row(mark("IR Attenuation"), "retouch", &["ir_attenuation"]),
            ],
        ),
        (
            mark("Metadata"),
            vec![
                formatted(
                    row(mark("Analog Gear"), "metadata", GEAR_FIELDS),
                    RowFormat::Gear,
                ),
                row(mark("Capture Date"), "metadata", &["capture_date"]),
                formatted(
                    row(
                        mark("Location"),
                        "metadata",
                        &[
                            "location_city",
                            "location_state",
                            "location_country",
                            "gps_latitude",
                            "gps_longitude",
                        ],
                    ),
                    RowFormat::Default,
                ),
                formatted(
                    row(mark("Process"), "metadata", PROCESS_FIELDS),
                    RowFormat::Process,
                ),
                row(mark("Scanning"), "metadata", &["scanning"]),
                row(mark("Roll"), "metadata", &["capture_roll"]),
                row(mark("Exposure"), "metadata", &["exposure_override"]),
                sticky(row(
                    mark("Protect Original Metadata"),
                    "metadata",
                    &["protect_original_metadata"],
                )),
                sticky(row(
                    mark("Description Fields"),
                    "metadata",
                    &["description_fields"],
                )),
            ],
        ),
        (
            mark("Export"),
            vec![
                sticky(row(mark("Format"), "export", &["export_fmt"])),
                sticky(row(mark("Bit Depth"), "export", &["export_bit_depth"])),
                sticky(row(mark("JPEG Quality"), "export", &["jpeg_quality"])),
                sticky(row(
                    mark("JPEG Progressive"),
                    "export",
                    &["jpeg_progressive"],
                )),
                sticky(row(
                    mark("TIFF Compression"),
                    "export",
                    &["tiff_compression"],
                )),
                sticky(row(
                    mark("PNG Compression"),
                    "export",
                    &["png_compress_level"],
                )),
                sticky(row(mark("JXL Lossless"), "export", &["jxl_lossless"])),
                sticky(row(mark("JXL Distance"), "export", &["jxl_distance"])),
                sticky(row(mark("JXL Effort"), "export", &["jxl_effort"])),
                sticky(row(mark("WebP Quality"), "export", &["webp_quality"])),
                sticky(row(mark("WebP Lossless"), "export", &["webp_lossless"])),
                sticky(row(mark("WebP Method"), "export", &["webp_method"])),
                sticky(row(
                    mark("Size Mode"),
                    "export",
                    &["export_resolution_mode"],
                )),
                sticky(row(mark("Paper Ratio"), "export", &["paper_aspect_ratio"])),
                sticky(row(mark("Print Size"), "export", &["export_print_size"])),
                sticky(row(mark("DPI"), "export", &["export_dpi"])),
                sticky(row(
                    mark("Long Edge"),
                    "export",
                    &["export_target_long_edge_px"],
                )),
                sticky(row(
                    mark("Export Profile"),
                    "export",
                    &["export_color_space"],
                )),
                sticky(row(
                    mark("Filename Pattern"),
                    "export",
                    &["filename_pattern"],
                )),
                sticky(row(
                    mark("Overwrite Existing Files"),
                    "export",
                    &["overwrite"],
                )),
                sticky(row(mark("Output Intent"), "export", &["output_mode"])),
                sticky(row(
                    mark("Save Sidecars on Export"),
                    "export",
                    &["export_sidecars_enabled"],
                )),
            ],
        ),
    ]
}

/// The fields a metadata row carries, in the Python tuple order.
pub const PROCESS_FIELDS: &[&str] = &[
    "developer",
    "process_dilution",
    "push_pull",
    "process_time_seconds",
    "process_temperature_c",
    "process_id",
];

/// The fade card's fields, including the crossover table and its tag.
pub const FADE_CARD_FIELDS: &[&str] = &[
    "fade_ratio_r",
    "fade_density_shift",
    "fade_ratio_g",
    "fade_ratio_b",
    "fade_white_anchor",
    "fade_white_r",
    "fade_white_g",
    "fade_white_b",
    "fade_white_space",
    "fade_crossover",
    "fade_crossover_table",
    "fade_crossover_space",
    "fade_auto_red",
    "fade_neutrals_mix",
];

/// The fade fields that feed the meters.
pub const FADE_METER_FIELDS: &[&str] = &[
    "fade_ratio_r",
    "fade_density_shift",
    "fade_ratio_g",
    "fade_ratio_b",
    "fade_white_anchor",
    "fade_white_r",
    "fade_white_g",
    "fade_white_b",
    "fade_white_space",
    "fade_crossover",
    "fade_auto_red",
    "fade_neutrals_mix",
];

/// Highlight Recovery's three displayed choices.
pub const HIGHLIGHT_LEVELS: [(i64, &str); 3] = [(0, "Off"), (2, "Blend"), (5, "Reconstruct")];

/// The displayed bucket for a stored highlight level.
#[must_use]
pub fn highlight_bucket(level: i64) -> usize {
    if !VALID_HIGHLIGHT_LEVELS.contains(&level) || level == 0 {
        return 0;
    }
    if level == 2 { 1 } else { 2 }
}

/// Every catalog section with its rows.
#[must_use]
pub fn catalog_sections() -> Vec<(&'static str, Vec<SettingRow>)> {
    catalog()
}

/// Every catalog row in display order.
#[must_use]
pub fn all_rows() -> Vec<SettingRow> {
    catalog().into_iter().flat_map(|(_, rows)| rows).collect()
}

/// Rows keyed by their stable id.
#[must_use]
pub fn rows_by_id() -> std::collections::HashMap<String, SettingRow> {
    all_rows().into_iter().map(|row| (row.id(), row)).collect()
}

/// Every row touching one of `fields`.
#[must_use]
pub fn rows_for_fields(fields: &[&str]) -> Vec<SettingRow> {
    all_rows()
        .into_iter()
        .filter(|row| row.fields.iter().any(|field| fields.contains(field)))
        .collect()
}

/// Every row on one config section.
#[must_use]
pub fn rows_for_section(section: &str) -> Vec<SettingRow> {
    all_rows()
        .into_iter()
        .filter(|row| row.section == section)
        .collect()
}

/// Every row for a Roll card, or none when the card owns no catalog row.
#[must_use]
pub fn roll_card_rows(card_keys: &[&str]) -> Vec<SettingRow> {
    let fields: Vec<&str> = card_keys
        .iter()
        .filter_map(|card| card_fields(card))
        .flat_map(|(_, fields)| fields.iter().copied())
        .collect();
    rows_for_fields(&fields)
}

/// Every row for a frame card: its field tuple, or its whole config section.
#[must_use]
pub fn frame_card_rows(card_key: &str) -> Vec<SettingRow> {
    match FRAME_CARD_FIELDS.iter().find(|(key, _)| *key == card_key) {
        Some((_, None)) => rows_for_section(card_key),
        Some((_, Some(fields))) => rows_for_fields(fields),
        None => Vec::new(),
    }
}

/// The fields each frame card owns. `None` means the card owns its whole section.
pub const FRAME_CARD_FIELDS: &[(&str, Option<&[&str]>)] = &[
    ("autocrop", Some(CROP_FIELDS)),
    ("geometry", Some(GEOMETRY_FIELDS)),
    ("color", Some(COLOR_FIELDS)),
    ("tone", Some(TONE_FIELDS)),
    ("paper", Some(PAPER_FIELDS)),
    ("lab", None),
    ("altproc", None),
    ("toning", None),
    ("retouch", None),
    ("finish", None),
];

/// Filtration fields.
pub const COLOR_FIELDS: &[&str] = &[
    "wb_cyan",
    "wb_magenta",
    "wb_yellow",
    "shadow_cyan",
    "shadow_magenta",
    "shadow_yellow",
    "highlight_cyan",
    "highlight_magenta",
    "highlight_yellow",
    "cast_removal_strength",
    "bw_filter",
];

/// Framing fields.
pub const GEOMETRY_FIELDS: &[&str] = &[
    "rotation",
    "fine_rotation",
    "flip_horizontal",
    "flip_vertical",
    "converge_v",
    "converge_h",
    "crop_to_valid",
];

/// Crop fields.
pub const CROP_FIELDS: &[&str] = &[
    "crop_rect",
    "crop_from_auto",
    "crop_detect_key",
    "autocrop_mode",
    "autocrop_offset",
    "autocrop_rebate_trim",
    "autocrop_ratio",
    "autocrop_square",
];

/// Tone fields.
pub const TONE_FIELDS: &[&str] = &[
    "density",
    "grade",
    "grade_trim_red",
    "grade_trim_green",
    "grade_trim_blue",
    "shadow_density",
    "hold_black",
    "highlight_density",
    "shadow_grade",
    "highlight_grade",
    "shadow_grade_trim_red",
    "shadow_grade_trim_green",
    "shadow_grade_trim_blue",
    "highlight_grade_trim_red",
    "highlight_grade_trim_green",
    "highlight_grade_trim_blue",
    "auto_exposure",
    "auto_normalize_contrast",
    "contrast_mask_strength",
    "diffusion",
    "preflash",
];

/// Paper Response fields.
pub const PAPER_FIELDS: &[&str] = &[
    "paper_profile",
    "paper_black",
    "paper_dmin",
    "midtone_gamma",
    "midtone_gamma_trim_red",
    "midtone_gamma_trim_green",
    "midtone_gamma_trim_blue",
    "toe",
    "toe_width",
    "toe_trim_red",
    "toe_trim_green",
    "toe_trim_blue",
    "toe_width_trim_red",
    "toe_width_trim_green",
    "toe_width_trim_blue",
    "shoulder",
    "shoulder_width",
    "shoulder_trim_red",
    "shoulder_trim_green",
    "shoulder_trim_blue",
    "shoulder_width_trim_red",
    "shoulder_width_trim_green",
    "shoulder_width_trim_blue",
    "dye_separation",
    "dye_separation_trim_red",
    "dye_separation_trim_green",
    "dye_separation_trim_blue",
    "separation_damping",
];

/// Fields that decide how a frame is metered.
pub const BOUNDS_INPUT_FIELDS: &[&str] = &[
    "process_mode",
    "analysis_buffer",
    "luma_range_clip",
    "color_range_clip",
    "use_luma_average",
    "use_color_average",
    "crosstalk_strength",
    "crosstalk_profile",
    "crosstalk_matrix",
    "sensor_profile",
    "sensor_matrix",
    "sensor_unmix",
    "linear_raw",
    "source_encoding",
    "highlight_reconstruction",
    "demosaic_preview",
    "crop_from_auto",
    "autocrop_offset",
    "autocrop_rebate_trim",
    "autocrop_mode",
    "crop_rect",
    "crop_detect_key",
];

/// The rows selected out of the box.
#[must_use]
pub fn default_sticky_ids() -> std::collections::HashSet<String> {
    all_rows()
        .into_iter()
        .filter(|row| row.sticky)
        .map(|row| row.id())
        .collect()
}

/// Config attrs whose rows carry even onto a file with a saved edit.
pub const GLOBAL_TIER_SECTIONS: [&str; 2] = ["export", "metadata"];

/// The flat value of one field on a config.
#[must_use]
pub fn field_value(config: &WorkspaceConfig, section: &str, field: &str) -> Value {
    let mut flat = config.to_dict();
    if section == "local" {
        return flat.remove("local_masks").unwrap_or(Value::Null);
    }
    flat.remove(field).unwrap_or(Value::Null)
}

/// A flat dict of the chosen rows' fields. A row's fields travel as a unit.
#[must_use]
pub fn selected_flat_dict(config: &WorkspaceConfig, rows: &[SettingRow]) -> Map<String, Value> {
    let mut flat = Map::new();
    for row in rows {
        for field in row.fields {
            flat.insert(
                (*field).to_string(),
                field_value(config, row.section, field),
            );
        }
    }
    if flat.contains_key("crop_rect") {
        flat.insert(
            "crop_from_auto".to_string(),
            json!(config.geometry.crop_from_auto),
        );
    }
    flat
}

/// Overlay only the chosen rows' fields from `source` onto `target`.
#[must_use]
pub fn apply_selected_fields(
    source: &WorkspaceConfig,
    target: &WorkspaceConfig,
    rows: &[SettingRow],
) -> WorkspaceConfig {
    let mut by_section: std::collections::HashMap<&str, Map<String, Value>> =
        std::collections::HashMap::new();
    for row in rows {
        let changes = by_section.entry(row.section).or_default();
        for field in row.fields {
            changes.insert(
                (*field).to_string(),
                field_value(source, row.section, field),
            );
        }
    }
    if let Some(process) = by_section.get_mut("process")
        && process
            .get("fade_white_space")
            .and_then(Value::as_str)
            .is_some_and(|space| !space.is_empty())
    {
        for field in crate::services::assets::rolls::DECODE_WHITE_FIELDS {
            process.remove(*field);
        }
    }
    let mut out = target.clone();
    if let Some(process) = by_section.get_mut("process") {
        out = with_film_fields(out, process);
    }
    if let Some(geometry) = by_section.get_mut("geometry") {
        if geometry.contains_key("crop_rect") {
            geometry.insert(
                "crop_from_auto".to_string(),
                json!(source.geometry.crop_from_auto),
            );
        } else if geometry.contains_key("crop_from_auto") {
            out = with_auto_crop_field(out, geometry);
            geometry.remove("crop_from_auto");
        }
    }
    out = merge_fields(out, by_section);
    if rows.iter().any(|row| {
        row.fields
            .iter()
            .any(|field| BOUNDS_INPUT_FIELDS.contains(field))
    }) {
        let (invalidate, floors, ceils) =
            crate::domain::models::process::invalidate_local_bounds(&out.process);
        if invalidate {
            out.process.local_floors = floors;
            out.process.local_ceils = ceils;
        }
    }
    out
}

fn merge_fields(
    mut config: WorkspaceConfig,
    fields: std::collections::HashMap<&str, Map<String, Value>>,
) -> WorkspaceConfig {
    for (section, changes) in fields {
        let mut flat = config.to_dict();
        for (key, value) in changes {
            flat.insert(key, value);
        }
        config = WorkspaceConfig::from_flat_dict(&flat);
        log::trace!("applied catalog fields on {section}");
    }
    config
}

/// A preset's stored fields over defaults.
#[must_use]
pub fn preset_config(data: &Map<String, Value>) -> WorkspaceConfig {
    let mut base = WorkspaceConfig::default().to_dict();
    for (key, value) in preset_fields(data) {
        base.insert(key, value);
    }
    WorkspaceConfig::from_flat_dict(&base)
}

/// The rows a preset stores, optionally narrowed to one section.
#[must_use]
pub fn rows_for_keys(data: &Map<String, Value>, section: &str) -> Vec<SettingRow> {
    let fields = preset_fields(data);
    all_rows()
        .into_iter()
        .filter(|row| {
            (section.is_empty() || row.section == section)
                && row.fields.iter().any(|field| fields.contains_key(*field))
        })
        .collect()
}

/// The line a preset apply leaves in its card.
#[must_use]
pub fn preset_applied_message(name: &str, extra: usize) -> String {
    if extra == 0 {
        return format!("{name} applied to this frame.");
    }
    format!("{name} applied to this frame and {extra} more.")
}

/// Each row a preset stores, as `(label, formatted value)`.
#[must_use]
pub fn preset_values(data: &Map<String, Value>, section: &str) -> Vec<(&'static str, String)> {
    let config = preset_config(data);
    rows_for_keys(data, section)
        .into_iter()
        .map(|row| (row.label, format_row(&row, &config)))
        .collect()
}

/// One line per display section listing the settings a preset stores.
#[must_use]
pub fn preset_summary(data: &Map<String, Value>) -> String {
    let mut lines = Vec::new();
    for (title, rows) in catalog() {
        let labels: Vec<&str> = rows
            .iter()
            .filter(|row| row.fields.iter().any(|field| data.contains_key(*field)))
            .map(|row| row.label)
            .collect();
        if !labels.is_empty() {
            lines.push(format!("{title}: {}", labels.join(", ")));
        }
    }
    lines.join("\n")
}

/// The formatted value of one row.
#[must_use]
pub fn format_row(row: &SettingRow, config: &WorkspaceConfig) -> String {
    let values: Vec<Value> = row
        .fields
        .iter()
        .map(|field| field_value(config, row.section, field))
        .collect();
    match row.kind {
        RowFormat::SourceEncoding => {
            let encoding = values
                .first()
                .and_then(Value::as_str)
                .map(SourceEncoding::coerce)
                .unwrap_or_default();
            tr(SOURCE_ENCODING_LABELS[encoding as usize], &[])
        }
        RowFormat::PrintStock => {
            let key = values.first().and_then(Value::as_str).unwrap_or("");
            effective_paper_profile(key, None).label.to_string()
        }
        RowFormat::Process => format_process(&values),
        RowFormat::Gear => format_gear(&values),
        RowFormat::FlatField => format_flatfield(&values),
        RowFormat::WhiteBalance => tr(
            if values.first().and_then(Value::as_bool).unwrap_or(false) {
                "Linear RAW"
            } else {
                "Camera WB"
            },
            &[],
        ),
        RowFormat::Highlights => {
            let level = values.first().and_then(Value::as_i64).unwrap_or(0);
            tr(HIGHLIGHT_LEVELS[highlight_bucket(level)].1, &[])
        }
        RowFormat::Scalar => format_scalar(values.first().unwrap_or(&Value::Null)),
        RowFormat::Default => format_default(row, &values),
    }
}

fn format_default(row: &SettingRow, values: &[Value]) -> String {
    if values.len() == 1 {
        return format_scalar(&values[0]);
    }
    let channel_count = row.channels.chars().count();
    if channel_count == values.len() {
        return row
            .channels
            .chars()
            .zip(values)
            .map(|(channel, value)| format!("{channel}{}", format_scalar(value)))
            .collect::<Vec<_>>()
            .join(" ");
    }
    values
        .iter()
        .map(format_scalar)
        .collect::<Vec<_>>()
        .join(" / ")
}

fn format_scalar(value: &Value) -> String {
    match value {
        Value::Bool(on) => tr(if *on { "On" } else { "Off" }, &[]),
        Value::Null => "—".to_string(),
        Value::Number(number) => {
            let rounded = (number.as_f64().unwrap_or(0.0) * 10_000.0).round() / 10_000.0;
            format_general(rounded)
        }
        Value::Array(_) | Value::Object(_) => tr("Set", &[]),
        Value::String(text) => text.clone(),
    }
}

fn format_general(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let magnitude = value.abs();
    if !(1e-4..1e6).contains(&magnitude) {
        let mut text = format!("{value:e}");
        if let Some((mantissa, exponent)) = text.split_once('e') {
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            let exponent = exponent.trim_start_matches('+');
            let exponent = exponent.trim_start_matches('0');
            text = format!(
                "{mantissa}e{}{exponent}",
                if value < 0.0 { "" } else { "+" }
            );
        }
        return text;
    }
    let rounded = format!("{value:.4}");
    rounded
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn format_process(values: &[Value]) -> String {
    let text = |index: usize| values.get(index).and_then(Value::as_str).unwrap_or("");
    let recipe = [text(0), text(1)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let push = values.get(2).and_then(Value::as_i64).unwrap_or(0);
    let push_label = match push {
        -3 => "Pull -3",
        -2 => "Pull -2",
        -1 => "Pull -1",
        0 => "Normal",
        1 => "Push +1",
        2 => "Push +2",
        3 => "Push +3",
        _ => "",
    };
    if recipe.is_empty() {
        return if push == 0 {
            "—".to_string()
        } else {
            tr(push_label, &[])
        };
    }
    if push == 0 {
        recipe
    } else {
        format!("{recipe} · {}", tr(push_label, &[]))
    }
}

fn format_gear(values: &[Value]) -> String {
    let value = |name: &str| {
        let index = GEAR_FIELDS.iter().position(|field| *field == name);
        index
            .and_then(|index| values.get(index))
            .map(|value| match value {
                Value::String(text) => text.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    let film_format = if value("format") == "Other" && !value("format_other").is_empty() {
        value("format_other")
    } else {
        value("format")
    };
    let parts = [
        [value("camera_make"), value("camera_model")]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        [value("lens_make"), value("lens_model")]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        value("film"),
        film_format,
    ];
    let summary = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if summary.is_empty() {
        "—".to_string()
    } else {
        summary
    }
}

fn format_flatfield(values: &[Value]) -> String {
    let apply = values.first().and_then(Value::as_bool).unwrap_or(false);
    if !apply {
        return format_scalar(&Value::Bool(false));
    }
    let profile = values.get(1).and_then(Value::as_str).unwrap_or("");
    if profile.is_empty() {
        format_scalar(&Value::Bool(true))
    } else {
        profile.to_string()
    }
}

/// The card a change lives on when no catalog row lists its field.
#[must_use]
pub fn unlisted_card(section: &str) -> &'static str {
    match section {
        "process" => mark("Metering"),
        "exposure" | "export" => mark("Export"),
        "flatfield" => mark("Optics"),
        "rgbscan" => mark("Frame Assembly"),
        "stitch" => mark("Stitch"),
        "hdr" => mark("HDR"),
        "geometry" => mark("Crop"),
        "local" => mark("Dodge & Burn"),
        "retouch" => mark("Retouch"),
        "metadata" => mark("Metadata"),
        other => Box::leak(other.to_string().into_boxed_str()),
    }
}

/// The `(card, control)` entries that differ between two configs.
#[must_use]
pub fn changed_settings(
    prev: &WorkspaceConfig,
    config: &WorkspaceConfig,
) -> Vec<(&'static str, &'static str)> {
    let mut changed: Vec<(&'static str, &'static str)> = catalog()
        .into_iter()
        .flat_map(|(title, rows)| {
            rows.into_iter().filter_map(move |row| {
                row.fields
                    .iter()
                    .any(|field| {
                        field_value(prev, row.section, field)
                            != field_value(config, row.section, field)
                    })
                    .then_some((title, row.label))
            })
        })
        .collect();
    let before = prev.to_dict();
    let after = config.to_dict();
    let listed: std::collections::HashSet<&str> = all_rows()
        .iter()
        .flat_map(|row| row.fields.iter().copied())
        .collect();
    let measured: std::collections::HashSet<&str> =
        ["local_floors", "local_ceils", "meter_key", "source_color"]
            .into_iter()
            .collect();
    for (key, value) in after {
        if before.get(&key) != Some(&value)
            && !listed.contains(key.as_str())
            && !measured.contains(key.as_str())
        {
            let section = section_for_key(&key);
            changed.push((unlisted_card(section), ""));
        }
    }
    changed
}

fn section_for_key(key: &str) -> &'static str {
    match key {
        "local_masks" => "local",
        key if key.starts_with("fade_")
            || key.starts_with("sensor_")
            || key.starts_with("crosstalk_") =>
        {
            "process"
        }
        key if key.starts_with("crop")
            || key.starts_with("autocrop")
            || key.starts_with("rotation")
            || key.starts_with("converge")
            || key.starts_with("flip")
            || key.starts_with("distortion")
            || key.starts_with("lens_") =>
        {
            "geometry"
        }
        key if key.starts_with("wb_")
            || key.starts_with("shadow_")
            || key.starts_with("highlight_")
            || key.starts_with("toe")
            || key.starts_with("shoulder")
            || key.starts_with("paper_")
            || key.starts_with("midtone_")
            || key.starts_with("dye_")
            || key.starts_with("separation_")
            || key.starts_with("cast_")
            || key.starts_with("auto_")
            || key.starts_with("bw_")
            || key.starts_with("preflash")
            || key.starts_with("diffusion")
            || key.starts_with("density")
            || key.starts_with("grade") =>
        {
            "exposure"
        }
        key if key.starts_with("sharpen")
            || key.starts_with("chroma_")
            || key.starts_with("clahe")
            || key.starts_with("glow")
            || key.starts_with("halation")
            || key.starts_with("saturation")
            || key.starts_with("skin_") =>
        {
            "lab"
        }
        key if key.starts_with("alt_")
            || key.starts_with("lith_")
            || key.starts_with("cyano_")
            || key.starts_with("sabattier_") =>
        {
            "altproc"
        }
        key if key.ends_with("_strength")
            && (key.starts_with("selenium")
                || key.starts_with("sepia")
                || key.starts_with("gold")
                || key.starts_with("blue")
                || key.starts_with("copper")
                || key.starts_with("vanadium")
                || key.starts_with("shadow_tint")
                || key.starts_with("highlight_tint")) =>
        {
            "toning"
        }
        key if key.starts_with("vignette")
            || key.starts_with("carrier_")
            || key.starts_with("border_") =>
        {
            "finish"
        }
        key if key.starts_with("dust_")
            || key.starts_with("ir_")
            || key.starts_with("clone_")
            || key.starts_with("scratch_")
            || key.starts_with("manual_") =>
        {
            "retouch"
        }
        key if key.starts_with("export_")
            || key == "jpeg_quality"
            || key == "jpeg_progressive"
            || key == "tiff_compression"
            || key == "png_compress_level"
            || key.starts_with("jxl_")
            || key.starts_with("webp_")
            || key == "paper_aspect_ratio"
            || key == "filename_pattern"
            || key == "overwrite"
            || key == "output_mode"
            || key == "output_subfolder"
            || key == "icc_input_path"
            || key == "icc_output_path"
            || key.starts_with("contact_sheet_")
            || key == "userDir" =>
        {
            "export"
        }
        _ => "metadata",
    }
}

/// `unused_in_mode` for a B&W frame or a default paper profile.
#[must_use]
pub fn unused_in_mode(section: &str, field: &str, config: &WorkspaceConfig) -> bool {
    let mode = config.process.process_mode;
    if section == "exposure" && field == "paper_profile" {
        return effective_paper_profile(&config.exposure.paper_profile, Some(mode)).kind
            == "default";
    }
    section == "lab"
        && ["saturation", "skin_protection", "chroma_denoise"].contains(&field)
        && is_mono(&config.process) != matches!(mode, ProcessMode::C41)
}
