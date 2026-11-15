//! TOML matrix profiles for crosstalk and sensor correction.
//!
//! Mirrors `negpy/services/assets/crosstalk.py` and
//! `negpy/services/assets/sensor.py`: the bundled directory wins over the user
//! directory, and an invalid file is skipped rather than failing the scan.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;

use crate::domain::models::ProcessMode;
use crate::services::assets::naming::{escape_toml_string, slugify};
use crate::services::assets::toml_cache::load_toml_cached;

pub const CROSSTALK_DEFAULT: &str = "Generic C41";
pub const SENSOR_NONE: &str = "None";

/// Where a crosstalk profile's matrix came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosstalkType {
    Specsheet,
    Measured,
    Tuned,
    Builtin,
    Other,
}

impl CrosstalkType {
    /// The stored `type` value, lowercased.
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

    /// The dropdown group heading.
    #[must_use]
    pub fn heading(self) -> &'static str {
        match self {
            Self::Builtin => "Built-in",
            Self::Measured => "Measured",
            Self::Tuned => "Tuned on a rig",
            Self::Specsheet => "From spec sheets (approx)",
            Self::Other => "Other",
        }
    }

    fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "specsheet-based" => Self::Specsheet,
            "measured" => Self::Measured,
            "tuned" => Self::Tuned,
            "built-in" => Self::Builtin,
            _ => Self::Other,
        }
    }
}

/// A matrix profile's display name and row-major 3x3 values.
#[derive(Clone, Debug, PartialEq)]
pub struct MatrixProfile {
    pub name: String,
    pub matrix: [f64; 9],
    pub profile_type: CrosstalkType,
    pub process: ProcessMode,
    pub path: PathBuf,
}

