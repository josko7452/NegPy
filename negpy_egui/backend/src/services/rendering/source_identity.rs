//! Mirrors `negpy/services/rendering/source_identity.py`: what a decode reads.

use crate::domain::models::{
    LensCorrections, WorkspaceConfig, hdr_active, hdr_token, is_rgb_triplet, rgbscan_token,
    stitch_active, stitch_token,
};
use crate::features::flatfield::logic::{GainEntry, flatfield_token};
use crate::features::process::logic::{
    effective_highlight_reconstruction, highlight_reconstruction_bakes_wb,
};
use crate::features::process::path::{demosaic_token, effective_linear_raw, positive_decode};

/// The embedded lens corrections a decode applies; a composite registers its unwarped parts.
#[must_use]
pub fn metadata_lens_corrections(config: &WorkspaceConfig) -> LensCorrections {
    if stitch_active(&config.stitch) || hdr_active(&config.hdr) || is_rgb_triplet(&config.rgbscan) {
        return LensCorrections::default();
    }
    LensCorrections {
        distortion: config.geometry.lens_distortion_from_metadata,
        ca: config.geometry.lens_ca_from_metadata,
    }
}

/// The embedded-warp identity a decode folds in, plus the flat field it bakes first.
#[must_use]
pub fn lens_decode_token(
    corrections: LensCorrections,
    flatfield: &crate::domain::models::FlatFieldConfig,
    entry: Option<&GainEntry>,
) -> String {
    if !corrections.any() {
        return String::new();
    }
    format!(
        "|embedded-lens-v3-d{}-ca{}{}",
        u8::from(corrections.distortion),
        u8::from(corrections.ca),
        flatfield_token(flatfield, entry)
    )
}

/// What the decode reads, folded into one string.
#[must_use]
pub fn source_token(config: &WorkspaceConfig, flatfield: Option<&GainEntry>) -> String {
    let mut parts = vec![
        format!("|lr{}", u8::from(effective_linear_raw(&config.process))),
        format!("|hr{}", effective_highlight_reconstruction(&config.process)),
        format!(
            "|hrwb{}",
            u8::from(highlight_reconstruction_bakes_wb(&config.process))
        ),
        demosaic_token(config.process.demosaic_preview.as_str()),
        rgbscan_token(&config.rgbscan),
        stitch_token(&config.stitch),
        hdr_token(&config.hdr),
        lens_decode_token(
            metadata_lens_corrections(config),
            &config.flatfield,
            flatfield,
        ),
    ];
    if let Some(decode) = positive_decode(&config.process) {
        parts.push(format!("|pd{}", decode.as_str()));
    }
    if config.stitch.stitch_enabled {
        parts.push(flatfield_token(&config.flatfield, flatfield));
    }
    parts.concat()
}
