//! The strip painter's rebate, ink and perforation coverage.

use negpy_backend::services::film_edge::edge::edge_style;
use negpy_backend::services::film_edge::geometry::{SheetFormat, film_geometry};
use negpy_backend::services::film_edge::glyphs::Canvas;
use negpy_backend::services::film_edge::paint::{Palette, paint_strip};

fn palette() -> Palette {
    Palette {
        no_film: [255, 255, 255],
        rebate: [12, 12, 12],
        rim: Some([40, 40, 40]),
        ink: [220, 220, 220],
        label: [220, 220, 220],
    }
}

#[test]
fn the_rebate_and_perforations_both_appear() {
    let geometry = film_geometry(SheetFormat::FullFrame, "6x6");
    let style = edge_style(None, None, "Kodak Portra 400", "Kodak");
    let mut canvas = Canvas::new(800, 300);
    paint_strip(
        &mut canvas,
        0.0,
        0.0,
        &geometry,
        &style,
        6,
        0.0,
        38.0,
        8.0,
        &palette(),
        false,
        None,
    );
    assert!(canvas.pixels.chunks(3).any(|pixel| pixel == [12; 3]));
    assert!(canvas.pixels.chunks(3).any(|pixel| pixel == [255; 3]));
    assert!(canvas.pixels.chunks(3).any(|pixel| pixel == [40; 3]));
}

#[test]
fn edge_text_lights_pixels_inside_the_rebate() {
    let geometry = film_geometry(SheetFormat::FullFrame, "6x6");
    let style = edge_style(None, None, "Kodak Portra 400", "Kodak");
    let mut canvas = Canvas::new(800, 300);
    paint_strip(
        &mut canvas,
        0.0,
        0.0,
        &geometry,
        &style,
        6,
        0.0,
        38.0,
        8.0,
        &palette(),
        false,
        None,
    );
    assert!(
        canvas
            .pixels
            .chunks(3)
            .any(|pixel| pixel[0] > 180 && pixel[0] < 240)
    );
}
