//! The crosstalk and sensor profile API over `services/assets/profiles.rs`.

use std::fs;
use std::path::Path;

use negpy_backend::domain::models::ProcessMode;
use negpy_backend::services::assets::profiles::{
    CROSSTALK_DEFAULT, CrosstalkType, SENSOR_NONE, crosstalk_is_bundled, crosstalk_process,
    crosstalk_type, delete, grouped_profiles, list_profiles, list_sensor_profiles, matrix_for,
    save_crosstalk, save_crosstalk_renamed, save_sensor, sensor_is_bundled, sensor_matrix,
};

const IDENTITY: [f64; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

fn dirs() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

fn write_profile(folder: &Path, filename: &str, body: &str) {
    fs::create_dir_all(folder).unwrap();
    fs::write(folder.join(filename), body).unwrap();
}

#[test]
fn a_saved_crosstalk_profile_keeps_its_process_and_type() {
    let (user, bundled) = dirs();
    save_crosstalk(
        user.path(),
        "Slide",
        &IDENTITY,
        CrosstalkType::Tuned,
        ProcessMode::E6,
    )
    .unwrap();
    assert_eq!(
        crosstalk_process(user.path(), bundled.path(), "Slide"),
        ProcessMode::E6
    );
    assert_eq!(
        crosstalk_type(user.path(), bundled.path(), "Slide"),
        CrosstalkType::Tuned
    );
    assert_eq!(
        matrix_for(user.path(), bundled.path(), CROSSTALK_DEFAULT, "Slide"),
        Some(IDENTITY)
    );
}

#[test]
fn an_unknown_process_reads_as_color_negative() {
    let (user, bundled) = dirs();
    write_profile(
        user.path(),
        "odd.toml",
        "name = \"Odd\"\nprocess = \"K-14\"\nmatrix = [[1,0,0],[0,1,0],[0,0,1]]\n",
    );
    assert_eq!(
        crosstalk_process(user.path(), bundled.path(), "Odd"),
        ProcessMode::C41
    );
}

#[test]
fn an_unknown_type_groups_under_other() {
    let (user, bundled) = dirs();
    write_profile(
        user.path(),
        "odd.toml",
        "name = \"Odd\"\ntype = \"handed-down\"\nmatrix = [[1,0,0],[0,1,0],[0,0,1]]\n",
    );
    assert_eq!(
        crosstalk_type(user.path(), bundled.path(), "Odd"),
        CrosstalkType::Other
    );
}

#[test]
fn the_process_filter_hides_a_matrix_from_another_film() {
    let (user, bundled) = dirs();
    save_crosstalk(
        user.path(),
        "Mine",
        &IDENTITY,
        CrosstalkType::Tuned,
        ProcessMode::C41,
    )
    .unwrap();
    save_crosstalk(
        user.path(),
        "Slide",
        &IDENTITY,
        CrosstalkType::Tuned,
        ProcessMode::E6,
    )
    .unwrap();
    let c41: Vec<String> = grouped_profiles(user.path(), bundled.path(), Some(ProcessMode::C41))
        .into_iter()
        .flat_map(|(_, names)| names)
        .collect();
    assert_eq!(c41, [CROSSTALK_DEFAULT, "Mine"]);
    let e6: Vec<String> = grouped_profiles(user.path(), bundled.path(), Some(ProcessMode::E6))
        .into_iter()
        .flat_map(|(_, names)| names)
        .collect();
    assert_eq!(e6, ["Slide"]);
}

#[test]
fn bundled_and_builtin_crosstalk_is_read_only() {
    let (user, bundled) = dirs();
    save_crosstalk(
        bundled.path(),
        "Bundled",
        &IDENTITY,
        CrosstalkType::Measured,
        ProcessMode::C41,
    )
    .unwrap();
    assert!(crosstalk_is_bundled(bundled.path(), CROSSTALK_DEFAULT));
    assert!(crosstalk_is_bundled(bundled.path(), "Bundled"));
    assert!(!crosstalk_is_bundled(bundled.path(), "User only"));
    assert!(!list_profiles(user.path(), bundled.path(), CROSSTALK_DEFAULT).is_empty());
}

#[test]
fn a_rename_keeps_the_type_and_removes_the_old_file() {
    let (user, bundled) = dirs();
    save_crosstalk_renamed(
        user.path(),
        "Before",
        "After",
        &IDENTITY,
        CrosstalkType::Measured,
        ProcessMode::C41,
    )
    .unwrap();
    assert_eq!(
        crosstalk_type(user.path(), bundled.path(), "After"),
        CrosstalkType::Measured
    );
    assert!(
        !list_profiles(user.path(), bundled.path(), CROSSTALK_DEFAULT)
            .contains(&"Before".to_string())
    );
}

#[test]
fn a_sensor_profile_round_trips_and_none_means_no_correction() {
    let folder = tempfile::tempdir().unwrap();
    save_sensor(folder.path(), "Rig", &IDENTITY).unwrap();
    assert_eq!(list_sensor_profiles(folder.path()), [SENSOR_NONE, "Rig"]);
    assert_eq!(sensor_matrix(folder.path(), "Rig"), Some(IDENTITY));
    assert_eq!(sensor_matrix(folder.path(), SENSOR_NONE), None);
    assert!(sensor_is_bundled(SENSOR_NONE));
    assert!(!sensor_is_bundled("Rig"));
}

#[test]
fn deleting_a_profile_removes_only_that_name() {
    let folder = tempfile::tempdir().unwrap();
    save_sensor(folder.path(), "One", &IDENTITY).unwrap();
    save_sensor(folder.path(), "Two", &IDENTITY).unwrap();
    delete(folder.path(), "One");
    assert_eq!(list_sensor_profiles(folder.path()), [SENSOR_NONE, "Two"]);
}
