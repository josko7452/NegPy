//! Runtime lens-correction metadata, mirroring `negpy/features/lens/models.py`.
//!
//! This is not a saved `WorkspaceConfig` section: the source file carries the correction,
//! and the render only needs its availability and description.

/// Which corrections a warp can apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LensCorrections {
    pub distortion: bool,
    pub ca: bool,
}

impl LensCorrections {
    /// Whether either correction is enabled.
    #[must_use]
    pub fn any(self) -> bool {
        self.distortion || self.ca
    }
}

/// One embedded lens-warp record.
#[derive(Clone, Debug, PartialEq)]
pub struct LensWarp {
    pub has_distortion: bool,
    pub has_ca: bool,
}

/// The correction data read from a source file.
#[derive(Clone, Debug, PartialEq)]
pub struct LensMetadata {
    pub source: String,
    pub warps: Vec<LensWarp>,
    pub reason: String,
    pub active_area: Option<[i64; 4]>,
    pub buffer_area: Option<[i64; 4]>,
    pub fill_scale: f64,
}

impl Default for LensMetadata {
    fn default() -> Self {
        Self {
            source: String::new(),
            warps: Vec::new(),
            reason: "No embedded lens correction data.".to_string(),
            active_area: None,
            buffer_area: None,
            fill_scale: 1.0,
        }
    }
}

impl LensMetadata {
    /// Whether any warp carries distortion correction.
    #[must_use]
    pub fn distortion(&self) -> bool {
        self.warps.iter().any(|warp| warp.has_distortion)
    }

    /// Whether any warp carries lateral chromatic-aberration correction.
    #[must_use]
    pub fn ca(&self) -> bool {
        self.warps.iter().any(|warp| warp.has_ca)
    }

    /// Whether any correction is available.
    #[must_use]
    pub fn available(&self) -> bool {
        self.distortion() || self.ca()
    }

    /// The source and the corrections it offers, or the reason none apply.
    #[must_use]
    pub fn description(&self) -> String {
        if !self.available() {
            return self.reason.clone();
        }
        let corrections: Vec<&str> = [(self.distortion(), "distortion"), (self.ca(), "lateral CA")]
            .into_iter()
            .filter_map(|(enabled, label)| enabled.then_some(label))
            .collect();
        format!("{}: {}", self.source, corrections.join(" + "))
    }
}
