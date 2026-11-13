//! Ports the classification and assembly cases of tests/test_rgbscan.py.

use std::collections::HashMap;

use negpy_backend::features::rgbscan::{
    BLUE, GREEN, MIN_TRIPLET_AFFINITY, RED, Triplet, assemble_rgb, capture_ordered,
    classify_channel, frame_affinity, group_triplets, looks_narrowband, place_triplet,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

#[test]
fn the_dominant_channel_classifies_a_narrowband_exposure() {
    assert_eq!(classify_channel(&[500.0, 80.0, 20.0]), RED);
    assert_eq!(classify_channel(&[76.0, 710.0, 173.0]), GREEN);
    assert_eq!(classify_channel(&[40.0, 360.0, 867.0]), BLUE);
}

#[test]
fn assembling_picks_each_matching_channel() {
    let red = ImageBuffer::from_rgb(2, 2, [1.0, 0.0, 0.0].repeat(4));
    let green = ImageBuffer::from_rgb(2, 2, [0.0, 2.0, 0.0].repeat(4));
    let blue = ImageBuffer::from_rgb(2, 2, [0.0, 0.0, 3.0].repeat(4));
    let out = assemble_rgb(&red, &green, &blue, false).unwrap();
    assert!(
        out.pixels
            .chunks_exact(3)
            .all(|pixel| pixel == [1.0, 2.0, 3.0])
    );
}

#[test]
fn a_shape_mismatch_is_rejected() {
    let red = ImageBuffer::from_rgb(2, 2, vec![0.0; 12]);
    let green = ImageBuffer::from_rgb(2, 3, vec![0.0; 18]);
    let blue = ImageBuffer::from_rgb(2, 2, vec![0.0; 12]);
    assert!(assemble_rgb(&red, &green, &blue, false).is_none());
}

#[test]
fn affinity_separates_the_same_scene_from_another() {
    let one: Vec<f32> = (0..100).map(|i| (i as f32 * 0.3).sin()).collect();
    let two: Vec<f32> = (0..100).map(|i| (i as f32 * 0.7).cos()).collect();
    assert!(
        (frame_affinity(Some(&one), Some(&one))
            - one.iter().map(|v| v * v).sum::<f32>() as f64 / one.len() as f64)
            .abs()
            < 1e-6
    );
    assert!(frame_affinity(Some(&one), Some(&two)) < 0.5);
    assert_eq!(frame_affinity(None, Some(&one)), 0.0);
    assert_eq!(frame_affinity(Some(&one), Some(&one[..50])), 0.0);
}

#[test]
fn narrowband_folders_are_recognized_by_the_median_dominance() {
    assert!(looks_narrowband(&[
        [500.0, 80.0, 20.0],
        [40.0, 360.0, 867.0]
    ]));
    assert!(!looks_narrowband(&[[100.0, 110.0, 105.0]]));
    assert!(!looks_narrowband(&[]));
}

#[test]
fn capture_order_falls_back_when_any_time_is_missing() {
    let paths = vec!["b.raw".to_string(), "a.raw".to_string()];
    let mut times = HashMap::new();
    times.insert("a.raw".to_string(), "2026-01-01T00:00:01".to_string());
    assert_eq!(capture_ordered(&paths, &times), paths);
    times.insert("b.raw".to_string(), "2026-01-01T00:00:00".to_string());
    assert_eq!(capture_ordered(&paths, &times), vec!["b.raw", "a.raw"]);
}

#[test]
fn a_scrambled_chunk_is_placed_by_channel() {
    let items = vec![
        ("f1".to_string(), GREEN),
        ("f2".to_string(), RED),
        ("f3".to_string(), BLUE),
    ];
    let triplets = group_triplets(&items, None, MIN_TRIPLET_AFFINITY);
    assert_eq!(triplets.len(), 1);
    assert_eq!(
        (
            triplets[0].red.as_str(),
            triplets[0].green.as_str(),
            triplets[0].blue.as_str()
        ),
        ("f2", "f1", "f3")
    );
    assert!(triplets[0].ok);
}

#[test]
fn an_off_by_one_chunk_is_rejected_when_signatures_disagree() {
    let mut signatures = HashMap::new();
    for (path, seed) in [
        ("r1", 1.0),
        ("g1", 1.0),
        ("b2", 9.0),
        ("r1b", 1.0),
        ("g1b", 1.0),
        ("b1", 1.0),
    ] {
        let signature: Vec<f32> = (0..80)
            .map(|i| (i as f32 * 0.13 + seed).sin() + seed * 0.1)
            .collect();
        signatures.insert(path.to_string(), signature);
    }
    let items = vec![
        ("r1".to_string(), RED),
        ("g1".to_string(), GREEN),
        ("b2".to_string(), BLUE),
    ];
    assert!(!group_triplets(&items, Some(&signatures), MIN_TRIPLET_AFFINITY)[0].ok);
}

#[test]
fn a_hand_picked_triplet_is_assigned_by_dominant_channel() {
    use negpy_backend::features::rgbscan::FrameProbe;
    let mut probes = HashMap::new();
    probes.insert(
        "red.raw".to_string(),
        Some(FrameProbe {
            means: [500.0, 80.0, 20.0],
            signature: vec![1.0; 40],
        }),
    );
    probes.insert(
        "green.raw".to_string(),
        Some(FrameProbe {
            means: [76.0, 710.0, 173.0],
            signature: vec![1.0; 40],
        }),
    );
    probes.insert(
        "blue.raw".to_string(),
        Some(FrameProbe {
            means: [40.0, 360.0, 867.0],
            signature: vec![1.0; 40],
        }),
    );
    let (triplet, why) = place_triplet(&probes, MIN_TRIPLET_AFFINITY);
    assert_eq!(why, "");
    assert_eq!(
        triplet,
        Some(Triplet {
            red: "red.raw".to_string(),
            green: "green.raw".to_string(),
            blue: "blue.raw".to_string(),
            ok: true,
        })
    );
}
