//! Ports the observable rules of `tests/test_crosstalk_profiles.py`.

use negpy_backend::domain::models::ProcessMode;
use negpy_backend::services::assets::crosstalk::{
    CrosstalkDirs, CrosstalkType, DEFAULT_NAME, delete, get_matrix, get_process, get_type,
    grouped_profiles, list_profiles, save, scan_user,
};

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
fn the_bundled_gallery_parses_with_small_cross_terms() {
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crosstalk");
    let out = negpy_backend::services::assets::crosstalk::scan_dir(&shipped);
    assert!(!out.is_empty(), "scanned {shipped:?}");
    for (name, matrix) in &out {
        let mut normalized = [[0.0; 3]; 3];
        for row in 0..3 {
            let diagonal = matrix[row * 3 + row];
            assert_ne!(diagonal, 0.0);
            for column in 0..3 {
                normalized[row][column] = matrix[row * 3 + column] / diagonal;
            }
        }
        for row in 0..3 {
            for column in 0..3 {
                if row != column {
                    assert!(
                        normalized[row][column].abs() < 0.2,
                        "a cross term past 0.2 is a misread curve or a transposed matrix"
                    );
                }
            }
        }
    }
}

#[test]
fn a_saved_profile_is_tuned_and_lists_under_that_heading() {
    let (_temp, dirs) = dirs();
    save(&dirs, "Mine", IDENTITY, CrosstalkType::Tuned, None).unwrap();
    assert_eq!(get_type(&dirs, "Mine"), CrosstalkType::Tuned);
    let groups = grouped_profiles(&dirs, None);
    let tuned = groups
        .iter()
        .find(|(kind, _)| *kind == CrosstalkType::Tuned)
        .map(|(_, names)| names.clone())
        .unwrap_or_default();
    assert_eq!(tuned, ["Mine"]);
}

#[test]
fn a_legacy_process_name_still_reaches_the_transparency_dropdown() {
    let (_temp, dirs) = dirs();
    std::fs::write(
        dirs.user.join("slide.toml"),
        "name = \"Old Slide\"\nprocess = \"E-6\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
    )
    .unwrap();
    assert_eq!(get_process(&dirs, "Old Slide"), ProcessMode::E6);
    let slide = grouped_profiles(&dirs, Some(ProcessMode::E6));
    let names = slide
        .iter()
        .flat_map(|(_, names)| names.clone())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Old Slide"]);
    assert_eq!(
        grouped_profiles(&dirs, Some(ProcessMode::C41)),
        [(CrosstalkType::Builtin, vec![DEFAULT_NAME.to_string()])]
    );
}

#[test]
fn a_save_reuses_the_profile_file_after_a_delete_of_its_neighbour() {
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
    delete(&dirs, "Portra 400");
    save(
        &dirs,
        "Portra-400",
        [9.0, 0.0, 0.0, 0.0, 9.0, 0.0, 0.0, 0.0, 9.0],
        CrosstalkType::Tuned,
        None,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_dir(&dirs.user).unwrap().count(),
        1,
        "a later save must not strand the profile behind a stale copy"
    );
    assert_eq!(scan_user(&dirs)["Portra-400"][0], 9.0);
}

#[test]
fn grouping_never_drops_or_invents_a_profile() {
    let (_temp, dirs) = dirs();
    std::fs::write(
        dirs.user.join("weird.toml"),
        "name = \"Weird\"\ntype = \"handed-down-by-owls\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
    )
    .unwrap();
    std::fs::write(
        dirs.user.join("bare.toml"),
        "name = \"Bare\"\nmatrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]\n",
    )
    .unwrap();
    let groups = grouped_profiles(&dirs, None);
    let mut flat: Vec<String> = groups.iter().flat_map(|(_, names)| names.clone()).collect();
    let mut listed = list_profiles(&dirs);
    flat.sort();
    listed.sort();
    assert_eq!(flat, listed);
    assert_eq!(
        groups
            .iter()
            .find(|(kind, _)| *kind == CrosstalkType::Other)
            .map(|(_, names)| names.clone())
            .unwrap_or_default(),
        ["Bare", "Weird"]
    );
    assert!(get_matrix(&dirs, "Weird").is_some());
}
