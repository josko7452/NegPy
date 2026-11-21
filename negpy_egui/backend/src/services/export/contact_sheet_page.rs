//! One contact sheet page from configured frames and their rendered tiles.

use crate::domain::models::WorkspaceConfig;
use crate::features::metadata::gear::GearLibrary;
use crate::services::export::contact_sheet::{Tile, render_sheet};
use crate::services::export::contact_sheet_layout::{
    ContactSheetSettings, best_dpi, better_orientation, clamp_paper, plan_rows, plan_sheets,
};
use crate::services::export::contact_sheet_roll::{
    FrameFacts, SheetFrame, cropped_aspect, infer_format, roll_label_text, sheet_look, turns_for,
};
use crate::services::film_edge::geometry::film_geometry;
use crate::services::film_edge::glyphs::Canvas;

/// One frame on a sheet: its path, edit and rendered proxy.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetEntry {
    pub name: String,
    pub path: String,
    pub config: WorkspaceConfig,
    pub tile: Tile,
}

/// The layout a contact sheet derives from the frames and settings.
struct SheetLayout {
    plan: crate::services::export::contact_sheet_layout::SheetPlan,
    geometry: crate::services::film_edge::geometry::FilmGeometry,
    dpi: u32,
    look: crate::services::export::contact_sheet_roll::SheetLook,
}

fn layout(
    entries: &[SheetEntry],
    settings: &ContactSheetSettings,
    roll_name: &str,
    library: Option<&GearLibrary>,
) -> (Vec<SheetFrame>, SheetLayout) {
    let frames: Vec<SheetFrame> = entries
        .iter()
        .map(|entry| {
            let mut frame = SheetFrame::new(&entry.name, &entry.path, entry.config.clone());
            frame.facts = FrameFacts {
                scan_size: Some((entry.tile.width as i64, entry.tile.height as i64)),
                ..FrameFacts::default()
            };
            frame
        })
        .collect();
    let (format, size) = infer_format(&frames, false);
    let geometry = film_geometry(format, &size);
    let (paper_width, paper_height) = better_orientation(
        settings.paper_width,
        settings.paper_height,
        &geometry,
        frames.len(),
        settings.roll_label,
        None,
    );
    let (paper_width, paper_height) =
        clamp_paper(paper_width, paper_height, &geometry, settings.roll_label);
    let label = if settings.roll_label {
        roll_label_text(roll_name, &frames, library)
    } else {
        String::new()
    };
    let aspects: Vec<f64> = frames.iter().filter_map(cropped_aspect).collect();
    let plan = if settings.film_edges {
        plan_sheets(
            paper_width,
            paper_height,
            &geometry,
            frames.len(),
            settings.roll_label,
            &[],
        )
    } else {
        plan_rows(
            paper_width,
            paper_height,
            &geometry,
            &aspects,
            settings.roll_label,
            &[],
        )
    };
    let dpi = best_dpi(paper_width, paper_height, settings.dpi);
    let mut look = sheet_look(&frames, format, &label, library);
    look.white_paper = settings.white_paper;
    (
        frames,
        SheetLayout {
            plan,
            geometry,
            dpi,
            look,
        },
    )
}

/// Render page `page` of the configured frames. `draft` skips detail for a faster preview.
#[must_use]
pub fn render_contact_sheet(
    entries: &[SheetEntry],
    settings: &ContactSheetSettings,
    roll_name: &str,
    library: Option<&GearLibrary>,
    page: usize,
    draft: bool,
) -> Canvas {
    let (frames, layout) = layout(entries, settings, roll_name, library);
    let tiles: Vec<Option<Tile>> = entries
        .iter()
        .map(|entry| Some(entry.tile.clone()))
        .collect();
    let turns: Vec<i64> = frames
        .iter()
        .map(|frame| turns_for(frame, &layout.geometry, None))
        .collect();
    render_sheet(
        &layout.plan,
        page,
        &tiles,
        &turns,
        f64::from(layout.dpi) / crate::services::export::contact_sheet_layout::MM_PER_INCH,
        &layout.look,
        draft,
        None,
    )
}

/// The pages the configured frames plan to, from the same layout as
/// `render_contact_sheet`.
#[must_use]
pub fn planned_pages(
    entries: &[SheetEntry],
    settings: &ContactSheetSettings,
    roll_name: &str,
    library: Option<&GearLibrary>,
) -> usize {
    layout(entries, settings, roll_name, library)
        .1
        .plan
        .pages
        .len()
        .max(1)
}

/// Encode a rendered page as an 8-bit PNG.
#[must_use]
pub fn encode_png(canvas: &Canvas) -> Option<Vec<u8>> {
    crate::services::export::encoders::encode_png(
        &canvas.pixels,
        canvas.width,
        canvas.height,
        3,
        8,
        6,
        None,
        None,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> SheetEntry {
        SheetEntry {
            name: name.to_string(),
            path: format!("/roll/{name}.tif"),
            config: WorkspaceConfig::default(),
            tile: Tile::new(8, 8, vec![120; 8 * 8 * 3]),
        }
    }

    #[test]
    fn a_page_is_the_configured_paper_size() {
        let settings = ContactSheetSettings::default();
        let canvas = render_contact_sheet(&[entry("a")], &settings, "Roll 1", None, 0, true);
        let (width, height) = crate::services::export::contact_sheet_layout::sheet_pixels(
            settings.paper_width,
            settings.paper_height,
            settings.dpi,
        );
        assert_eq!(
            (canvas.width, canvas.height),
            (width as usize, height as usize)
        );
    }

    #[test]
    fn an_empty_roll_renders_paper() {
        let canvas = render_contact_sheet(
            &[],
            &ContactSheetSettings::default(),
            "Empty",
            None,
            0,
            true,
        );
        assert!(canvas.width > 0 && canvas.height > 0);
    }

    #[test]
    fn a_page_encodes_as_a_png() {
        let canvas = Canvas::new(4, 3);
        let png = encode_png(&canvas).expect("a png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
}
