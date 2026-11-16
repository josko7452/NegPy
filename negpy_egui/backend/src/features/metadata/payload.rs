//! Mirrors `negpy/features/metadata/payload.py`: resolve a `MetadataConfig`, the gear
//! library and the source EXIF into export-ready metadata.

use crate::domain::models::metadata::DESCRIPTION_FIELD_ORDER;
use crate::domain::models::{MetadataConfig, normalize_description_fields};
use crate::features::metadata::capture::{
    CaptureDate, format_coords, format_dev_time, format_temperature, parse_capture_date,
    place_summary,
};
use crate::features::metadata::exif_read::{
    ScanExif, SourceExif, extract_scan_from_exif, source_xmp_rating,
};
use crate::features::metadata::gear::GearLibrary;

/// The software every NegPy file names.
pub const NEGPY_SOFTWARE: &str = "NegPy";

/// The push or pull a stored integer means.
#[must_use]
pub fn push_pull_label(value: i64) -> &'static str {
    match value {
        3 => "Push +3",
        2 => "Push +2",
        1 => "Push +1",
        -1 => "Pull -1",
        -2 => "Pull -2",
        -3 => "Pull -3",
        _ => "Normal",
    }
}

/// Which standard EXIF fields to write from film capture, rather than leave the scan's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExifWriteFlags {
    pub camera: bool,
    pub lens: bool,
    pub film_iso: bool,
    pub exposure: bool,
}

impl ExifWriteFlags {
    /// Whether any capture field replaces the source scan's.
    #[must_use]
    pub fn strip_scan_residuals(&self) -> bool {
        self.camera || self.lens || self.film_iso || self.exposure
    }
}

/// True when the user provided gear or exposure data that should replace scan EXIF.
#[must_use]
pub fn has_capture_gear(config: &MetadataConfig) -> bool {
    !config.camera_id.is_empty()
        || !config.lens_id.is_empty()
        || !config.film_stock_id.is_empty()
        || !config.camera_make.is_empty()
        || !config.camera_model.is_empty()
        || !config.lens_make.is_empty()
        || !config.lens_model.is_empty()
        || !config.film.is_empty()
        || config.film_iso.is_some()
        || config.focal_length_mm.is_some()
        || config.max_aperture.is_some()
        || !config.exposure_override.trim().is_empty()
}

/// Decide which standard EXIF tags to overwrite with film capture data.
#[must_use]
pub fn compute_exif_write_flags(
    config: &MetadataConfig,
    payload: &MetadataPayload,
) -> ExifWriteFlags {
    if !has_capture_gear(config) {
        return ExifWriteFlags::default();
    }
    ExifWriteFlags {
        camera: !payload.camera_make.is_empty()
            || !payload.camera_model.is_empty()
            || !config.camera_id.is_empty(),
        lens: !payload.lens_make.is_empty()
            || !payload.lens_model.is_empty()
            || !config.lens_id.is_empty()
            || payload.focal_length_mm.is_some()
            || payload.max_aperture.is_some(),
        film_iso: payload.iso.is_some(),
        exposure: !payload.capture_exposure.is_empty(),
    }
}

/// Resolved metadata that will be written to exported files.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetadataPayload {
    pub camera_make: String,
    pub camera_model: String,
    pub lens_make: String,
    pub lens_model: String,
    pub focal_length_mm: Option<f64>,
    pub max_aperture: Option<f64>,
    pub iso: Option<i64>,
    pub film_stock: String,
    pub film_manufacturer: String,
    pub film_format: String,
    pub film_color_type: String,
    pub capture_exposure: String,
    pub capture_roll: String,
    pub capture_frame: Option<i64>,
    pub capture_date: Option<CaptureDate>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub location_city: String,
    pub location_state: String,
    pub location_country: String,
    pub scan_camera_make: String,
    pub scan_camera_model: String,
    pub scan_lens_make: String,
    pub scan_lens_model: String,
    pub scan_focal_length_mm: Option<f64>,
    pub scan_aperture: Option<f64>,
    pub scan_iso: Option<i64>,
    pub scan_exposure: String,
    pub scan_method: String,
    pub scan_datetime: String,
    pub scan_gps_latitude: Option<f64>,
    pub scan_gps_longitude: Option<f64>,
    pub image_description: String,
    pub developer: String,
    pub dilution: String,
    pub push_pull: String,
    pub development_time: String,
    pub development_temperature: String,
    pub notes: String,
    pub rating: String,
    pub exif_flags: ExifWriteFlags,
}

