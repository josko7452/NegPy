//! Scalar values of `EXPOSURE_CONSTANTS` (negpy/features/exposure/models.py).

/// Max absolute density offset per CMY white-balance slider unit.
pub const CMY_MAX_DENSITY: f64 = 0.2;
/// Scales the density slider's effect on the exposure pivot.
pub const DENSITY_MULTIPLIER: f64 = 0.2;
/// Density the reference tone prints at.
pub const REFERENCE_PRINT_DENSITY: f64 = 0.75;
/// Auto Density: density the metered anchor prints at.
pub const ANCHOR_TARGET_DENSITY: f64 = 0.75;
/// Mid-sparing sigmoid sharpness for the zone-density weights.
pub const ZONE_DENSITY_SHARPNESS: f64 = 4.0;
/// Shadow zone offset from the reference print density.
pub const ZONE_DENSITY_SHADOW_OFFSET: f64 = 0.75;
/// Highlight zone offset from the reference print density.
pub const ZONE_DENSITY_HIGHLIGHT_OFFSET: f64 = -0.40;
/// Hold Black's taper span below the paper-black ceiling.
pub const ZONE_BLACK_HOLD_TAPER: f64 = 0.6;
/// Default normalized midtone reference with Auto Density off.
pub const ASSUMED_ANCHOR: f64 = 0.46;
pub const ISO_R_MIN: f64 = 50.0;
pub const ISO_R_MAX: f64 = 180.0;
pub const SLOPE_MIN: f64 = 2.0;
pub const SLOPE_MAX: f64 = 10.0;
pub const D_MAX: f64 = 2.3;
pub const D_MIN: f64 = 0.06;
pub const TOE_SHOULDER_STRENGTH: f64 = 0.85;
pub const TOE_SHARPNESS_BASE: f64 = 6.0;
pub const SHOULDER_SHARPNESS_BASE: f64 = 3.0;
pub const TOESHOULDER_WIDTH_REF: f64 = 2.5;
pub const TOE_HEIGHT: f64 = 0.90;
pub const SHOULDER_HEIGHT: f64 = 0.35;
pub const GRADE_CONTRAST_SCALE: f64 = 2.9;
/// Grade-coupled toe strength: `0.15 * 0.35 / 0.90`.
pub const TOE_GRADE_STRENGTH: f64 = 0.15 * 0.35 / 0.90;
pub const SHOULDER_GRADE_STRENGTH: f64 = 0.12;
pub const AUTO_GRADE_TARGET: f64 = 0.85;
pub const AUTO_GRADE_STRENGTH: f64 = 0.4;
pub const AUTO_GRADE_MAX_OVERFILL: f64 = 1.2;
pub const AUTO_GRADE_NOMINAL_RANGE: f64 = 0.9;
pub const AUTO_GRADE_NOMINAL_RATIO: f64 = 1.5;
pub const SHADOW_REACH_DENSITY: f64 = 1.9;
pub const HIGHLIGHT_HOLD_DENSITY: f64 = 0.10;
pub const HIGHLIGHT_HOLD_MAX: f64 = 0.5;
pub const SHADOW_HOLD_STRENGTH: f64 = 0.40;
pub const SHADOW_HOLD_OVERSHOOT: f64 = 0.15;
pub const SHADOW_HOLD_MAX: f64 = 0.3;
pub const PREFLASH_THRESHOLD_DENSITY: f64 = 0.04;
pub const FLAT_LOG_GAIN: f64 = 0.65;
pub const FLAT_LOG_LIFT: f64 = 0.10;
pub const PAPER_MIDTONE_GAMMA: f64 = 0.05;
pub const PAPER_GAMMA_WIDTH: f64 = 0.6;
pub const SEPARATION_DAMPING_REF_SPREAD: f64 = 0.35;

/// Luminance row of Adobe RGB (1998), the working space.
pub const LUMA_R: f64 = 0.2974;
pub const LUMA_G: f64 = 0.6273;
pub const LUMA_B: f64 = 0.0753;
