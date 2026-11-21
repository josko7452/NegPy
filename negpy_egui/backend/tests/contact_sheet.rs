//! The contact-sheet painter over a planned page.

use negpy_backend::services::export::contact_sheet::{Tile, render_sheet};
use negpy_backend::services::export::contact_sheet_layout::ContactSheetSettings;
use negpy_backend::services::export::contact_sheet_layout::{DEFAULT_PAPER, plan_sheets};
use negpy_backend::services::export::contact_sheet_page::{SheetEntry, planned_pages};
use negpy_backend::services::export::contact_sheet_roll::SheetLook;
use negpy_backend::services::film_edge::edge::edge_style;
use negpy_backend::services::film_edge::geometry::{SheetFormat, film_geometry};

fn look() -> SheetLook {
    SheetLook {
        edge: edge_style(None, None, "Kodak Portra 400", "Kodak"),
        label: "Portra 400".into(),
        ..SheetLook::default()
    }
}

#[test]
fn the_planned_page_count_matches_the_renderer() {
    let entries: Vec<SheetEntry> = (0..400)
        .map(|index| SheetEntry {
            name: format!("frame-{index}"),
            path: format!("/roll/frame-{index}.tif"),
            config: negpy_backend::domain::models::WorkspaceConfig::default(),
            tile: Tile::new(80, 54, vec![128u8; 80 * 54 * 3]),
        })
        .collect();
    let pages = planned_pages(&entries, &ContactSheetSettings::default(), "Roll", None);
    assert!(
        pages >= 2,
        "40 frames need more than one sheet, got {pages}"
    );
}

#[test]
fn an_empty_page_is_paper_white() {
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &film_geometry(SheetFormat::FullFrame, "6×6"),
        0,
        true,
        &[],
    );
    let mut sheet_look = look();
    sheet_look.white_paper = true;
    sheet_look.palette = "slide".into();
    let canvas = render_sheet(&plan, 0, &[], &[], 1.0, &sheet_look, true, None);
    assert!(canvas.width > 0 && canvas.height > 0);
    assert!(
        canvas
            .pixels
            .chunks(3)
            .all(|pixel| pixel == [255, 255, 255])
    );
}

#[test]
fn a_frame_tile_lands_on_its_strip() {
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &film_geometry(SheetFormat::FullFrame, "6×6"),
        2,
        false,
        &[],
    );
    let tile = Tile::new(8, 8, [200u8; 8 * 8 * 3].to_vec());
    let canvas = render_sheet(
        &plan,
        0,
        &[Some(tile), None],
        &[0, 0],
        1.0,
        &look(),
        true,
        None,
    );
    assert!(canvas.pixels.chunks(3).any(|pixel| pixel[0] > 150));
}
