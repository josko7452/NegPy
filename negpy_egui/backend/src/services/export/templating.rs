//! Mirrors `negpy/services/export/templating.py`: export filename templates.

use std::collections::HashMap;

use minijinja::{Environment, ErrorKind, value::Value};

use crate::domain::models::{ExportConfig, ExportPreset, ExportResolutionMode, MetadataConfig};
use crate::features::metadata::capture::{format_dev_time, format_temperature, parse_capture_date};

/// Parse Scanlight `{roll}_Frame{NNN}` from a basename stem.
#[must_use]
pub fn parse_capture_stem(stem: &str) -> (String, Option<i64>) {
    let lower = stem.to_lowercase();
    let Some(index) = lower.find("_frame") else {
        return (String::new(), None);
    };
    let roll = &stem[..index];
    let digits: String = stem[index + 6..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if roll.is_empty() || digits.is_empty() {
        return (String::new(), None);
    }
    (roll.to_string(), digits.parse().ok())
}

/// Make a value safe for a filename or folder name.
#[must_use]
pub fn path_safe(value: &Value) -> String {
    if value.is_none() {
        return String::new();
    }
    let text = match value.as_str() {
        Some(text) => text.trim().to_string(),
        None if value.is_integer() => value
            .as_i64()
            .map_or_else(|| value.to_string(), |number| number.to_string()),
        None if value.is_number() => value
            .to_string()
            .parse::<f64>()
            .ok()
            .filter(|number| number.fract() == 0.0)
            .map_or_else(|| value.to_string(), |number| format!("{}", number as i64)),
        None => value.to_string(),
    };
    let cleaned: String = text
        .chars()
        .filter(|ch| {
            !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !ch.is_control()
        })
        .collect();
    cleaned.trim_matches([' ', '.']).to_string()
}

/// Zero-pad an integer for filenames; empty when unset.
#[must_use]
pub fn pad_filter(value: &Value, width: usize) -> String {
    let Some(number) = value.as_i64() else {
        return String::new();
    };
    format!("{number:0width$}")
}

fn film_format(metadata: &MetadataConfig) -> String {
    if metadata.format == "Other" {
        metadata.format_other.trim().to_string()
    } else {
        metadata.format.trim().to_string()
    }
}

fn metadata_context(
    original_stem: &str,
    metadata: Option<&MetadataConfig>,
) -> HashMap<String, Value> {
    let default = MetadataConfig::default();
    let meta = metadata.unwrap_or(&default);
    let (parsed_roll, parsed_frame) = parse_capture_stem(original_stem);
    let roll = if !meta.capture_roll.trim().is_empty() {
        meta.capture_roll.trim().to_string()
    } else {
        parsed_roll
    };
    let frame = meta.capture_frame.or(parsed_frame);
    let camera = format!("{} {}", meta.camera_make.trim(), meta.camera_model.trim())
        .trim()
        .to_string();
    let lens = if !meta.lens_model.trim().is_empty() {
        meta.lens_model.trim()
    } else {
        meta.lens_make.trim()
    };
    let captured = parse_capture_date(&meta.capture_date);
    let mut context = HashMap::new();
    let mut insert = |key: &str, value: Value| {
        context.insert(key.to_string(), value);
    };
    insert("roll", Value::from(path_safe(&Value::from(roll))));
    insert("frame", frame.map_or(Value::from(()), Value::from));
    insert(
        "frame_padded",
        Value::from(frame.map_or_else(String::new, |value| format!("{value:03}"))),
    );
    insert("camera", Value::from(path_safe(&Value::from(camera))));
    insert(
        "camera_make",
        Value::from(path_safe(&Value::from(meta.camera_make.clone()))),
    );
    insert(
        "camera_model",
        Value::from(path_safe(&Value::from(meta.camera_model.clone()))),
    );
    insert("lens", Value::from(path_safe(&Value::from(lens))));
    insert(
        "lens_make",
        Value::from(path_safe(&Value::from(meta.lens_make.clone()))),
    );
    insert(
        "lens_model",
        Value::from(path_safe(&Value::from(meta.lens_model.clone()))),
    );
    insert(
        "focal_length",
        Value::from(
            meta.focal_length_mm
                .map_or_else(String::new, |value| path_safe(&Value::from(value))),
        ),
    );
    insert(
        "film",
        Value::from(path_safe(&Value::from(meta.film.clone()))),
    );
    insert(
        "film_iso",
        Value::from(
            meta.film_iso
                .map_or_else(String::new, |value| value.to_string()),
        ),
    );
    insert(
        "film_manufacturer",
        Value::from(path_safe(&Value::from(meta.film_manufacturer.clone()))),
    );
    insert(
        "film_color_type",
        Value::from(path_safe(&Value::from(meta.film_color_type.clone()))),
    );
    insert(
        "film_format",
        Value::from(path_safe(&Value::from(film_format(meta)))),
    );
    insert(
        "developer",
        Value::from(path_safe(&Value::from(meta.developer.clone()))),
    );
    insert(
        "dilution",
        Value::from(path_safe(&Value::from(meta.process_dilution.clone()))),
    );
    insert("push_pull", Value::from(meta.push_pull));
    insert(
        "development_time",
        Value::from(path_safe(&Value::from(
            format_dev_time(meta.process_time_seconds).replace(':', "-"),
        ))),
    );
    insert(
        "development_temperature",
        Value::from(path_safe(&Value::from(format_temperature(
            meta.process_temperature_c,
        )))),
    );
    insert(
        "scanning",
        Value::from(path_safe(&Value::from(meta.scanning.clone()))),
    );
    insert(
        "exposure",
        Value::from(path_safe(&Value::from(meta.exposure_override.clone()))),
    );
    insert(
        "capture_date",
        Value::from(
            captured
                .as_ref()
                .map_or_else(String::new, |date| date.compact()),
        ),
    );
    insert(
        "capture_year",
        Value::from(
            captured
                .as_ref()
                .map_or_else(String::new, |date| date.year().to_string()),
        ),
    );
    drop(insert);
    context
}

/// The export settings both `ExportConfig` and `ExportPreset` provide.
struct TemplateSettings<'a> {
    mode: ExportResolutionMode,
    color_space: &'a str,
    format: &'a str,
    paper_ratio: &'a str,
    print_size: f64,
    dpi: i64,
    target_px: i64,
    pattern: &'a str,
}

