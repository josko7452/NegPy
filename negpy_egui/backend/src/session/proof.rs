//! App-level soft-proof settings.
//!
//! Mirrors the proof fields in `negpy/desktop/session.py`. A proof describes the
//! printer and paper in the room, so it stays outside the per-frame edit.

use serde_json::{Map, json};

use crate::infrastructure::storage::repository::{RepositoryError, StorageRepository};

pub const SOFT_PROOF_KEY: &str = "soft_proof_enabled";
pub const PROOF_ICC_KEY: &str = "proof_icc_path";
pub const PROOF_INTENT_KEY: &str = "proof_intent";
pub const PROOF_BLACK_POINT_KEY: &str = "proof_black_point";
pub const PROOF_PAPER_WHITE_KEY: &str = "proof_paper_white";
pub const PROOF_INK_BLACK_KEY: &str = "proof_ink_black";
pub const PROOF_GAMUT_WARNING_KEY: &str = "proof_gamut_warning";

/// The proof condition the preview simulates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofSettings {
    pub enabled: bool,
    pub icc_path: Option<String>,
    pub intent: String,
    pub black_point: bool,
    pub paper_white: bool,
    pub ink_black: bool,
    pub gamut_warning: bool,
}

impl Default for ProofSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            icc_path: None,
            intent: "relative_colorimetric".to_string(),
            black_point: false,
            paper_white: false,
            ink_black: false,
            gamut_warning: false,
        }
    }
}

impl ProofSettings {
    /// Read the proof keys from the profile settings database.
    #[must_use]
    pub fn load(repo: &StorageRepository) -> Self {
        let defaults = Self::default();
        let value = |key: &str| repo.global_setting(key).ok().flatten();
        let path = value(PROOF_ICC_KEY)
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|path| std::path::Path::new(path).is_file());
        let intent = value(PROOF_INTENT_KEY)
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|intent| {
                matches!(
                    intent.as_str(),
                    "relative_colorimetric" | "perceptual" | "saturation"
                )
            })
            .unwrap_or(defaults.intent);
        Self {
            enabled: value(SOFT_PROOF_KEY)
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.enabled),
            icc_path: path,
            intent,
            black_point: value(PROOF_BLACK_POINT_KEY)
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.black_point),
            paper_white: value(PROOF_PAPER_WHITE_KEY)
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.paper_white),
            ink_black: value(PROOF_INK_BLACK_KEY)
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.ink_black),
            gamut_warning: value(PROOF_GAMUT_WARNING_KEY)
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.gamut_warning),
        }
    }

    /// Write the proof keys to the profile settings database.
    pub fn save(&self, repo: &mut StorageRepository) -> Result<(), RepositoryError> {
        let mut settings = Map::new();
        settings.insert(SOFT_PROOF_KEY.to_string(), json!(self.enabled));
        settings.insert(PROOF_ICC_KEY.to_string(), json!(self.icc_path));
        settings.insert(PROOF_INTENT_KEY.to_string(), json!(self.intent));
        settings.insert(PROOF_BLACK_POINT_KEY.to_string(), json!(self.black_point));
        settings.insert(PROOF_PAPER_WHITE_KEY.to_string(), json!(self.paper_white));
        settings.insert(PROOF_INK_BLACK_KEY.to_string(), json!(self.ink_black));
        settings.insert(
            PROOF_GAMUT_WARNING_KEY.to_string(),
            json!(self.gamut_warning),
        );
        repo.save_global_settings(&settings)
    }
}
