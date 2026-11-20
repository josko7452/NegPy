//! Ports geometry helper cases from tests/test_geometry_logic.py and test_meter_rect.py.

use negpy_backend::domain::models::{AspectRatio, GeometryConfig};
use negpy_backend::features::geometry::logic::{
    FINE_ROTATION_LIMIT, autocrop_detection_key, crops_frame, flip_geometry_and_analysis,
    has_manual_crop, holds_own_skew, meter_detection_key, mirror_normalized_rect,
    reshape_normalized_rect, rotate_geometry_and_analysis, rotate_normalized_rect,
    rotation_drag_angle, squares_on_arm, straighten_delta_degrees, toggle_flip,
    translate_normalized_rect,
};

fn close(actual: [f64; 4], expected: [f64; 4]) {
    for (got, want) in actual.iter().zip(expected) {
        assert!((got - want).abs() < 1e-9, "{actual:?} != {expected:?}");
    }
}

#[test]
fn translating_a_rect_keeps_its_size_and_clamps() {
    close(
        translate_normalized_rect([0.1, 0.2, 0.4, 0.6], 0.1, 0.05),
        [0.2, 0.25, 0.5, 0.65],
    );
    close(
        translate_normalized_rect([0.8, 0.8, 1.0, 1.0], 0.5, 0.5),
        [0.8, 0.8, 1.0, 1.0],
    );
}

#[test]
fn a_ratio_reshape_stays_inside_the_drawn_rect() {
    let rect = reshape_normalized_rect([0.25, 0.25, 0.75, 0.75], AspectRatio::R4x5);
    assert!((rect[2] - rect[0] - 0.4).abs() < 1e-9);
    assert!((rect[3] - rect[1] - 0.5).abs() < 1e-9);
    assert!((rect[0] - 0.3).abs() < 1e-9 && (rect[1] - 0.25).abs() < 1e-9);
}

#[test]
fn a_free_ratio_keeps_the_drawn_rect() {
    let rect = [0.2, 0.2, 0.8, 0.6];
    assert_eq!(reshape_normalized_rect(rect, AspectRatio::Free), rect);
}

#[test]
fn mirroring_each_axis_keeps_corners_ordered() {
    close(
        mirror_normalized_rect([0.1, 0.2, 0.5, 0.7], true),
        [0.5, 0.2, 0.9, 0.7],
    );
    close(
        mirror_normalized_rect([0.1, 0.2, 0.5, 0.7], false),
        [0.1, 0.3, 0.5, 0.8],
    );
}

#[test]
fn rotating_a_rect_four_times_is_identity() {
    let rect = [0.1, 0.2, 0.5, 0.7];
    close(rotate_normalized_rect(rect, 1), [0.2, 0.5, 0.7, 0.9]);
    close(rotate_normalized_rect(rect, 4), rect);
    close(rotate_normalized_rect(rect, -1), [0.3, 0.1, 0.8, 0.5]);
}

#[test]
fn detection_keys_ignore_offset_and_include_aspect() {
    let base = GeometryConfig::default();
    let offset = GeometryConfig {
        autocrop_offset: 40,
        ..base.clone()
    };
    assert_eq!(
        autocrop_detection_key(&base),
        autocrop_detection_key(&offset)
    );
    assert_ne!(
        autocrop_detection_key(&base),
        autocrop_detection_key(&GeometryConfig {
            autocrop_ratio: AspectRatio::R5x4,
            ..base.clone()
        })
    );
    assert_eq!(
        meter_detection_key(&base, 1200, 1800),
        meter_detection_key(&base, 600, 900)
    );
    assert_ne!(
        meter_detection_key(&base, 600, 900),
        meter_detection_key(&base, 600, 450)
    );
}

#[test]
fn square_appends_its_key_only_when_on() {
    let base = GeometryConfig::default();
    assert!(!autocrop_detection_key(&base).ends_with("square"));
    assert_eq!(
        autocrop_detection_key(&GeometryConfig {
            autocrop_square: true,
            ..base.clone()
        }),
        format!("{}|square", autocrop_detection_key(&base))
    );
}

#[test]
fn manual_crop_means_a_rect_not_from_auto() {
    let auto = GeometryConfig {
        crop_rect: Some(vec![0.1, 0.1, 0.9, 0.9]),
        crop_from_auto: true,
        ..GeometryConfig::default()
    };
    assert!(!has_manual_crop(&auto));
    let manual = GeometryConfig {
        crop_from_auto: false,
        ..auto
    };
    assert!(has_manual_crop(&manual));
}

#[test]
fn cropping_states_follow_the_three_roi_branches() {
    let base = GeometryConfig::default();
    assert!(!crops_frame(&base));
    assert!(crops_frame(&GeometryConfig {
        crop_rect: Some(vec![0.1, 0.1, 0.9, 0.9]),
        ..base.clone()
    }));
    assert!(crops_frame(&GeometryConfig {
        crop_to_valid: true,
        ..base.clone()
    }));
    assert!(!crops_frame(&GeometryConfig {
        crop_to_valid: true,
        crop_from_auto: true,
        ..base.clone()
    }));
    assert!(crops_frame(&GeometryConfig {
        autocrop_offset: 4,
        ..base
    }));
}

