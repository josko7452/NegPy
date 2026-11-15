//! Mirrors `negpy/services/scanning/templating.py`: the scan filename template.

use minijinja::{Environment, ErrorKind};

/// The basename a scan template gives one capture. An invalid template or a failed render
/// falls back to `<date>_<seq>`.
#[must_use]
pub fn render_scan_filename(pattern: &str, date: &str, seq: i64) -> String {
    let fallback = format!("{date}_{seq:03}");
    let mut env = Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    let Ok(template) = env.template_from_str(pattern) else {
        return fallback;
    };
    let rendered = template.render(minijinja::context! { date => date, seq => seq });
    let Ok(rendered) = rendered else {
        return fallback;
    };
    let stripped = strip_tiff_extension(&rendered);
    let cleaned = clean_separators(&stripped);
    if cleaned.is_empty() {
        fallback
    } else {
        cleaned
    }
}

fn strip_tiff_extension(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for extension in [".tiff", ".tif"] {
        if lower.ends_with(extension) {
            return value[..value.len() - extension.len()].to_string();
        }
    }
    value.to_string()
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

/// Reject a template that cannot allocate a second basename.
pub fn require_sequence_varying_scan_filename(
    pattern: &str,
    date: &str,
    sequence: i64,
) -> Result<(), String> {
    if sequence < 1 {
        return Err("sequence must be a positive integer".to_string());
    }
    let current = render_scan_filename(pattern, date, sequence);
    let following = render_scan_filename(pattern, date, sequence + 1);
    if current == following {
        return Err(
            "filename pattern must produce a different basename when seq changes".to_string(),
        );
    }
    Ok(())
}

/// Whether `minijinja` rejected the template as malformed, as opposed to a failed render.
#[must_use]
pub fn is_template_syntax_error(pattern: &str) -> bool {
    let env = Environment::new();
    match env.template_from_str(pattern) {
        Ok(template) => template
            .render(minijinja::context! { date => "", seq => 1i64 })
            .err()
            .is_some_and(|error| error.kind() == ErrorKind::InvalidOperation),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_takes_the_date_and_the_sequence() {
        assert_eq!(
            render_scan_filename("scan_{{ date }}_{{ '%03d'|format(seq) }}", "2026-01-02", 7),
            "scan_2026_01_02_007"
        );
    }

    #[test]
    fn a_written_extension_is_dropped() {
        assert_eq!(
            render_scan_filename("scan_{{ seq }}.tif", "2026-01-02", 1),
            "scan_1"
        );
    }

    #[test]
    fn separators_collapse_to_one_underscore() {
        assert_eq!(
            render_scan_filename("a  b - c_{{ seq }}", "d", 2),
            "a_b_c_2"
        );
    }

    #[test]
    fn a_constant_template_is_rejected() {
        assert!(require_sequence_varying_scan_filename("scan", "2026-01-02", 1).is_err());
        assert!(require_sequence_varying_scan_filename("scan_{{ seq }}", "2026-01-02", 1).is_ok());
    }

    #[test]
    fn an_invalid_template_falls_back_to_the_date_and_sequence() {
        assert_eq!(render_scan_filename("{{", "2026-01-02", 3), "2026-01-02_003");
    }
}
