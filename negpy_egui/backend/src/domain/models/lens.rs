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

/// The six coefficients of a rectilinear warp, in DNG's order.
pub const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

/// One embedded lens-warp record.
#[derive(Clone, Debug, PartialEq)]
pub enum LensWarp {
    /// DNG `WarpRectilinear`: one coefficient set, or one per channel for lateral CA.
    Rectilinear {
        coefficients: Vec<[f64; 6]>,
        center: (f64, f64),
    },
    /// Panasonic's Ru = Rd + scale*(a*Rd^3 + b*Rd^5 + c*Rd^7).
    Panasonic {
        scale: f64,
        a: f64,
        b: f64,
        c: f64,
    },
    /// Sony's radial knot factors.
    Sony {
        distortion: Vec<f64>,
        ca_red: Vec<f64>,
        ca_blue: Vec<f64>,
    },
}

impl LensWarp {
    /// Whether this warp carries distortion correction.
    #[must_use]
    pub fn has_distortion(&self) -> bool {
        match self {
            Self::Rectilinear { coefficients, .. } => {
                let common = coefficients[if coefficients.len() == 1 { 0 } else { 1 }];
                common != IDENTITY
            }
            Self::Panasonic { scale, a, b, c } => {
                *scale != 1.0 || *a != 0.0 || *b != 0.0 || *c != 0.0
            }
            Self::Sony { distortion, .. } => distortion.iter().any(|value| *value != 0.0),
        }
    }

    /// Whether this warp carries lateral chromatic-aberration correction.
    #[must_use]
    pub fn has_ca(&self) -> bool {
        match self {
            Self::Rectilinear { coefficients, .. } => {
                coefficients.len() == 3
                    && (coefficients[0] != coefficients[1] || coefficients[2] != coefficients[1])
            }
            Self::Panasonic { .. } => false,
            Self::Sony {
                ca_red, ca_blue, ..
            } => ca_red.iter().any(|value| *value != 0.0) || ca_blue.iter().any(|value| *value != 0.0),
        }
    }
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
        self.warps.iter().any(LensWarp::has_distortion)
    }

    /// Whether any warp carries lateral chromatic-aberration correction.
    #[must_use]
    pub fn ca(&self) -> bool {
        self.warps.iter().any(LensWarp::has_ca)
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