impl MetadataPayload {
    /// Developer at its working strength, the way a note reads it.
    #[must_use]
    pub fn developer_display(&self) -> String {
        join_parts(&[&self.developer, &self.dilution])
    }

    /// The camera make and model on one line.
    #[must_use]
    pub fn camera_display(&self) -> String {
        join_parts(&[&self.camera_make, &self.camera_model])
    }

    /// The lens model, or its make when no model is set.
    #[must_use]
    pub fn lens_display(&self) -> String {
        if self.lens_model.is_empty() {
            self.lens_make.clone()
        } else {
            self.lens_model.clone()
        }
    }

    /// The scan camera make and model on one line.
    #[must_use]
    pub fn scan_camera_display(&self) -> String {
        join_parts(&[&self.scan_camera_make, &self.scan_camera_model])
    }

    /// The place names, else the coordinates.
    #[must_use]
    pub fn place_display(&self) -> String {
        place_summary(
            &self.location_city,
            &self.location_state,
            &self.location_country,
            self.gps_latitude,
            self.gps_longitude,
        )
    }

    /// Whether the capture has both coordinates.
    #[must_use]
    pub fn has_coords(&self) -> bool {
        self.gps_latitude.is_some() && self.gps_longitude.is_some()
    }

    /// Grouped preview: original capture, scan rig, process, file.
    #[must_use]
    pub fn to_preview_sections(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut sections = Vec::new();
        let mut capture = Vec::new();
        if let Some(date) = &self.capture_date {
            capture.push(("Date".to_string(), date.text.clone()));
        }
        for (key, value) in [
            ("Camera make", &self.camera_make),
            ("Camera model", &self.camera_model),
            ("Lens make", &self.lens_make),
            ("Lens model", &self.lens_model),
        ] {
            if !value.is_empty() {
                capture.push((key.to_string(), value.clone()));
            }
        }
        if let Some(value) = self.focal_length_mm {
            capture.push(("Focal length".to_string(), format!("{value} mm")));
        }
        if let Some(value) = self.max_aperture {
            capture.push(("Max aperture".to_string(), format!("f/{value}")));
        }
        for (key, value) in [
            ("Exposure", &self.capture_exposure),
            ("Film stock", &self.film_stock),
            ("Film manufacturer", &self.film_manufacturer),
            ("Film format", &self.film_format),
            ("Film type", &self.film_color_type),
        ] {
            if !value.is_empty() {
                capture.push((key.to_string(), value.clone()));
            }
        }
        if let Some(iso) = self.iso {
            capture.push(("Film ISO".to_string(), iso.to_string()));
        }
        let place = self.place_display();
        if !place.is_empty() {
            capture.push(("Place".to_string(), place.clone()));
        }
        if let (Some(lat), Some(lon)) = (self.gps_latitude, self.gps_longitude) {
            let coords = format_coords(lat, lon);
            if coords != place {
                capture.push(("Coordinates".to_string(), coords));
            }
        }
        let has_capture = !capture.is_empty();
        if has_capture {
            sections.push(("Original capture".to_string(), capture));
        }

        let mut scan = Vec::new();
        for (key, value) in [
            ("Camera make", &self.scan_camera_make),
            ("Camera model", &self.scan_camera_model),
            ("Lens make", &self.scan_lens_make),
            ("Lens model", &self.scan_lens_model),
        ] {
            if !value.is_empty() {
                scan.push((key.to_string(), value.clone()));
            }
        }
        if let Some(value) = self.scan_focal_length_mm {
            scan.push(("Focal length".to_string(), format!("{value} mm")));
        }
        if let Some(value) = self.scan_aperture {
            scan.push(("Aperture".to_string(), format!("f/{value}")));
        }
        for (key, value) in [
            ("Exposure", &self.scan_exposure),
            ("Scan method", &self.scan_method),
            ("Scan date", &self.scan_datetime),
            ("Roll", &self.capture_roll),
        ] {
            if !value.is_empty() {
                scan.push((key.to_string(), value.clone()));
            }
        }
        if let Some(iso) = self.scan_iso {
            scan.push(("ISO".to_string(), iso.to_string()));
        }
        if let (Some(lat), Some(lon)) = (self.scan_gps_latitude, self.scan_gps_longitude) {
            scan.push(("Scan place".to_string(), format_coords(lat, lon)));
        }
        if let Some(frame) = self.capture_frame {
            scan.push(("Frame".to_string(), frame.to_string()));
        }
        if !scan.is_empty() {
            sections.push(("Scan".to_string(), scan));
        }

        let mut process = Vec::new();
        if !self.film_format.is_empty() && !has_capture {
            process.push(("Film format".to_string(), self.film_format.clone()));
        }
        for (key, value) in [
            ("Developer", &self.developer),
            ("Dilution", &self.dilution),
            ("Development time", &self.development_time),
            ("Development temperature", &self.development_temperature),
        ] {
            if !value.is_empty() {
                process.push((key.to_string(), value.clone()));
            }
        }
        if !self.push_pull.is_empty() && self.push_pull != "Normal" {
            process.push(("Push / pull".to_string(), self.push_pull.clone()));
        }
        if !process.is_empty() {
            sections.push(("Process".to_string(), process));
        }

        let mut summary = Vec::new();
        if !self.image_description.is_empty() {
            summary.push((
                "Image description".to_string(),
                self.image_description.clone(),
            ));
        }
        summary.push(("Software".to_string(), NEGPY_SOFTWARE.to_string()));
        if self.exif_flags.strip_scan_residuals() {
            summary.push((
                "EXIF capture tags".to_string(),
                "Written to standard EXIF".to_string(),
            ));
        } else if !self.scan_camera_make.is_empty() || !self.scan_camera_model.is_empty() {
            summary.push((
                "EXIF camera/lens".to_string(),
                "Source scan device (capture not set)".to_string(),
            ));
        }
        if !summary.is_empty() {
            sections.push(("File".to_string(), summary));
        }
        sections
    }

