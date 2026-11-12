//! Spot densitometer read-out in this scan's normalized-log units.

use crate::domain::types::{LUMA_B, LUMA_G, LUMA_R};
use crate::features::exposure::analysis::{working_oetf_decode, zone_of_encoded};
use crate::features::exposure::normalization::LogNegativeBounds;

const PRINT_DENSITY_MAX: f64 = 4.0;
const ROMAN: [&str; 11] = [
    "0", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X",
];
const THIRDS: [&str; 3] = ["", "⅓", "⅔"];

/// Normalized display coordinates and the crop they map into.
pub type ContentRect = (f64, f64, f64, f64);
pub type ActiveRoi = (f64, f64, f64, f64);

/// Normalized display coordinates to pixel coordinates in the normalized-log frame.
#[must_use]
pub fn map_display_to_norm(
    nx: f64,
    ny: f64,
    display_width: usize,
    display_height: usize,
    content_rect: Option<ContentRect>,
    active_roi: Option<ActiveRoi>,
    crop_full: bool,
    norm_width: usize,
    norm_height: usize,
) -> Option<(usize, usize)> {
    let (fx, fy) = (nx * display_width as f64, ny * display_height as f64);
    let (u, v) = if let Some((offset_x, offset_y, width, height)) = content_rect {
        if width <= 0.0
            || height <= 0.0
            || fx < offset_x
            || fx >= offset_x + width
            || fy < offset_y
            || fy >= offset_y + height
        {
            return None;
        }
        ((fx - offset_x) / width, (fy - offset_y) / height)
    } else {
        (nx, ny)
    };
    let (x, y) = if crop_full {
        (u * norm_width as f64, v * norm_height as f64)
    } else if let Some((y1, y2, x1, x2)) = active_roi {
        (x1 + u * (x2 - x1), y1 + v * (y2 - y1))
    } else {
        (u * norm_width as f64, v * norm_height as f64)
    };
    Some((
        (x.clamp(0.0, (norm_width.saturating_sub(1)) as f64)) as usize,
        (y.clamp(0.0, (norm_height.saturating_sub(1)) as f64)) as usize,
    ))
}

/// One probed pixel, in darkroom units.
#[derive(Clone, Debug, PartialEq)]
pub struct DensitometerReading {
    pub dd_rgb: [f64; 3],
    pub val_luma: f64,
    pub print_density: f64,
    pub zone: f64,
    pub val_rgb: Option<[f64; 3]>,
}

/// The densitometer read-out at one normalized-log sample and its displayed color.
#[must_use]
pub fn compute_reading(
    val_rgb: [f64; 3],
    bounds: LogNegativeBounds,
    display_rgb: [f64; 3],
) -> DensitometerReading {
    let floors = bounds.floors;
    let ceils = bounds.ceils;
    let negative_window = ceils.iter().sum::<f64>() > floors.iter().sum::<f64>();
    let thin = if negative_window { ceils } else { floors };
    let dd_rgb: [f64; 3] = std::array::from_fn(|index| {
        thin[index] - floors[index] - val_rgb[index] * (ceils[index] - floors[index])
    });
    let val_luma =
        LUMA_R as f64 * val_rgb[0] + LUMA_G as f64 * val_rgb[1] + LUMA_B as f64 * val_rgb[2];
    let luma = LUMA_R as f64 * display_rgb[0]
        + LUMA_G as f64 * display_rgb[1]
        + LUMA_B as f64 * display_rgb[2];
    let decoded = working_oetf_decode(&[luma as f32])[0] as f64;
    let decoded = decoded.max(10f64.powf(-PRINT_DENSITY_MAX));
    let print_density = (-decoded.log10()).min(PRINT_DENSITY_MAX);
    DensitometerReading {
        dd_rgb,
        val_luma,
        print_density,
        zone: zone_of_encoded(luma as f32) as f64,
        val_rgb: Some(val_rgb),
    }
}

/// Zone in roman numerals with one-third-stop fractions.
#[must_use]
pub fn zone_roman(zone: f64) -> String {
    let thirds = (zone.clamp(0.0, 10.0) * 3.0).round_ties_even() as usize;
    let (base, fraction) = (thirds / 3, thirds % 3);
    if base >= 10 {
        ROMAN[10].to_string()
    } else {
        format!("{}{}", ROMAN[base], THIRDS[fraction])
    }
}

/// The one-line densitometer read-out.
#[must_use]
pub fn format_reading(reading: &DensitometerReading) -> String {
    let [red, green, blue] = reading.dd_rgb;
    format!(
        "ΔD {red:.2}·{green:.2}·{blue:.2} · D {:.2} · {}",
        reading.print_density,
        zone_roman(reading.zone)
    )
}