#[test]
fn a_frame_holds_its_own_skew_when_any_of_the_three_is_set() {
    let base = GeometryConfig::default();
    assert!(!holds_own_skew(&base));
    for geometry in [
        GeometryConfig {
            fine_rotation: 0.2,
            ..base.clone()
        },
        GeometryConfig {
            converge_v: 0.2,
            ..base.clone()
        },
        GeometryConfig {
            converge_h: 0.2,
            ..base.clone()
        },
    ] {
        assert!(holds_own_skew(&geometry));
    }
}

#[test]
fn squaring_waits_for_a_fresh_arm_without_its_own_skew() {
    let square = GeometryConfig {
        autocrop_square: true,
        ..GeometryConfig::default()
    };
    assert!(squares_on_arm(&square));
    assert!(!squares_on_arm(&GeometryConfig {
        autocrop_squared: true,
        ..square.clone()
    }));
    assert!(!squares_on_arm(&GeometryConfig {
        fine_rotation: 0.2,
        ..square.clone()
    }));
    assert!(!squares_on_arm(&GeometryConfig {
        autocrop_square: false,
        ..square
    }));
}

#[test]
fn toggling_a_flip_negates_rotation_and_convergence() {
    let geometry = GeometryConfig {
        fine_rotation: 2.5,
        converge_h: 0.8,
        converge_v: -0.4,
        crop_rect: Some(vec![0.1, 0.2, 0.5, 0.7]),
        ..GeometryConfig::default()
    };
    let flipped = toggle_flip(&geometry, true);
    assert_eq!(flipped.fine_rotation, -2.5);
    assert_eq!(flipped.converge_h, -0.8);
    assert_eq!(flipped.crop_rect, Some(vec![0.5, 0.2, 0.9, 0.7]));
    let round_trip = toggle_flip(&flipped, true);
    assert_eq!(round_trip.fine_rotation, geometry.fine_rotation);
    assert_eq!(round_trip.converge_h, geometry.converge_h);
    assert_eq!(round_trip.converge_v, geometry.converge_v);
    close(
        round_trip.crop_rect.unwrap().try_into().unwrap(),
        [0.1, 0.2, 0.5, 0.7],
    );
}

#[test]
fn zero_fine_rotation_stays_zero_through_a_flip() {
    assert_eq!(
        toggle_flip(&GeometryConfig::default(), true).fine_rotation,
        0.0
    );
}

#[test]
fn flipping_geometry_mirrors_the_analysis_rect() {
    let (_, rect) =
        flip_geometry_and_analysis(&GeometryConfig::default(), Some([0.1, 0.2, 0.5, 0.7]), true);
    close(rect.unwrap(), [0.5, 0.2, 0.9, 0.7]);
}

#[test]
fn rotating_quarters_the_geometry_and_analysis_rect() {
    let geometry = GeometryConfig {
        rotation: 1,
        crop_rect: Some(vec![0.1, 0.2, 0.5, 0.7]),
        ..GeometryConfig::default()
    };
    let (rotated, rect) = rotate_geometry_and_analysis(&geometry, Some([0.1, 0.2, 0.5, 0.7]), 1);
    assert_eq!(rotated.rotation, 2);
    assert_eq!(rotated.crop_rect, Some(vec![0.2, 0.5, 0.7, 0.9]));
    close(rect.unwrap(), [0.2, 0.5, 0.7, 0.9]);
}

#[test]
fn straighten_snaps_to_the_nearest_axis() {
    assert!((straighten_delta_degrees(10.0, 1.0) - 5.710_593_178_586_373).abs() < 1e-6);
    assert!((straighten_delta_degrees(1.0, 10.0) + 5.710_593_178_586_373).abs() < 1e-6);
    assert!((straighten_delta_degrees(-10.0, -1.0) - 5.710_593_178_586_373).abs() < 1e-6);
}

#[test]
fn a_drag_rotation_follows_the_cursor_like_a_wheel() {
    let center = (0.0, 0.0);
    assert!(
        (rotation_drag_angle(
            0.0,
            center,
            (100.0, 0.0),
            (0.0, -100.0),
            1.0,
            FINE_ROTATION_LIMIT
        ) - 45.0)
            .abs()
            < 1e-9
    );
    assert!(
        (rotation_drag_angle(
            0.0,
            center,
            (100.0, 0.0),
            (0.0, 100.0),
            1.0,
            FINE_ROTATION_LIMIT
        ) + 45.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn drag_rotation_clamps_and_scales() {
    let center = (0.0, 0.0);
    assert_eq!(
        rotation_drag_angle(
            40.0,
            center,
            (100.0, 0.0),
            (0.0, -100.0),
            1.0,
            FINE_ROTATION_LIMIT
        ),
        FINE_ROTATION_LIMIT
    );
    let full = rotation_drag_angle(
        0.0,
        center,
        (100.0, 0.0),
        (100.0, -10.0),
        1.0,
        FINE_ROTATION_LIMIT,
    );
    let fine = rotation_drag_angle(
        0.0,
        center,
        (100.0, 0.0),
        (100.0, -10.0),
        0.2,
        FINE_ROTATION_LIMIT,
    );
    assert!((fine - full * 0.2).abs() < 1e-9);
}
