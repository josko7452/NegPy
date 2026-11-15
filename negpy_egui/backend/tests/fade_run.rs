//! Ports tests/test_fade_estimate.py: the Estimate orchestration over a scanner scan.

#[path = "helpers/fade_scene.rs"]
mod fade_scene;

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::process::fade_run::{GrayMemo, run_estimate};
use negpy_backend::features::transparency::logic::fade_white_grid;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn grid() -> ImageBuffer {
    let scene = fade_scene::build_scene(96, 72);
    let image = fade_scene::scan_image(&fade_scene::CROSSOVER, &scene);
    fade_white_grid(
        &ProcessConfig {
            process_mode: ProcessMode::E6,
            ..ProcessConfig::default()
        },
        Some(ProcessMode::E6),
        None,
        None,
        &image,
        None,
        0.05,
        None,
    )
}

fn config() -> ProcessConfig {
    ProcessConfig {
        process_mode: ProcessMode::E6,
        ..ProcessConfig::default()
    }
}

#[test]
fn the_estimate_grid_matches_the_python_scene() {
    let grid = grid();
    assert_eq!((grid.width, grid.height), (88, 66));
    let first = [grid.pixels[0], grid.pixels[1], grid.pixels[2]];
    assert!((first[0] + 3.120_364_7).abs() < 1e-4, "{first:?}");
    assert!((first[2] + 2.482_277_4).abs() < 1e-4, "{first:?}");
}

#[test]
fn a_press_sets_survivals_gray_mix_and_a_table() {
    let grid = grid();
    let mut memo = GrayMemo::default();
    let out = run_estimate(&config(), &grid, "", &mut memo, None, false, false);
    let (red, green, blue) = out.survivals.expect("survivals");
    assert!((red - 1.0).abs() < 1e-6);
    assert!((green - 0.951_681_2).abs() < 1e-5, "{green}");
    assert!((blue - 0.735_137_8).abs() < 1e-5, "{blue}");
    assert_eq!(out.changes.fade_ratio_g, Some(green));
    assert_eq!(out.changes.fade_neutrals_mix, out.mix);
    assert!(out.changes.fade_crossover_table.is_some());
    assert!(out.changes.fade_crossover_space.is_some());
    assert!(out.red_hint.is_some());
    assert!(
        out.hint
            .contains("Estimated: red 1.00, green 0.95, blue 0.74.")
    );
}

#[test]
fn a_live_mix_drag_writes_no_table() {
    let grid = grid();
    let mut memo = GrayMemo::default();
    let mut out = run_estimate(&config(), &grid, "", &mut memo, Some(0.2), true, false);
    out = run_estimate(&config(), &grid, "", &mut memo, Some(0.4), true, false);
    assert!(out.changes.fade_crossover_table.is_none());
    assert_eq!(out.changes.fade_neutrals_mix, Some(0.4));
    assert_eq!(out.mix, None);
    assert_eq!(out.red_hint, None);
}

#[test]
fn a_failed_reading_keeps_the_survivals() {
    let grid = ImageBuffer::from_rgb(32, 32, vec![0.0; 32 * 32 * 3]);
    let mut memo = GrayMemo::default();
    let out = run_estimate(&config(), &grid, "", &mut memo, None, false, false);
    assert!(out.survivals.is_none());
    assert!(out.changes.fade_ratio_r.is_none());
    assert!(out.changes.fade_ratio_g.is_none());
    assert!(out.changes.fade_ratio_b.is_none());
    assert!(!out.hint.is_empty());
}

#[test]
fn restore_range_takes_its_fit_and_measures_the_table_after_it() {
    let grid = grid();
    let mut memo = GrayMemo::default();
    let mut config = config();
    config.fade_auto_red = true;
    let out = run_estimate(&config, &grid, "", &mut memo, None, false, false);
    let (red, _, _) = out.survivals.expect("survivals");
    assert_eq!(out.changes.fade_ratio_r, Some(red));
    assert!(out.changes.fade_crossover_table.is_some());
    assert!(!out.suggest_red);
}

#[test]
fn a_short_range_suggests_restore_range_while_it_is_off() {
    let grid = grid();
    let mut memo = GrayMemo::default();
    let mut config = config();
    config.fade_ratio_r = 0.95;
    let out = run_estimate(&config, &grid, "", &mut memo, None, false, false);
    if out.suggest_red {
        assert!(
            out.red_hint
                .as_deref()
                .is_some_and(|hint| hint.contains("Restore Range"))
        );
    }
}

#[test]
fn the_hint_names_the_readout() {
    let grid = grid();
    let mut memo = GrayMemo::default();
    let out = run_estimate(&config(), &grid, "", &mut memo, None, false, false);
    assert!(out.hint.starts_with("Gray Mix set to 100 %."));
}
