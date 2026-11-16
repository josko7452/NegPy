//! Pick Better proposals, route and temperature.

use negpy_backend::domain::models::{ExposureConfig, ProcessMode, WorkspaceConfig};
use negpy_backend::features::pickbetter::{
    DEFAULT_ROUTE, PICK_PARAMS, Phase, PickBetterProgress, apply_pick, current_pair,
    effective_order, is_finished, kelvin_to_wb, move_summary, param_value, reject_and_advance,
    set_param, start_pick_better, step_for, wb_to_kelvin,
};

fn config_with(mut change: impl FnMut(&mut ExposureConfig)) -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    change(&mut config.exposure);
    config
}

#[test]
fn the_first_round_is_bold_grade() {
    let progress = start_pick_better();
    assert_eq!(progress.phase, Phase::Fixed);
    assert_eq!(progress.param_index, 0);
    assert_eq!(progress.round_number, 1);
    assert_eq!(step_for(&PICK_PARAMS[0], 0), PICK_PARAMS[0].strong_step);
    assert_eq!(progress.param_order, DEFAULT_ROUTE);
    assert!(!progress.randomize);
    assert_eq!(progress.randomize_order, std::array::from_fn(|i| i));
}

#[test]
fn the_first_pair_is_center_plus_minus_step() {
    let config = WorkspaceConfig::default();
    let progress = start_pick_better();
    let (left, right) = current_pair(&config, &progress, 1.0);
    assert_eq!(left.exposure.grade, 135.0);
    assert_eq!(right.exposure.grade, 95.0);
}

#[test]
fn a_pick_commits_and_advances() {
    let config = WorkspaceConfig::default();
    let progress = start_pick_better();
    let (next, advanced) = apply_pick(&config, &progress, 0, 1.0);
    assert_eq!(next.exposure.grade, 135.0);
    assert_eq!(advanced.phase, Phase::Fixed);
    assert_eq!(advanced.param_index, 1);
    assert_eq!(advanced.round_number, 2);
    assert_eq!(advanced.selection_counts[0], 1);
}

#[test]
fn the_fixed_sequence_visits_each_parameter_once() {
    let mut config = WorkspaceConfig::default();
    let mut progress = start_pick_better();
    let mut seen = Vec::new();
    for _ in 0..PICK_PARAMS.len() {
        seen.push(effective_order(&progress)[progress.param_index]);
        (config, progress) = apply_pick(&config, &progress, 0, 1.0);
    }
    seen.sort_unstable();
    assert_eq!(seen, (0..PICK_PARAMS.len()).collect::<Vec<_>>());
    assert_eq!(progress.phase, Phase::Refine);
    assert_eq!(progress.param_index, 0);
    assert_eq!(progress.round_number, PICK_PARAMS.len() + 1);
}

#[test]
fn candidates_clamp_and_keep_the_pair_symmetric() {
    let config = config_with(|e| e.grade = 175.0);
    let progress = start_pick_better();
    let (left, right) = current_pair(&config, &progress, 1.0);
    assert_eq!(left.exposure.grade, 180.0);
    assert_eq!(right.exposure.grade, 155.0);

    let config = config_with(|e| e.highlight_density = -0.4);
    let position = DEFAULT_ROUTE.iter().position(|&index| index == 4).unwrap();
    let progress = PickBetterProgress {
        param_index: position,
        ..start_pick_better()
    };
    let (left, right) = current_pair(&config, &progress, 1.0);
    assert_eq!(left.exposure.highlight_density, -0.2);
    assert_eq!(right.exposure.highlight_density, -0.5);
}

#[test]
fn a_bound_nudges_the_candidate_onto_the_open_side() {
    let config = config_with(|e| e.grade = 180.0);
    let progress = start_pick_better();
    let (left, right) = current_pair(&config, &progress, 1.0);
    assert_eq!(left.exposure.grade, 170.0);
    assert_eq!(right.exposure.grade, 160.0);
}

#[test]
fn a_reject_keeps_the_config_and_does_not_count() {
    let config = config_with(|e| e.grade = 135.0);
    let progress = PickBetterProgress {
        selection_counts: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ..start_pick_better()
    };
    let (next, advanced) = reject_and_advance(&config, &progress);
    assert_eq!(next.exposure.grade, 135.0);
    assert_eq!(advanced.selection_counts, progress.selection_counts);
    assert_eq!(advanced.param_index, 1);
    assert_eq!(advanced.round_number, 2);
}

