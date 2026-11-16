//! Mirrors `negpy/features/metadata/xmp.py`: the Adobe XMP packet of an export.

use crate::features::metadata::capture::xmp_coord;
use crate::features::metadata::payload::MetadataPayload;

const XMP_BEGIN: &str = "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>";
const XMP_END: &str = "<?xpacket end=\"w\"?>";
const XMP_NS: &str = "http://ns.adobe.com/xap/1.0/";
const NEGPY_NS: &str = "https://negpy.app/ns/1.0/";
const EXIF_NS: &str = "http://ns.adobe.com/exif/1.0/";
const PHOTOSHOP_NS: &str = "http://ns.adobe.com/photoshop/1.0/";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn rational(value: f64) -> String {
    format!("{}/1000", (value * 1000.0).round() as i64)
}

struct Packet {
    body: String,
}

impl Packet {
    fn new() -> Self {
        Self {
            body: String::new(),
        }
    }

    fn element(&mut self, prefix: &str, tag: &str, text: &str) {
        self.body.push_str(&format!(
            "<{prefix}:{tag}>{}</{prefix}:{tag}>\n",
            escape(text)
        ));
    }

    fn finish(self, standalone: bool) -> String {
        let description = format!(
            "<rdf:Description rdf:about=\"\" xmlns:dc=\"{DC_NS}\" xmlns:photoshop=\"{PHOTOSHOP_NS}\" xmlns:exif=\"{EXIF_NS}\" xmlns:xmp=\"{XMP_NS}\" xmlns:negpy=\"{NEGPY_NS}\">\n{}</rdf:Description>",
            self.body
        );
        let body = format!(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n{description}\n</rdf:RDF>\n</x:xmpmeta>"
        );
        if standalone {
            format!("{XMP_BEGIN}\n{body}\n{XMP_END}")
        } else {
            body
        }
    }
}

impl MetadataPayload {
    /// `CaptureDate::xmp_text`: the truncated date with its offset.
    fn capture_date_text(&self) -> Option<String> {
        self.capture_date
            .as_ref()
            .map(|date| format!("{}{}", date.text, date.tz_offset))
    }
}

/// Build a standards-compliant XMP packet string.
#[must_use]
pub fn build_xmp_xml(payload: &MetadataPayload, standalone: bool) -> String {
    let mut packet = Packet::new();
    for (prefix, tag, value) in [
        ("negpy", "CaptureCameraMake", &payload.camera_make),
        ("negpy", "CaptureCameraModel", &payload.camera_model),
        ("negpy", "CaptureLensMake", &payload.lens_make),
        ("negpy", "CaptureLensModel", &payload.lens_model),
    ] {
        if !value.is_empty() {
            packet.element(prefix, tag, value);
        }
    }
    if let Some(value) = payload.focal_length_mm {
        packet.element("negpy", "CaptureFocalLength", &rational(value));
    }
    if let Some(value) = payload.max_aperture {
        packet.element("negpy", "CaptureMaxAperture", &rational(value));
    }
    if !payload.capture_exposure.is_empty() {
        packet.element("negpy", "CaptureExposure", &payload.capture_exposure);
    }
    if let Some(date) = payload.capture_date_text() {
        packet.element("photoshop", "DateCreated", &date);
        if let Some(capture_date) = &payload.capture_date {
            packet.element("negpy", "CaptureDatePrecision", capture_date.precision);
        }
    }
    if let Some(iso) = payload.iso {
        packet.element("negpy", "CaptureFilmISO", &iso.to_string());
    }
    for (tag, value) in [
        ("CaptureFilmStock", &payload.film_stock),
        ("CaptureFilmManufacturer", &payload.film_manufacturer),
        ("CaptureFilmFormat", &payload.film_format),
        ("CaptureFilmColorType", &payload.film_color_type),
        ("Developer", &payload.developer),
        ("DevelopmentDilution", &payload.dilution),
        ("DevelopmentTime", &payload.development_time),
        ("DevelopmentTemperature", &payload.development_temperature),
        ("Notes", &payload.notes),
        ("ScanMethod", &payload.scan_method),
        ("CaptureRoll", &payload.capture_roll),
    ] {
        if !value.is_empty() {
            packet.element("negpy", tag, value);
        }
    }
    if !payload.push_pull.is_empty() && payload.push_pull != "Normal" {
        packet.element("negpy", "PushPull", &payload.push_pull);
    }
    for (tag, value) in [
        ("City", &payload.location_city),
        ("State", &payload.location_state),
        ("Country", &payload.location_country),
    ] {
        if !value.is_empty() {
            packet.element("photoshop", tag, value);
        }
    }
    if let (Some(lat), Some(lon)) = (payload.gps_latitude, payload.gps_longitude) {
        packet.element("exif", "GPSLatitude", &xmp_coord(lat, 'N', 'S'));
        packet.element("exif", "GPSLongitude", &xmp_coord(lon, 'E', 'W'));
        packet.element("exif", "GPSMapDatum", "WGS-84");
    }
    if let Some(frame) = payload.capture_frame {
        packet.element("negpy", "CaptureFrame", &frame.to_string());
    }
    for (tag, value) in [
        ("ScanCameraMake", &payload.scan_camera_make),
        ("ScanCameraModel", &payload.scan_camera_model),
        ("ScanLensMake", &payload.scan_lens_make),
        ("ScanLensModel", &payload.scan_lens_model),
        ("ScanExposure", &payload.scan_exposure),
        ("ScanDateTime", &payload.scan_datetime),
    ] {
        if !value.is_empty() {
            packet.element("negpy", tag, value);
        }
    }
    if let Some(value) = payload.scan_focal_length_mm {
        packet.element("negpy", "ScanFocalLength", &rational(value));
    }
    if let Some(value) = payload.scan_aperture {
        packet.element("negpy", "ScanAperture", &rational(value));
    }
    if let Some(iso) = payload.scan_iso {
        packet.element("negpy", "ScanISO", &iso.to_string());
    }
    if !payload.rating.is_empty() {
        packet.element("xmp", "Rating", &payload.rating);
    }

    let mut keywords: Vec<&String> = Vec::new();
    for value in [
        &payload.film_stock,
        &payload.film_manufacturer,
        &payload.film_format,
        &payload.film_color_type,
    ] {
        if !value.is_empty() && !keywords.iter().any(|known| *known == value) {
            keywords.push(value);
        }
    }
    if !keywords.is_empty() {
        let items: String = keywords
            .iter()
            .map(|value| format!("<rdf:li>{}</rdf:li>", escape(value)))
            .collect();
        packet.body.push_str(&format!(
            "<dc:subject><rdf:Bag>{items}</rdf:Bag></dc:subject>\n"
        ));
    }
    packet.finish(standalone)
}

