//! Ports tests/test_contact_sheet_layout.py.

use negpy_backend::services::export::contact_sheet_layout::{
    ContactSheetSettings, DEFAULT_PAPER, ILFORD_PAPERS, MAX_STRETCH, PAPER_MAX, ROW_BORDER,
    ROW_GAP, SHEET_MARGIN, best_dpi, better_orientation, clamp_paper, cut_strips, dpi_allowed,
    min_paper_size, paper_preset, plan_capacity, plan_rows, plan_sheets, sheet_pixels, snap_paper,
};
use negpy_backend::services::film_edge::geometry::{SheetFormat, film_geometry};

fn full() -> negpy_backend::services::film_edge::geometry::FilmGeometry {
    film_geometry(SheetFormat::FullFrame, "6×6")
}

fn half() -> negpy_backend::services::film_edge::geometry::FilmGeometry {
    film_geometry(SheetFormat::HalfFrame, "6×6")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn the_default_paper_is_ilford_9_5_x_12_in_true_size() {
    assert_eq!(DEFAULT_PAPER.label, "24 × 30.5 cm");
    assert_eq!((DEFAULT_PAPER.width, DEFAULT_PAPER.height), (241.3, 304.8));
    assert_eq!(
        sheet_pixels(DEFAULT_PAPER.width, DEFAULT_PAPER.height, 300),
        (2850, 3600)
    );
}

#[test]
fn the_default_paper_holds_a_36_exposure_roll_with_the_label() {
    assert_eq!(
        plan_capacity(DEFAULT_PAPER.width, DEFAULT_PAPER.height, &full(), true),
        (6, 7)
    );
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        38,
        true,
        &[],
    );
    assert_eq!(plan.pages.len(), 1);
    assert_eq!(
        plan.pages[0]
            .strips
            .iter()
            .map(|strip| strip.count)
            .collect::<Vec<_>>(),
        [6, 6, 6, 6, 6, 6, 2]
    );
}

#[test]
fn the_default_paper_turned_holds_35_so_portrait_is_the_default() {
    assert_eq!(
        plan_capacity(DEFAULT_PAPER.height, DEFAULT_PAPER.width, &full(), true),
        (7, 5)
    );
}

#[test]
fn the_default_paper_holds_72_half_frames_in_strips_of_12() {
    assert_eq!(
        plan_capacity(DEFAULT_PAPER.width, DEFAULT_PAPER.height, &half(), true),
        (12, 7)
    );
    assert_eq!(
        plan_sheets(
            DEFAULT_PAPER.width,
            DEFAULT_PAPER.height,
            &half(),
            72,
            true,
            &[]
        )
        .pages
        .len(),
        1
    );
}

#[test]
fn the_120_rolls_fit_the_default_paper() {
    for (size, portrait, roll) in [
        ("6×4.5", (5, 4), 16),
        ("6×6", (3, 4), 12),
        ("6×7", (3, 4), 10),
        ("6×9", (2, 4), 8),
        ("6×17", (1, 4), 4),
    ] {
        let geometry = film_geometry(SheetFormat::Medium, size);
        assert_eq!(
            plan_capacity(DEFAULT_PAPER.width, DEFAULT_PAPER.height, &geometry, true),
            portrait
        );
        assert!(portrait.0 * portrait.1 >= roll);
    }
}

#[test]
fn the_6x8_and_6x12_rolls_need_the_paper_turned() {
    for (size, roll) in [("6×8", 9), ("6×12", 6)] {
        let geometry = film_geometry(SheetFormat::Medium, size);
        let (per, strips) =
            plan_capacity(DEFAULT_PAPER.width, DEFAULT_PAPER.height, &geometry, true);
        assert!(per * strips < roll);
        let turned = better_orientation(
            DEFAULT_PAPER.width,
            DEFAULT_PAPER.height,
            &geometry,
            roll,
            true,
            None,
        );
        assert_eq!(turned, (DEFAULT_PAPER.height, DEFAULT_PAPER.width));
        let (per, strips) = plan_capacity(turned.0, turned.1, &geometry, true);
        assert!(per * strips >= roll);
    }
}

#[test]
fn the_8x10_sheet_holds_30_frames() {
    let (per, strips) = plan_capacity(203.2, 254.0, &full(), true);
    assert_eq!(per * strips, 30);
}

#[test]
fn the_block_is_centered_and_a_short_strip_starts_at_its_left_edge() {
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        38,
        true,
        &[],
    );
    let page = &plan.pages[0];
    let (x, y, w, h) = page.block;
    assert!(close(x, (DEFAULT_PAPER.width - w) / 2.0));
    assert!(close(w, 6.0 * 38.0));
    assert!(close(y + h / 2.0, DEFAULT_PAPER.height / 2.0));
    assert!(page.strips.iter().all(|strip| close(strip.x, x)));
    assert!(close(page.strips.last().unwrap().length, 2.0 * 38.0));
    assert_eq!(
        page.strips
            .iter()
            .map(|strip| strip.first)
            .collect::<Vec<_>>(),
        [0, 6, 12, 18, 24, 30, 36]
    );
    assert_eq!(
        page.strips
            .iter()
            .map(|strip| strip.roll_start)
            .collect::<Vec<_>>(),
        (0..7)
            .map(|index| index as f64 * 6.0 * 38.0)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_single_short_strip_is_centered_on_its_own_length() {
    let page = &plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        3,
        true,
        &[],
    )
    .pages[0];
    assert!(close(page.block.2, 3.0 * 38.0));
    assert!(close(
        page.block.0,
        (DEFAULT_PAPER.width - page.block.2) / 2.0
    ));
}

