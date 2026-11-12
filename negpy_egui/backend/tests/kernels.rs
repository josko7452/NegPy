//! The Layer 1 gate: `negpy-backend` reaches the `negpy_core` kernels with no Python.
//!
//! The visibility audit found every pure kernel `pub` behind its `py/*` wrapper, so these
//! calls need no promotion. A break here means a kernel or a view type went private again.

use negpy_core::color::LabSpace;
use negpy_core::{color, convert, stats};

/// The Adobe RGB (1998) -> XYZ matrix and D65 white of `kernel/image/logic.py`, verbatim.
/// The decimals are longer than f32 holds because they mirror the Python np.float32 source.
#[allow(clippy::excessive_precision)]
const WORKING_TO_XYZ: [[f32; 3]; 3] = [
    [0.5767309, 0.1855540, 0.1881852],
    [0.2973769, 0.6273491, 0.0752741],
    [0.0270343, 0.0706872, 0.9911085],
];
const WORKING_WHITE: [f32; 3] = [0.95047, 1.00000, 1.08883];

#[test]
fn percentiles_read_a_slice() {
    let values = [1.0f32, 2.0, 3.0, 4.0];
    let qs = [50.0, 100.0];
    let got = stats::percentiles(&values, &qs).expect("non-empty input");
    assert_eq!(got, vec![2.5f32, 4.0]);
    assert!(stats::percentiles(&[], &qs).is_none());
}

#[test]
fn rgb_to_lab_fills_the_destination() {
    let space = LabSpace::new(WORKING_TO_XYZ, WORKING_WHITE, 0.008856, 7.787);
    let mut dst = [0f32; 3];
    color::rgb_to_lab(&[0.5, 0.5, 0.5], &mut dst, space);
    // A neutral in equals a neutral out: a* and b* stay near zero. The matrix rows
    // do not sum to the white exactly, so the residue is float rounding, not a hue.
    assert!(dst[1].abs() < 1e-4, "a* {}", dst[1]);
    assert!(dst[2].abs() < 1e-4, "b* {}", dst[2]);
    assert!(dst[0] > 0.0 && dst[0] < 100.0);
}

#[test]
fn to_uint_quantizes_a_slice() {
    let src = [0.0f32, 1.0, 0.5];
    let mut u8_dst = [0u8; 3];
    convert::to_uint(&src, &mut u8_dst);
    assert_eq!(u8_dst, [0, 255, 128]);

    let mut u16_dst = [0u16; 3];
    convert::to_uint(&src, &mut u16_dst);
    assert_eq!(u16_dst, [0, 65535, 32768]);
}
