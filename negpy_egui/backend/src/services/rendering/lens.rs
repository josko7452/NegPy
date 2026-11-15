//! Mirrors `negpy/services/rendering/lens.py`: the source a decode produces before either
//! engine reads it. A flat field is baked in sensor positions, so it runs before the warp
//! moves the samples.

use crate::domain::models::{LensCorrections, LensMetadata, WorkspaceConfig};
use crate::features::flatfield::logic::{GainEntry, apply_flatfield};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::services::assets::half_frame::{HalfSlice, slice_chain};
use crate::services::rendering::source_identity::metadata_lens_corrections;

/// The source flat-fielded as the render reads it. `gain_slices` are the cuts that took
/// this frame out of the decoded scan. A stitch decode corrects its parts, and an
/// embedded-lens decode bakes the flat field before its warp.
#[must_use]
pub fn flatfield_source(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    entry: Option<&GainEntry>,
    gain_slices: &[HalfSlice],
    orientation: Option<u8>,
) -> ImageBuffer {
    if config.stitch.stitch_enabled || metadata_lens_corrections(config).any() {
        return image.clone();
    }
    let crop = (!gain_slices.is_empty()).then(|| {
        let slices = gain_slices.to_vec();
        move |gain: &ImageBuffer| slice_chain(gain, &slices)
    });
    let crop_ref = crop
        .as_ref()
        .map(|crop| crop as &dyn Fn(&ImageBuffer) -> ImageBuffer);
    apply_flatfield(image, &config.flatfield, entry, orientation, crop_ref)
}

/// Flat-field in sensor positions, then apply the embedded warp when the frame carries one.
#[must_use]
pub fn prepare_lens_source(
    image: &ImageBuffer,
    lens: Option<&LensMetadata>,
    flatfield: &crate::domain::models::FlatFieldConfig,
    entry: Option<&GainEntry>,
    corrections: LensCorrections,
    orientation: u8,
    has_ir: bool,
) -> ImageBuffer {
    let mut image = apply_flatfield(image, flatfield, entry, Some(orientation), None);
    if let Some(lens) = lens {
        if !has_ir {
            image = lens.apply(&image, orientation, corrections);
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: f32) -> ImageBuffer {
        ImageBuffer::from_rgb(20, 12, vec![value; 20 * 12 * 3])
    }

    #[test]
    fn a_flat_field_is_baked_before_the_warp() {
        let image = frame(0.5);
        let mut config = WorkspaceConfig::default();
        config.flatfield.apply = true;
        config.flatfield.profile_id = "profile".to_string();
        let gain = ImageBuffer::from_rgb(20, 12, vec![2.0; 20 * 12 * 3]);
        let entry = GainEntry {
            gain,
            token: "token".to_string(),
            sensor_layout: false,
        };
        let lens = LensMetadata {
            warps: vec![crate::domain::models::LensWarp::Rectilinear {
                coefficients: vec![[1.0, -0.1, 0.0, 0.0, 0.0, 0.0]],
                center: (0.5, 0.5),
            }],
            ..LensMetadata::default()
        };
        let prepared = prepare_lens_source(
            &image,
            Some(&lens),
            &config.flatfield,
            Some(&entry),
            LensCorrections {
                distortion: true,
                ca: false,
            },
            1,
            false,
        );
        assert_eq!((prepared.width, prepared.height), (20, 12));
        assert_ne!(prepared.pixels, image.pixels);
        let flat_only = apply_flatfield(&image, &config.flatfield, Some(&entry), Some(1), None);
        assert_ne!(flat_only.pixels, image.pixels);
    }

    #[test]
    fn an_inactive_flat_field_leaves_the_source() {
        let image = frame(0.4);
        let config = WorkspaceConfig::default();
        assert_eq!(flatfield_source(&image, &config, None, &[], Some(1)), image);
    }
}
