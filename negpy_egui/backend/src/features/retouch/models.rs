//! The clone brush stroke shape the repair uses.

/// One clone brush stroke in raw-normalized coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneStrokeConfig {
    pub points: Vec<[f64; 2]>,
    pub size: f64,
    pub dx: f64,
    pub dy: f64,
    pub strength: f64,
    pub feather: f64,
    pub match_tone: bool,
}
