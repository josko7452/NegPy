//! The local stage's map accessors for the exposure processor.

use crate::domain::models::LocalAdjustmentsConfig;
use crate::features::local::logic::{LocalGeometry, compute_local_maps, limited_indices};

/// The maps a frame's exposure pass consumes.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalMaps {
    pub planes: Vec<f32>,
    pub height: usize,
    pub width: usize,
    pub plane_count: usize,
}

impl LocalMaps {
    #[must_use]
    pub fn stops(&self, x: usize, y: usize) -> f32 {
        self.planes[(y * self.width + x) * self.plane_count]
    }

    #[must_use]
    pub fn grade(&self, x: usize, y: usize) -> f32 {
        self.planes[(y * self.width + x) * self.plane_count + 1]
    }

    #[must_use]
    pub fn flash(&self, x: usize, y: usize) -> f32 {
        self.planes[(y * self.width + x) * self.plane_count + 2]
    }

    #[must_use]
    pub fn keyed(&self, k: usize, x: usize, y: usize) -> f32 {
        self.planes[(y * self.width + x) * self.plane_count + 3 + k]
    }
}

/// Build the maps for a frame and its geometry.
#[must_use]
pub fn build(
    config: &LocalAdjustmentsConfig,
    height: usize,
    width: usize,
    orig_shape: (usize, usize),
    geometry: LocalGeometry,
) -> LocalMaps {
    let plane_count = 3 + limited_indices(config).len();
    LocalMaps {
        planes: compute_local_maps(config, height, width, orig_shape, geometry),
        height,
        width,
        plane_count,
    }
}
