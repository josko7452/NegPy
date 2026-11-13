//! Resolve gear library selections into metadata fields.
//!
//! Mirrors `negpy/features/metadata/gear_logic.py`.

use crate::domain::models::MetadataConfig;
use crate::features::metadata::gear::GearLibrary;

/// Build metadata from gear ids. `None` leaves an id unchanged; `Some("")` clears it.
#[must_use]
pub fn metadata_from_gear(
    config: &MetadataConfig,
    library: &GearLibrary,
    camera_id: Option<&str>,
    lens_id: Option<&str>,
    film_stock_id: Option<&str>,
) -> MetadataConfig {
    let camera_id = camera_id.unwrap_or(&config.camera_id).to_string();
    let lens_id = lens_id.unwrap_or(&config.lens_id).to_string();
    let film_stock_id = film_stock_id.unwrap_or(&config.film_stock_id).to_string();

    let mut out = config.clone();
    out.camera_id = camera_id.clone();
    out.lens_id = lens_id.clone();
    out.film_stock_id = film_stock_id.clone();
    out.camera_make.clear();
    out.camera_model.clear();
    out.lens_make.clear();
    out.lens_model.clear();
    out.focal_length_mm = None;
    out.max_aperture = None;
    out.film_manufacturer.clear();
    out.film_iso = None;
    out.film_color_type.clear();

    if let Some(camera) = library.camera(&camera_id) {
        out.camera_make = camera.make.clone();
        out.camera_model = camera.model.clone();
    }
    if let Some(lens) = library.lens(&lens_id) {
        out.lens_make = lens.make.clone();
        out.lens_model = if lens.lens_model.trim().is_empty() {
            lens.resolved_display_name()
        } else {
            lens.lens_model.clone()
        };
        out.focal_length_mm = lens.focal_length_mm;
        out.max_aperture = lens.max_aperture;
    }
    if let Some(stock) = library.film_stock(&film_stock_id) {
        out.film = stock.full_film_label();
        out.film_manufacturer = stock.manufacturer.clone();
        out.film_iso = Some(stock.iso);
        out.format = stock.format.clone();
        out.film_color_type = stock.color_type.clone();
    }
    out
}
