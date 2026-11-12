//! Backward-compat migrations for persisted configs (edits rows, sidecars, presets).
//!
//! Mirrors `negpy/domain/migrations.py`. Every legacy key rename, value rewrite and
//! retired-field drop belongs here, never inline in the deserializer. The config payload
//! is a JSON object, so a migration reads and rewrites a field map by name.

use serde_json::{Map, Value, json};

/// A flat config payload: JSON field name to JSON value, as a persisted config stores it.
pub type FlatConfig = Map<String, Value>;

/// Old field name -> new field name.
pub fn key_renames() -> Vec<(&'static str, &'static str)> {
    vec![
        ("export_border_size", "border_size"),
        ("export_border_color", "border_color"),
        ("auto_shadow_neutral", "cast_removal_strength"),
        ("cast_removal", "cast_removal_strength"),
        ("drange_clip", "luma_range_clip"),
        ("density_saturation", "dye_separation"),
        ("density_saturation_trim_red", "dye_separation_trim_red"),
        ("density_saturation_trim_green", "dye_separation_trim_green"),
        ("density_saturation_trim_blue", "dye_separation_trim_blue"),
        ("use_colour_average", "use_color_average"),
        ("manual_crop_rect", "crop_rect"),
        ("auto_crop_enabled", "crop_from_auto"),
        ("k1", "distortion_k1"),
    ]
}

/// Fields removed over time. Old saves still carry them, so drop them silently.
pub const DROPPED_KEYS: &[&str] = &[
    "flare",
    "ir_inpaint_radius",
    "ir_method",
    "auto_cast_removal",
    "DEFAULT_MATRIX",
    "surround",
    "density_chroma_damping",
    "density_divergence_damping",
    "chroma_damping",
    "density_saturation_damping",
    "density_damping_spatial",
    "vibrance",
    "reference_path",
    "lith_enabled",
    "gear_preset_id",
    "dust_exclusions",
    "mask_blur",
    "mask_spacer",
    "e6_normalize",
    "sync_to_batch",
    "narrowband_scan",
    "output_sharpen",
    "contact_sheet_cell_px",
    "contact_sheet_gap",
    "contact_sheet_margin",
    "contact_sheet_max_tiles",
    "contact_sheet_show_labels",
    "contact_sheet_background_color",
    "contact_sheet_label_color",
    "contact_sheet_template",
    "contact_sheet_default_cell_px",
    "contact_sheet_default_gap",
    "contact_sheet_default_margin",
    "contact_sheet_default_max_tiles",
    "contact_sheet_default_show_labels",
    "contact_sheet_default_background_color",
    "contact_sheet_default_label_color",
    "export_bake_auto_density_grade",
];

/// Retired export formats -> the closest surviving one. DNG maps to TIFF, the other
/// 16-bit master, so a saved DNG preset keeps its bit depth.
pub fn retired_export_formats() -> Vec<(&'static str, &'static str)> {
    vec![("DNG", "TIFF")]
}

/// Crosstalk profile display names, old -> current.
pub fn crosstalk_profile_renames() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Default", "Generic C41"),
        (
            "Kodak Portra 160 (approx)",
            "Kodak Ektacolor Pro 160 / Portra 160 (approx)",
        ),
        (
            "Kodak Portra 400 (approx)",
            "Kodak Ektacolor Pro 400 / Portra 400 (approx)",
        ),
        (
            "Kodak Portra 800 (approx)",
            "Kodak Ektacolor Pro 800 / Portra 800 (approx)",
        ),
    ]
}

/// Map a retired export format to its replacement.
#[must_use]
pub fn migrate_export_fmt(fmt: &str) -> String {
    retired_export_formats()
        .iter()
        .find(|(old, _)| *old == fmt)
        .map_or_else(|| fmt.to_string(), |(_, new)| (*new).to_string())
}

