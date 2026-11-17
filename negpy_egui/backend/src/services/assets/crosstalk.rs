//! Spectral-crosstalk profile files.
//!
//! Mirrors `negpy/services/assets/crosstalk.py`: a profile is one `.toml` file with a
//! display name, an optional origin type and the film process it describes. Bundled
//! profiles win over a user file with the same display name, and the built-in
//! `Generic C41` matrix is never a file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::domain::models::ProcessMode;
use crate::kernel::system::i18n::mark;
use crate::services::assets::naming::{escape_toml_string, slugify};
use crate::services::assets::toml_cache::load_toml_cached;

/// The built-in profile name, which has no file and no matrix.
pub const DEFAULT_NAME: &str = "Generic C41";

/// Where a profile's numbers came from. A `type` outside this set groups under `Other`
/// rather than disappearing, so reads stay plain strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrosstalkType {
    Specsheet,
    Measured,
    Tuned,
    Builtin,
    Other,
}

impl CrosstalkType {
    /// The stored spelling; `Other` is never written.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Specsheet => "specsheet-based",
            Self::Measured => "measured",
            Self::Tuned => "tuned",
            Self::Builtin => "built-in",
            Self::Other => "",
        }
    }

    /// The type for a stored value; an absent or unrecognised one is `Other`.
    #[must_use]
    pub fn coerce(value: &str) -> Self {
        match value {
            "specsheet-based" => Self::Specsheet,
            "measured" => Self::Measured,
            "tuned" => Self::Tuned,
            "built-in" => Self::Builtin,
            _ => Self::Other,
        }
    }
}

/// Dropdown group order and headings; the trailing entry is the catch-all.
pub const GROUP_ORDER: [(CrosstalkType, &str); 5] = [
    (CrosstalkType::Builtin, "Built-in"),
    (CrosstalkType::Measured, "Measured"),
    (CrosstalkType::Tuned, "Tuned on a rig"),
    (CrosstalkType::Specsheet, "From spec sheets (approx)"),
    (CrosstalkType::Other, "Other"),
];

/// The heading a group shows; the catalog holds the translated text.
#[must_use]
pub fn group_heading(kind: CrosstalkType) -> &'static str {
    GROUP_ORDER
        .iter()
        .find(|(candidate, _)| *candidate == kind)
        .map_or("", |(_, heading)| mark(heading))
}

/// One parsed profile file, before its filename fallback applies.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedProfile {
    pub name: Option<String>,
    pub matrix: [f64; 9],
}

/// The profile directories a lookup scans.
#[derive(Clone, Debug)]
pub struct CrosstalkDirs {
    pub user: PathBuf,
    pub bundled: PathBuf,
}

impl CrosstalkDirs {
    /// The user directory plus the shipped gallery.
    #[must_use]
    pub fn new(user: impl Into<PathBuf>) -> Self {
        Self {
            user: user.into(),
            bundled: crate::kernel::system::paths::resource_path("crosstalk"),
        }
    }
}

fn toml_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    files
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

fn text_field(path: &Path, key: &str) -> Option<String> {
    let data = load_toml_cached(path)?;
    data.get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
}

/// Parse one profile file; `None` for an absent or malformed file and for the built-in
/// name, which always reads from the code.
#[must_use]
pub fn parse_file(path: &Path) -> Option<ParsedProfile> {
    let data = load_toml_cached(path)?;
    let rows = data.get("matrix")?.as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut matrix = [0.0; 9];
    for (row, values) in rows.iter().enumerate() {
        let values = values.as_array()?;
        if values.len() != 3 {
            return None;
        }
        for (column, value) in values.iter().enumerate() {
            let value = match value {
                toml::Value::Float(value) => *value,
                toml::Value::Integer(value) => *value as f64,
                _ => return None,
            };
            matrix[row * 3 + column] = value;
            if !value.is_finite() {
                return None;
            }
        }
    }
    let name = text_field(path, "name").filter(|name| !name.is_empty());
    if name.as_deref() == Some(DEFAULT_NAME) {
        return None;
    }
    Some(ParsedProfile { name, matrix })
}

/// Every valid profile in one directory, keyed by display name.
#[must_use]
pub fn scan_dir(directory: &Path) -> BTreeMap<String, [f64; 9]> {
    let mut out = BTreeMap::new();
    for path in toml_files(directory) {
        let Some(parsed) = parse_file(&path) else {
            continue;
        };
        let name = parsed.name.unwrap_or_else(|| stem(&path));
        if name != DEFAULT_NAME {
            out.insert(name, parsed.matrix);
        }
    }
    out
}

/// The bundled matrices a read-only profile can name.
#[must_use]
pub fn scan_bundled(dirs: &CrosstalkDirs) -> BTreeMap<String, [f64; 9]> {
    scan_dir(&dirs.bundled)
}

