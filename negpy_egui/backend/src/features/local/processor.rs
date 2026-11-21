//! The local stage's map accessors for the exposure processor.

use crate::domain::models::LocalAdjustmentsConfig;
use crate::features::exposure::constants as c;
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

    /// The stops channel, contiguous for the print curve.
    #[must_use]
    pub fn stops_plane(&self) -> Vec<f32> {
        self.channel(0)
    }

    /// The grade-delta channel turned into a slope multiplier at `grade`.
    #[must_use]
    pub fn grade_plane(&self, grade: f64) -> Vec<f32> {
        let r0 = grade.clamp(c::ISO_R_MIN, c::ISO_R_MAX);
        self.channel(1)
            .into_iter()
            .map(|delta| {
                let r1 = (r0 + f64::from(delta)).clamp(c::ISO_R_MIN, c::ISO_R_MAX);
                (r0 / r1) as f32
            })
            .collect()
    }

    /// The preflash channel.
    #[must_use]
    pub fn flash_plane(&self) -> Vec<f32> {
        self.channel(2)
    }

    /// The raw grade-delta channel, before the slope-ratio conversion.
    #[must_use]
    pub fn grade_delta_plane(&self) -> Vec<f32> {
        self.channel(1)
    }

    /// The tone-limited alphas, one plane per mask, interleaved by pixel.
    #[must_use]
    pub fn key_alpha(&self) -> Option<(Vec<f32>, usize)> {
        let count = self.plane_count.saturating_sub(3);
        (count > 0).then(|| {
            let alpha = self
                .planes
                .chunks_exact(self.plane_count)
                .flat_map(|pixel| pixel[3..].iter().copied())
                .collect();
            (alpha, count)
        })
    }

    fn channel(&self, index: usize) -> Vec<f32> {
        self.planes
            .chunks_exact(self.plane_count)
            .map(|pixel| pixel[index])
            .collect()
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
