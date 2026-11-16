//! The crop and meter boxes a render resolves once, for the controller to freeze.
//!
//! Mirrors `_resolve_armed_autocrop` and `_resolve_meter_rect` in
//! `negpy/services/rendering/image_processor.py`. It runs before either engine, so a preview
//! and a full-resolution export crop the same frame at the same normalized rect.

use crate::domain::models::{ProcessMode, WorkspaceConfig};
use crate::features::geometry::logic::{
    autocrop_detection_key, crops_frame, meter_detection_key, squares_on_arm,
};
use crate::features::geometry::processor::{resolve_autocrop_rect, resolve_meter_rect};
use crate::features::geometry::skew::squared_geometry;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The detections a render keeps while the freeze has not landed yet.
const METER_MEMO_SIZE: usize = 64;

/// What an armed Auto Crop resolved, for the controller to freeze.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmedCrop {
    /// `None` when no crop was found.
    pub rect: Option<[f64; 4]>,
    /// The `autocrop_detection_key` of the geometry the rect was found on.
    pub key: String,
    /// Fine Rotation, Tilt and Swing squared to; `None` when not measured.
    pub skew: Option<[f64; 3]>,
    /// Whether Square Before Crop ran for this arm.
    pub squared: bool,
}

/// Turn an armed Auto Crop into a rect, once, before either engine runs.
///
/// Returns this render's settings (marked squared when Square Before Crop ran) and what to
/// freeze, or `None`.
pub fn resolve_armed_autocrop(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    preview_size: f64,
) -> (WorkspaceConfig, Option<ArmedCrop>) {
    resolve_armed_autocrop_with(
        image,
        settings,
        preview_size,
        |image, geometry, mode| {
            squared_geometry(image, geometry, mode).map(|(squared, skew)| {
                (
                    squared,
                    [
                        skew.fine_rotation,
                        skew.converge_v.unwrap_or(0.0),
                        skew.converge_h.unwrap_or(0.0),
                    ],
                )
            })
        },
        |image, geometry, size| resolve_autocrop_rect(image, geometry, size, false),
    )
}

/// The resolver over injected fit and detection functions, so the arm states are testable.
pub fn resolve_armed_autocrop_with(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    preview_size: f64,
    fit: impl Fn(
        &ImageBuffer,
        &crate::domain::models::GeometryConfig,
        ProcessMode,
    ) -> Option<(crate::domain::models::GeometryConfig, [f64; 3])>,
    detect: impl Fn(&ImageBuffer, &crate::domain::models::GeometryConfig, f64) -> Option<[f64; 4]>,
) -> (WorkspaceConfig, Option<ArmedCrop>) {
    let mut geometry = settings.geometry.clone();
    if !geometry.crop_from_auto {
        return (settings.clone(), None);
    }
    let mut key = autocrop_detection_key(&geometry);
    if geometry.crop_rect.is_some() && geometry.crop_detect_key == key {
        return (settings.clone(), None);
    }
    let mut skew = None;
    let tried = squares_on_arm(&geometry);
    if tried {
        if let Some((fitted, measured)) = fit(image, &geometry, settings.process.process_mode) {
            geometry = fitted;
            skew = Some(measured);
            key = autocrop_detection_key(&geometry);
        }
        // Marked here too, so a later resolver call on these settings does not fit again.
        geometry.autocrop_squared = true;
    }
    let rect = detect(image, &geometry, preview_size);
    if rect.is_none() && !tried {
        return (settings.clone(), None);
    }
    let mut config = settings.clone();
    config.geometry = geometry;
    if let Some(rect) = rect {
        config.geometry.crop_rect = Some(rect.to_vec());
        config.geometry.crop_detect_key = key.clone();
    }
    (
        config,
        Some(ArmedCrop {
            rect,
            key,
            skew,
            squared: tried,
        }),
    )
}

/// The meter rect a render resolved, for the controller to freeze.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterBox {
    /// `None` when the film fills the frame.
    pub rect: Option<[f64; 4]>,
    /// The `meter_detection_key` it was found under.
    pub key: String,
    /// The `autocrop_detection_key` of that geometry, which the controller can check.
    pub geometry_key: String,
}

/// Find the picture box once on a frame with no crop, so its meters skip a holder or bare
/// light. Bounds measured without the new box are dropped, so this render meters again.
///
/// `memo` holds detections by `(source_key, key)` for renders that arrive before the freeze,
/// which would otherwise each detect again.
pub fn resolve_meter_rect_for(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    preview_size: f64,
    memo: Option<&mut std::collections::HashMap<(String, String), Option<[f64; 4]>>>,
    source_key: &str,
) -> (WorkspaceConfig, Option<MeterBox>) {
    resolve_meter_rect_with(
        image,
        settings,
        preview_size,
        memo,
        source_key,
        |image, geometry, size| resolve_meter_rect(image, geometry, size),
    )
}

/// The meter resolver over an injected detection function.
pub fn resolve_meter_rect_with(
    image: &ImageBuffer,
    settings: &WorkspaceConfig,
    preview_size: f64,
    memo: Option<&mut std::collections::HashMap<(String, String), Option<[f64; 4]>>>,
    source_key: &str,
    detect: impl FnOnce(&ImageBuffer, &crate::domain::models::GeometryConfig, f64) -> Option<[f64; 4]>,
) -> (WorkspaceConfig, Option<MeterBox>) {
    let geometry = &settings.geometry;
    if crops_frame(geometry) || geometry.crop_from_auto || settings.process.analysis_rect.is_some()
    {
        return (settings.clone(), None);
    }
    let key = meter_detection_key(geometry, image.width, image.height);
    if settings.process.meter_key == key {
        return (settings.clone(), None);
    }
    let memo_key = (source_key.to_string(), key.clone());
    let cached = memo.as_ref().and_then(|memo| memo.get(&memo_key).copied());
    let rect = match cached {
        Some(rect) => rect,
        None => {
            let rect = detect(image, geometry, preview_size);
            if let Some(memo) = memo {
                if memo.len() >= METER_MEMO_SIZE {
                    if let Some(oldest) = memo.keys().next().cloned() {
                        memo.remove(&oldest);
                    }
                }
                memo.insert(memo_key, rect);
            }
            rect
        }
    };
    let previous = settings
        .process
        .meter_rect
        .as_ref()
        .and_then(|rect| <[f64; 4]>::try_from(rect.as_slice()).ok());
    let mut config = settings.clone();
    config.process.meter_rect = rect.map(Vec::from);
    config.process.meter_key = key.clone();
    if rect != previous {
        let (invalidate, floors, ceils) =
            crate::domain::models::process::invalidate_local_bounds(&config.process);
        if invalidate {
            config.process.local_floors = floors;
            config.process.local_ceils = ceils;
        }
    }
    let geometry_key = autocrop_detection_key(&config.geometry);
    (
        config,
        Some(MeterBox {
            rect,
            key,
            geometry_key,
        }),
    )
}