/// The user's editable matrices.
#[must_use]
pub fn scan_user(dirs: &CrosstalkDirs) -> BTreeMap<String, [f64; 9]> {
    scan_dir(&dirs.user)
}

/// Bundled union user matrices, with the bundled profile winning a name collision.
#[must_use]
pub fn scan(dirs: &CrosstalkDirs) -> BTreeMap<String, [f64; 9]> {
    let mut out = scan_user(dirs);
    out.extend(scan_bundled(dirs));
    out
}

/// `["Generic C41", *sorted custom display names]`.
#[must_use]
pub fn list_profiles(dirs: &CrosstalkDirs) -> Vec<String> {
    let mut names = vec![DEFAULT_NAME.to_string()];
    names.extend(scan(dirs).into_keys());
    names
}

/// The flat row-major matrix for a profile; `None` means the built-in default or an
/// absent profile.
#[must_use]
pub fn get_matrix(dirs: &CrosstalkDirs, name: &str) -> Option<[f64; 9]> {
    if name == DEFAULT_NAME {
        return None;
    }
    scan(dirs).get(name).copied()
}

/// Whether a profile is read-only: the built-in or a bundled matrix.
#[must_use]
pub fn is_bundled(dirs: &CrosstalkDirs, name: &str) -> bool {
    name == DEFAULT_NAME || scan_bundled(dirs).contains_key(name)
}

fn scan_types(dirs: &CrosstalkDirs) -> BTreeMap<String, CrosstalkType> {
    let mut out = BTreeMap::new();
    for directory in [&dirs.user, &dirs.bundled] {
        for path in toml_files(directory) {
            let Some(parsed) = parse_file(&path) else {
                continue;
            };
            let name = parsed.name.unwrap_or_else(|| stem(&path));
            if name == DEFAULT_NAME {
                continue;
            }
            let kind = text_field(&path, "type").map_or(CrosstalkType::Other, |value| {
                CrosstalkType::coerce(&value.to_lowercase())
            });
            out.insert(name, kind);
        }
    }
    out
}

fn scan_processes(dirs: &CrosstalkDirs) -> BTreeMap<String, ProcessMode> {
    let mut out = BTreeMap::new();
    for directory in [&dirs.user, &dirs.bundled] {
        for path in toml_files(directory) {
            if parse_file(&path).is_none() {
                continue;
            }
            let name = text_field(&path, "name").unwrap_or_else(|| stem(&path));
            let process = text_field(&path, "process")
                .map_or(ProcessMode::C41, |value| ProcessMode::coerce(&value));
            out.insert(name, process);
        }
    }
    out.insert(DEFAULT_NAME.to_string(), ProcessMode::C41);
    out
}

/// The profile's origin type, or `Built-in`, or `Other` when unknown.
#[must_use]
pub fn get_type(dirs: &CrosstalkDirs, name: &str) -> CrosstalkType {
    if name == DEFAULT_NAME {
        return CrosstalkType::Builtin;
    }
    scan_types(dirs)
        .get(name)
        .copied()
        .unwrap_or(CrosstalkType::Other)
}

/// The film process a profile was derived for; C-41 when unknown.
#[must_use]
pub fn get_process(dirs: &CrosstalkDirs, name: &str) -> ProcessMode {
    scan_processes(dirs)
        .get(name)
        .copied()
        .unwrap_or(ProcessMode::C41)
}

/// Profiles grouped for the dropdown in `GROUP_ORDER`, empty groups omitted.
///
/// `process_mode` keeps the list to matrices derived for the film being processed; an
/// unrecognised type cannot drop a profile.
#[must_use]
pub fn grouped_profiles(
    dirs: &CrosstalkDirs,
    process_mode: Option<ProcessMode>,
) -> Vec<(CrosstalkType, Vec<String>)> {
    let types = scan_types(dirs);
    let processes = scan_processes(dirs);
    let mut buckets: BTreeMap<CrosstalkType, Vec<String>> = BTreeMap::new();
    if process_mode.is_none_or(|mode| mode == ProcessMode::C41) {
        buckets
            .entry(CrosstalkType::Builtin)
            .or_default()
            .push(DEFAULT_NAME.to_string());
    }
    for (name, kind) in types {
        if process_mode.is_some_and(|mode| processes.get(&name).copied() != Some(mode)) {
            continue;
        }
        buckets.entry(kind).or_default().push(name);
    }
    GROUP_ORDER
        .iter()
        .filter_map(|(kind, _)| {
            let names = buckets.remove(kind)?;
            (!names.is_empty()).then_some((*kind, names))
        })
        .collect()
}

