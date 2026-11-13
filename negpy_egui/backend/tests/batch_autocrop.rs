//! Ports the template cases of tests/test_batch_autocrop.py.

use negpy_backend::features::geometry::batch_autocrop::{
    CropEvidence, build_roll_template, resolve_roll_crops,
};

fn evidence(key: &str, roi: Option<(i64, i64, i64, i64)>) -> CropEvidence {
    CropEvidence {
        key: key.to_string(),
        canvas_shape: (1000, 1000),
        roi,
        correction_angle: 1.0,
        confidence: 0.9,
        target_ratio: "3:2".to_string(),
        rebate_trim: 1.0,
        supported_sides: vec!["left", "right"],
        geometry_score: 0.8,
        ..CropEvidence::default()
    }
}

#[test]
fn a_roll_template_rejects_width_and_angle_outliers_deterministically() {
    let evidence = vec![
        evidence("frame-a", Some((100, 900, 100, 900))),
        evidence("frame-b", Some((100, 900, 105, 905))),
        evidence("frame-c", Some((100, 900, 95, 895))),
        evidence("frame-d", Some((100, 900, 100, 900))),
        CropEvidence {
            correction_angle: 6.0,
            ..evidence("outlier", Some((250, 750, 300, 700)))
        },
    ];
    let forward = build_roll_template(&evidence).expect("a template");
    let mut reversed = evidence.clone();
    reversed.reverse();
    let reverse = build_roll_template(&reversed).expect("a template");
    assert_eq!(forward, reverse);
    assert_eq!(forward.sample_count, 4);
    assert!((forward.width - 0.8).abs() < 1e-9);
    assert!((forward.correction_angle - 1.0).abs() < 1e-9);
}

#[test]
fn a_borderless_roll_builds_a_fallback_template() {
    let roll: Vec<CropEvidence> = (0..6)
        .map(|index| CropEvidence {
            key: format!("borderless-{index}"),
            canvas_shape: (1000, 1600),
            roi: None,
            fallback_roi: Some((0, 1000, 0, 1600)),
            correction_angle: 0.0,
            confidence: 0.0,
            border: vec![0.0, 0.0, 0.0625, 0.0],
            target_ratio: "3:2".to_string(),
            reason: "no_consensus".to_string(),
            ..CropEvidence::default()
        })
        .collect();
    let template = build_roll_template(&roll).expect("a fallback");
    assert!(template.from_fallback);
    assert_eq!(template.sample_count, 6);
    assert_eq!(template.border, vec![0.0, 0.0, 0.0625, 0.0]);
}

#[test]
fn an_inset_threshold_box_does_not_make_a_template() {
    let roll: Vec<CropEvidence> = (0..6)
        .map(|index| CropEvidence {
            key: format!("inset-{index}"),
            canvas_shape: (1000, 1000),
            roi: None,
            fallback_roi: Some((20, 980, 220, 980)),
            confidence: 0.0,
            target_ratio: "3:2".to_string(),
            reason: "no_consensus".to_string(),
            ..CropEvidence::default()
        })
        .collect();
    assert!(build_roll_template(&roll).is_none());
}

#[test]
fn one_trusted_frame_does_not_make_the_roll_template() {
    let mut roll = vec![CropEvidence {
        key: "small-box".to_string(),
        canvas_shape: (1000, 1000),
        roi: Some((300, 700, 100, 700)),
        confidence: 0.9,
        supported_sides: vec!["left", "right"],
        geometry_score: 0.8,
        correction_angle: 1.0,
        fallback_roi: Some((0, 1000, 0, 1000)),
        ..CropEvidence::default()
    }];
    for index in 0..5 {
        roll.push(CropEvidence {
            key: format!("weak-{index}"),
            canvas_shape: (1000, 1000),
            roi: None,
            fallback_roi: Some((0, 1000, 0, 1000)),
            confidence: 0.0,
            target_ratio: "3:2".to_string(),
            reason: "no_consensus".to_string(),
            ..CropEvidence::default()
        });
    }
    assert!(build_roll_template(&roll).is_some_and(|template| template.from_fallback));
}

fn resolved(
    evidence: &[CropEvidence],
) -> std::collections::HashMap<
    String,
    negpy_backend::features::geometry::batch_autocrop::ResolvedCrop,
> {
    resolve_roll_crops(evidence, 0.0)
        .into_iter()
        .map(|crop| (crop.key.clone(), crop))
        .collect()
}

