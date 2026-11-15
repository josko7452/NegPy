//! Ports the geometry and frame-turn checks in tests/test_contact_sheet_layout.py.

use negpy_backend::services::film_edge::geometry::{
    DEFAULT_FRAME_120, FRAME_135_PITCH, FRAME_SIZES_120, HALF_FRAME_PITCH, PERF_ALONG, SheetFormat,
    film_geometry, perforation_centers,
};
use negpy_backend::services::film_edge::layout::frame_turns;

fn full() -> negpy_backend::services::film_edge::geometry::FilmGeometry {
    film_geometry(SheetFormat::FullFrame, DEFAULT_FRAME_120)
}

fn half() -> negpy_backend::services::film_edge::geometry::FilmGeometry {
    film_geometry(SheetFormat::HalfFrame, DEFAULT_FRAME_120)
}

#[test]
fn the_135_frame_dimensions_match_the_film_standard() {
    let geometry = full();
    assert_eq!(
        (
            geometry.frame_along,
            geometry.frame_across,
            geometry.pitch,
            geometry.width,
        ),
        (36.0, 24.0, FRAME_135_PITCH, 35.0)
    );
    assert_eq!(
        (half().frame_along, half().frame_across, half().pitch),
        (18.0, 24.0, HALF_FRAME_PITCH)
    );
}

#[test]
fn the_120_sizes_match_the_declared_table() {
    assert_eq!(
        FRAME_SIZES_120,
        [
            ("6×4.5", 41.5),
            ("6×6", 56.0),
            ("6×7", 70.0),
            ("6×8", 77.0),
            ("6×9", 84.0),
            ("6×12", 112.0),
            ("6×17", 168.0)
        ]
    );
    let six_seven = film_geometry(SheetFormat::Medium, "6×7");
    assert_eq!(
        (
            six_seven.frame_along,
            six_seven.frame_across,
            six_seven.width,
            six_seven.pitch,
        ),
        (70.0, 56.0, 61.0, 74.0)
    );
    assert_eq!(
        film_geometry(SheetFormat::Medium, "nonsense").frame_size,
        DEFAULT_FRAME_120
    );
}

#[test]
fn frame_centers_start_at_half_a_pitch() {
    assert_eq!(full().frame_center(0), 19.0);
    assert_eq!(full().frame_center(1), 57.0);
    assert_eq!(half().frame_center(0), 9.5);
}

#[test]
fn no_strip_cut_lands_on_a_perforation() {
    for geometry in [full(), half()] {
        for strip in 0..6 {
            let cut = strip as f64 * 6.0 * geometry.pitch;
            for center in perforation_centers(cut - 10.0, cut + 10.0) {
                assert!(f64::abs(center - cut) >= PERF_ALONG / 2.0 + 1.0);
            }
        }
    }
}

#[test]
fn the_perforations_are_symmetric_about_a_frame() {
    let center = full().frame_center(0);
    let holes = perforation_centers(center - 19.0, center + 19.0);
    assert_eq!(holes.len(), 8);
    let offset: f64 = holes.iter().map(|hole| hole - center).sum();
    assert!(offset.abs() < 1e-9);
}

#[test]
fn matching_orientations_never_turn() {
    assert_eq!(frame_turns(Some(1.5), &full(), 0, false, false), 0);
    assert_eq!(frame_turns(Some(0.75), &half(), 0, false, false), 0);
    assert_eq!(
        frame_turns(
            Some(0.67),
            &film_geometry(SheetFormat::Medium, "6×6"),
            1,
            false,
            false
        ),
        0
    );
    assert_eq!(frame_turns(Some(1.0), &full(), 0, false, false), 0);
    assert_eq!(frame_turns(None, &full(), 1, false, false), 0);
}

#[test]
fn an_upright_frame_turns_back() {
    assert_eq!(frame_turns(Some(0.67), &full(), 1, false, false), 3);
    assert_eq!(frame_turns(Some(0.67), &full(), 3, false, false), 1);
    assert_eq!(frame_turns(Some(0.67), &full(), 1, true, false), 1);
    assert_eq!(frame_turns(Some(0.67), &full(), 1, true, true), 3);
    assert_eq!(frame_turns(Some(0.67), &full(), 0, false, false), 3);
    assert_eq!(frame_turns(Some(1.33), &half(), 2, false, false), 3);
    assert_eq!(frame_turns(Some(1.5), &full(), 1, false, false), 0);
}