/// Rewrite a flat config from any older version in place. Value rewrites run before
/// [`DROPPED_KEYS`] is applied, because several of them read a key on their way to
/// deleting it.
pub fn migrate_flat_config(data: &mut FlatConfig) {
    // The per-pixel Dye Separation merged into the frame-wide control and took its name,
    // so an old signed value would read as heavy desaturation. A dict carrying
    // density_saturation predates the merge, and anything under the new floor can only be
    // the old signed control. Not in DROPPED_KEYS: those pops run last and would take it back.
    if data.contains_key("density_saturation") || number(data, "dye_separation", 1.0) < 0.25 {
        data.remove("dye_separation");
    }
    data.remove("density_vibrance");

    for (old_key, new_key) in key_renames() {
        if let Some(value) = data.remove(old_key) {
            data.insert(new_key.to_string(), value);
        }
    }

    // Slide Normalize retired. A normalized slide keeps a metered render: Auto Density and
    // Auto Grade on. On a raw slide both were inert, so they start off as a slide now does.
    if let Some(value) = data.get("e6_normalize") {
        let enabled = truthy(value);
        let process_mode = string(data, "process_mode");
        if matches!(process_mode.as_str(), "Transparency" | "E-6") {
            if enabled {
                data.insert("auto_exposure".into(), json!(true));
                data.insert("auto_normalize_contrast".into(), json!(true));
            } else if !data.get("positive_source").is_some_and(truthy) {
                data.insert("auto_exposure".into(), json!(false));
                data.insert("auto_normalize_contrast".into(), json!(false));
            }
        }
    }

    // The linear Contrast Mask (gamma ±0.5) became the one-way tone mask (0..1). Its
    // positive side maps across the range; the negative contrast boost has no equivalent.
    if let Some(value) = data.remove("contrast_mask") {
        let legacy = value.as_f64().unwrap_or(0.0);
        data.entry("contrast_mask_strength".to_string())
            .or_insert_with(|| json!(f64::min(f64::max(legacy, 0.0) / 0.5, 1.0)));
    }

    // True Black (BPC on) renamed to Paper Black with inverted polarity.
    if let Some(value) = data.remove("true_black") {
        data.insert("paper_black".into(), json!(!truthy(&value)));
    }

    // Single roll-average toggle split into independent luma + color axes.
    if let Some(value) = data.remove("use_roll_average") {
        let legacy = truthy(&value);
        data.entry("use_luma_average".to_string())
            .or_insert_with(|| json!(legacy));
        data.entry("use_color_average".to_string())
            .or_insert_with(|| json!(legacy));
    }

    if let Some(value) = data.remove("lens_from_metadata") {
        let legacy = truthy(&value);
        data.entry("lens_distortion_from_metadata".to_string())
            .or_insert_with(|| json!(legacy));
        data.entry("lens_ca_from_metadata".to_string())
            .or_insert_with(|| json!(legacy));
    }

    // Lab "Separation" moved to ProcessConfig crosstalk: the 1.0-2.0 slider maps to 0-1.
    if let Some(value) = data.get("color_separation").cloned()
        && !data.contains_key("crosstalk_strength")
    {
        let strength = (value.as_f64().unwrap_or(0.0) - 1.0).clamp(0.0, 1.0);
        data.insert("crosstalk_strength".into(), json!(strength));
    }
    data.remove("color_separation");

    // Vignette became an exposure-domain burn: old ±1 strength maps to stops.
    if let Some(value) = data.get("vignette_strength").cloned()
        && !data.contains_key("vignette_stops")
    {
        data.insert(
            "vignette_stops".into(),
            json!(-2.0 * value.as_f64().unwrap_or(0.0)),
        );
    }
    data.remove("vignette_strength");

    // The filed carrier's toggle folded into carrier_width (0 = off).
    if let Some(value) = data.remove("carrier_enabled")
        && !truthy(&value)
    {
        data.insert("carrier_width".into(), json!(0.0));
    }

    if let Some(value) = data.get("use_original_res").cloned()
        && !data.contains_key("export_resolution_mode")
    {
        data.insert(
            "export_resolution_mode".into(),
            json!(if truthy(&value) { "original" } else { "print" }),
        );
    }
    data.remove("use_original_res");

    if let Some(value) = data.get("same_as_source").cloned()
        && !data.contains_key("output_mode")
    {
        data.insert(
            "output_mode".into(),
            json!(if truthy(&value) {
                "same_as_source"
            } else {
                "absolute"
            }),
        );
    }
    data.remove("same_as_source");

    // Metadata: migrate the legacy combined override fields to structured gear fields.
    if data.get("camera_override").is_some_and(truthy) && !data.contains_key("camera_model") {
        let combined = string(data, "camera_override").trim().to_string();
        data.remove("camera_override");
        if !combined.is_empty() && !data.get("camera_make").is_some_and(truthy) {
            match combined.split_once(char::is_whitespace) {
                Some((make, model)) => {
                    data.insert("camera_make".into(), json!(make));
                    data.insert("camera_model".into(), json!(model.trim()));
                }
                None => {
                    data.insert("camera_model".into(), json!(combined));
                }
            }
        } else if !combined.is_empty() {
            data.insert("camera_model".into(), json!(combined));
        }
    } else {
        data.remove("camera_override");
    }
    if data.get("lens_override").is_some_and(truthy) && !data.contains_key("lens_model") {
        let lens = string(data, "lens_override").trim().to_string();
        data.remove("lens_override");
        data.insert("lens_model".into(), json!(lens));
    } else {
        data.remove("lens_override");
    }

    // Lith and Cyanotype are mutually exclusive, so the panel keeps one enum instead of a
    // bool each. The three lith_* sliders kept their names.
    if let Some(value) = data.get("lith_enabled").cloned()
        && !data.contains_key("alt_process")
    {
        data.insert(
            "alt_process".into(),
            json!(if truthy(&value) { "lith" } else { "none" }),
        );
    }

    // Narrowband applied RGBScan.icc on a negative's print, which the Pakon stock
    // reproduces on Digital's curve. Another stock or a slide or B&W edit keeps its paper.
    let process_mode = string(data, "process_mode");
    if data.get("narrowband_scan").is_some_and(truthy)
        && !matches!(
            process_mode.as_str(),
            "B&W Negative" | "B&W" | "Transparency" | "E-6"
        )
        && data
            .get("paper_profile")
            .is_none_or(|value| string_value(value) == "neutral")
    {
        data.insert("paper_profile".into(), json!("pakon"));
    }

    if let Some(value) = data.get("export_fmt").cloned() {
        data.insert(
            "export_fmt".into(),
            json!(migrate_export_fmt(&string_value(&value))),
        );
    }

    for key in DROPPED_KEYS {
        data.remove(*key);
    }
}

/// Whether a JSON value is truthy, matching Python's `bool`: `0`, `""`, `[]` and `{}` are
/// false and everything else is true, except that a JSON `null` is false.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

fn number(data: &FlatConfig, key: &str, default: f64) -> f64 {
    data.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn string(data: &FlatConfig, key: &str) -> String {
    data.get(key).map_or_else(String::new, string_value)
}

fn string_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        _ => value.to_string(),
    }
}

/// The crosstalk profile display names, old -> current, as a lookup a `__post_init__`
/// applies when it overlays raw roll defaults. Provided here so the map has one home.
#[must_use]
pub fn renamed_crosstalk_profile(name: &str) -> String {
    crosstalk_profile_renames()
        .iter()
        .find(|(old, _)| *old == name)
        .map_or_else(|| name.to_string(), |(_, new)| (*new).to_string())
}
