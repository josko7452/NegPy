//! Ports the template cases of tests/test_batch_autocrop.py.

use negpy_backend::features::geometry::batch_autocrop::{CropEvidence, build_roll_template};

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
            reason: "no_consensus".to_string(),
            ..CropEvidence::default()
        });
    }
    assert!(build_roll_template(&roll).is_some_and(|template| template.from_fallback));
}