/// The file a user profile with this display name would use.
///
/// Two names can slugify to the same stem, so an existing file with this display name is
/// reused and a free stem is chosen otherwise.
#[must_use]
pub fn path_for_name(dirs: &CrosstalkDirs, name: &str) -> PathBuf {
    for path in toml_files(&dirs.user) {
        let Some(parsed) = parse_file(&path) else {
            continue;
        };
        let found = parsed.name.unwrap_or_else(|| stem(&path));
        if found == name {
            return path;
        }
    }
    let base = slugify(name, "crosstalk");
    let first = dirs.user.join(format!("{base}.toml"));
    if !first.is_file() {
        return first;
    }
    for index in 2..1000 {
        let candidate = dirs.user.join(format!("{base}_{index}.toml"));
        if !candidate.is_file() {
            return candidate;
        }
    }
    first
}

/// Python's `{value:.6g}`, which a saved profile writes and reads back.
fn format_general(value: f64) -> String {
    let precision = 6usize;
    if value == 0.0 {
        return "0".to_string();
    }
    let exponent = value.abs().log10().floor() as i32;
    if exponent < -4 || exponent >= precision as i32 {
        let text = format!("{:.*e}", precision - 1, value);
        let Some((mantissa, exponent)) = text.split_once('e') else {
            return text;
        };
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        let exponent: i32 = exponent.parse().unwrap_or(0);
        format!("{mantissa}e{exponent:+03}")
    } else {
        let decimals = (precision as i32 - 1 - exponent).max(0) as usize;
        format!("{value:.decimals$}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Write a user profile and return its path.
///
/// The type defaults to `tuned`, so an editor save is not grouped with spec-sheet
/// estimates. The process is always written, because a matrix only reaches the render in
/// the process it declares.
pub fn save(
    dirs: &CrosstalkDirs,
    name: &str,
    matrix: [f64; 9],
    kind: CrosstalkType,
    process: Option<ProcessMode>,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(&dirs.user)?;
    let rows = (0..3)
        .map(|row| {
            format!(
                "  [{}, {}, {}],",
                format_general(matrix[row * 3]),
                format_general(matrix[row * 3 + 1]),
                format_general(matrix[row * 3 + 2])
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let content = format!(
        "name = \"{}\"\ntype = \"{}\"\nprocess = \"{}\"\nmatrix = [\n{}\n]\n",
        escape_toml_string(name),
        escape_toml_string(kind.as_str()),
        escape_toml_string((process.unwrap_or(ProcessMode::C41)).as_str()),
        rows
    );
    let path = path_for_name(dirs, name);
    std::fs::write(&path, content)?;
    Ok(path)
}

/// Remove the user profile whose display name matches; absent is a no-op.
pub fn delete(dirs: &CrosstalkDirs, name: &str) {
    for path in toml_files(&dirs.user) {
        let Some(parsed) = parse_file(&path) else {
            continue;
        };
        let found = parsed.name.unwrap_or_else(|| stem(&path));
        if found == name {
            let _ = std::fs::remove_file(path);
            return;
        }
    }
}

/// Make sure the user's profile directory exists; nothing is seeded.
pub fn ensure_user_dir(dirs: &CrosstalkDirs) -> std::io::Result<()> {
    std::fs::create_dir_all(&dirs.user)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, CrosstalkDirs) {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user");
        let bundled = temp.path().join("bundled");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&bundled).unwrap();
        (temp, CrosstalkDirs { user, bundled })
    }

    const IDENTITY: [f64; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

    #[test]
    fn a_custom_matrix_is_listed_and_read() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.user.join("portra.toml"),
            "name = \"Portra 400\"\nmatrix = [[1.0, -0.1, 0.0], [0.0, 1.0, -0.1], [0.0, 0.0, 1.0]]\n",
        )
        .unwrap();
        assert_eq!(list_profiles(&dirs), ["Generic C41", "Portra 400"]);
        assert_eq!(
            get_matrix(&dirs, "Portra 400"),
            Some([1.0, -0.1, 0.0, 0.0, 1.0, -0.1, 0.0, 0.0, 1.0])
        );
    }

    #[test]
    fn a_missing_name_falls_back_to_the_stem() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.user.join("my_film.toml"),
            "matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
        )
        .unwrap();
        assert!(list_profiles(&dirs).contains(&"my_film".to_string()));
    }

    #[test]
    fn the_builtin_and_absent_profiles_have_no_matrix() {
        let (_temp, dirs) = dirs();
        assert_eq!(get_matrix(&dirs, DEFAULT_NAME), None);
        assert_eq!(get_matrix(&dirs, "nonexistent"), None);
    }

    #[test]
    fn a_malformed_file_is_skipped() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.user.join("bad_shape.toml"),
            "matrix = [[1.0, 0.0], [0.0, 1.0]]\n",
        )
        .unwrap();
        std::fs::write(dirs.user.join("bad_toml.toml"), "matrix = [[[not valid\n").unwrap();
        std::fs::write(dirs.user.join("no_matrix.toml"), "name = \"x\"\n").unwrap();
        assert_eq!(list_profiles(&dirs), [DEFAULT_NAME]);
    }

    #[test]
    fn the_scan_merges_bundled_and_user_with_bundled_winning() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.bundled.join("portra_400.toml"),
            "name = \"Portra 400\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
        )
        .unwrap();
        std::fs::write(
            dirs.user.join("my_film.toml"),
            "name = \"My Film\"\nmatrix = [[0.9, 0.1, 0.0], [0.0, 0.9, 0.1], [0.0, 0.0, 0.9]]\n",
        )
        .unwrap();
        std::fs::write(
            dirs.user.join("old_copy.toml"),
            "name = \"Portra 400\"\nmatrix = [[9.0, 9.0, 9.0], [9.0, 9.0, 9.0], [9.0, 9.0, 9.0]]\n",
        )
        .unwrap();
        assert_eq!(
            list_profiles(&dirs),
            ["Generic C41", "My Film", "Portra 400"]
        );
        assert_eq!(get_matrix(&dirs, "Portra 400"), Some(IDENTITY));
        assert!(is_bundled(&dirs, "Portra 400"));
        assert!(!is_bundled(&dirs, "My Film"));
    }

    #[test]
    fn saving_round_trips_and_marks_a_tuned_profile() {
        let (_temp, dirs) = dirs();
        let matrix = [
            1.004, -0.02, 0.001, -0.118, 1.01, -0.04, -0.042, -0.149, 1.006,
        ];
        let path = save(&dirs, "My Film", matrix, CrosstalkType::Tuned, None).unwrap();
        assert!(path.is_file());
        assert_eq!(get_matrix(&dirs, "My Film"), Some(matrix));
        assert_eq!(get_type(&dirs, "My Film"), CrosstalkType::Tuned);
        let groups = grouped_profiles(&dirs, None);
        assert!(
            groups
                .iter()
                .any(|(kind, names)| *kind == CrosstalkType::Tuned && names == &["My Film"])
        );
    }

    #[test]
    fn an_unrecognised_type_groups_under_other_without_disappearing() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.user.join("weird.toml"),
            "name = \"Weird\"\ntype = \"handed-down-by-owls\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
        )
        .unwrap();
        let groups = grouped_profiles(&dirs, None);
        assert!(
            groups
                .iter()
                .any(|(kind, names)| *kind == CrosstalkType::Other && names == &["Weird"])
        );
        assert_eq!(get_type(&dirs, "Weird"), CrosstalkType::Other);
    }

    #[test]
    fn deleting_removes_only_the_matching_profile() {
        let (_temp, dirs) = dirs();
        save(&dirs, "Alpha", IDENTITY, CrosstalkType::Tuned, None).unwrap();
        save(&dirs, "Beta", IDENTITY, CrosstalkType::Tuned, None).unwrap();
        delete(&dirs, "Alpha");
        assert_eq!(get_matrix(&dirs, "Alpha"), None);
        assert!(get_matrix(&dirs, "Beta").is_some());
        delete(&dirs, "Nonexistent");
    }

    #[test]
    fn two_names_with_one_slug_stay_separate_profiles() {
        let (_temp, dirs) = dirs();
        save(&dirs, "Portra 400", IDENTITY, CrosstalkType::Tuned, None).unwrap();
        save(
            &dirs,
            "Portra-400",
            [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0],
            CrosstalkType::Tuned,
            None,
        )
        .unwrap();
        let found = scan_user(&dirs);
        assert_eq!(found.len(), 2);
        assert_eq!(found["Portra 400"][0], 1.0);
        assert_eq!(found["Portra-400"][0], 2.0);
    }

    #[test]
    fn a_process_gate_hides_matrices_from_another_film() {
        let (_temp, dirs) = dirs();
        std::fs::write(
            dirs.user.join("slide.toml"),
            "name = \"Old Slide\"\nprocess = \"E-6\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
        )
        .unwrap();
        assert_eq!(get_process(&dirs, "Old Slide"), ProcessMode::E6);
        let slide = grouped_profiles(&dirs, Some(ProcessMode::E6));
        assert_eq!(
            slide
                .iter()
                .flat_map(|(_, names)| names.clone())
                .collect::<Vec<_>>(),
            ["Old Slide"]
        );
        assert_eq!(
            grouped_profiles(&dirs, Some(ProcessMode::C41)),
            [(CrosstalkType::Builtin, vec![DEFAULT_NAME.to_string()])]
        );
    }

    #[test]
    fn the_builtin_is_c41_even_without_a_file() {
        let (_temp, dirs) = dirs();
        assert_eq!(get_process(&dirs, DEFAULT_NAME), ProcessMode::C41);
        assert_eq!(get_type(&dirs, DEFAULT_NAME), CrosstalkType::Builtin);
    }
}
