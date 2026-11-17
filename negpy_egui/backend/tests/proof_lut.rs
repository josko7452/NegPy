//! Ports the LUT-level proof and gamut cases in `tests/test_gamut_readout.py` and
//! `tests/test_soft_proof_lut.py`.

use negpy_backend::infrastructure::display::color_mgmt::{WORKING_COLOR_SPACE, profile_bytes};
use negpy_backend::infrastructure::display::icc_lut::Lut3Data;
use negpy_backend::infrastructure::display::proof_lut::{
    GAMUT_LUT_SIZE, GAMUT_WARNING_COLOR, ProofCondition, ProofIntent, gamut_lut, soft_proof_lut,
};
use std::path::Path;

fn working() -> Vec<u8> {
    profile_bytes(WORKING_COLOR_SPACE, Path::new("/nonexistent"))
        .expect("working profile")
        .bytes
}

fn space(name: &str) -> Vec<u8> {
    profile_bytes(name, Path::new("/nonexistent"))
        .expect("bundled profile")
        .bytes
}

fn proof(output: &[u8], size: usize) -> Lut3Data {
    soft_proof_lut(
        &ProofCondition {
            output_icc: Some(output.to_vec()),
            ..ProofCondition::default()
        },
        &working(),
        size,
    )
    .expect("proof LUT")
}

#[test]
fn no_output_profile_is_no_gamut_mask() {
    assert!(gamut_lut("Adobe RGB", &working(), None, None, GAMUT_LUT_SIZE).is_none());
}

#[test]
fn a_profile_cannot_fail_its_own_colors() {
    let mask = gamut_lut(
        "Adobe RGB",
        &working(),
        None,
        Some(&working()),
        GAMUT_LUT_SIZE,
    )
    .expect("mask");
    assert_eq!(mask.iter().filter(|flagged| **flagged).count(), 0);
}

#[test]
fn a_wider_destination_clips_nothing() {
    let mask = gamut_lut(
        "Adobe RGB",
        &space("sRGB"),
        None,
        Some(&space("Rec 2020")),
        GAMUT_LUT_SIZE,
    )
    .expect("mask");
    assert_eq!(mask.iter().filter(|flagged| **flagged).count(), 0);
}

#[test]
fn a_narrower_destination_clips_saturated_color_but_not_neutrals() {
    let mask = gamut_lut(
        "Adobe RGB",
        &working(),
        None,
        Some(&space("sRGB")),
        GAMUT_LUT_SIZE,
    )
    .expect("mask");
    let flagged = mask.iter().filter(|flagged| **flagged).count();
    let total = mask.len();
    assert!(flagged as f64 / total as f64 > 0.1);
    let axis = |r: usize, g: usize, b: usize| (r * GAMUT_LUT_SIZE + g) * GAMUT_LUT_SIZE + b;
    assert!(!mask[axis(0, 0, 0)]);
    assert!(!mask[axis(GAMUT_LUT_SIZE - 1, GAMUT_LUT_SIZE - 1, GAMUT_LUT_SIZE - 1)]);
    assert!(mask[axis(0, GAMUT_LUT_SIZE - 1, 0)]);
}

#[test]
fn the_proof_lands_on_the_output_space() {
    let srgb = proof(&space("sRGB"), 17);
    let prophoto = proof(&space("ProPhoto RGB"), 17);
    assert_ne!(srgb.table, prophoto.table);
}

#[test]
fn an_unknown_intent_reads_as_relative() {
    assert_eq!(
        ProofIntent::coerce("nonsense"),
        ProofIntent::RelativeColorimetric
    );
}

#[test]
fn the_gamut_warning_paints_exactly_the_masked_nodes() {
    let plain = proof(&space("sRGB"), 17);
    let condition = ProofCondition {
        output_icc: Some(space("sRGB")),
        gamut_warning: true,
        ..ProofCondition::default()
    };
    let warned = soft_proof_lut(&condition, &working(), 17).expect("warned proof");
    let mask = gamut_lut("Adobe RGB", &working(), None, Some(&space("sRGB")), 17).expect("mask");
    for (index, flagged) in mask.into_iter().enumerate() {
        let expected = if flagged {
            [GAMUT_WARNING_COLOR; 3]
        } else {
            [
                plain.table[index * 3],
                plain.table[index * 3 + 1],
                plain.table[index * 3 + 2],
            ]
        };
        assert_eq!(
            &warned.table[index * 3..index * 3 + 3],
            &expected,
            "node {index}"
        );
    }
}

#[test]
fn a_monitor_profile_changes_the_proof_table() {
    let out = space("sRGB");
    let plain = proof(&out, 17);
    let on_p3 = soft_proof_lut(
        &ProofCondition {
            output_icc: Some(out),
            monitor_icc: Some(space("P3 D65")),
            ..ProofCondition::default()
        },
        &working(),
        17,
    )
    .expect("monitor proof");
    assert_ne!(plain.table, on_p3.table);
}

#[test]
fn an_rgb_profile_is_not_a_print_proof() {
    let condition = ProofCondition::default();
    assert!(
        negpy_backend::infrastructure::display::proof_lut::per_pixel_print_proof(
            &[10, 20, 30],
            &working(),
            &space("sRGB"),
            None,
            &condition,
        )
        .is_none()
    );
}
