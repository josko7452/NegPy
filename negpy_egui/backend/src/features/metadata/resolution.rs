//! A file's declared resolution, kept as the rationals and unit it carries.
//!
//! Mirrors `negpy/features/metadata/resolution.py` for EXIF IFD0. A first pass
//! copies the source's numbers rather than a rounded DPI, so Protect original
//! metadata writes back exactly what it read.

use crate::features::metadata::exif_read::{ExifTags, ExifValue};

pub const INCH: u16 = 2;
pub const CENTIMETRE: u16 = 3;
/// A PNG pHYs density, in pixels per metre.
pub const METRE: u16 = 0;
const MAX_DENOMINATOR: u32 = 1000;
pub const TAG_X_RESOLUTION: u16 = 282;
pub const TAG_Y_RESOLUTION: u16 = 283;
pub const TAG_RESOLUTION_UNIT: u16 = 296;

/// X and Y resolution rationals with the TIFF unit they use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub x: (i64, i64),
    pub y: (i64, i64),
    pub unit: u16,
}

impl Resolution {
    /// A resolution from one DPI value, in inches.
    #[must_use]
    pub fn from_dpi(dpi: f64) -> Self {
        let value = to_rational(dpi);
        Self {
            x: value,
            y: value,
            unit: INCH,
        }
    }

    #[must_use]
    pub fn x_dpi(&self) -> f64 {
        in_inches(self.x, self.unit)
    }

    #[must_use]
    pub fn y_dpi(&self) -> f64 {
        in_inches(self.y, self.unit)
    }

    /// The single rounded value some callers need.
    #[must_use]
    pub fn dpi(&self) -> i64 {
        (self.x_dpi().round() as i64).max(1)
    }

    /// This resolution expressed in centimetres, for a writer that needs absolute units.
    #[must_use]
    pub fn to_centimetres(&self) -> Self {
        if self.unit == METRE {
            return Self {
                x: (self.x.0, self.x.1 * 100),
                y: (self.y.0, self.y.1 * 100),
                unit: CENTIMETRE,
            };
        }
        *self
    }
}

fn in_inches(rational: (i64, i64), unit: u16) -> f64 {
    if rational.1 == 0 {
        return 0.0;
    }
    let value = rational.0 as f64 / rational.1 as f64;
    if unit == CENTIMETRE {
        value * 2.54
    } else {
        value
    }
}

/// `Fraction(value).limit_denominator(1000)` for a finite positive value.
#[must_use]
pub fn to_rational(value: f64) -> (i64, i64) {
    if !value.is_finite() {
        return (0, 1);
    }
    if value.fract() == 0.0 {
        return (value as i64, 1);
    }
    let mut best = (1i64, 1i64);
    let mut best_error = f64::INFINITY;
    for denominator in 1..=MAX_DENOMINATOR as i64 {
        let numerator = (value * denominator as f64).round() as i64;
        let error = (numerator as f64 / denominator as f64 - value).abs();
        if error < best_error {
            best = (numerator, denominator);
            best_error = error;
        }
        if error < 1e-12 {
            break;
        }
    }
    let divisor = gcd(best.0.unsigned_abs(), best.1.unsigned_abs()).max(1) as i64;
    (best.0 / divisor, best.1 / divisor)
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn rational(value: Option<&ExifValue>) -> Option<(i64, i64)> {
    match value? {
        ExifValue::Rational(num, den) if *num > 0 && *den > 0 => Some((*num, *den)),
        ExifValue::UInt(value) if *value > 0 => Some((*value as i64, 1)),
        ExifValue::SInt(value) if *value > 0 => Some((*value, 1)),
        _ => None,
    }
}

/// Resolution declared in an EXIF/TIFF IFD0, or None.
///
/// Unit 1 means "no absolute unit"; any unit other than inches or centimetres is
/// treated the same way.
#[must_use]
pub fn from_exif(zeroth: &ExifTags) -> Option<Resolution> {
    let x = rational(zeroth.get(&TAG_X_RESOLUTION))?;
    let y = rational(zeroth.get(&TAG_Y_RESOLUTION)).unwrap_or(x);
    let unit = match zeroth.get(&TAG_RESOLUTION_UNIT)? {
        ExifValue::UInt(value) => *value,
        ExifValue::SInt(value) if *value >= 0 => *value as u64,
        _ => return None,
    };
    if unit != u64::from(INCH) && unit != u64::from(CENTIMETRE) {
        return None;
    }
    Some(Resolution {
        x,
        y,
        unit: unit as u16,
    })
}