#[test]
fn a_reject_at_the_boundaries_moves_phase_and_wraps() {
    let config = WorkspaceConfig::default();
    let last_fixed = PickBetterProgress {
        param_index: 10,
        round_number: 11,
        ..start_pick_better()
    };
    let (_, refine) = reject_and_advance(&config, &last_fixed);
    assert_eq!(refine.phase, Phase::Refine);
    assert_eq!(refine.param_index, 0);
    let last_refine = PickBetterProgress {
        phase: Phase::Refine,
        param_index: 10,
        round_number: 12,
        ..start_pick_better()
    };
    let (_, wrapped) = reject_and_advance(&config, &last_refine);
    assert_eq!(wrapped.param_index, 0);
}

#[test]
fn selection_counts_shrink_the_step() {
    assert_eq!(step_for(&PICK_PARAMS[0], 0), 20.0);
    assert!((step_for(&PICK_PARAMS[0], 1) - 20.0 * 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(step_for(&PICK_PARAMS[0], 100), PICK_PARAMS[0].floor);
}

#[test]
fn the_scale_multiplies_the_spread_about_the_current() {
    let config = WorkspaceConfig::default();
    let progress = start_pick_better();
    let (left, right) = current_pair(&config, &progress, 2.0);
    assert_eq!(left.exposure.grade, 155.0);
    assert_eq!(right.exposure.grade, 75.0);
    for scale in [0.5, 1.0, 2.0, 4.0] {
        let config = config_with(|e| e.grade = 115.0);
        let (left, right) = current_pair(&config, &progress, scale);
        assert!(((left.exposure.grade + right.exposure.grade) / 2.0 - 115.0).abs() < 1e-12);
    }
}

#[test]
fn the_compound_temperature_reads_and_writes_kelvin() {
    let config = WorkspaceConfig::default();
    assert_eq!(param_value(&config, &PICK_PARAMS[10]), 5500.0);
    let next = set_param(&config, &PICK_PARAMS[10], 6000.0);
    assert_ne!(next.exposure.wb_magenta, 0.0);
    assert!((param_value(&next, &PICK_PARAMS[10]) - 6000.0).abs() < 1e-9);
    assert_eq!(config.exposure.wb_magenta, 0.0);
    let low = set_param(&config, &PICK_PARAMS[10], 2000.0);
    let high = set_param(&config, &PICK_PARAMS[10], 20000.0);
    assert!((param_value(&low, &PICK_PARAMS[10]) - 3000.0).abs() < 1e-9);
    assert!((param_value(&high, &PICK_PARAMS[10]) - 12000.0).abs() < 1e-9);
}

#[test]
fn a_slide_temperature_uses_the_cyan_yellow_pair() {
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::E6;
    config.process.positive_source = false;
    let next = set_param(&config, &PICK_PARAMS[10], 6000.0);
    assert_ne!(next.exposure.wb_cyan, 0.0);
    assert!((param_value(&next, &PICK_PARAMS[10]) - 6000.0).abs() < 1e-6);
}

#[test]
fn wb_and_kelvin_round_trip() {
    let (m, y) = kelvin_to_wb(6000.0, 0.0, 0.0);
    assert!((wb_to_kelvin(m, y) - 6000.0).abs() < 1e-9);
}

#[test]
fn a_move_summary_names_the_real_parameter_and_delta() {
    let old = WorkspaceConfig::default();
    let new = config_with(|e| e.grade = 135.0);
    let progress = start_pick_better();
    assert_eq!(move_summary(&old, &new, &progress), "ISO-R Grade +20");
    assert_eq!(move_summary(&new, &new, &progress), "ISO-R Grade");
    let mut order = start_pick_better();
    order.param_order[0] = 1;
    order.param_order[1] = 0;
    let new = config_with(|e| e.density = 1.3);
    assert_eq!(move_summary(&old, &new, &order), "Print Density +0.3");
}

#[test]
fn randomize_reads_its_own_order() {
    let mut progress = start_pick_better();
    progress.randomize = true;
    progress.randomize_order = [3, 2, 1, 0, 4, 5, 6, 7, 8, 9, 10];
    assert_eq!(effective_order(&progress), progress.randomize_order);
    progress.randomize = false;
    assert_eq!(effective_order(&progress), progress.param_order);
}

#[test]
fn is_finished_is_always_false() {
    assert!(!is_finished(&start_pick_better()));
    let refine = PickBetterProgress {
        phase: Phase::Refine,
        param_index: 5,
        round_number: 12,
        ..start_pick_better()
    };
    assert!(!is_finished(&refine));
}

#[test]
fn inputs_are_not_mutated() {
    let config = config_with(|e| e.grade = 135.0);
    let progress = PickBetterProgress {
        selection_counts: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ..start_pick_better()
    };
    let _ = apply_pick(&config, &progress, 0, 1.0);
    assert_eq!(config.exposure.grade, 135.0);
    assert_eq!(progress.selection_counts, [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}
