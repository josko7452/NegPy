//! Mirrors `negpy/services/assets/search.py`: one query parser and one matcher, shared by
//! the film-strip filter and any cross-folder search.

use serde_json::Value;

/// Terms that read a boolean fact.
pub const FLAG_FIELDS: [&str; 3] = ["keeper", "rejected", "edited"];
/// Terms compared as numbers.
pub const NUMERIC_FIELDS: [&str; 5] = ["iso", "frame", "push", "devtime", "temp"];
/// Terms compared as text.
pub const TEXT_FIELDS: [&str; 15] = [
    "name",
    "path",
    "ext",
    "film",
    "camera",
    "lens",
    "developer",
    "dilution",
    "format",
    "scanning",
    "roll",
    "date",
    "shot",
    "place",
    "scene",
];

/// One ANDed query term. An empty field matches the filename as a bare word.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub field: String,
    pub op: String,
    pub value: String,
    pub negate: bool,
}

fn is_field(name: &str) -> bool {
    FLAG_FIELDS.contains(&name) || NUMERIC_FIELDS.contains(&name) || TEXT_FIELDS.contains(&name)
}

/// Split a query into ANDed terms. An unparseable token is a bare word, so a live search
/// box never rejects a half-typed quote.
#[must_use]
pub fn parse_query(text: &str) -> Vec<Term> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    tokenize(text)
        .iter()
        .filter_map(|token| term(token))
        .collect()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn term(token: &str) -> Option<Term> {
    let (negate, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if token.is_empty() {
        return None;
    }
    let (field, value) = match token.split_once(':') {
        Some((field, value)) => (field.to_lowercase(), value),
        None => {
            return Some(Term {
                field: String::new(),
                op: ":".to_string(),
                value: token.to_lowercase(),
                negate,
            });
        }
    };
    if !is_field(&field) {
        return Some(Term {
            field: String::new(),
            op: ":".to_string(),
            value: token.to_lowercase(),
            negate,
        });
    }
    for candidate in [">=", "<=", ">", "<"] {
        if let Some(rest) = value.strip_prefix(candidate) {
            return Some(Term {
                field,
                op: candidate.to_string(),
                value: rest.to_lowercase(),
                negate,
            });
        }
    }
    Some(Term {
        field,
        op: ":".to_string(),
        value: value.to_lowercase(),
        negate,
    })
}

/// Whether every term holds for these facts. An empty query matches everything.
#[must_use]
pub fn matches(terms: &[Term], facts: &Value) -> bool {
    terms.iter().all(|term| term.negate != holds(term, facts))
}

fn holds(term: &Term, facts: &Value) -> bool {
    if FLAG_FIELDS.contains(&term.field.as_str()) {
        return facts
            .get(&term.field)
            .and_then(Value::as_bool)
            .unwrap_or(false);
    }
    let key = if term.field.is_empty() {
        "name"
    } else {
        &term.field
    };
    let Some(fact) = facts.get(key) else {
        return false;
    };
    if fact.is_null() || fact.as_str() == Some("") {
        return false;
    }
    if NUMERIC_FIELDS.contains(&term.field.as_str()) {
        let (Some(left), Ok(right)) = (fact.as_f64(), term.value.parse::<f64>()) else {
            return false;
        };
        return apply_op(&term.op, left, right);
    }
    let fact = value_text(fact).to_lowercase();
    if term.op == ":" {
        return fact.contains(&term.value);
    }
    let prefix = fact
        .chars()
        .take(term.value.chars().count())
        .collect::<String>();
    apply_op(&term.op, prefix.as_str(), term.value.as_str())
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn apply_op<T: PartialOrd>(op: &str, left: T, right: T) -> bool {
    match op {
        ">" => left > right,
        ">=" => left >= right,
        "<" => left < right,
        "<=" => left <= right,
        _ => left == right,
    }
}

/// Searchable facts for one asset: its file identity plus, when it has a saved edit, that
/// edit's archival metadata.
#[must_use]
pub fn facts_for(asset: &Value, config: Option<&crate::domain::models::WorkspaceConfig>) -> Value {
    let name = asset
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = asset
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let ext = std::path::Path::new(name)
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut facts = serde_json::json!({
        "name": name.to_lowercase(),
        "path": path.to_lowercase(),
        "ext": ext,
        "date": date_string(asset.get("mtime")),
        "keeper": asset.get("keeper").and_then(Value::as_bool).unwrap_or(false),
        "rejected": asset.get("excluded").and_then(Value::as_bool).unwrap_or(false),
        "edited": config.is_some(),
        "scene": asset.get("scene").and_then(|scene| scene.get(2))
            .and_then(Value::as_str).unwrap_or_default().to_lowercase(),
    });
    let Some(config) = config else {
        return facts;
    };
    let join = |parts: [&str; 2]| {
        parts
            .iter()
            .filter(|part| !part.is_empty())
            .map(|part| part.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let metadata = &config.metadata;
    let format = if metadata.format == "Other" {
        metadata.format_other.clone()
    } else {
        metadata.format.clone()
    };
    facts["film"] = join([&metadata.film_manufacturer, &metadata.film]).into();
    facts["camera"] = join([&metadata.camera_make, &metadata.camera_model]).into();
    facts["lens"] = join([&metadata.lens_make, &metadata.lens_model]).into();
    facts["developer"] = metadata.developer.to_lowercase().into();
    facts["dilution"] = metadata.process_dilution.to_lowercase().into();
    facts["devtime"] = metadata
        .process_time_seconds
        .map_or(Value::Null, |seconds| {
            serde_json::json!(seconds as f64 / 60.0)
        });
    facts["temp"] = metadata
        .process_temperature_c
        .map_or(Value::Null, |value| serde_json::json!(value));
    facts["format"] = format.to_lowercase().into();
    facts["scanning"] = metadata.scanning.to_lowercase().into();
    let roll_name = config.process.roll_name.clone().unwrap_or_default();
    facts["roll"] = join([&metadata.capture_roll, &roll_name]).into();
    facts["capture_roll"] = metadata.capture_roll.to_lowercase().into();
    facts["frame"] = serde_json::json!(metadata.capture_frame);
    facts["iso"] = metadata
        .film_iso
        .map_or(Value::Null, |iso| serde_json::json!(iso));
    facts["push"] = serde_json::json!(metadata.push_pull);
    facts["shot"] = metadata.capture_date.to_lowercase().into();
    facts["place"] = [
        &metadata.location_city,
        &metadata.location_state,
        &metadata.location_country,
    ]
    .iter()
    .filter(|part| !part.is_empty())
    .map(|part| part.to_lowercase())
    .collect::<Vec<_>>()
    .join(" ")
    .into();
    facts
}

fn date_string(mtime: Option<&Value>) -> String {
    let Some(seconds) = mtime.and_then(Value::as_f64) else {
        return String::new();
    };
    chrono::DateTime::from_timestamp(seconds as i64, 0)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Value {
        serde_json::json!({
            "name": "frame_01.dng",
            "path": "/scans/frame_01.dng",
            "iso": 400,
            "date": "2026-11-01",
            "keeper": true,
        })
    }

    #[test]
    fn an_empty_query_matches_everything() {
        assert!(matches(&parse_query(""), &facts()));
    }

    #[test]
    fn a_bare_word_reads_the_name() {
        let terms = parse_query("frame");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].field, "");
        assert!(matches(&terms, &facts()));
    }

    #[test]
    fn a_field_term_reads_its_fact() {
        assert!(matches(&parse_query("iso:400"), &facts()));
        assert!(!matches(&parse_query("iso:800"), &facts()));
        assert!(matches(&parse_query("iso:>=400"), &facts()));
    }

    #[test]
    fn a_negated_term_inverts() {
        assert!(!matches(&parse_query("-keeper:true"), &facts()));
        assert!(matches(&parse_query("-iso:800"), &facts()));
    }

    #[test]
    fn a_quoted_phrase_stays_one_token() {
        let terms = parse_query("\"frame 01\"");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].value, "frame 01");
    }

    #[test]
    fn a_date_prefix_orders_lexically() {
        assert!(matches(&parse_query("date:>=2026-11"), &facts()));
        assert!(!matches(&parse_query("date:>=2026-12"), &facts()));
    }

    #[test]
    fn facts_read_the_saved_metadata() {
        let mut config = crate::domain::models::WorkspaceConfig::default();
        config.metadata.film = "Portra".to_string();
        config.metadata.film_manufacturer = "Kodak".to_string();
        config.metadata.capture_frame = Some(7);
        let facts = facts_for(&facts(), Some(&config));
        assert_eq!(facts["film"], "kodak portra");
        assert_eq!(facts["frame"], 7);
    }
}
