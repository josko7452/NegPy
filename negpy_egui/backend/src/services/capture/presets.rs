//! Film-stock capture recipes and their session store.
//!
//! Mirrors `negpy/services/capture/presets.py`. The recipes live under one global
//! setting, so a profile measured on one film stock is recalled by name.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::{RepositoryError, StorageRepository};

/// Framing runs all channels against a single-channel exposure, so it is dimmed.
pub const FRAMING_DIM_STOPS: u32 = 3;
/// The dimmest level that visibly lights an RGB-only Scanlight.
pub const FRAMING_MIN_LEVEL_RGB_ONLY: u32 = 115;
/// The global-setting key the recipes live under.
pub const PRESETS_KEY: &str = "scanlight_presets";

/// One film stock's capture recipe.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanlightPreset {
    #[serde(default = "default_level")]
    pub r_level: u32,
    #[serde(default = "default_level")]
    pub g_level: u32,
    #[serde(default = "default_level")]
    pub b_level: u32,
    #[serde(default)]
    pub w_level: u32,
    #[serde(default)]
    pub shutter_r: String,
    #[serde(default)]
    pub shutter_g: String,
    #[serde(default)]
    pub shutter_b: String,
    #[serde(default)]
    pub shutter_ir: String,
    #[serde(default)]
    pub infrared: bool,
    #[serde(default)]
    pub iso: String,
    #[serde(default)]
    pub aperture: String,
    #[serde(default)]
    pub single_capture: bool,
    #[serde(default)]
    pub blend_capture: bool,
    #[serde(default)]
    pub sensor_profile: String,
}

fn default_level() -> u32 {
    255
}

/// The framing-light mix: the scan recipe, dimmed, with an RGB-only floor.
#[must_use]
pub fn framing_levels(r: u32, g: u32, b: u32, stops: u32, rgb_only: bool) -> [u32; 3] {
    let dim = |level: u32| {
        if level == 0 {
            return 0;
        }
        let dimmed = (level >> stops).max(1);
        if rgb_only {
            level.min(dimmed.max(FRAMING_MIN_LEVEL_RGB_ONLY))
        } else {
            dimmed
        }
    };
    [dim(r), dim(g), dim(b)]
}

/// The framing levels at the shipped dimming.
#[must_use]
pub fn default_framing_levels(r: u32, g: u32, b: u32, rgb_only: bool) -> [u32; 3] {
    framing_levels(r, g, b, FRAMING_DIM_STOPS, rgb_only)
}

/// Named recipes persisted under one global-setting key.
#[derive(Clone, Debug)]
pub struct PresetStore {
    repo: StorageRepository,
}

impl PresetStore {
    #[must_use]
    pub fn new(repo: StorageRepository) -> Self {
        Self { repo }
    }

    fn all(&self) -> Result<Map<String, Value>, RepositoryError> {
        Ok(self
            .repo
            .global_setting(PRESETS_KEY)?
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default())
    }

    /// Every stored recipe name, sorted.
    pub fn names(&self) -> Result<Vec<String>, RepositoryError> {
        let mut names: Vec<String> = self.all()?.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    /// One recipe; an entry that is not an object, or cannot build, reads as missing.
    pub fn get(&self, name: &str) -> Result<Option<ScanlightPreset>, RepositoryError> {
        let all = self.all()?;
        let Some(raw) = all.get(name).and_then(Value::as_object) else {
            return Ok(None);
        };
        let filtered: Map<String, Value> = raw
            .iter()
            .filter(|(key, _)| is_preset_field(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        Ok(serde_json::from_value(Value::Object(filtered)).ok())
    }

    /// Whether a preset was calibrated with this sensor profile.
    pub fn owns_sensor_profile(&self, profile: &str) -> Result<bool, RepositoryError> {
        if profile.is_empty() {
            return Ok(false);
        }
        Ok(self.all()?.values().any(|raw| {
            raw.get("sensor_profile")
                .and_then(Value::as_str)
                .is_some_and(|value| value == profile)
        }))
    }

    /// Save a recipe under `name`.
    pub fn save(&self, name: &str, preset: &ScanlightPreset) -> Result<(), RepositoryError> {
        let mut all = self.all()?;
        let value = serde_json::to_value(preset)
            .map_err(|error| RepositoryError::Serialize(error.to_string()))?;
        all.insert(name.to_string(), value);
        let mut settings = Map::new();
        settings.insert(PRESETS_KEY.to_string(), Value::Object(all));
        let mut repo = self.repo.clone();
        repo.save_global_settings(&settings)
    }

    /// Remove one recipe. A missing name is a no-op.
    pub fn delete(&self, name: &str) -> Result<(), RepositoryError> {
        let mut all = self.all()?;
        if all.remove(name).is_none() {
            return Ok(());
        }
        let mut settings = Map::new();
        settings.insert(PRESETS_KEY.to_string(), Value::Object(all));
        let mut repo = self.repo.clone();
        repo.save_global_settings(&settings)
    }
}

fn is_preset_field(key: &str) -> bool {
    matches!(
        key,
        "r_level"
            | "g_level"
            | "b_level"
            | "w_level"
            | "shutter_r"
            | "shutter_g"
            | "shutter_b"
            | "shutter_ir"
            | "infrared"
            | "iso"
            | "aperture"
            | "single_capture"
            | "blend_capture"
            | "sensor_profile"
    )
}
