//! Declared resolution from EXIF IFD0 and the label rationals.

use negpy_backend::features::metadata::exif_read::{ExifTags, ExifValue};
use negpy_backend::features::metadata::resolution::{
    CENTIMETRE, INCH, Resolution, TAG_RESOLUTION_UNIT, TAG_X_RESOLUTION, TAG_Y_RESOLUTION,
    from_exif, to_rational,
};

fn tags(x: Option<(i64, i64)>, y: Option<(i64, i64)>, unit: u64) -> ExifTags {
    let mut tags = ExifTags::new();
    if let Some((num, den)) = x {
        tags.insert(TAG_X_RESOLUTION, ExifValue::Rational(num, den));
    }
    if let Some((num, den)) = y {
        tags.insert(TAG_Y_RESOLUTION, ExifValue::Rational(num, den));
    }
    tags.insert(TAG_RESOLUTION_UNIT, ExifValue::UInt(unit));
    tags
}

#[test]
fn exif_keeps_the_native_unit_and_axes() {
    assert_eq!(
        from_exif(&tags(Some((3600, 1)), Some((3600, 1)), u64::from(INCH))),
        Some(Resolution {
            x: (3600, 1),
            y: (3600, 1),
            unit: INCH
        })
    );
    assert_eq!(
        from_exif(&tags(
            Some((1417, 1)),
            Some((1417, 1)),
            u64::from(CENTIMETRE)
        )),
        Some(Resolution {
            x: (1417, 1),
            y: (1417, 1),
            unit: CENTIMETRE
        })
    );
    assert_eq!(
        from_exif(&tags(Some((600, 1)), Some((300, 1)), u64::from(INCH))),
        Some(Resolution {
            x: (600, 1),
            y: (300, 1),
            unit: INCH
        })
    );
}

#[test]
fn a_missing_y_axis_uses_x() {
    assert_eq!(
        from_exif(&tags(Some((600, 1)), None, u64::from(INCH))),
        Some(Resolution {
            x: (600, 1),
            y: (600, 1),
            unit: INCH
        })
    );
}

#[test]
fn no_absolute_unit_is_not_a_resolution() {
    assert_eq!(from_exif(&tags(Some((3600, 1)), None, 1)), None);
    assert_eq!(
        from_exif(&tags(None, Some((300, 1)), u64::from(INCH))),
        None
    );
}

#[test]
fn dpi_converts_centimetres_and_rounds_the_single_value() {
    let resolution = Resolution {
        x: (100, 1),
        y: (50, 1),
        unit: CENTIMETRE,
    };
    assert!((resolution.x_dpi() - 254.0).abs() < 1e-9);
    assert!((resolution.y_dpi() - 127.0).abs() < 1e-9);
    assert_eq!(resolution.dpi(), 254);
    let inches = Resolution::from_dpi(300.0);
    assert_eq!(
        inches,
        Resolution {
            x: (300, 1),
            y: (300, 1),
            unit: INCH
        }
    );
}

#[test]
fn a_fractional_dpi_keeps_a_small_denominator() {
    assert_eq!(to_rational(300.0), (300, 1));
    let (num, den) = to_rational(72.5);
    assert_eq!((num, den), (145, 2));
}

#[test]
fn a_zero_denominator_does_not_divide() {
    let resolution = Resolution {
        x: (600, 0),
        y: (600, 1),
        unit: INCH,
    };
    assert_eq!(resolution.x_dpi(), 0.0);
    assert_eq!(resolution.dpi(), 1);
}
