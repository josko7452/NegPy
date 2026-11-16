//! Smoke tests for the print wrapper over `negpy_core::print_curve`.

use negpy_backend::domain::models::{ExposureConfig, ProcessConfig, ProcessMode};
use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::papers::resolve_paper;
use negpy_backend::features::exposure::print::{
    FlatRenderSettings, PrintParams, PrintSettings, black_density_rgb, flat_curve_params,
    paper_dmin_rgb, per_channel_dye_separation, per_channel_toe_shoulder, per_channel_widths,
    render_flat, render_print,
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
fn dye_separation_changes_a_colored_print() {
    let image: Vec<f32> = (0..8 * 8)
        .flat_map(|index| {
            let value = 0.05 + 0.8 * index as f32 / 63.0;
            [value, value * 0.8, value * 0.6]
        })
        .collect();
    let neutral = PrintSettings::default();
    let separated = PrintSettings {
        dye_separation: [1.4, 0.8, 1.2],
        ..PrintSettings::default()
    };
    assert_ne!(
        render_print(&image, 8, &neutral, None),
        render_print(&image, 8, &separated, None)
    );
}

#[test]
fn separation_damping_uses_the_per_pixel_chroma_path() {
    let image: Vec<f32> = (0..8 * 8)
        .flat_map(|index| {
            let value = 0.05 + 0.8 * index as f32 / 63.0;
            [value, value * 0.8, value * 0.6]
        })
        .collect();
    let matrix = PrintSettings {
        dye_separation: [1.4, 0.8, 1.2],
        ..PrintSettings::default()
    };
    let damped = PrintSettings {
        separation_damping: 0.8,
        ..matrix
    };
    assert_ne!(
        render_print(&image, 8, &matrix, None),
        render_print(&image, 8, &damped, None)
    );
}

#[test]
fn the_flat_master_parameters_are_fixed() {
    assert_eq!(flat_curve_params(), (0.65, 0.10));
}

#[test]
fn the_flat_master_emits_the_log_code_value() {
    let image: Vec<f32> = (0..32)
        .map(|index| 0.05 + 0.9 * index as f32 / 31.0)
        .collect();
    let image: Vec<f32> = image.iter().flat_map(|value| [*value; 3]).collect();
    let out = render_flat(
        &image,
        &FlatRenderSettings {
            exposure: &ExposureConfig::default(),
            process: &ProcessConfig::default(),
            mode: ProcessMode::C41,
            bounds: None,
        },
    );
    let (gain, lift) = flat_curve_params();
    for (got, value) in out.iter().zip(&image) {
        let expected = (lift as f32 + gain as f32 * (1.0 - value)).clamp(0.0, 1.0);
        assert!((got - expected).abs() < 1e-6, "{got} != {expected}");
    }
}

#[test]
fn a_flat_master_ignores_the_print_decisions() {
    let image = vec![0.35; 16 * 16 * 3];
    let plain = render_flat(
        &image,
        &FlatRenderSettings {
            exposure: &ExposureConfig::default(),
            process: &ProcessConfig::default(),
            mode: ProcessMode::C41,
            bounds: None,
        },
    );
    let loud = ExposureConfig {
        auto_exposure: true,
        auto_normalize_contrast: true,
        cast_removal_strength: 1.0,
        grade: 50.0,
        density: 1.8,
        toe: 0.9,
        shoulder: 0.9,
        paper_dmin: true,
        ..ExposureConfig::default()
    };
    let varied = render_flat(
        &image,
        &FlatRenderSettings {
            exposure: &loud,
            process: &ProcessConfig::default(),
            mode: ProcessMode::C41,
            bounds: None,
        },
    );
    assert_eq!(plain, varied);
}

#[test]
fn a_flat_bw_master_collapses_to_a_neutral() {
    let image: Vec<f32> = (0..8 * 8)
        .flat_map(|index| {
            let value = 0.05 + 0.8 * index as f32 / 63.0;
            [value, value * 0.92, value * 0.84]
        })
        .collect();
    let out = render_flat(
        &image,
        &FlatRenderSettings {
            exposure: &ExposureConfig::default(),
            process: &ProcessConfig::default(),
            mode: ProcessMode::BW,
            bounds: None,
        },
    );
    for pixel in out.chunks_exact(3) {
        assert!((pixel[0] - pixel[1]).abs() < 1e-6);
        assert!((pixel[1] - pixel[2]).abs() < 1e-6);
    }
}
