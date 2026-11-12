//! Ports of the observable rules in `tests/test_local_logic.py`, `test_local_overlap.py`,
//! `test_local_flash.py` and `test_local_key.py`.

use negpy_backend::domain::models::{LocalAdjustmentsConfig, LocalMask, MaskKey, MaskShape};
use negpy_backend::features::local::logic::{
    LocalGeometry, compute_local_maps, limited_indices, overlapping_masks, rasterise,
};
use negpy_backend::features::local::processor::build;

fn square() -> Vec<[f64; 2]> {
    vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]]
}

fn mask(stops: f64, feather: f64) -> LocalMask {
    LocalMask {
        vertices: square(),
        stops,
        feather,
        ..LocalMask::default()
    }
}

fn ev(config: &LocalAdjustmentsConfig) -> Vec<f32> {
    let maps = compute_local_maps(config, 100, 100, (100, 100), LocalGeometry::default());
    (0..100 * 100).map(|i| maps[i * 3]).collect()
}

fn at(plane: &[f32], x: usize, y: usize, w: usize) -> f32 {
    plane[y * w + x]
}

#[test]
fn an_empty_config_is_all_zeros() {
    let config = LocalAdjustmentsConfig::default();
    assert!(
        compute_local_maps(&config, 100, 100, (100, 100), LocalGeometry::default())
            .iter()
            .all(|v| *v == 0.0)
    );
}

#[test]
fn a_polygon_fills_its_interior_with_its_stops() {
    let config = LocalAdjustmentsConfig {
        masks: vec![mask(1.0, 0.0)],
    };
    let plane = ev(&config);
    assert!((at(&plane, 50, 50, 100) - 1.0).abs() < 1e-5);
    assert_eq!(at(&plane, 5, 5, 100), 0.0);
}

#[test]
fn overlapping_masks_add_their_stops() {
    let config = LocalAdjustmentsConfig {
        masks: vec![mask(0.5, 0.0), mask(0.75, 0.0)],
    };
    assert!((at(&ev(&config), 50, 50, 100) - 1.25).abs() < 1e-5);
}

#[test]
fn a_degenerate_mask_is_skipped() {
    let mut short = LocalMask {
        stops: -1.0,
        ..LocalMask::default()
    };
    short.vertices = vec![[0.4, 0.4], [0.6, 0.6]];
    let config = LocalAdjustmentsConfig { masks: vec![short] };
    assert!(ev(&config).iter().all(|v| *v == 0.0));
}

#[test]
fn a_feather_stays_within_its_stops() {
    let config = LocalAdjustmentsConfig {
        masks: vec![mask(1.0, 0.05)],
    };
    let plane = ev(&config);
    assert!(plane.iter().all(|v| (0.0..=1.0 + 1e-5).contains(v)));
    assert!(at(&plane, 50, 50, 100) > 0.9);
}

#[test]
fn an_oval_fills_its_axes_and_not_its_box_corners() {
    let oval = LocalMask {
        vertices: vec![[0.5, 0.5], [0.75, 0.5], [0.5, 0.75]],
        stops: 1.0,
        shape: MaskShape::Oval,
        feather: 0.0,
        ..LocalMask::default()
    };
    let plane = ev(&LocalAdjustmentsConfig { masks: vec![oval] });
    assert!((at(&plane, 50, 50, 100) - 1.0).abs() < 1e-5);
    assert!((at(&plane, 72, 50, 100) - 1.0).abs() < 1e-5);
    assert_eq!(at(&plane, 72, 72, 100), 0.0);
}

#[test]
fn a_card_edge_ramps_from_full_to_nothing() {
    let gradient = LocalMask {
        vertices: vec![[0.25, 0.5], [0.75, 0.5]],
        stops: 1.0,
        shape: MaskShape::Gradient,
        ..LocalMask::default()
    };
    let plane = ev(&LocalAdjustmentsConfig {
        masks: vec![gradient],
    });
    assert!((at(&plane, 25, 50, 100) - 1.0).abs() < 1e-4);
    assert!(at(&plane, 75, 50, 100) < 1e-4);
}

#[test]
fn a_limited_mask_gets_its_own_shape_plane() {
    let limited = LocalMask {
        vertices: square(),
        stops: 1.0,
        key: MaskKey::Highlights,
        feather: 0.0,
        ..LocalMask::default()
    };
    let config = LocalAdjustmentsConfig {
        masks: vec![limited],
    };
    assert_eq!(limited_indices(&config), vec![0]);
    let maps = compute_local_maps(&config, 100, 100, (100, 100), LocalGeometry::default());
    assert_eq!(maps.len(), 100 * 100 * 4);
    assert!((maps[(50 * 100 + 50) * 4 + 3] - 1.0).abs() < 1e-5);
    assert!((maps[(50 * 100 + 50) * 4] - 0.0).abs() < 1e-5);
}

