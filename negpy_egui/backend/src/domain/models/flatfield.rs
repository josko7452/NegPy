//! Mirrors `negpy/features/flatfield/models.py`: the flat-field correction selection.

use serde::{Deserialize, Serialize};

/// The flat-field profile a frame corrects with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlatFieldConfig {
    #[serde(default)]
    pub apply: bool,
    /// Empty = no profile selected.
    #[serde(default)]
    pub profile_id: String,
}
