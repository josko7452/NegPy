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

/// Linear level the scan-exposure warning counts as sensor white.
pub const SCAN_CLIP_LEVEL: f32 = 0.99;
/// Grid side the block-median prefilter targets.
pub const ANALYSIS_GRID: usize = 1024;
/// Activity gate block side, density span and voter fraction.
pub const ACTIVITY_BLOCK: usize = 8;
pub const ACTIVITY_GATE_DENSITY: f32 = 0.05;
pub const ACTIVITY_MIN_FRACTION: f64 = 0.05;
/// Per-tail percentile trim of the anchor meter.
pub const ANCHOR_TRIM_CLIP: f64 = 5.0;
/// Safety band and pull toward the metered anchor.
pub const ANCHOR_METER_BAND: f64 = 0.12;
pub const ANCHOR_METER_STRENGTH: f64 = 0.2;
/// Percentile clip of the textural range meter.
pub const TEXTURAL_RANGE_CLIP: f64 = 10.0;
/// Textured-tail percentiles for Shadow Reach and Highlight Hold.
pub const SHADOW_REACH_PERCENTILE: f64 = 99.0;
pub const HIGHLIGHT_HOLD_PERCENTILE: f64 = 2.0;

/// Neutral-axis bands in normalized luma.
pub const NEUTRAL_AXIS_HIGHLIGHT_BAND: (f32, f32) = (0.10, 0.30);
pub const NEUTRAL_AXIS_MID_BAND: (f32, f32) = (0.40, 0.60);
pub const NEUTRAL_AXIS_SHADOW_BAND: (f32, f32) = (0.72, 0.92);
/// Lowest-chroma fraction of each band kept as the near-neutral set.
pub const NEUTRAL_AXIS_CHROMA_QUANTILE: f64 = 0.30;
/// Chroma ceilings: pass 2 and pass 1.
pub const NEUTRAL_AXIS_CHROMA_CAP: f64 = 0.29;
pub const NEUTRAL_AXIS_FIRST_PASS_CAP: f64 = 0.55;
pub const NEUTRAL_AXIS_MIN_PIXELS: usize = 64;
/// Foliage and sky sectors: chroma floor and hue range in degrees.
pub const NEUTRAL_AXIS_FOLIAGE: (f32, f32, f32) = (0.03, -25.0, 80.0);
pub const NEUTRAL_AXIS_SKY: (f32, f32, f32) = (0.06, -150.0, -60.0);
/// Confidence sample-size half-point and the agreement and width roll-offs.
pub const NEUTRAL_AXIS_CONFIDENCE_N0: f64 = 256.0;
pub const NEUTRAL_AXIS_AGREEMENT: (f64, f64) = (0.10, 0.20);
pub const NEUTRAL_AXIS_WIDTH: (f64, f64) = (0.15, 0.30);
/// Clamp on each channel's deviation from green and the curvature ratio.
pub const MIDTONE_CAST_MAX_OFFSET: f64 = 0.2;
pub const NEUTRAL_AXIS_CURV_MAX_RATIO: f64 = 0.45;

/// Luminance row of Adobe RGB (1998), the working space.
pub const LUMA_R: f64 = 0.2974;
pub const LUMA_G: f64 = 0.6273;
pub const LUMA_B: f64 = 0.0753;