    /// Every preview row, without the section titles.
    #[must_use]
    pub fn to_preview_pairs(&self) -> Vec<(String, String)> {
        self.to_preview_sections()
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .collect()
    }

    /// Whether any metadata field is set.
    #[must_use]
    pub fn has_any_data(&self) -> bool {
        !self.rating.is_empty() || !self.to_preview_pairs().is_empty()
    }
}

fn join_parts(parts: &[&String]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `_apex_from_f_number`: the aperture as an APEX value.
#[must_use]
pub fn apex_from_f_number(f_number: f64) -> f64 {
    2.0 * f_number.log2()
}

/// Human-readable EXIF ImageDescription from the selected field set.
#[must_use]
pub fn build_image_description(payload: &MetadataPayload, fields: Option<&[String]>) -> String {
    let enabled = normalize_description_fields(fields);
    let on = |key: &str| enabled.iter().any(|field| field == key);
    let mut parts = Vec::new();
    if on("camera") {
        let camera = payload.camera_display();
        if !camera.is_empty() {
            parts.push(camera);
        }
    }
    if on("lens") {
        let lens = payload.lens_display();
        if !lens.is_empty() {
            parts.push(lens);
        }
    }
    if on("film") && !payload.film_stock.is_empty() {
        parts.push(payload.film_stock.clone());
    }
    if on("iso")
        && let Some(iso) = payload.iso
    {
        parts.push(format!("ISO {iso}"));
    }
    if on("format") && !payload.film_format.is_empty() {
        parts.push(payload.film_format.clone());
    }
    if on("developer") && !payload.developer.is_empty() {
        parts.push(payload.developer_display());
    }
    if on("push_pull") && !payload.push_pull.is_empty() && payload.push_pull != "Normal" {
        parts.push(payload.push_pull.clone());
    }
    if on("scanning") && !payload.scan_method.is_empty() {
        parts.push(payload.scan_method.clone());
    }
    if !parts.is_empty() {
        return parts.join(" • ");
    }
    if on("film") {
        return payload.film_stock.clone();
    }
    String::new()
}

/// `resolve_description_fields`: per-frame fields if set, else the gear-only defaults.
#[must_use]
pub fn resolve_description_fields(fields: Option<&[String]>) -> Vec<String> {
    match fields {
        Some(fields) => normalize_description_fields(Some(fields)),
        None => DESCRIPTION_FIELD_ORDER
            .iter()
            .filter(|key| ["camera", "lens", "film", "iso"].contains(key))
            .map(|key| (*key).to_string())
            .collect(),
    }
}

/// Resolve gear ids, config fields and source scan EXIF into an export payload.
#[must_use]
pub fn build_metadata_payload(
    config: &MetadataConfig,
    gear: Option<&GearLibrary>,
    source_exif: Option<&SourceExif>,
) -> MetadataPayload {
    let mut camera_make = config.camera_make.clone();
    let mut camera_model = config.camera_model.clone();
    let mut lens_make = config.lens_make.clone();
    let mut lens_model = config.lens_model.clone();
    let mut focal_length = config.focal_length_mm;
    let mut max_aperture = config.max_aperture;
    let mut iso = config.film_iso;
    let mut film_manufacturer = config.film_manufacturer.clone();
    let mut film_format = if config.format == "Other" {
        config.format_other.clone()
    } else {
        config.format.clone()
    };
    let mut film_color_type = config.film_color_type.clone();
    let mut film_stock = config.film.clone();

    if let Some(gear) = gear {
        if !config.camera_id.is_empty()
            && let Some(camera) = gear.camera(&config.camera_id)
        {
            if camera_make.trim().is_empty() {
                camera_make = camera.make.clone();
            }
            if camera_model.trim().is_empty() {
                camera_model = camera.model.clone();
            }
        }
        if !config.lens_id.is_empty()
            && let Some(lens) = gear.lens(&config.lens_id)
        {
            if lens_make.trim().is_empty() {
                lens_make = lens.make.clone();
            }
            if lens_model.trim().is_empty() {
                lens_model = if lens.lens_model.trim().is_empty() {
                    lens.resolved_display_name()
                } else {
                    lens.lens_model.clone()
                };
            }
            if focal_length.is_none() {
                focal_length = lens.focal_length_mm;
            }
            if max_aperture.is_none() {
                max_aperture = lens.max_aperture;
            }
        }
        if !config.film_stock_id.is_empty()
            && let Some(stock) = gear.film_stock(&config.film_stock_id)
        {
            if film_stock.trim().is_empty() {
                film_stock = stock.full_film_label();
            }
            if film_manufacturer.trim().is_empty() {
                film_manufacturer = stock.manufacturer.clone();
            }
            if iso.is_none() {
                iso = Some(stock.iso);
            }
            if film_format.trim().is_empty() {
                film_format = stock.format.clone();
            }
            if film_color_type.trim().is_empty() {
                film_color_type = stock.color_type.clone();
            }
        }
    }

    let scan: ScanExif = extract_scan_from_exif(source_exif);
    let capture_date = parse_capture_date(&config.capture_date);
    let mut draft = MetadataPayload {
        camera_make: camera_make.trim().to_string(),
        camera_model: camera_model.trim().to_string(),
        lens_make: lens_make.trim().to_string(),
        lens_model: lens_model.trim().to_string(),
        focal_length_mm: focal_length,
        max_aperture,
        iso,
        film_stock: film_stock.trim().to_string(),
        film_manufacturer: film_manufacturer.trim().to_string(),
        film_format: film_format.trim().to_string(),
        film_color_type: film_color_type.trim().to_string(),
        capture_exposure: config.exposure_override.trim().to_string(),
        capture_roll: config.capture_roll.trim().to_string(),
        capture_frame: config.capture_frame,
        capture_date,
        gps_latitude: config.gps_latitude,
        gps_longitude: config.gps_longitude,
        location_city: config.location_city.trim().to_string(),
        location_state: config.location_state.trim().to_string(),
        location_country: config.location_country.trim().to_string(),
        scan_camera_make: scan.camera_make,
        scan_camera_model: scan.camera_model,
        scan_lens_make: scan.lens_make,
        scan_lens_model: scan.lens_model,
        scan_focal_length_mm: scan.focal_length_mm,
        scan_aperture: scan.aperture,
        scan_iso: scan.iso,
        scan_exposure: scan.exposure,
        scan_method: config.scanning.trim().to_string(),
        scan_datetime: scan.datetime_original,
        scan_gps_latitude: scan.gps_latitude,
        scan_gps_longitude: scan.gps_longitude,
        developer: config.developer.trim().to_string(),
        dilution: config.process_dilution.trim().to_string(),
        push_pull: push_pull_label(config.push_pull).to_string(),
        development_time: format_dev_time(config.process_time_seconds),
        development_temperature: config
            .process_temperature_c
            .map_or_else(String::new, |value| {
                format!("{} °C", format_temperature(Some(value)))
            }),
        rating: source_xmp_rating(source_exif),
        ..MetadataPayload::default()
    };
    let fields = resolve_description_fields(config.description_fields.as_deref());
    let mut desc = build_image_description(&draft, Some(&fields));
    if desc.is_empty() && !config.film.is_empty() && fields.iter().any(|field| field == "film") {
        desc = config.film.trim().to_string();
    }
    draft.image_description = desc;
    let exif_flags = compute_exif_write_flags(config, &draft);
    draft.exif_flags = exif_flags;
    draft
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata::gear::{Camera, FilmStock, GearLibrary};

    fn library() -> GearLibrary {
        GearLibrary {
            cameras: vec![Camera {
                id: "pen".into(),
                make: "Olympus".into(),
                model: "PEN F".into(),
                display_name: "PEN F".into(),
                ..Camera::default()
            }],
            film_stocks: vec![FilmStock {
                id: "hp5".into(),
                manufacturer: "Ilford".into(),
                stock_name: "HP5 Plus".into(),
                iso: 400,
                format: "35mm".into(),
                color_type: "B&W Negative".into(),
                ..FilmStock::default()
            }],
            ..GearLibrary::default()
        }
    }

    #[test]
    fn gear_ids_resolve_into_the_payload() {
        let mut config = MetadataConfig::default();
        config.camera_id = "pen".into();
        config.film_stock_id = "hp5".into();
        let payload = build_metadata_payload(&config, Some(&library()), None);
        assert_eq!(payload.camera_make, "Olympus");
        assert_eq!(payload.camera_model, "PEN F");
        assert_eq!(payload.film_stock, "Ilford HP5 Plus");
        assert_eq!(payload.iso, Some(400));
        assert_eq!(payload.film_format, "35mm");
        assert_eq!(payload.film_color_type, "B&W Negative");
    }

    #[test]
    fn the_description_joins_the_default_fields() {
        let mut config = MetadataConfig::default();
        config.camera_make = "Olympus".into();
        config.camera_model = "PEN F".into();
        config.film = "HP5 Plus".into();
        config.film_iso = Some(400);
        let payload = build_metadata_payload(&config, None, None);
        assert_eq!(
            payload.image_description,
            "Olympus PEN F • HP5 Plus • ISO 400"
        );
    }

    #[test]
    fn a_positive_film_name_falls_back_to_the_description() {
        let mut config = MetadataConfig::default();
        config.film = "Portra 400".into();
        config.description_fields = Some(vec!["film".into()]);
        let payload = build_metadata_payload(&config, None, None);
        assert_eq!(payload.image_description, "Portra 400");
    }

    #[test]
    fn the_scan_record_is_carried_into_the_payload() {
        use crate::features::metadata::exif_read::{ExifTags, ExifValue, SourceExif};

        let mut zeroth = ExifTags::new();
        zeroth.insert(271, ExifValue::Text("Plustek".into()));
        zeroth.insert(272, ExifValue::Text("OpticFilm".into()));
        let mut exif = ExifTags::new();
        exif.insert(34855, ExifValue::UInt(100));
        let source = SourceExif {
            zeroth,
            exif,
            gps: ExifTags::new(),
        };
        let payload = build_metadata_payload(&MetadataConfig::default(), None, Some(&source));
        assert_eq!(payload.scan_camera_make, "Plustek");
        assert_eq!(payload.scan_camera_model, "OpticFilm");
        assert_eq!(payload.scan_iso, Some(100));
    }

    #[test]
    fn the_push_pull_label_is_normal_by_default() {
        let payload = build_metadata_payload(&MetadataConfig::default(), None, None);
        assert_eq!(payload.push_pull, "Normal");
    }

    #[test]
    fn the_preview_groups_capture_and_file() {
        let mut config = MetadataConfig::default();
        config.camera_make = "Olympus".into();
        config.camera_model = "PEN F".into();
        config.film = "HP5 Plus".into();
        let payload = build_metadata_payload(&config, None, None);
        let sections = payload.to_preview_sections();
        let titles: Vec<&str> = sections.iter().map(|(title, _)| title.as_str()).collect();
        assert!(titles.contains(&"Original capture"));
        assert!(titles.contains(&"File"));
        assert!(payload.has_any_data());
    }

    #[test]
    fn the_exif_flags_follow_the_capture_gear() {
        let mut config = MetadataConfig::default();
        config.camera_make = "Olympus".into();
        config.camera_model = "PEN F".into();
        let payload = build_metadata_payload(&config, None, None);
        assert!(payload.exif_flags.camera);
        assert!(!payload.exif_flags.lens);
        assert!(payload.exif_flags.strip_scan_residuals());
    }
}
