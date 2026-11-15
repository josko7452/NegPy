//! Suggest a camera and film stock from a folder name.
//!
//! Mirrors `negpy/services/assets/gear_match.py`: shared words and squashed runs match
//! the gear catalog, and a plain number or date fills a missing field.

use serde_json::Value;

use crate::features::metadata::capture::parse_capture_date;
use crate::features::metadata::gear::{Camera, FilmStock, GearLibrary};

const MIN_TOKEN_LEN: usize = 2;
const MIN_SQUASH_LEN: usize = 4;

const ISO_VALUES: [i64; 22] = [
    25, 32, 40, 50, 64, 80, 100, 125, 160, 200, 250, 320, 400, 500, 640, 800, 1000, 1250, 1600,
    3200, 6400, 12800,
];

/// A folder's suggested gear and capture facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GearMatch {
    pub camera_id: String,
    pub film_stock_id: String,
    pub iso: Option<i64>,
    pub capture_date: String,
}

impl GearMatch {
    /// Whether the match names any field.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.camera_id.is_empty()
            || !self.film_stock_id.is_empty()
            || self.iso.is_some()
            || !self.capture_date.is_empty()
    }
}

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            if current.chars().count() >= MIN_TOKEN_LEN {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.chars().count() >= MIN_TOKEN_LEN {
        out.push(current);
    }
    out
}

