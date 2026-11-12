//! UI language, message catalogs and formatting.
//!
//! Mirrors `negpy/kernel/system/i18n.py`. The language is chosen at startup because
//! module constants translate on load, so `configure` runs before any consumer reads a
//! translated string and a change needs a restart. Catalogs load from the resource tree
//! at runtime, as the Python side reads them.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Mark source text in static catalogs; call [`tr`] when displaying it.
#[must_use]
pub fn mark(source: &str) -> &str {
    source
}

/// The selectable UI languages: the "system" choice first, then the shipped catalogs.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("system", "System Default"),
    ("en", "English"),
    ("zh_CN", "简体中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("de", "Deutsch"),
    ("vi", "Tiếng Việt"),
    ("nb", "Norsk bokmål"),
];

/// One placeholder of a message: its field name, format spec and conversion, all empty
/// when the message writes it as a bare `{name}`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Field {
    pub name: String,
    pub spec: String,
    pub conversion: Option<char>,
}

/// A value inserted into a message. A string is inserted verbatim, never re-formatted.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    Str(&'a str),
    Int(i64),
    Float(f64),
}

struct Catalog {
    language: String,
    messages: HashMap<String, String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            messages: HashMap::new(),
        }
    }
}

fn state() -> &'static RwLock<Catalog> {
    static STATE: OnceLock<RwLock<Catalog>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(Catalog::default()))
}

/// Resolve the UI language, with an English fallback for unsupported choices.
pub fn resolve_language(choice: &str, system_languages: Option<&[String]>) -> String {
    let mut choice = choice.to_string();
    if choice == "system" {
        let list = system_languages.unwrap_or(&[]);
        choice = list.first().cloned().unwrap_or_else(|| "en".to_string());
    }
    let tag = choice.replace('-', "_").to_lowercase();
    if matches!(
        tag.as_str(),
        "zh" | "zh_cn" | "zh_sg" | "zh_hans" | "zh_hans_cn" | "zh_hans_sg"
    ) {
        return "zh_CN".to_string();
    }
    let mut base = tag.split('_').next().unwrap_or("").to_string();
    // Systems tag Norwegian as macrolanguage "no" or as Nynorsk; Bokmål is the one catalog.
    if base == "no" || base == "nn" {
        base = "nb".to_string();
    }
    let locales: HashMap<String, &str> = LANGUAGES
        .iter()
        .filter(|(locale, _)| *locale != "system")
        .map(|(locale, _)| (locale.to_lowercase(), *locale))
        .collect();
    if let Some(locale) = locales.get(&tag) {
        return (*locale).to_string();
    }
    locales
        .get(&base)
        .map_or_else(|| "en".to_string(), |locale| (*locale).to_string())
}

/// Load the catalog for `choice` before a consumer fixes the language of module
/// constants. Returns the resolved language tag.
pub fn configure(choice: &str, system_languages: Option<&[String]>) -> String {
    let language = resolve_language(choice, system_languages);
    let messages = if language == "en" {
        HashMap::new()
    } else {
        load_catalog(&language)
    };
    let mut state = state().write().unwrap();
    state.language = language.clone();
    state.messages = messages;
    language
}

/// The shipped catalog for a locale, embedded at compile time so a packaged binary is
/// self-contained. The JSON stays the one source of truth under `negpy/translations/`.
fn catalog_source(language: &str) -> Option<&'static str> {
    Some(match language {
        "zh_CN" => include_str!("../../../../../negpy/translations/zh_CN.json"),
        "ja" => include_str!("../../../../../negpy/translations/ja.json"),
        "ko" => include_str!("../../../../../negpy/translations/ko.json"),
        "de" => include_str!("../../../../../negpy/translations/de.json"),
        "vi" => include_str!("../../../../../negpy/translations/vi.json"),
        "nb" => include_str!("../../../../../negpy/translations/nb.json"),
        _ => return None,
    })
}

fn load_catalog(language: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();
    let Some(text) = catalog_source(language) else {
        log_unavailable(language);
        return messages;
    };
    let Ok(raw) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text) else {
        log_unavailable(language);
        return messages;
    };
    for (source, translated) in raw {
        let Some(translated) = translated.as_str() else {
            continue;
        };
        if translated.is_empty() {
            continue;
        }
        match fields(&source) {
            Ok(spec) if spec == fields(translated).unwrap_or_default() => {
                messages.insert(source, translated.to_string());
            }
            _ => log_invalid(&source),
        }
    }
    messages
}

fn log_invalid(source: &str) {
    eprintln!("negpy: invalid translation format: {source}");
}

fn log_unavailable(language: &str) {
    eprintln!("negpy: translation catalog unavailable: {language}");
}

/// The current UI language tag.
pub fn language() -> String {
    state().read().unwrap().language.clone()
}

