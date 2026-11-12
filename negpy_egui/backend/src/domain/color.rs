//! The working color space: Adobe RGB (1998) primaries with D65 white, and its 563/256
//! transfer curve. Mirrors the constants in `negpy/kernel/image/logic.py`.

use negpy_core::color::LabSpace;

/// Adobe RGB (1998) -> XYZ D65.
#[allow(clippy::excessive_precision)]
pub const WORKING_TO_XYZ: [[f32; 3]; 3] = [
    [0.5767309, 0.1855540, 0.1881852],
    [0.2973769, 0.6273491, 0.0752741],
    [0.0270343, 0.0706872, 0.9911085],
];

/// XYZ D65 -> Adobe RGB (1998).
#[allow(clippy::excessive_precision)]
pub const XYZ_TO_WORKING: [[f32; 3]; 3] = [
    [2.0413690, -0.5649464, -0.3446944],
    [-0.9692660, 1.8760108, 0.0415560],
    [0.0134474, -0.1183897, 1.0154096],
];

pub const WORKING_WHITE: [f32; 3] = [0.95047, 1.00000, 1.08883];

const LAB_EPS: f32 = 0.008856;
const LAB_KAPPA: f32 = 7.787;

/// The working-space TRC exponent: 563/256, with no linear segment.
pub const WORKING_GAMMA: f64 = 563.0 / 256.0;
/// `np.float32(1.0 / WORKING_GAMMA)`.
pub const WORKING_INV_GAMMA: f32 = (1.0f64 / WORKING_GAMMA) as f32;

/// The pair of Lab spaces the Lab-stage kernels take: RGB to Lab and back.
#[must_use]
pub fn working_lab() -> (LabSpace, LabSpace) {
    (
        LabSpace::new(WORKING_TO_XYZ, WORKING_WHITE, LAB_EPS, LAB_KAPPA),
        LabSpace::new(XYZ_TO_WORKING, WORKING_WHITE, LAB_EPS, LAB_KAPPA),
    )
}

/// `np.clip(v, 0.0, 1.0)`: a NaN comes through.
#[must_use]
pub fn clip01(value: f32) -> f32 {
    let low = if value.is_nan() || value > 0.0 {
        value
    } else {
        0.0
    };
    if low.is_nan() || low < 1.0 { low } else { 1.0 }
}
