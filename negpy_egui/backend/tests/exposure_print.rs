//! Smoke tests for the print wrapper over `negpy_core::print_curve`.

use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::papers::resolve_paper;
use negpy_backend::features::exposure::print::{
    PrintParams, PrintSettings, black_density_rgb, flat_curve_params, paper_dmin_rgb,
    per_channel_dye_separation, per_channel_toe_shoulder, per_channel_widths, render_print,
};

#[test]
fn a_neutral_print_outputs_linear_reflectance() {
    let image: Vec<f32> = (0..8 * 8 * 3)
        .map(|index| 0.05 + 0.9 * index as f32 / (8 * 8 * 3 - 1) as f32)
        .collect();
    let params = PrintParams {
        slope: 3.0,
        pivot: 0.5,
        d_min: 0.0,
        midtone_gamma: c::PAPER_MIDTONE_GAMMA,
    };
    let out = render_print(
        &image,
        8,
        &PrintSettings {
            params,
            ..PrintSettings::default()
        },
        None,
    );
    assert!(out.iter().all(|value| value.is_finite()));
    assert!(out.iter().any(|value| (*value - out[0]).abs() > 1e-6));
}

#[test]
fn grade_and_density_make_a_lighter_print() {
    let image = vec![0.2; 4 * 4 * 3];
    let dark = PrintSettings {
        params: PrintParams::from_grade(110.0, 1.5, 0.0, None, None, None),
        ..PrintSettings::default()
    };
    let light = PrintSettings {
        params: PrintParams::from_grade(110.0, 0.5, 0.0, None, None, None),
        ..PrintSettings::default()
    };
    let dark_mean: f32 = render_print(&image, 4, &dark, None).iter().sum::<f32>();
    let light_mean: f32 = render_print(&image, 4, &light, None).iter().sum::<f32>();
    assert!(light_mean > dark_mean);
}

#[test]
fn the_paper_profile_changes_the_print() {
    let image = vec![0.3; 4 * 4 * 3];
    let settings = PrintSettings::default();
    let neutral = render_print(&image, 4, &settings, None);
    let classic = render_print(
        &image,
        4,
        &settings,
        Some(resolve_paper("ilford_fb_classic")),
    );
    assert_ne!(neutral, classic);
}

#[test]
fn a_paper_tint_tints_its_white_floor() {
    let paper = resolve_paper("fuji_crystal");
    let floor = paper_dmin_rgb(paper.tonal.d_min, Some(paper));
    assert!((floor[0] - paper.tonal.d_min - paper.base_tint_cmy[0]).abs() < 1e-12);
    assert_eq!(paper_dmin_rgb(0.0, Some(paper)), [0.0; 3]);
}

#[test]
fn toe_shoulder_and_width_are_per_layer() {
    let (toe, shoulder) = per_channel_toe_shoulder(0.5, -0.2, [0.6, 0.0, -0.6], [0.0; 3]);
    assert!(
        (toe[0] - 1.0).abs() < 1e-12
            && (toe[1] - 0.5).abs() < 1e-12
            && (toe[2] + 0.1).abs() < 1e-12
    );
    assert_eq!(shoulder, [-0.2; 3]);
    let (toe_width, shoulder_width) = per_channel_widths(2.5, 2.5, [3.0, 0.0, -3.0], [0.0; 3]);
    assert_eq!(toe_width, [5.0, 2.5, 0.1]);
    assert_eq!(shoulder_width, [2.5; 3]);
    assert_eq!(
        per_channel_dye_separation(1.0, [3.0, -1.0, 0.5]),
        [3.0, 0.0, 1.5]
    );
}

#[test]
fn black_density_follows_the_toe_sign() {
    let black = black_density_rgb(2.3, [0.0, -0.5, 0.5], 0.9);
    assert!((black[0] - 2.3).abs() < 1e-12);
    assert!((black[1] - 1.85).abs() < 1e-12);
    assert!((black[2] - 2.3).abs() < 1e-12);
}

#[test]
fn the_flat_master_parameters_are_fixed() {
    assert_eq!(flat_curve_params(), (0.65, 0.10));
}