/// Translate a template; a value is inserted after translation, never re-formatted.
pub fn tr(source: &str, args: &[(&str, Arg<'_>)]) -> String {
    let text = state()
        .read()
        .unwrap()
        .messages
        .get(source)
        .cloned()
        .unwrap_or_else(|| source.to_string());
    if args.is_empty() {
        return text;
    }
    format_message(&text, args)
}

/// Keep placeholder counts, formats and conversions for catalog validation, sorted as
/// Python's `sorted` orders the `(name, spec, conversion)` triples.
pub fn fields(text: &str) -> Result<Vec<Field>, String> {
    let mut found = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '{' if chars.peek().is_some_and(|(_, next)| *next == '{') => {
                chars.next();
            }
            '}' if chars.peek().is_some_and(|(_, next)| *next == '}') => {
                chars.next();
            }
            '{' => {
                let start = index + 1;
                let mut end = None;
                for (offset, inner) in chars.by_ref() {
                    if inner == '}' {
                        end = Some(offset);
                        break;
                    }
                }
                let Some(end) = end else {
                    return Err("unmatched '{'".to_string());
                };
                found.push(parse_field(&text[start..end])?);
            }
            '}' => return Err("single '}' encountered".to_string()),
            _ => {}
        }
    }
    found.sort();
    Ok(found)
}

fn parse_field(body: &str) -> Result<Field, String> {
    let (name_and_spec, conversion) = match body.split_once('!') {
        Some((head, conv)) => {
            let conv = conv
                .chars()
                .next()
                .ok_or_else(|| "empty conversion".to_string())?;
            (head, Some(conv))
        }
        None => (body, None),
    };
    let (name, spec) = match name_and_spec.split_once(':') {
        Some((name, spec)) => (name, spec),
        None => (name_and_spec, ""),
    };
    Ok(Field {
        name: name.to_string(),
        spec: spec.to_string(),
        conversion,
    })
}

/// Substitute named placeholders in `template`. A missing argument leaves its
/// placeholder verbatim, which mirrors the no-argument path of the Python helper.
fn format_message(template: &str, args: &[(&str, Arg<'_>)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '{' if chars.peek().is_some_and(|(_, next)| *next == '{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek().is_some_and(|(_, next)| *next == '}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let start = index + 1;
                let mut end = None;
                for (offset, inner) in chars.by_ref() {
                    if inner == '}' {
                        end = Some(offset);
                        break;
                    }
                }
                let Some(end) = end else {
                    out.push('{');
                    continue;
                };
                let body = &template[start..end];
                match parse_field(body).ok().and_then(|field| {
                    args.iter()
                        .find(|(name, _)| *name == field.name)
                        .map(|(_, value)| (field, value))
                }) {
                    Some((field, value)) => match format_value(value, &field.spec) {
                        Some(text) => out.push_str(&text),
                        None => {
                            out.push('{');
                            out.push_str(body);
                            out.push('}');
                        }
                    },
                    None => {
                        out.push('{');
                        out.push_str(body);
                        out.push('}');
                    }
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Format one value under a Python `str.format` spec. The supported specs are the ones
/// the shipped catalogs use: plain field, `f`, `%` and `g`, with an optional sign and
/// precision.
fn format_value(value: &Arg<'_>, spec: &str) -> Option<String> {
    if let Arg::Str(text) = value {
        return spec.is_empty().then(|| (*text).to_string());
    }
    let mut rest = spec;
    let plus = rest.starts_with('+');
    if plus {
        rest = &rest[1..];
    }
    let (precision, ty) = parse_precision(rest)?;
    let number = match value {
        Arg::Str(_) => return None,
        Arg::Int(int) => *int as f64,
        Arg::Float(float) => *float,
    };
    let digits = precision.unwrap_or(6);
    let body = match ty {
        'f' => format!("{number:.digits$}"),
        '%' => format!("{:.digits$}%", number * 100.0),
        'g' => format_general(number),
        _ => return None,
    };
    // A leading '+' is added only for a non-negative value, as Python does.
    Some(if plus && number >= 0.0 {
        format!("+{body}")
    } else {
        body
    })
}

fn parse_precision(rest: &str) -> Option<(Option<usize>, char)> {
    let (precision, ty) = if let Some(after) = rest.strip_prefix('.') {
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        let ty = after[digits.len()..].chars().next()?;
        (Some(digits.parse().ok()?), ty)
    } else {
        (None, rest.chars().next()?)
    };
    Some((precision, ty))
}

/// Python's `g` at the default precision of six significant digits: the shorter of fixed
/// and exponent notation, with trailing zeros trimmed.
fn format_general(number: f64) -> String {
    if number == 0.0 {
        return "0".to_string();
    }
    const PRECISION: i32 = 6;
    let exponent = number.abs().log10().floor() as i32;
    if !(-4..PRECISION).contains(&exponent) {
        let mantissa = number / 10f64.powi(exponent);
        let mantissa = trim_zeros(&format!(
            "{mantissa:.prec$}",
            prec = (PRECISION - 1) as usize
        ));
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exponent.abs())
    } else {
        let decimals = (PRECISION - 1 - exponent).max(0) as usize;
        trim_zeros(&format!("{number:.decimals$}"))
    }
}

fn trim_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text.to_string()
    }
}
