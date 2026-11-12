//! The geometry and crop processors in `negpy/features/geometry/processor.py` order.

use crate::domain::models::GeometryConfig;
use crate::features::geometry::logic::{
    Roi, apply_fine_rotation, apply_geometry_orientation, apply_keystone, apply_margin_to_roi,
    apply_radial_distortion, compute_geometry_crop_rect, crop, get_manual_rect_coords,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The frame inputs the geometry stage reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct GeometryContext {
    pub scale_factor: f64,
}

/// The transforms that coordinate mappers replay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeometryParams {
    pub rotation: i64,
    pub fine_rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub converge_v: f64,
    pub converge_h: f64,
}

/// The geometry output and the downstream coordinate correction.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryResult {
    pub image: ImageBuffer,
    pub active_roi: Option<Roi>,
    pub distortion_k1: f64,
    pub geometry_params: GeometryParams,
}

/// Orientation, fine rotation, distortion, keystone and the active ROI.
#[must_use]
pub fn process(
    img: &ImageBuffer,
    config: &GeometryConfig,
    context: GeometryContext,
) -> GeometryResult {
    let mut image = apply_geometry_orientation(
        img,
        config.rotation,
        config.flip_horizontal,
        config.flip_vertical,
    );
    if config.fine_rotation != 0.0 {
        image = apply_fine_rotation(&image, config.fine_rotation);
    }
    if config.distortion_k1 != 0.0 {
        image = apply_radial_distortion(&image, config.distortion_k1);
    }
    image = apply_keystone(&image, config.converge_v, config.converge_h);

    let active_roi = if let Some(rect) = config.crop_rect.as_deref().filter(|rect| rect.len() == 4)
    {
        Some(get_manual_rect_coords(
            (image.height, image.width),
            rect,
            config.autocrop_offset,
            context.scale_factor,
        ))
    } else if config.crop_to_valid && !config.crop_from_auto {
        let rect = compute_geometry_crop_rect(
            config.fine_rotation,
            config.converge_v,
            config.converge_h,
            image.width,
            image.height,
        );
        Some(get_manual_rect_coords(
            (image.height, image.width),
            &[rect.0, rect.1, rect.2, rect.3],
            config.autocrop_offset,
            context.scale_factor,
        ))
    } else if config.autocrop_offset > 0 {
        let margin = config.autocrop_offset as f64 * context.scale_factor;
        Some(apply_margin_to_roi(
            (0, image.height as i64, 0, image.width as i64),
            image.height,
            image.width,
            margin,
        ))
    } else {
        None
    };

    GeometryResult {
        image,
        active_roi,
        distortion_k1: config.distortion_k1,
        geometry_params: GeometryParams {
            rotation: config.rotation,
            fine_rotation: config.fine_rotation,
            flip_horizontal: config.flip_horizontal,
            flip_vertical: config.flip_vertical,
            converge_v: config.converge_v,
            converge_h: config.converge_h,
        },
    }
}

/// `CropProcessor`: apply the resolved window.
#[must_use]
pub fn crop_image(image: &ImageBuffer, active_roi: Option<Roi>) -> ImageBuffer {
    crop(image, active_roi)
}