#[test]
fn a_limited_mask_skips_the_flash_plane() {
    let limited = LocalMask {
        vertices: square(),
        flash: 0.5,
        key: MaskKey::Shadows,
        ..LocalMask::default()
    };
    let config = LocalAdjustmentsConfig {
        masks: vec![limited],
    };
    let maps = compute_local_maps(&config, 100, 100, (100, 100), LocalGeometry::default());
    assert_eq!(maps[(50 * 100 + 50) * 4 + 2], 0.0);
}

#[test]
fn overlap_names_only_the_intersecting_mask() {
    let left = LocalMask {
        vertices: vec![[0.1, 0.1], [0.45, 0.1], [0.45, 0.45], [0.1, 0.45]],
        ..LocalMask::default()
    };
    let straddle = LocalMask {
        vertices: vec![[0.35, 0.35], [0.75, 0.35], [0.75, 0.75], [0.35, 0.75]],
        ..LocalMask::default()
    };
    let clear = LocalMask {
        vertices: vec![[0.75, 0.05], [0.95, 0.05], [0.95, 0.25], [0.75, 0.25]],
        ..LocalMask::default()
    };
    let config = LocalAdjustmentsConfig {
        masks: vec![left, straddle, clear],
    };
    assert_eq!(overlapping_masks(&config, 0), vec![1]);
    assert_eq!(overlapping_masks(&config, 1), vec![0]);
    assert!(overlapping_masks(&config, 2).is_empty());
    assert!(overlapping_masks(&config, 7).is_empty());
}

#[test]
fn an_inverted_mask_meets_its_surround() {
    let left = LocalMask {
        vertices: vec![[0.1, 0.1], [0.45, 0.1], [0.45, 0.45], [0.1, 0.45]],
        feather: 0.0,
        invert: true,
        ..LocalMask::default()
    };
    let clear = LocalMask {
        vertices: vec![[0.75, 0.05], [0.95, 0.05], [0.95, 0.25], [0.75, 0.25]],
        feather: 0.0,
        ..LocalMask::default()
    };
    assert_eq!(
        overlapping_masks(
            &LocalAdjustmentsConfig {
                masks: vec![left, clear]
            },
            0
        ),
        vec![1]
    );
}

#[test]
fn rasterise_is_opaque_inside_and_zero_outside() {
    let alpha = rasterise(MaskShape::Polygon, &square(), 100, 100, 0.0, false);
    assert_eq!(at(&alpha, 50, 50, 100), 1.0);
    assert_eq!(at(&alpha, 5, 5, 100), 0.0);
    let inverted = rasterise(MaskShape::Polygon, &square(), 100, 100, 0.0, true);
    assert_eq!(at(&inverted, 50, 50, 100), 0.0);
    assert_eq!(at(&inverted, 5, 5, 100), 1.0);
}

#[test]
fn the_map_builder_exposes_the_three_unlimited_planes() {
    let config = LocalAdjustmentsConfig {
        masks: vec![LocalMask {
            vertices: square(),
            stops: 1.0,
            grade: -20.0,
            flash: 0.5,
            feather: 0.0,
            ..LocalMask::default()
        }],
    };
    let maps = build(&config, 100, 100, (100, 100), LocalGeometry::default());
    assert_eq!(maps.plane_count, 3);
    assert!((maps.stops(50, 50) - 1.0).abs() < 1e-5);
    assert!((maps.grade(50, 50) + 20.0).abs() < 1e-5);
    assert!((maps.flash(50, 50) - 0.5).abs() < 1e-5);
}

#[test]
fn a_quarter_turn_moves_a_mask_with_the_picture() {
    let config = LocalAdjustmentsConfig {
        masks: vec![LocalMask {
            vertices: vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]],
            stops: 1.0,
            feather: 0.0,
            ..LocalMask::default()
        }],
    };
    let plain = compute_local_maps(&config, 100, 100, (100, 100), LocalGeometry::default());
    let turned = compute_local_maps(
        &config,
        100,
        100,
        (100, 100),
        LocalGeometry {
            rotation: 1,
            ..LocalGeometry::default()
        },
    );
    let coverage = |plane: &[f32]| plane.iter().filter(|v| **v > 0.5).count();
    assert!(coverage(&plain) > 0);
    assert!(coverage(&plain).abs_diff(coverage(&turned)) <= 4);
}
