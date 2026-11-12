//! Mirrors `negpy/features/stitch/models.py`: panorama stitch from overlapping parts.

use serde::{Deserialize, Serialize};

use crate::domain::tokens::composite_token;

/// One frame assembled from overlapping part scans. Transforms are full-res part->canvas
/// affines fixed at registration time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StitchConfig {
    #[serde(default)]
    pub stitch_enabled: bool,
    /// Non-primary parts, registration order.
    #[serde(default)]
    pub stitch_paths: Vec<String>,
    /// Per part including the primary, 2x3 row-major.
    #[serde(default)]
    pub stitch_transforms: Vec<Vec<f64>>,
    /// Full-res `(W, H)`.
    #[serde(default)]
    pub stitch_canvas: [i64; 2],
    /// Full-res decoded `(W, H)` per part.
    #[serde(default)]
    pub stitch_sizes: Vec<[i64; 2]>,
    /// RGB-scan `(green, blue)` per part including the primary.
    #[serde(default)]
    pub stitch_triplets: Vec<[String; 2]>,
    #[serde(default = "default_true")]
    pub stitch_align: bool,
}

impl Default for StitchConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the stitch defaults deserialize")
    }
}

/// True when any part is assembled from an R/G/B exposure triplet.
#[must_use]
pub fn stitch_has_triplets(config: &StitchConfig) -> bool {
    config
        .stitch_triplets
        .iter()
        .any(|[green, blue]| !green.is_empty() && !blue.is_empty())
}

/// The predicate the decode paths use to decide to assemble a composite.
#[must_use]
pub fn stitch_active(config: &StitchConfig) -> bool {
    config.stitch_enabled && !config.stitch_paths.is_empty()
}

/// Identity of the active stitch, folded into the render source hash.
#[must_use]
pub fn stitch_token(config: &StitchConfig) -> String {
    if !stitch_active(config) {
        return String::new();
    }
    // composite_token digests only StitchConfig fields, so an assembly detail with no field
    // of its own is versioned here or a cached source outlives it.
    let mut paths: Vec<&str> = config.stitch_paths.iter().map(String::as_str).collect();
    for [green, blue] in &config.stitch_triplets {
        paths.push(green);
        paths.push(blue);
    }
    composite_token("stitch-v3", config, &paths)
}

/// Display name of a composite: joined part stems, elided beyond three parts.
#[must_use]
pub fn stitch_name(part_paths: &[String]) -> String {
    let stems: Vec<String> = part_paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_stem()
                .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
        })
        .collect();
    if stems.len() > 3 {
        format!("{} +{} (Stitch)", stems[0], stems.len() - 1)
    } else {
        format!("{} (Stitch)", stems.join("+"))
    }
}

const fn default_true() -> bool {
    true
}