fn text(data: &toml::Table, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn matrix(data: &toml::Table) -> Option<[f64; 9]> {
    let rows = data.get("matrix")?.as_array()?;
    if rows.len() != 3 {
        return None;
    }
    let mut out = [0.0; 9];
    for (row_index, row) in rows.iter().enumerate() {
        let values = row.as_array()?;
        if values.len() != 3 {
            return None;
        }
        for (column, value) in values.iter().enumerate() {
            out[row_index * 3 + column] = match value {
                Value::Float(value) => *value,
                Value::Integer(value) => *value as f64,
                _ => return None,
            };
        }
    }
    Some(out)
}

fn parse(path: &Path) -> Option<MatrixProfile> {
    let data = load_toml_cached(path)?;
    let matrix = matrix(&data)?;
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    Some(MatrixProfile {
        name: text(&data, "name").unwrap_or(stem),
        matrix,
        profile_type: CrosstalkType::from_str(&text(&data, "type").unwrap_or_default()),
        process: ProcessMode::coerce(&text(&data, "process").unwrap_or_default()),
        path: path.to_path_buf(),
    })
}

fn list_toml(folder: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(folder)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    paths
}

fn scan_dir(folder: &Path, skip: &str) -> BTreeMap<String, MatrixProfile> {
    let mut out = BTreeMap::new();
    for path in list_toml(folder) {
        let Some(profile) = parse(&path) else {
            continue;
        };
        if profile.name != skip {
            out.insert(profile.name.clone(), profile);
        }
    }
    out
}

/// Bundled and user profiles, with bundled entries winning by display name.
#[must_use]
pub fn scan_profiles(
    user_dir: &Path,
    bundled_dir: &Path,
    skip: &str,
) -> BTreeMap<String, MatrixProfile> {
    let mut out = scan_dir(user_dir, skip);
    out.extend(scan_dir(bundled_dir, skip));
    out
}

/// Whether `name` is the read-only built-in entry.
#[must_use]
pub fn is_builtin(name: &str, default: &str) -> bool {
    name == default
}

/// The profile names a picker lists, built-in first.
#[must_use]
pub fn list_profiles(user_dir: &Path, bundled_dir: &Path, default: &str) -> Vec<String> {
    let mut names = vec![default.to_string()];
    names.extend(scan_profiles(user_dir, bundled_dir, default).into_keys());
    names
}

/// A profile's flat matrix, or `None` for the built-in, a missing name, or an invalid file.
#[must_use]
pub fn matrix_for(
    user_dir: &Path,
    bundled_dir: &Path,
    default: &str,
    name: &str,
) -> Option<[f64; 9]> {
    if name == default {
        return None;
    }
    scan_profiles(user_dir, bundled_dir, default)
        .get(name)
        .map(|profile| profile.matrix)
}

/// `[(heading, names)]` in picker order, skipping empty groups.
#[must_use]
pub fn grouped_profiles(
    user_dir: &Path,
    bundled_dir: &Path,
    process: Option<ProcessMode>,
) -> Vec<(&'static str, Vec<String>)> {
    let profiles = scan_profiles(user_dir, bundled_dir, CROSSTALK_DEFAULT);
    let mut groups: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    if process.is_none_or(|mode| mode == ProcessMode::C41) {
        groups
            .entry(CrosstalkType::Builtin.heading())
            .or_default()
            .push(CROSSTALK_DEFAULT.to_string());
    }
    for (name, profile) in profiles {
        if process.is_some_and(|mode| profile.process != mode) {
            continue;
        }
        groups
            .entry(profile.profile_type.heading())
            .or_default()
            .push(name);
    }
    let order = [
        CrosstalkType::Builtin.heading(),
        CrosstalkType::Measured.heading(),
        CrosstalkType::Tuned.heading(),
        CrosstalkType::Specsheet.heading(),
        CrosstalkType::Other.heading(),
    ];
    order
        .into_iter()
        .filter_map(|heading| groups.remove(heading).map(|names| (heading, names)))
        .collect()
}

/// The path a user profile with this name uses, stepping past another profile's file.
#[must_use]
pub fn path_for_name(folder: &Path, name: &str, fallback: &str) -> PathBuf {
    for path in list_toml(folder) {
        let Some(profile) = parse(&path) else {
            continue;
        };
        if profile.name == name {
            return path;
        }
    }
    let base = slugify(name, fallback);
    let first = folder.join(format!("{base}.toml"));
    if !first.exists() {
        return first;
    }
    for index in 2..1000 {
        let candidate = folder.join(format!("{base}_{index}.toml"));
        if !candidate.exists() {
            return candidate;
        }
    }
    first
}

fn row(matrix: &[f64]) -> String {
    format!("  [{:.6}, {:.6}, {:.6}],", matrix[0], matrix[1], matrix[2])
}

/// Write a user crosstalk profile and return its path.
pub fn save_crosstalk(
    folder: &Path,
    name: &str,
    matrix: &[f64; 9],
    profile_type: CrosstalkType,
    process: ProcessMode,
) -> std::io::Result<PathBuf> {
    fs::create_dir_all(folder)?;
    let content = format!(
        "name = \"{}\"\ntype = \"{}\"\nprocess = \"{}\"\nmatrix = [\n{}\n{}\n{}\n]\n",
        escape_toml_string(name),
        escape_toml_string(profile_type.as_str()),
        escape_toml_string(&process.to_string()),
        row(&matrix[0..3]),
        row(&matrix[3..6]),
        row(&matrix[6..9]),
    );
    let path = path_for_name(folder, name, "crosstalk");
    fs::write(&path, content)?;
    Ok(path)
}

/// Write a user sensor profile and return its path.
pub fn save_sensor(folder: &Path, name: &str, matrix: &[f64; 9]) -> std::io::Result<PathBuf> {
    fs::create_dir_all(folder)?;
    let content = format!(
        "name = \"{}\"\nmatrix = [\n{}\n{}\n{}\n]\n",
        escape_toml_string(name),
        row(&matrix[0..3]),
        row(&matrix[3..6]),
        row(&matrix[6..9]),
    );
    let path = path_for_name(folder, name, "sensor");
    fs::write(&path, content)?;
    Ok(path)
}

/// Remove the user profile whose display name matches.
pub fn delete(folder: &Path, name: &str) {
    for path in list_toml(folder) {
        if parse(&path).is_some_and(|profile| profile.name == name) {
            let _ = fs::remove_file(path);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(folder: &Path, name: &str, text: &str) {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join(name), text).unwrap();
    }

    #[test]
    fn a_user_matrix_lists_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "portra.toml",
            "name = \"Portra 400\"\nmatrix = [[1, -0.1, 0], [0, 1, -0.1], [0, 0, 1]]\n",
        );
        let missing = dir.path().join("missing");
        assert_eq!(
            list_profiles(dir.path(), &missing, CROSSTALK_DEFAULT),
            ["Generic C41", "Portra 400"]
        );
        assert_eq!(
            matrix_for(dir.path(), &missing, CROSSTALK_DEFAULT, "Portra 400"),
            Some([1.0, -0.1, 0.0, 0.0, 1.0, -0.1, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            matrix_for(dir.path(), &missing, CROSSTALK_DEFAULT, "Generic C41"),
            None
        );
    }

    #[test]
    fn a_bundled_profile_wins_a_name_collision() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        let bundled = dir.path().join("bundled");
        write(
            &user,
            "p.toml",
            "name = \"Portra 400\"\nmatrix = [[1, 0, 0], [0, 1, 0], [0, 0, 1]]\n",
        );
        write(
            &bundled,
            "p.toml",
            "name = \"Portra 400\"\nmatrix = [[2, 0, 0], [0, 2, 0], [0, 0, 2]]\n",
        );
        assert_eq!(
            matrix_for(&user, &bundled, CROSSTALK_DEFAULT, "Portra 400"),
            Some([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0])
        );
    }

    #[test]
    fn malformed_profiles_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "bad.toml", "matrix = [[1, 0], [0, 1]]\n");
        write(dir.path(), "no_matrix.toml", "name = \"x\"\n");
        assert_eq!(
            list_profiles(dir.path(), &dir.path().join("missing"), SENSOR_NONE),
            ["None"]
        );
    }

    #[test]
    fn save_round_trips_and_delete_removes_only_one() {
        let dir = tempfile::tempdir().unwrap();
        let matrix = [1.0, -0.1, 0.0, 0.0, 1.0, -0.1, 0.0, 0.0, 1.0];
        save_sensor(dir.path(), "Setup A", &matrix).unwrap();
        save_sensor(dir.path(), "Setup B", &[2.0; 9]).unwrap();
        assert_eq!(
            matrix_for(
                dir.path(),
                &dir.path().join("missing"),
                SENSOR_NONE,
                "Setup A"
            ),
            Some(matrix)
        );
        delete(dir.path(), "Setup A");
        assert_eq!(
            matrix_for(
                dir.path(),
                &dir.path().join("missing"),
                SENSOR_NONE,
                "Setup A"
            ),
            None
        );
        assert!(
            matrix_for(
                dir.path(),
                &dir.path().join("missing"),
                SENSOR_NONE,
                "Setup B"
            )
            .is_some()
        );
    }

    #[test]
    fn groups_follow_the_picker_order_and_process_filter() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tuned.toml",
            "name = \"Mine\"\ntype = \"tuned\"\nprocess = \"Transparency\"\nmatrix = [[1, 0, 0], [0, 1, 0], [0, 0, 1]]\n",
        );
        let missing = dir.path().join("missing");
        assert_eq!(
            grouped_profiles(dir.path(), &missing, None),
            [
                ("Built-in", vec!["Generic C41".to_string()]),
                ("Tuned on a rig", vec!["Mine".to_string()])
            ]
        );
        assert_eq!(
            grouped_profiles(dir.path(), &missing, Some(ProcessMode::E6)),
            [("Tuned on a rig", vec!["Mine".to_string()])]
        );
    }
}
