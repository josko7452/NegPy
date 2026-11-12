//! Ports of the profile assertions in `tests/test_paper_profiles.py` and
//! `tests/test_physical_print.py`.

use negpy_backend::domain::models::ProcessMode;
use negpy_backend::features::exposure::papers::{
    DEFAULT_PROFILE_KEY, PAPER_PROFILES, PrintColor, compose_density_matrices, effective_constants,
    effective_paper_profile, profiles_for_mode, resolve_flat_matrix, resolve_paper,
    resolve_print_color, resolve_saturation_matrix, slope_gain,
};

#[test]
fn the_registry_is_well_formed() {
    assert_eq!(PAPER_PROFILES[0].0, DEFAULT_PROFILE_KEY);
    assert_eq!(PAPER_PROFILES[0].1.kind, "default");
    assert!(PAPER_PROFILES.iter().any(|(_, p)| p.kind == "bw"));
    assert!(PAPER_PROFILES.iter().any(|(_, p)| p.kind == "ra4"));
}

#[test]
fn every_profile_leaves_snap_headroom() {
    for (key, profile) in PAPER_PROFILES {
        assert!(
            profile.tonal.paper_midtone_gamma.abs() <= 0.25,
            "{key} has gamma {}",
            profile.tonal.paper_midtone_gamma
        );
    }
}

#[test]
fn the_default_leaves_every_tonal_key_untouched() {
    let built_in = effective_constants(None);
    let default = effective_constants(Some(resolve_paper(DEFAULT_PROFILE_KEY)));
    assert_eq!(default, built_in);
}

#[test]
fn an_unknown_key_falls_back_to_the_default() {
    assert_eq!(
        resolve_paper("does-not-exist"),
        resolve_paper(DEFAULT_PROFILE_KEY)
    );
}

#[test]
fn a_profile_overrides_its_tonal_keys() {
    let classic = PAPER_PROFILES
        .iter()
        .find(|(key, _)| *key == "ilford_fb_classic")
        .map(|(_, profile)| profile)
        .unwrap();
    let constants = effective_constants(Some(classic));
    assert_eq!(constants.d_max, classic.tonal.d_max);
    assert_ne!(constants.d_max, effective_constants(None).d_max);
}

#[test]
fn mode_gating_keeps_only_the_matching_kind() {
    let neutral = resolve_paper(DEFAULT_PROFILE_KEY);
    assert_eq!(
        effective_paper_profile("kodak_endura", Some(ProcessMode::C41)).label,
        "Kodak Endura Premier"
    );
    assert_eq!(
        effective_paper_profile("ilford_mg_rc", Some(ProcessMode::BW)).label,
        "Ilford Multigrade RC"
    );
    for mode in [Some(ProcessMode::BW), Some(ProcessMode::E6), None] {
        assert_eq!(
            effective_paper_profile("kodak_endura", mode).label,
            neutral.label
        );
    }
    assert_eq!(
        effective_paper_profile("ilford_mg_rc", None).label,
        neutral.label
    );
    for mode in [
        ProcessMode::C41,
        ProcessMode::BW,
        ProcessMode::E6,
        ProcessMode::default(),
    ] {
        assert_eq!(
            effective_paper_profile(DEFAULT_PROFILE_KEY, Some(mode)).label,
            neutral.label
        );
    }
}

#[test]
fn a_mode_lists_the_default_then_its_own_papers() {
    let c41 = profiles_for_mode(ProcessMode::C41);
    let bw = profiles_for_mode(ProcessMode::BW);
    let e6 = profiles_for_mode(ProcessMode::E6);
    for list in [&c41, &bw, &e6] {
        assert_eq!(list[0].0, DEFAULT_PROFILE_KEY);
    }
    assert!(c41[1..].iter().all(|(_, profile)| profile.kind == "ra4"));
    assert!(bw[1..].iter().all(|(_, profile)| profile.kind == "bw"));
    assert_eq!(e6.len(), 1);
    assert_eq!(
        c41.len(),
        1 + PAPER_PROFILES
            .iter()
            .filter(|(_, p)| p.kind == "ra4")
            .count()
    );
    assert_eq!(
        bw.len(),
        1 + PAPER_PROFILES
            .iter()
            .filter(|(_, p)| p.kind == "bw")
            .count()
    );
}

