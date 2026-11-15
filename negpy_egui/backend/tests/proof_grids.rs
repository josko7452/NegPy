//! Ports the strip, ring, paper and proof-grid layout rules in
//! `negpy/features/exposure/analysis.py`.

use negpy_backend::features::exposure::analysis::{
    PAPER_GRID_MAX_RATIO, RING_CC_PER_UNIT, RING_GRID, STRIP_DENSITIES, STRIP_GRADES, STRIP_GRID,
    paper_grid, paper_grid_cells, paper_grid_mosaic, proof_grid, ring_cc, ring_cells,
    ring_nearest_cell, ring_rungs, rotate_grid, rotated_cell, rotation_index, strip_bound,
    strip_cell_at, strip_cells, strip_center, strip_mosaic, strip_nearest_cell, strip_patch_rect,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

#[test]
fn the_strip_ladders_are_centered_on_the_defaults() {
    assert_eq!(strip_center(), (1.0, 115.0));
    assert_eq!(STRIP_GRID, (5, 5));
    assert_eq!(STRIP_GRADES[0], 75.0);
    assert_eq!(STRIP_DENSITIES[4], 1.6);
}

#[test]
fn the_strip_cells_run_row_major_by_grade() {
    let cells = strip_cells();
    assert_eq!(cells.len(), 25);
    assert_eq!(cells[0], (0, 0, 0.4, 75.0));
    assert_eq!(cells[6], (1, 1, 0.7, 95.0));
    assert_eq!(cells[24], (4, 4, 1.6, 155.0));
}

#[test]
fn the_ring_rungs_are_absolute_and_centered() {
    let rungs = ring_rungs();
    assert_eq!(rungs, vec![-0.2, -0.1, 0.0, 0.1, 0.2]);
    assert_eq!(ring_cc(2), 0.0);
    assert_eq!(ring_cc(4), 0.2 * RING_CC_PER_UNIT);
    assert_eq!(RING_GRID, (5, 5));
}

#[test]
fn the_ring_cells_step_magenta_by_row_and_yellow_by_column() {
    let cells = ring_cells();
    assert_eq!(cells[0], (0, 0, -0.2, -0.2));
    assert_eq!(cells[12], (2, 2, 0.0, 0.0));
    assert_eq!(cells[24], (4, 4, 0.2, 0.2));
    assert_eq!(ring_nearest_cell(0.03, -0.19), (2, 0));
}

#[test]
fn a_patch_boundary_rounds_half_to_even() {
    assert_eq!(strip_bound(100, 3, 1), 33);
    assert_eq!(strip_bound(100, 3, 2), 67);
    assert_eq!(strip_bound(90, 3, 1), 30);
    // 0.5 rounds to the even side, as Python's round does.
    assert_eq!(strip_bound(1, 2, 1), 0);
    assert_eq!(strip_bound(3, 2, 1), 2);
}

#[test]
fn a_patch_rect_is_the_cell_the_mosaic_fills() {
    assert_eq!(strip_patch_rect(100, 100, 0, 0, (2, 2)), (0, 0, 50, 50));
    assert_eq!(strip_patch_rect(100, 100, 1, 1, (2, 2)), (50, 50, 100, 100));
}

#[test]
fn the_mosaic_keeps_each_tiles_own_patch() {
    let tile = |value: f32| ImageBuffer::from_rgb(4, 4, vec![value; 48]);
    let tiles: Vec<ImageBuffer> = (0..4).map(|index| tile(index as f32)).collect();
    let out = strip_mosaic(&tiles, (2, 2)).expect("mosaic");
    let at = |x: usize, y: usize| out.pixels[(y * 4 + x) * 3];
    assert_eq!(at(0, 0), 0.0);
    assert_eq!(at(3, 0), 1.0);
    assert_eq!(at(0, 3), 2.0);
    assert_eq!(at(3, 3), 3.0);
}

#[test]
fn a_mosaic_of_the_wrong_tile_count_is_refused() {
    let tiles = vec![ImageBuffer::from_rgb(4, 4, vec![0.0; 48])];
    assert!(strip_mosaic(&tiles, (2, 2)).is_none());
}

#[test]
fn a_content_position_selects_its_cell() {
    assert_eq!(strip_cell_at(0.0, 0.0, (5, 5)), (0, 0));
    assert_eq!(strip_cell_at(0.5, 0.5, (5, 5)), (2, 2));
    // A position at the far edge stays inside the grid.
    assert_eq!(strip_cell_at(1.0, 1.0, (5, 5)), (4, 4));
}

#[test]
fn the_nearest_strip_cell_tracks_the_settings_in_force() {
    assert_eq!(strip_nearest_cell(1.0, 115.0), (2, 2));
    assert_eq!(strip_nearest_cell(0.45, 80.0), (0, 0));
    assert_eq!(strip_nearest_cell(1.55, 150.0), (4, 4));
}

#[test]
fn a_quarter_turn_swaps_the_grid_sides() {
    assert_eq!(proof_grid((3, 5), 0), (3, 5));
    assert_eq!(proof_grid((3, 5), 1), (5, 3));
    assert_eq!(proof_grid((3, 5), 2), (3, 5));
    assert_eq!(proof_grid((3, 5), 3), (5, 3));
}

#[test]
fn a_quarter_turn_rotates_counter_clockwise() {
    // np.rot90 of a 2x3 grid: [[0,1,2],[3,4,5]] becomes [[2,5],[1,4],[0,3]].
    assert_eq!(rotation_index((2, 3), 1), vec![2, 5, 1, 4, 0, 3]);
    assert_eq!(
        rotate_grid(&[0, 1, 2, 3, 4, 5], (2, 3), 1),
        vec![2, 5, 1, 4, 0, 3]
    );
    assert_eq!(rotation_index((2, 3), 4), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn a_rotated_cell_lands_where_the_rotation_puts_it() {
    // The rotated grid is 3 rows by 2 columns.
    assert_eq!(rotated_cell((0, 0), (2, 3), 1), Some((2, 0)));
    assert_eq!(rotated_cell((1, 2), (2, 3), 1), Some((0, 1)));
    assert_eq!(rotated_cell((0, 0), (2, 3), 0), Some((0, 0)));
}

#[test]
fn an_even_paper_count_takes_the_nearest_square_grid() {
    assert_eq!(paper_grid(4), (2, 2));
    assert_eq!(paper_grid(6), (2, 3));
    assert_eq!(paper_grid(9), (3, 3));
    assert_eq!(paper_grid(1), (1, 1));
}

#[test]
fn an_uneven_paper_count_takes_a_near_square_grid() {
    let (rows, cols) = paper_grid(7);
    assert_eq!((rows, cols), (3, 3));
    let (rows, cols) = paper_grid(5);
    assert_eq!((rows, cols), (2, 3));
    assert!(cols <= PAPER_GRID_MAX_RATIO * rows);
}

#[test]
fn the_paper_cells_cover_the_frame_and_center_a_short_row() {
    let cells = paper_grid_cells(4);
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[0], (0.0, 0.0, 0.5, 0.5));
    assert_eq!(cells[3], (0.5, 0.5, 1.0, 1.0));
    let short = paper_grid_cells(2);
    assert_eq!(short[0], (0.0, 0.25, 0.5, 0.75));
}

#[test]
fn the_paper_mosaic_scales_each_tile_into_its_cell() {
    let tile = |value: f32| ImageBuffer::from_rgb(8, 8, vec![value; 8 * 8 * 3]);
    let tiles = vec![tile(0.25), tile(0.75), tile(0.5), tile(1.0)];
    let out = paper_grid_mosaic(&tiles).expect("mosaic");
    assert_eq!((out.width, out.height), (8, 8));
    let at = |x: usize, y: usize| out.pixels[(y * 8 + x) * 3];
    assert_eq!(at(1, 1), 0.25);
    assert_eq!(at(6, 1), 0.75);
    assert_eq!(at(1, 6), 0.5);
    assert_eq!(at(6, 6), 1.0);
}

#[test]
fn the_ring_axis_labels_are_cc() {
    assert_eq!(RING_CC_PER_UNIT * 0.1, 2.0);
}
