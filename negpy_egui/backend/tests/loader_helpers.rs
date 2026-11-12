//! Ports the lightweight loader-helper rules from tests/test_unsupported_raw_reason.py and IR-page tests.

use negpy_backend::domain::models::DemosaicMode;
use negpy_backend::infrastructure::loaders::helpers::{
    CfaLayout, camera_wb_multipliers, camera_xyz_matrix, get_best_demosaic_algorithm, is_xtrans,
    resolve_demosaic, supported_raw_wildcards,
};
use negpy_backend::infrastructure::loaders::ir_planes::{
    IrPage, find_ir_plane, is_ir_page, normalize_ir,
};

#[test]
fn auto_uses_ahd_on_a_bayer_cfa() {
    assert_eq!(
        get_best_demosaic_algorithm(DemosaicMode::Auto, CfaLayout::Bayer),
        DemosaicMode::Ahd
    );
}

#[test]
fn a_chosen_bayer_mode_is_kept() {
    assert_eq!(
        resolve_demosaic(DemosaicMode::Vng, CfaLayout::Bayer),
        (DemosaicMode::Vng, None)
    );
}

#[test]
fn a_source_without_a_cfa_reads_linear() {
    assert_eq!(
        resolve_demosaic(DemosaicMode::Ahd, CfaLayout::None),
        (DemosaicMode::Linear, None)
    );
}

#[test]
fn xtrans_routes_to_markesteijn() {
    let (mode, label) = resolve_demosaic(DemosaicMode::Auto, CfaLayout::XTrans);
    assert_eq!(mode, DemosaicMode::Ahd);
    assert_eq!(label, Some("Markesteijn 3-pass"));
    assert!(is_xtrans(CfaLayout::XTrans));
    assert!(!is_xtrans(CfaLayout::Bayer));
}

#[test]
fn xtrans_keeps_ppg_on_the_one_pass_route() {
    let (_, label) = resolve_demosaic(DemosaicMode::Ppg, CfaLayout::XTrans);
    assert_eq!(label, Some("Markesteijn 1-pass"));
}

#[test]
fn a_none_or_nonfinite_camera_matrix_is_rejected() {
    assert_eq!(camera_xyz_matrix(None), None);
    assert_eq!(
        camera_xyz_matrix(Some([
            [f64::NAN, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0]
        ])),
        None
    );
    assert_eq!(camera_xyz_matrix(Some([[0.0; 3]; 3])), None);
    let matrix = [[0.5, 0.1, 0.2], [0.1, 0.8, 0.1], [0.0, 0.2, 0.7]];
    assert_eq!(camera_xyz_matrix(Some(matrix)), Some(matrix));
}

#[test]
fn white_balance_requires_three_positive_finite_gains() {
    assert_eq!(camera_wb_multipliers(None), None);
    assert_eq!(camera_wb_multipliers(Some([2.0, 1.0, 0.0])), None);
    assert_eq!(
        camera_wb_multipliers(Some([2.0, 1.0, 1.5])),
        Some([2.0, 1.0, 1.5])
    );
}

#[test]
fn raw_wildcards_list_both_cases() {
    let wildcards = supported_raw_wildcards();
    assert!(wildcards.contains("*.nef"));
    assert!(wildcards.contains("*.NEF"));
    assert!(wildcards.contains("*.dng"));
}

#[test]
fn an_ir_page_must_match_the_main_dimensions_and_marker() {
    let page = IrPage {
        width: 100,
        height: 80,
        new_subfile_type: Some(4),
        photometric: None,
    };
    assert!(is_ir_page(page, 100, 80));
    assert!(!is_ir_page(page, 50, 40));
    let grayscale = IrPage {
        new_subfile_type: None,
        photometric: Some(1),
        ..page
    };
    assert!(is_ir_page(grayscale, 100, 80));
    let color = IrPage {
        new_subfile_type: None,
        photometric: Some(2),
        ..page
    };
    assert!(!is_ir_page(color, 100, 80));
}

#[test]
fn the_first_exact_ir_page_wins() {
    let pages = [
        IrPage {
            width: 50,
            height: 40,
            new_subfile_type: Some(4),
            photometric: None,
        },
        IrPage {
            width: 100,
            height: 80,
            new_subfile_type: Some(4),
            photometric: None,
        },
        IrPage {
            width: 100,
            height: 80,
            new_subfile_type: None,
            photometric: Some(1),
        },
    ];
    assert_eq!(find_ir_plane(&pages, 100, 80), Some(1));
    assert_eq!(find_ir_plane(&pages, 120, 90), None);
}

#[test]
fn ir_samples_normalize_to_unit_float() {
    assert_eq!(
        normalize_ir(&[0, 32768, 65535], 65535),
        vec![0.0, 0.500_007_6, 1.0]
    );
}
