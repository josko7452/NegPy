//! The export metadata plan and its embed step.
//!
//! Mirrors `export_embed_plan` and `embed_metadata` in
//! `negpy/features/metadata/writer.py`: a plan is built before the first encode so a TIFF
//! can carry its Exif IFD, and every container gets its own rewrite.

use std::path::Path;

use crate::domain::models::{ExportFormat, MetadataConfig};
use crate::features::metadata::exif_read::SourceExif;
use crate::features::metadata::gear::GearLibrary;
use crate::features::metadata::payload::{MetadataPayload, build_metadata_payload};
use crate::features::metadata::resolution::Resolution;
use crate::features::metadata::writer::{
    ExifData, attach_exif_ifd, dump_exif, embed_metadata, merge_exif, sanitize_exif,
    with_resolution,
};
use crate::features::metadata::xmp::build_xmp_bytes;
use crate::infrastructure::loaders::exif_block::read_exif_file;

/// The bytes to embed and whether the user comment folds into the image description.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbedPlan {
    pub exif: Vec<u8>,
    pub xmp: Option<Vec<u8>>,
    pub fold_user_comment: bool,
}

impl EmbedPlan {
    /// Build a plan for an encoded buffer without resanitizing the source.
    #[must_use]
    pub fn embed(&self, bytes: Vec<u8>, format: ExportFormat) -> Vec<u8> {
        match format {
            ExportFormat::Tiff => attach_exif_ifd(&bytes, &self.exif),
            ExportFormat::Jpeg => {
                let with_exif =
                    crate::features::metadata::writer::inject_jpeg_exif(&bytes, &self.exif);
                match &self.xmp {
                    Some(xmp) => {
                        crate::features::metadata::writer::inject_jpeg_xmp(&with_exif, xmp)
                    }
                    None => with_exif,
                }
            }
            ExportFormat::Png => crate::features::metadata::writer::rewrite_png_metadata(
                &bytes,
                Some(&self.exif),
                self.xmp.as_deref(),
            ),
            ExportFormat::Webp => crate::features::metadata::writer::rewrite_webp_metadata(
                &bytes,
                Some(&self.exif),
                self.xmp.as_deref(),
            ),
            ExportFormat::Jxl => bytes,
        }
    }
}

/// `_load_source_exif`: a file's own EXIF, or the caller's copy when the file has none.
#[must_use]
pub fn load_source_exif(path: &Path, cached: Option<&SourceExif>) -> Option<ExifData> {
    read_exif_file(path)
        .map(ExifData::from)
        .or_else(|| cached.map(ExifData::from))
}

/// `_source_xmp`: the source file's own XMP packet, when it carries one.
#[must_use]
pub fn source_xmp(path: &Path, source: &ExifData) -> Option<Vec<u8>> {
    let from_file = read_exif_file(path).and_then(|raw| {
        crate::features::metadata::exif_read::xmp_packet_from_exif(Some(&SourceExif {
            zeroth: raw.zeroth,
            exif: raw.exif,
            gps: raw.gps,
        }))
    });
    from_file.or_else(|| {
        crate::features::metadata::exif_read::xmp_packet_from_exif(Some(&SourceExif {
            zeroth: source.zeroth.clone(),
            exif: source.exif.clone(),
            gps: source.gps.clone(),
        }))
    })
}

/// `export_embed_plan`: the bytes to embed at the first encode.
///
/// `None` when a protected source has no EXIF at all. The caller then keeps the output
/// untagged, as the Python path does.
#[must_use]
pub fn export_embed_plan(
    config: &MetadataConfig,
    source_exif: Option<&SourceExif>,
    source_path: &Path,
    gear: Option<&GearLibrary>,
    resolution: Option<&Resolution>,
) -> Option<EmbedPlan> {
    if config.protect_original_metadata {
        let exif = load_source_exif(source_path, source_exif)?;
        if exif.zeroth.is_empty() && exif.exif.is_empty() && exif.gps.is_empty() {
            return None;
        }
        let xmp = source_xmp(source_path, &exif);
        return Some(EmbedPlan {
            exif: dump_exif(&sanitize_exif(&exif)),
            xmp,
            fold_user_comment: false,
        });
    }
    let payload = build_metadata_payload(config, gear, source_exif);
    let merged = merge_exif(source_exif.map(ExifData::from).as_ref(), &payload);
    let xmp = payload
        .has_any_data()
        .then(|| build_xmp_bytes(&payload, true));
    Some(EmbedPlan {
        exif: dump_exif(&sanitize_exif(&with_resolution(merged, resolution))),
        xmp,
        fold_user_comment: true,
    })
}

/// Embed merged metadata after the first encode, for a caller that has no plan.
#[must_use]
pub fn embed_after_encode(
    bytes: &[u8],
    config: &MetadataConfig,
    source_exif: Option<&SourceExif>,
    gear: Option<&GearLibrary>,
    resolution: Option<&Resolution>,
) -> Vec<u8> {
    if config.protect_original_metadata {
        return bytes.to_vec();
    }
    let payload = build_metadata_payload(config, gear, source_exif);
    embed_metadata(bytes, &payload, source_exif, resolution)
}

/// The metadata payload an export plan resolves, for a caller that also needs the fields.
#[must_use]
pub fn payload_for(
    config: &MetadataConfig,
    source_exif: Option<&SourceExif>,
    gear: Option<&GearLibrary>,
) -> MetadataPayload {
    build_metadata_payload(config, gear, source_exif)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata::exif_read::ExifValue;
    use crate::services::export::encoders::{TiffCompression, encode_tiff};

    fn tiff() -> Vec<u8> {
        encode_tiff(
            &vec![10u8; 8 * 6 * 3],
            8,
            6,
            3,
            8,
            TiffCompression::None,
            None,
            None,
        )
        .expect("a baseline TIFF")
    }

    #[test]
    fn a_plan_puts_the_capture_date_into_a_tiff() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("frame.tiff");
        std::fs::write(&source, tiff()).unwrap();
        let plan = export_embed_plan(&MetadataConfig::default(), None, &source, None, None)
            .expect("a plan");
        let bytes = plan.embed(tiff(), ExportFormat::Tiff);
        let raw = crate::infrastructure::loaders::exif_block::parse_exif_block(&bytes)
            .expect("a readable TIFF");
        assert!(!raw.zeroth.is_empty());
    }

    #[test]
    fn a_protected_source_copies_its_own_exif() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("frame.tiff");
        let mut data = ExifData::default();
        data.zeroth.insert(271, ExifValue::Text("Canon".into()));
        std::fs::write(&source, dump_exif(&data)).unwrap();
        let mut config = MetadataConfig::default();
        config.protect_original_metadata = true;
        let plan = export_embed_plan(&config, None, &source, None, None).expect("a plan");
        assert!(!plan.fold_user_comment);
        let raw = crate::infrastructure::loaders::exif_block::parse_exif_block(&plan.exif)
            .expect("a readable block");
        assert_eq!(raw.zeroth.get(&271), Some(&ExifValue::Text("Canon".into())));
    }

    #[test]
    fn a_protected_source_without_exif_has_no_plan() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("plain.bin");
        std::fs::write(&source, b"x").unwrap();
        let mut config = MetadataConfig::default();
        config.protect_original_metadata = true;
        assert!(export_embed_plan(&config, None, &source, None, None).is_none());
    }
}