fn squash(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn digits(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse() {
                out.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty()
        && let Ok(value) = current.parse()
    {
        out.push(value);
    }
    out
}

fn infer_iso(folder_name: &str) -> Option<i64> {
    let mut found: Vec<i64> = tokens(folder_name)
        .iter()
        .flat_map(|token| digits(token))
        .filter(|value| ISO_VALUES.contains(value))
        .collect();
    found.sort_unstable();
    found.dedup();
    (found.len() == 1).then(|| found[0])
}

fn infer_capture_date(folder_name: &str) -> String {
    let chars: Vec<char> = folder_name.chars().collect();
    let mut found: Vec<String> = Vec::new();
    for index in 0..chars.len().saturating_sub(7) {
        if index > 0 && chars[index - 1].is_ascii_digit() {
            continue;
        }
        let year: String = chars[index..index + 4].iter().collect();
        if !year.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let mut month = String::new();
        let mut day = String::new();
        let mut cursor = index + 4;
        let mut separated = false;
        if cursor < chars.len() && matches!(chars[cursor], '-' | '_' | '.' | ' ') {
            separated = true;
            cursor += 1;
        }
        for _ in 0..2 {
            if cursor < chars.len() {
                month.push(chars[cursor]);
                cursor += 1;
            }
        }
        if separated && cursor < chars.len() && matches!(chars[cursor], '-' | '_' | '.' | ' ') {
            cursor += 1;
        }
        for _ in 0..2 {
            if cursor < chars.len() {
                day.push(chars[cursor]);
                cursor += 1;
            }
        }
        if month.chars().all(|ch| ch.is_ascii_digit())
            && day.chars().all(|ch| ch.is_ascii_digit())
            && month.len() == 2
            && day.len() == 2
        {
            if let Some(parsed) = parse_capture_date(&format!("{year}-{month}-{day}")) {
                found.push(parsed.text);
            }
        }
    }
    found.sort();
    found.dedup();
    if found.len() == 1 {
        found.remove(0)
    } else {
        String::new()
    }
}

fn match_strength(folder_tokens: &[String], folder_squashed: &str, parts: &[&str]) -> (u8, usize) {
    let name = parts
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        return (0, 0);
    }
    let candidate = tokens(&name);
    let candidate_squashed = squash(&name);
    let mut shared: Vec<&String> = folder_tokens
        .iter()
        .filter(|token| candidate.contains(token))
        .collect();
    for token in folder_tokens {
        if token.chars().count() >= 3
            && candidate_squashed.contains(token)
            && !shared.contains(&token)
        {
            shared.push(token);
        }
    }
    let specificity = shared.len();
    if shared
        .iter()
        .any(|token| token.chars().any(|ch| ch.is_ascii_digit()))
    {
        return (2, specificity);
    }
    if candidate_squashed.chars().count() >= MIN_SQUASH_LEN
        && folder_squashed.contains(&candidate_squashed)
    {
        return (2, specificity);
    }
    if shared.is_empty() { (0, 0) } else { (1, 0) }
}

fn best_match<T>(
    folder_tokens: &[String],
    folder_squashed: &str,
    items: &[T],
    name_parts: impl Fn(&T) -> Vec<&str>,
    is_bundled: impl Fn(&T) -> bool,
    id: impl Fn(&T) -> &str,
) -> Option<String> {
    let scored: Vec<(&str, (u8, usize), bool)> = items
        .iter()
        .map(|item| {
            (
                id(item),
                match_strength(folder_tokens, folder_squashed, &name_parts(item)),
                is_bundled(item),
            )
        })
        .collect();
    let best = scored
        .iter()
        .map(|(_, strength, bundled)| (strength.0, strength.1, *bundled))
        .max()
        .unwrap_or((0, 0, false));
    let matches: Vec<&str> = scored
        .iter()
        .filter(|(_, strength, bundled)| {
            (strength.0, strength.1, *bundled) == best && strength.0 > 0
        })
        .map(|(id, _, _)| *id)
        .collect();
    (matches.len() == 1).then(|| matches[0].to_string())
}

/// Match a folder name against the catalogs and infer a missing ISO or date.
#[must_use]
pub fn match_gear_for_folder(folder_name: &str, library: &GearLibrary) -> GearMatch {
    let folder_tokens = tokens(folder_name);
    let folder_squashed = squash(folder_name);
    let camera_id = best_match(
        &folder_tokens,
        &folder_squashed,
        &library.cameras,
        |camera: &Camera| vec![&camera.make, &camera.model, &camera.display_name],
        |_| false,
        |camera| &camera.id,
    )
    .unwrap_or_default();
    let film_stock_id = best_match(
        &folder_tokens,
        &folder_squashed,
        &library.film_stocks,
        |stock: &FilmStock| vec![&stock.manufacturer, &stock.stock_name, &stock.display_name],
        |_| false,
        |stock| &stock.id,
    )
    .unwrap_or_default();
    GearMatch {
        iso: if film_stock_id.is_empty() {
            infer_iso(folder_name)
        } else {
            None
        },
        capture_date: infer_capture_date(folder_name),
        camera_id,
        film_stock_id,
    }
}

/// The folder name to match gear against, from a roll or the selected frame.
#[must_use]
pub fn folder_name_for_active_context(
    state: &Value,
    _repo: &crate::infrastructure::storage::repository::StorageRepository,
) -> String {
    let roll_id = state
        .get("active_roll_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !roll_id.is_empty() {
        return roll_id.to_string();
    }
    let index = state
        .get("selected_file_idx")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let Some(files) = state.get("uploaded_files").and_then(Value::as_array) else {
        return String::new();
    };
    let Some(file) = usize::try_from(index)
        .ok()
        .and_then(|index| files.get(index))
    else {
        return String::new();
    };
    let path = file.get("path").and_then(Value::as_str).unwrap_or("");
    std::path::Path::new(path)
        .parent()
        .map_or_else(String::new, |folder| {
            folder
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata::gear::{Camera, FilmStock};

    fn library(cameras: Vec<Camera>, stocks: Vec<FilmStock>) -> GearLibrary {
        GearLibrary {
            cameras,
            film_stocks: stocks,
            ..GearLibrary::default()
        }
    }

    #[test]
    fn a_shared_word_matches_a_film_stock() {
        let gold = FilmStock {
            id: "gold".into(),
            manufacturer: "Kodak".into(),
            stock_name: "Gold 200".into(),
            ..FilmStock::default()
        };
        let matched = match_gear_for_folder("08_penf_gold_marbella", &library(vec![], vec![gold]));
        assert_eq!(matched.film_stock_id, "gold");
    }

    #[test]
    fn a_squashed_abbreviation_matches_a_camera() {
        let pen = Camera {
            id: "pen".into(),
            make: "Olympus".into(),
            model: "Pen F".into(),
            ..Camera::default()
        };
        let matched = match_gear_for_folder("08_penf_gold_marbella", &library(vec![pen], vec![]));
        assert_eq!(matched.camera_id, "pen");
    }

    #[test]
    fn two_candidates_sharing_a_word_do_not_match() {
        let plus = FilmStock {
            id: "plus".into(),
            manufacturer: "Ilford".into(),
            stock_name: "HP5 Plus 400".into(),
            ..FilmStock::default()
        };
        let fp = FilmStock {
            id: "fp".into(),
            manufacturer: "Ilford".into(),
            stock_name: "FP4 Plus 125".into(),
            ..FilmStock::default()
        };
        let matched = match_gear_for_folder("roll_plus_test", &library(vec![], vec![plus, fp]));
        assert!(matched.film_stock_id.is_empty());
    }

    #[test]
    fn only_one_plausible_iso_is_used() {
        assert_eq!(
            match_gear_for_folder("06_scala_50_vietnam", &GearLibrary::default()).iso,
            Some(50)
        );
        assert_eq!(
            match_gear_for_folder("08_bronica_ilford_120_marbella", &GearLibrary::default()).iso,
            None
        );
        assert_eq!(
            match_gear_for_folder("50_roll_400_iso", &GearLibrary::default()).iso,
            None
        );
    }

    #[test]
    fn a_folder_date_is_inferred_but_an_impossible_one_is_not() {
        assert_eq!(
            match_gear_for_folder("2024-10-17_family_trip", &GearLibrary::default()).capture_date,
            "2024-10-17"
        );
        assert_eq!(
            match_gear_for_folder("2024_10_17_family_trip", &GearLibrary::default()).capture_date,
            "2024-10-17"
        );
        assert!(
            match_gear_for_folder("2024-13-40_family_trip", &GearLibrary::default())
                .capture_date
                .is_empty()
        );
    }

    #[test]
    fn a_matched_stock_suppresses_the_standalone_iso() {
        let gold = FilmStock {
            id: "gold".into(),
            manufacturer: "Kodak".into(),
            stock_name: "Gold 200".into(),
            ..FilmStock::default()
        };
        let matched = match_gear_for_folder(
            "05_penees_kodakgold200_tailandia",
            &library(vec![], vec![gold]),
        );
        assert_eq!(matched.film_stock_id, "gold");
        assert_eq!(matched.iso, None);
    }
}