#[test]
fn a_full_block_leaves_the_label_band_clear() {
    let page = &plan_sheets(DEFAULT_PAPER.width, 263.0, &full(), 42, true, &[]).pages[0];
    assert!(page.block.1 >= SHEET_MARGIN + 8.0 - 1e-9);
}

#[test]
fn a_roll_that_does_not_fit_paginates() {
    let plan = plan_sheets(203.2, 254.0, &full(), 38, true, &[]);
    assert_eq!(plan.capacity(), 30);
    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.strips.iter().map(|strip| strip.count).sum::<usize>())
            .collect::<Vec<_>>(),
        [30, 8]
    );
    assert_eq!(plan.pages[1].strips[0].first, 30);
}

#[test]
fn a_zero_capacity_plan_says_why() {
    let plan = plan_sheets(
        127.0,
        177.8,
        &film_geometry(SheetFormat::Medium, "6×17"),
        4,
        true,
        &[],
    );
    assert!(plan.pages.is_empty());
    assert!(plan.reason.contains("narrow"));
}

#[test]
fn scene_breaks_start_new_strips() {
    assert_eq!(cut_strips(10, 6, &[4]), vec![(0, 4), (4, 6)]);
    assert_eq!(
        cut_strips(14, 6, &[3, 5]),
        vec![(0, 3), (3, 2), (5, 6), (11, 3)]
    );
    assert_eq!(cut_strips(8, 6, &[0, 8, 99]), vec![(0, 6), (6, 2)]);
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        10,
        true,
        &[4],
    );
    assert_eq!(
        plan.pages[0]
            .strips
            .iter()
            .map(|strip| (strip.first, strip.count))
            .collect::<Vec<_>>(),
        [(0, 4), (4, 6)]
    );
    assert!(close(plan.pages[0].block.2, 6.0 * 38.0));
    assert!(close(plan.pages[0].strips[0].x, plan.pages[0].strips[1].x));
}

#[test]
fn scene_breaks_paginate_by_strips() {
    let plan = plan_sheets(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        16,
        true,
        &(1..16).collect::<Vec<_>>(),
    );
    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.strips.len())
            .collect::<Vec<_>>(),
        [7, 7, 2]
    );
}

#[test]
fn a_full_row_fills_the_paper_and_the_last_row_keeps_the_film_height() {
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[1.5; 10],
        false,
        &[],
    );
    let first = &plan.pages[0].rows[0];
    let last = &plan.pages[0].rows[plan.pages[0].rows.len() - 1];
    let inner = DEFAULT_PAPER.width - 2.0 * SHEET_MARGIN - 2.0 * ROW_BORDER;
    assert!(close(first.width, inner + 2.0 * ROW_BORDER));
    assert!(first.frames[0].height >= full().frame_across);
    assert!(close(last.frames[0].height, full().frame_across));
    assert!(last.width < first.width);
}

#[test]
fn frames_keep_their_aspect_and_order() {
    let aspects = [1.5, 2.0 / 3.0, 1.0, 1.5, 2.0 / 3.0, 3.0, 1.5];
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &aspects,
        false,
        &[],
    );
    let placed: Vec<_> = plan.pages[0]
        .rows
        .iter()
        .flat_map(|row| row.frames.clone())
        .collect();
    assert_eq!(
        placed.iter().map(|frame| frame.slot).collect::<Vec<_>>(),
        (0..aspects.len()).collect::<Vec<_>>()
    );
    for frame in &placed {
        assert!(close(frame.width / frame.height, aspects[frame.slot]));
    }
    assert_eq!(
        plan.pages[0].slots(),
        (0..aspects.len()).collect::<Vec<_>>()
    );
}

#[test]
fn frames_in_a_row_sit_one_gap_apart_inside_the_band() {
    let row = &plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[1.5; 10],
        false,
        &[],
    )
    .pages[0]
        .rows[0];
    for pair in row.frames.windows(2) {
        assert!(close(pair[1].x - (pair[0].x + pair[0].width), ROW_GAP));
    }
    assert!(close(row.frames[0].x, row.x + ROW_BORDER));
    assert!(close(row.frames[0].y, row.y + ROW_BORDER));
}

#[test]
fn a_break_starts_a_new_row_and_the_row_before_it_stays_short() {
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[1.5; 6],
        false,
        &[2],
    );
    assert_eq!(
        plan.pages[0]
            .rows
            .iter()
            .map(|row| row.frames.len())
            .collect::<Vec<_>>(),
        [2, 4]
    );
    assert!(close(
        plan.pages[0].rows[0].frames[0].height,
        full().frame_across
    ));
}