#[test]
fn papers_sort_by_medium_then_label() {
    let order = ["digital", "paper", "film"];
    for mode in [ProcessMode::C41, ProcessMode::BW] {
        let listed = &profiles_for_mode(mode)[1..];
        let media: Vec<_> = listed.iter().map(|(_, p)| p.medium).collect();
        let mut sorted = media.clone();
        sorted.sort_by_key(|medium| order.iter().position(|m| m == medium).unwrap_or(3));
        assert_eq!(media, sorted);
        for medium in ["digital", "paper", "film"] {
            let labels: Vec<_> = listed
                .iter()
                .filter(|(_, p)| p.medium == medium)
                .map(|(_, p)| p.label.to_lowercase())
                .collect();
            let mut sorted = labels.clone();
            sorted.sort();
            assert_eq!(labels, sorted);
        }
    }
}

#[test]
fn every_stock_has_a_distinct_short_name() {
    for mode in [ProcessMode::C41, ProcessMode::BW] {
        let shorts: Vec<_> = profiles_for_mode(mode)
            .iter()
            .map(|(_, p)| p.short)
            .collect();
        assert!(shorts.iter().all(|short| !short.is_empty()));
        let mut sorted = shorts.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(shorts.len(), sorted.len());
    }
}

#[test]
fn the_frontier_gain_is_carried() {
    assert_eq!(slope_gain(Some(resolve_paper("frontier"))), 1.2);
    assert_eq!(slope_gain(None), 1.0);
}

#[test]
fn a_matrix_stock_is_row_normalized() {
    let paper = negpy_backend::features::exposure::papers::PaperProfile {
        label: "test",
        short: "test",
        kind: "ra4",
        medium: "paper",
        tonal: effective_constants(None),
        channel_gamma: [1.0; 3],
        slope_gain: 1.0,
        base_tint_cmy: [0.0; 3],
        print_color: PrintColor::Matrix([[2.0, 1.0, 1.0], [0.0, 4.0, 0.0], [0.0, 0.0, 1.0]]),
        lith_path: [[0.0; 2]; 4],
    };
    let (matrix, lut) = resolve_print_color(Some(&paper));
    assert_eq!(lut, None);
    let matrix = matrix.unwrap();
    for row in matrix {
        assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }
    assert_eq!(resolve_flat_matrix(Some(&paper)), Some(matrix));
}

#[test]
fn the_default_and_a_lut_stock_have_no_dye_matrix() {
    assert_eq!(resolve_print_color(None), (None, None));
    assert_eq!(
        resolve_print_color(Some(resolve_paper(DEFAULT_PROFILE_KEY))),
        (None, None)
    );
    assert_eq!(
        resolve_print_color(Some(resolve_paper("kodak_endura"))),
        (None, Some("kodak_e4070"))
    );
}

#[test]
fn saturation_keeps_each_row_summing_to_one() {
    assert_eq!(resolve_saturation_matrix([1.0; 3]), None);
    let matrix = resolve_saturation_matrix([0.5, 1.5, 0.0]).unwrap();
    for row in matrix {
        assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
    assert!((matrix[1][1] - (1.5 - 0.5 / 3.0)).abs() < 1e-12);
    assert!((matrix[2][0] - 1.0 / 3.0).abs() < 1e-12);
}

#[test]
fn composition_applies_saturation_outermost() {
    let dye = [[0.5, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let sat = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
    let composed = compose_density_matrices(Some(dye), Some(sat)).unwrap();
    assert_eq!(composed[0], dye[0]);
    assert_eq!(composed[1], dye[2]);
    assert_eq!(composed[2], dye[1]);
    assert_eq!(compose_density_matrices(None, None), None);
}
