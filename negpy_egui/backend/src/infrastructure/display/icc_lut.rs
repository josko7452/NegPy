//! 3D LUT ICC transform for 16-bit RGB buffers.
//!
//! Mirrors `negpy/infrastructure/display/icc_lut.py`: sample the profile pair on an
//! 8-bit grid, then trilinearly interpolate the table onto the image. The kneaded
//! transform comes from `qcms`; a profile the transform cannot build reads as a miss.

use qcms::{DataType, Intent, Profile, Transform};

use negpy_core::display::{Lut3, apply_lut_f32 as core_lut_f32, apply_lut_u16 as core_lut_u16};

/// The grid edge the Python default builds.
pub const DEFAULT_LUT_SIZE: usize = 33;

/// An N x N x N x 3 float table in 0..1, indexed by its red, green and blue code.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3Data {
    pub size: usize,
    pub table: Vec<f32>,
}

/// `numpy.round`, which ties to even.
fn round_half_even(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if (fraction - 0.5).abs() < f64::EPSILON {
        if (floor as i64) % 2 == 0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        value.round()
    }
}

/// The 8-bit code at each grid step, as `np.linspace(0, 255, size).round()`.
#[must_use]
pub fn grid_codes(size: usize) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }
    if size == 1 {
        return vec![0];
    }
    let step = 255.0 / (size - 1) as f64;
    (0..size)
        .map(|index| round_half_even(index as f64 * step) as u8)
        .collect()
}

/// Build the LUT between two ICC profile byte blobs.
///
/// `intent` is the ICC rendering intent number: 0 perceptual, 1 relative colorimetric,
/// 2 saturation, 3 absolute colorimetric. An unreadable profile or a transform the
/// engine rejects reads as `None`, as the Python caller's fallback does.
#[must_use]
pub fn build_3d_lut(
    source_icc: &[u8],
    dest_icc: &[u8],
    intent: u8,
    size: usize,
) -> Option<Lut3Data> {
    let source = Profile::new_from_slice(source_icc, false)?;
    let dest = Profile::new_from_slice(dest_icc, false)?;
    sample_lut(&source, &dest, intent, size)
}

/// Build the LUT with the built-in sRGB profile as the destination, which is what the
/// Python display path uses when no monitor profile is set.
#[must_use]
pub fn build_3d_lut_to_srgb(source_icc: &[u8], intent: u8, size: usize) -> Option<Lut3Data> {
    let source = Profile::new_from_slice(source_icc, false)?;
    let dest = Profile::new_sRGB();
    sample_lut(&source, &dest, intent, size)
}

fn sample_lut(source: &Profile, dest: &Profile, intent: u8, size: usize) -> Option<Lut3Data> {
    if size < 2 {
        return None;
    }
    let transform = Transform::new(source, dest, DataType::RGB8, intent_from(intent))?;

    let codes = grid_codes(size);
    let mut grid = Vec::with_capacity(size * size * size * 3);
    for red in &codes {
        for green in &codes {
            for blue in &codes {
                grid.extend_from_slice(&[*red, *green, *blue]);
            }
        }
    }
    transform.apply(&mut grid);
    Some(Lut3Data {
        size,
        table: grid.iter().map(|value| f32::from(*value) / 255.0).collect(),
    })
}

fn intent_from(intent: u8) -> Intent {
    match intent {
        0 => Intent::Perceptual,
        2 => Intent::Saturation,
        3 => Intent::AbsoluteColorimetric,
        _ => Intent::RelativeColorimetric,
    }
}

/// Trilinearly interpolate the table onto a uint16 RGB image.
#[must_use]
pub fn apply_lut_u16(img: &[u16], lut: &Lut3Data) -> Option<Vec<u16>> {
    if img.len() % 3 != 0 || lut.table.len() != lut.size * lut.size * lut.size * 3 {
        return None;
    }
    let mut out = vec![0u16; img.len()];
    core_lut_u16(
        img,
        Lut3 {
            table: &lut.table,
            n: lut.size,
        },
        &mut out,
    );
    Some(out)
}

/// Trilinearly interpolate the table onto a float RGB image in 0..1.
#[must_use]
pub fn apply_lut_f32(img: &[f32], lut: &Lut3Data) -> Option<Vec<f32>> {
    if img.len() % 3 != 0 || lut.table.len() != lut.size * lut.size * lut.size * 3 {
        return None;
    }
    let mut out = vec![0.0f32; img.len()];
    core_lut_f32(
        img,
        Lut3 {
            table: &lut.table,
            n: lut.size,
        },
        &mut out,
    );
    Some(out)
}

/// Build the pair's LUT and apply it to a uint16 RGB image.
#[must_use]
pub fn apply_icc_u16_rgb(
    img: &[u16],
    source_icc: &[u8],
    dest_icc: &[u8],
    intent: u8,
    size: usize,
) -> Option<Vec<u16>> {
    let lut = build_3d_lut(source_icc, dest_icc, intent, size)?;
    apply_lut_u16(img, &lut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> Vec<u8> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read(root.join("icc").join(name)).expect("bundled profile")
    }

    #[test]
    fn the_grid_rounds_like_numpy() {
        assert_eq!(grid_codes(3), [0, 128, 255]);
        assert_eq!(grid_codes(17)[1], 16);
        assert_eq!(grid_codes(33)[16], 128);
        assert_eq!(grid_codes(33)[32], 255);
    }

    #[test]
    fn an_identity_pair_builds_a_bounded_cube() {
        let icc = profile("sRGB-v4.icc");
        let lut = build_3d_lut(&icc, &icc, 1, 17).expect("sRGB transform");
        assert_eq!(lut.size, 17);
        assert_eq!(lut.table.len(), 17 * 17 * 17 * 3);
        assert!(lut.table.iter().all(|value| (0.0..=1.0).contains(value)));
        assert!((lut.table[0] - 0.0).abs() < 1e-6);
        let last = lut.table.len() - 1;
        assert!((lut.table[last] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn an_identity_transform_keeps_a_code_within_one_gradient_step() {
        let icc = profile("sRGB-v4.icc");
        let img = [0u16, 0, 0, 65535, 65535, 65535, 32768, 16384, 49152];
        let out = apply_icc_u16_rgb(&img, &icc, &icc, 1, 33).expect("sRGB transform");
        assert_eq!(out.len(), img.len());
        for (before, after) in img.iter().zip(&out) {
            assert!(before.abs_diff(*after) < 600);
        }
    }

    #[test]
    fn a_float_image_passes_through_the_same_cube() {
        let icc = profile("sRGB-v4.icc");
        let lut = build_3d_lut(&icc, &icc, 1, 17).expect("sRGB transform");
        let out = apply_lut_f32(&[0.0, 0.5, 1.0], &lut).expect("three samples");
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.0).abs() < 1e-5);
        assert!((out[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_malformed_profile_is_a_miss() {
        assert!(build_3d_lut(b"not an icc profile", b"not an icc profile", 1, 2).is_none());
    }
}