#[test]
fn a_row_that_would_stretch_too_far_stays_short() {
    let inner = DEFAULT_PAPER.width - 2.0 * SHEET_MARGIN - 2.0 * ROW_BORDER;
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[2.0 / 3.0, inner / full().frame_across],
        false,
        &[],
    );
    let first = &plan.pages[0].rows[0];
    assert_eq!(first.frames.len(), 1);
    assert!(first.frames[0].height <= full().frame_across * MAX_STRETCH);
}

#[test]
fn a_frame_wider_than_the_paper_shrinks_to_fit() {
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[20.0],
        false,
        &[],
    );
    let inner = DEFAULT_PAPER.width - 2.0 * SHEET_MARGIN - 2.0 * ROW_BORDER;
    assert!(close(plan.pages[0].rows[0].frames[0].width, inner));
}

#[test]
fn rows_fill_a_page_then_start_the_next() {
    let plan = plan_rows(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        &[1.5; 120],
        false,
        &[],
    );
    assert!(plan.pages.len() > 1);
    assert_eq!(
        plan.pages
            .iter()
            .map(|page| page.slots().len())
            .sum::<usize>(),
        120
    );
    for page in &plan.pages {
        assert!(
            page.rows
                .iter()
                .map(|row| row.y + row.height)
                .fold(0.0, f64::max)
                <= DEFAULT_PAPER.height - SHEET_MARGIN + 1e-6
        );
    }
}

#[test]
fn paper_too_short_for_one_row_says_so() {
    let plan = plan_rows(DEFAULT_PAPER.width, 20.0, &full(), &[1.5], false, &[]);
    assert!(plan.pages.is_empty());
    assert!(plan.reason.contains("too short"));
}

#[test]
fn better_orientation_counts_rows_when_given_aspects() {
    let (width, height) = better_orientation(
        DEFAULT_PAPER.width,
        DEFAULT_PAPER.height,
        &full(),
        36,
        false,
        Some(&[1.5; 36]),
    );
    let rows = plan_rows(width, height, &full(), &[1.5; 36], false, &[]);
    let turned = plan_rows(height, width, &full(), &[1.5; 36], false, &[]);
    assert!(rows.pages.len() <= turned.pages.len());
}

#[test]
fn the_paper_helpers_match_ilford_sizes() {
    assert_eq!(snap_paper(242.5, 303.9, 2.0), (241.3, 304.8));
    assert_eq!(snap_paper(303.9, 242.5, 2.0), (304.8, 241.3));
    assert_eq!(snap_paper(250.4, 280.6, 2.0), (250.0, 281.0));
    assert_eq!(paper_preset(304.8, 241.3, 0.05), Some(DEFAULT_PAPER));
    assert_eq!(paper_preset(250.0, 281.0, 0.05), None);
    let (min_w, min_h) = min_paper_size(&film_geometry(SheetFormat::Medium, "6×17"), true);
    assert_eq!(
        clamp_paper(
            10.0,
            10.0,
            &film_geometry(SheetFormat::Medium, "6×17"),
            true
        ),
        (min_w, min_h)
    );
    assert_eq!(
        clamp_paper(9999.0, 9999.0, &full(), true),
        (PAPER_MAX, PAPER_MAX)
    );
}

#[test]
fn dpi_stays_inside_the_pixel_budget() {
    assert!(dpi_allowed(304.8, 406.4, 600));
    assert!(!dpi_allowed(406.4, 508.0, 600));
    assert_eq!(best_dpi(406.4, 508.0, 600), 300);
    assert_eq!(
        best_dpi(DEFAULT_PAPER.width, DEFAULT_PAPER.height, 600),
        600
    );
}

#[test]
fn settings_round_trip_and_fall_back() {
    let settings = ContactSheetSettings {
        paper_width: 250.0,
        paper_height: 300.0,
        dpi: 600,
        roll_label: false,
        edge_print: false,
        ..ContactSheetSettings::default()
    };
    assert_eq!(
        ContactSheetSettings::from_dict(Some(&settings.to_dict())),
        settings
    );
    assert!(ContactSheetSettings::from_dict(None) == ContactSheetSettings::default());
    let stored = serde_json::json!({"paper_width": "wide", "paper_height": 99999, "dpi": 72, "roll_label": 1, "cell_px": 600});
    let restored = ContactSheetSettings::from_dict(stored.as_object());
    assert_eq!(restored.paper_width, DEFAULT_PAPER.width);
    assert_eq!(restored.paper_height, PAPER_MAX);
    assert_eq!(restored.dpi, 300);
    assert!(restored.roll_label);
    let white = serde_json::json!({"white_paper": "yes", "film_edges": "no", "order": "size"});
    let restored = ContactSheetSettings::from_dict(white.as_object());
    assert!(!restored.white_paper && restored.film_edges && restored.order == "name");
    assert!(
        ILFORD_PAPERS
            .iter()
            .all(|paper| paper.width <= paper.height)
    );
}