/// The XMP packet bytes an export embeds.
#[must_use]
pub fn build_xmp_bytes(payload: &MetadataPayload, standalone: bool) -> Vec<u8> {
    build_xmp_xml(payload, standalone).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> MetadataPayload {
        MetadataPayload {
            camera_make: "Olympus".into(),
            camera_model: "PEN F".into(),
            lens_model: "Zuiko 38".into(),
            focal_length_mm: Some(38.0),
            max_aperture: Some(1.8),
            capture_exposure: "1/125s  f/1.8  ISO 400".into(),
            iso: Some(400),
            film_stock: "Ilford HP5 Plus".into(),
            film_manufacturer: "Ilford".into(),
            film_format: "35mm".into(),
            film_color_type: "B&W Negative".into(),
            developer: "ID-11".into(),
            dilution: "1+1".into(),
            push_pull: "Push +1".into(),
            development_time: "13:00".into(),
            development_temperature: "20 °C".into(),
            location_city: "Bratislava".into(),
            location_country: "Slovakia".into(),
            gps_latitude: Some(48.1486),
            gps_longitude: Some(17.1077),
            scan_camera_make: "Plustek".into(),
            scan_camera_model: "OpticFilm 8100".into(),
            scan_iso: Some(100),
            rating: "4".into(),
            ..MetadataPayload::default()
        }
    }

    #[test]
    fn the_packet_carries_the_capture_and_scan_fields() {
        let xml = build_xmp_xml(&payload(), true);
        assert!(xml.starts_with("<?xpacket begin="));
        assert!(xml.ends_with("<?xpacket end=\"w\"?>"));
        assert!(xml.contains("<negpy:CaptureCameraMake>Olympus</negpy:CaptureCameraMake>"));
        assert!(xml.contains("<negpy:CaptureFilmStock>Ilford HP5 Plus</negpy:CaptureFilmStock>"));
        assert!(xml.contains("<negpy:CaptureFocalLength>38000/1000</negpy:CaptureFocalLength>"));
        assert!(xml.contains("<negpy:PushPull>Push +1</negpy:PushPull>"));
        assert!(xml.contains("<negpy:ScanCameraMake>Plustek</negpy:ScanCameraMake>"));
        assert!(xml.contains("<xmp:Rating>4</xmp:Rating>"));
        assert!(xml.contains("<exif:GPSLatitude>48,8.9160N</exif:GPSLatitude>"));
        assert!(xml.contains("<dc:subject><rdf:Bag>"));
    }

    #[test]
    fn a_normal_push_is_not_written() {
        let mut payload = payload();
        payload.push_pull = "Normal".into();
        assert!(!build_xmp_xml(&payload, true).contains("PushPull"));
    }

    #[test]
    fn the_keywords_are_unique() {
        let mut payload = payload();
        payload.film_format = "Ilford".into();
        let xml = build_xmp_xml(&payload, true);
        assert_eq!(xml.matches("<rdf:li>Ilford</rdf:li>").count(), 1);
    }

    #[test]
    fn a_non_standalone_packet_drops_the_wrapper() {
        let xml = build_xmp_xml(&payload(), false);
        assert!(xml.starts_with("<x:xmpmeta"));
        assert!(!xml.contains("<?xpacket"));
    }

    #[test]
    fn markup_in_a_value_is_escaped() {
        let mut payload = payload();
        payload.film_stock = "A & B <1>".into();
        let xml = build_xmp_xml(&payload, false);
        assert!(xml.contains("A &amp; B &lt;1&gt;"));
    }
}