macro_rules! settings_from {
    ($value:expr) => {
        TemplateSettings {
            mode: $value.export_resolution_mode,
            color_space: &$value.export_color_space,
            format: $value.export_fmt.as_str(),
            paper_ratio: &$value.paper_aspect_ratio,
            print_size: $value.export_print_size,
            dpi: $value.export_dpi,
            target_px: $value.export_target_long_edge_px,
            pattern: &$value.filename_pattern,
        }
    };
}

fn render(
    original_path: &str,
    settings: TemplateSettings<'_>,
    border_size: f64,
    half: i64,
    metadata: Option<&MetadataConfig>,
    composite: &str,
) -> String {
    let mut original_name = std::path::Path::new(original_path)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    if half != 0 {
        original_name = format!("{original_name}_{half}");
    }
    if !composite.is_empty() {
        original_name = format!("{original_name}-{composite}");
    }
    let placeholder = "\u{0}ORIG\u{0}";
    let is_print = settings.mode == ExportResolutionMode::Print;
    let is_target = settings.mode == ExportResolutionMode::TargetPx;
    let stem_for_parse = std::path::Path::new(original_path)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    let mut context = metadata_context(&stem_for_parse, metadata);
    context.insert("original_name".to_string(), Value::from(placeholder));
    context.insert("colorspace".to_string(), Value::from(settings.color_space));
    context.insert("format".to_string(), Value::from(settings.format));
    context.insert("paper_ratio".to_string(), Value::from(settings.paper_ratio));
    context.insert(
        "size".to_string(),
        Value::from(if is_print {
            format!("{:.0}cm", settings.print_size)
        } else {
            String::new()
        }),
    );
    context.insert(
        "dpi".to_string(),
        Value::from(if is_print {
            format!("{}dpi", settings.dpi)
        } else {
            String::new()
        }),
    );
    context.insert(
        "target_px".to_string(),
        Value::from(if is_target {
            format!("{}px", settings.target_px)
        } else {
            String::new()
        }),
    );
    context.insert(
        "border".to_string(),
        Value::from(if border_size > 0.0 { "border" } else { "" }),
    );
    context.insert(
        "date".to_string(),
        Value::from(chrono::Local::now().format("%Y%m%d").to_string()),
    );
    let mut env = Environment::new();
    env.add_filter("pad", |value: Value, width: Option<usize>| {
        pad_filter(&value, width.unwrap_or(3))
    });
    let rendered = match env.render_str(settings.pattern, &context) {
        Ok(value) => value,
        Err(error) if error.kind() == ErrorKind::UndefinedError => return original_name,
        Err(_) => return original_name,
    };
    let cleaned = clean_separators(&rendered);
    if cleaned.is_empty() {
        return original_name;
    }
    cleaned.replace(placeholder, &original_name)
}

fn clean_separators(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut separator = false;
    for ch in value.chars() {
        if matches!(ch, ' ' | '_' | '-') {
            separator = true;
        } else {
            if separator && !out.is_empty() {
                out.push('_');
            }
            separator = false;
            out.push(ch);
        }
    }
    out.trim_matches('_').to_string()
}

/// Render the export filename for one frame.
#[must_use]
pub fn render_export_filename(
    original_path: &str,
    export: &ExportConfig,
    border_size: f64,
    half: i64,
    metadata: Option<&MetadataConfig>,
    composite: &str,
) -> String {
    render(
        original_path,
        settings_from!(export),
        border_size,
        half,
        metadata,
        composite,
    )
}

/// Render the export filename against a preset.
#[must_use]
pub fn render_preset_filename(
    original_path: &str,
    preset: &ExportPreset,
    border_size: f64,
    half: i64,
    metadata: Option<&MetadataConfig>,
    composite: &str,
) -> String {
    render(
        original_path,
        settings_from!(preset),
        border_size,
        half,
        metadata,
        composite,
    )
}