#[test]
fn a_borderless_roll_crops_from_its_measured_borders() {
    let roll: Vec<CropEvidence> = (0..6)
        .map(|index| CropEvidence {
            key: format!("borderless-{index}"),
            canvas_shape: (1000, 1600),
            roi: None,
            fallback_roi: Some((0, 1000, 0, 1600)),
            border: vec![0.0, 0.0, 0.0625, 0.0],
            target_ratio: "3:2".to_string(),
            reason: "no_consensus".to_string(),
            ..CropEvidence::default()
        })
        .collect();
    let crops = resolved(&roll);
    assert_eq!(crops.len(), 6);
    let first = &crops["borderless-0"];
    assert!(first.calibrated);
    assert!(first.confidence < 0.58);
    assert_eq!(
        first.crop_rect[0],
        0.0625,
        "{:?} {:?}",
        first.crop_rect,
        build_roll_template(&roll)
    );
    for (got, expected) in first.crop_rect.iter().zip([0.0625, 0.0, 1.0, 1.0]) {
        assert!((got - expected).abs() < 2e-3);
    }
}

#[test]
fn a_real_template_ignores_a_fallback_box() {
    let mut roll: Vec<CropEvidence> = (0..3)
        .map(|index| evidence(&format!("trusted-{index}"), Some((100, 900, 100, 900))))
        .collect();
    roll.push(CropEvidence {
        key: "weak".to_string(),
        canvas_shape: (1000, 1000),
        roi: None,
        fallback_roi: Some((0, 1000, 0, 1000)),
        confidence: 0.0,
        vertical_edge_profile: vec![0.0; 101],
        vertical_edge_contrast: 0.01,
        target_ratio: "3:2".to_string(),
        reason: "no_consensus".to_string(),
        ..CropEvidence::default()
    });
    let crops = resolved(&roll);
    assert!(!crops.contains_key("weak"));
}

#[test]
fn a_portrait_frame_resolves_from_its_own_crop() {
    let portrait = CropEvidence {
        key: "only".to_string(),
        canvas_shape: (120, 80),
        roi: None,
        fallback_roi: Some((10, 110, 5, 75)),
        confidence: 0.0,
        target_ratio: "3:2".to_string(),
        reason: "unsupported_orientation".to_string(),
        ..CropEvidence::default()
    };
    let crops = resolved(&[portrait]);
    assert_eq!(crops.len(), 1);
    let crop = &crops["only"];
    assert!(!crop.calibrated);
    for (got, expected) in
        crop.crop_rect
            .iter()
            .zip([5.0 / 80.0, 10.0 / 120.0, 75.0 / 80.0, 110.0 / 120.0])
    {
        assert!((got - expected).abs() < 1e-9);
    }
}

#[test]
fn a_blank_portrait_frame_abstains() {
    let portrait = CropEvidence {
        key: "blank".to_string(),
        canvas_shape: (120, 80),
        fallback_roi: Some((0, 0, 0, 0)),
        target_ratio: "3:2".to_string(),
        reason: "unsupported_orientation".to_string(),
        ..CropEvidence::default()
    };
    let mut roll: Vec<CropEvidence> = (0..3)
        .map(|index| evidence(&format!("trusted-{index}"), Some((100, 900, 100, 900))))
        .collect();
    roll.push(portrait);
    assert!(!resolved(&roll).contains_key("blank"));
}

#[test]
fn a_weak_frame_resolves_from_the_template() {
    let mut roll: Vec<CropEvidence> = (0..3)
        .map(|index| evidence(&format!("trusted-{index}"), Some((100, 900, 100, 900))))
        .collect();
    roll.push(CropEvidence {
        key: "weak".to_string(),
        canvas_shape: (1000, 1000),
        roi: Some((100, 900, 150, 870)),
        confidence: 0.3,
        supported_sides: vec!["left"],
        geometry_score: 0.5,
        correction_angle: 1.0,
        target_ratio: "3:2".to_string(),
        ..CropEvidence::default()
    });
    let crops = resolved(&roll);
    let weak = &crops["weak"];
    assert!(weak.calibrated);
    assert!((weak.crop_rect[2] - weak.crop_rect[0] - 0.8).abs() < 1e-6);
}

#[test]
fn the_safety_border_pads_or_stops_at_a_common_edge() {
    let padded = negpy_backend::features::geometry::batch_autocrop::resolve_roll_crops(
        &[CropEvidence {
            key: "trusted".to_string(),
            canvas_shape: (1000, 2000),
            roi: Some((100, 900, 200, 1800)),
            confidence: 0.9,
            supported_sides: vec!["left", "right"],
            geometry_score: 0.8,
            correction_angle: 1.0,
            ..CropEvidence::default()
        }],
        0.01,
    );
    assert!(padded.is_empty());
}
