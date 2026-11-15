//! File-stem and TOML-string helpers.
//!
//! Mirrors `negpy/services/assets/naming.py`.

/// A lowercase underscore slug, or `fallback` when nothing remains.
#[must_use]
pub fn slugify(name: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch.is_whitespace() {
            out.push(if ch.is_whitespace() { ' ' } else { ch });
        }
    }
    let mut slug = String::with_capacity(out.len());
    let mut separator = false;
    for ch in out.chars() {
        if ch == '-' || ch == ' ' || ch == '_' {
            separator = true;
        } else {
            if separator && !slug.is_empty() {
                slug.push('_');
            }
            separator = false;
            slug.push(ch);
        }
    }
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

/// Escape the backslash and double quote a TOML basic string cannot carry raw.
#[must_use]
pub fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_lowercase_underscore_slug() {
        assert_eq!(slugify("Generic C41", "fallback"), "generic_c41");
        assert_eq!(
            slugify("  Kodak  Tri-X 400! ", "fallback"),
            "kodak_tri_x_400"
        );
        assert_eq!(slugify("!!!", "fallback"), "fallback");
    }

    #[test]
    fn a_toml_string_escapes_backslashes_and_quotes() {
        assert_eq!(escape_toml_string(r#"a\b"c"#), r#"a\\b\"c"#);
    }
}
