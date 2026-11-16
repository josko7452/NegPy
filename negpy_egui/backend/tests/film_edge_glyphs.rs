//! The film-edge face and its measured runs.

use negpy_backend::services::film_edge::glyphs::{Canvas, EDGE_CAP_EM, TextRun};

#[test]
fn the_cap_height_drives_the_point_size() {
    let run = TextRun::capped("HP5", 2.8);
    assert!((run.size as f64 - 2.8 / EDGE_CAP_EM).abs() < 0.01);
    assert!(run.width > 0.0);
}

#[test]
fn a_canvas_blends_a_coverage_onto_its_pixels() {
    let mut canvas = Canvas::new(2, 1);
    canvas.blend(0, 0, 2, 1, &[1.0, 0.0], [200, 100, 50]);
    assert_eq!(&canvas.pixels[..3], &[200, 100, 50]);
    assert_eq!(&canvas.pixels[3..6], &[0, 0, 0]);
}
