//! Mirrors the proof preferences in `negpy/desktop/session.py`.

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::session::proof::{
    PROOF_BLACK_POINT_KEY, PROOF_ICC_KEY, PROOF_INTENT_KEY, ProofSettings,
};

fn repo(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

#[test]
fn the_proof_defaults_match_the_python_session() {
    let settings = ProofSettings::default();
    assert!(settings.enabled);
    assert_eq!(settings.icc_path, None);
    assert_eq!(settings.intent, "relative_colorimetric");
    assert!(!settings.black_point);
    assert!(!settings.paper_white);
    assert!(!settings.ink_black);
    assert!(!settings.gamut_warning);
}

#[test]
fn a_saved_proof_round_trips_through_the_settings_database() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repo(dir.path());
    let icc = dir.path().join("paper.icc");
    std::fs::write(&icc, b"profile").unwrap();
    let settings = ProofSettings {
        enabled: false,
        icc_path: Some(icc.display().to_string()),
        intent: "perceptual".to_string(),
        black_point: true,
        paper_white: true,
        ink_black: true,
        gamut_warning: true,
    };
    settings.save(&mut repo).unwrap();
    assert_eq!(ProofSettings::load(&repo), settings);
}

#[test]
fn a_missing_profile_path_is_not_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repo(dir.path());
    let mut values = serde_json::Map::new();
    values.insert(
        PROOF_ICC_KEY.to_string(),
        serde_json::json!("/missing/paper.icc"),
    );
    values.insert(PROOF_INTENT_KEY.to_string(), serde_json::json!("nonsense"));
    values.insert(PROOF_BLACK_POINT_KEY.to_string(), serde_json::json!(true));
    repo.save_global_settings(&values).unwrap();
    let settings = ProofSettings::load(&repo);
    assert_eq!(settings.icc_path, None);
    assert_eq!(settings.intent, "relative_colorimetric");
    assert!(settings.black_point);
}
