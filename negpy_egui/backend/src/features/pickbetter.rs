//! Pick Better: the eleven Exposure controls proposed as A/B pairs.
//!
//! Mirrors `negpy/features/pickbetter/`. The pair is pure config math; the two
//! renders and their selection remain the caller's job.

use crate::domain::models::{ExposureConfig, WorkspaceConfig};
use crate::features::process::path::{RenderPath, render_path};
use crate::features::transparency::logic::{slide_kelvin_to_wb, slide_wb_to_kelvin};

pub const TEMP_REF_KELVIN: f64 = 5500.0;
pub const TEMP_MIN_KELVIN: f64 = 3000.0;
pub const TEMP_MAX_KELVIN: f64 = 12000.0;
const TEMP_K_MAGENTA: f64 = 0.0029;
const TEMP_K_YELLOW: f64 = 0.0057;

/// Which iteration phase the proposal is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Fixed,
    Refine,
}

/// One tunable control the picker proposes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickParam {
    pub field: &'static str,
    pub name: &'static str,
    pub strong_step: f64,
    pub floor: f64,
    pub lo: f64,
    pub hi: f64,
    pub compound: bool,
}

/// The eleven controls, by parameter index.
pub const PICK_PARAMS: [PickParam; 11] = [
    PickParam {
        field: "grade",
        name: "ISO-R Grade",
        strong_step: 20.0,
        floor: 1.0,
        lo: 50.0,
        hi: 180.0,
        compound: false,
    },
    PickParam {
        field: "density",
        name: "Print Density",
        strong_step: 0.3,
        floor: 0.01,
        lo: 0.0,
        hi: 2.0,
        compound: false,
    },
    PickParam {
        field: "highlight_grade",
        name: "Highlights Grade",
        strong_step: 15.0,
        floor: 1.0,
        lo: -50.0,
        hi: 50.0,
        compound: false,
    },
    PickParam {
        field: "shadow_grade",
        name: "Shadows Grade",
        strong_step: 15.0,
        floor: 1.0,
        lo: -50.0,
        hi: 50.0,
        compound: false,
    },
    PickParam {
        field: "highlight_density",
        name: "Highlights Density",
        strong_step: 0.2,
        floor: 0.01,
        lo: -0.5,
        hi: 0.5,
        compound: false,
    },
    PickParam {
        field: "shadow_density",
        name: "Shadows Density",
        strong_step: 0.3,
        floor: 0.02,
        lo: -0.9,
        hi: 0.9,
        compound: false,
    },
    PickParam {
        field: "preflash",
        name: "Preflash",
        strong_step: 0.1,
        floor: 0.01,
        lo: 0.0,
        hi: 1.0,
        compound: false,
    },
    PickParam {
        field: "dye_separation",
        name: "Dye Separation",
        strong_step: 0.1,
        floor: 0.01,
        lo: 0.5,
        hi: 1.5,
        compound: false,
    },
    PickParam {
        field: "midtone_gamma",
        name: "Snap",
        strong_step: 0.05,
        floor: 0.005,
        lo: -0.5,
        hi: 0.5,
        compound: false,
    },
    PickParam {
        field: "toe",
        name: "Toe",
        strong_step: 0.1,
        floor: 0.01,
        lo: -1.0,
        hi: 1.0,
        compound: false,
    },
    PickParam {
        field: "temperature",
        name: "Temperature",
        strong_step: 500.0,
        floor: 50.0,
        lo: 3000.0,
        hi: 12000.0,
        compound: true,
    },
];

/// The initial route: the two main controls, Temperature, Dye Separation, then the rest.
pub const DEFAULT_ROUTE: [usize; 11] = [0, 1, 10, 7, 2, 3, 4, 5, 6, 8, 9];

/// The iteration cursor the caller advances after each pick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickBetterProgress {
    pub phase: Phase,
    pub param_index: usize,
    pub round_number: usize,
    pub selection_counts: [usize; 11],
    pub param_order: [usize; 11],
    pub randomize: bool,
    pub randomize_order: [usize; 11],
}

/// `wb_to_kelvin`: the print temperature of a magenta/yellow pair.
#[must_use]
pub fn wb_to_kelvin(magenta: f64, yellow: f64) -> f64 {
    let dmu = (TEMP_K_MAGENTA * magenta + TEMP_K_YELLOW * yellow)
        / (TEMP_K_MAGENTA * TEMP_K_MAGENTA + TEMP_K_YELLOW * TEMP_K_YELLOW);
    let mu = (1e6 / TEMP_REF_KELVIN + dmu)
        .max(1e6 / TEMP_MAX_KELVIN)
        .min(1e6 / TEMP_MIN_KELVIN);
    1e6 / mu
}

/// `kelvin_to_wb`: move the magenta/yellow pair along the Planckian direction.
#[must_use]
pub fn kelvin_to_wb(kelvin: f64, magenta: f64, yellow: f64) -> (f64, f64) {
    let kelvin = kelvin.clamp(TEMP_MIN_KELVIN, TEMP_MAX_KELVIN);
    let dmu_cur = (TEMP_K_MAGENTA * magenta + TEMP_K_YELLOW * yellow)
        / (TEMP_K_MAGENTA * TEMP_K_MAGENTA + TEMP_K_YELLOW * TEMP_K_YELLOW);
    let d = (1e6 / kelvin - 1e6 / TEMP_REF_KELVIN) - dmu_cur;
    (
        (magenta + TEMP_K_MAGENTA * d).clamp(-1.0, 1.0),
        (yellow + TEMP_K_YELLOW * d).clamp(-1.0, 1.0),
    )
}

/// The order the rounds actually visit.
#[must_use]
pub fn effective_order(progress: &PickBetterProgress) -> [usize; 11] {
    if progress.randomize {
        progress.randomize_order
    } else {
        progress.param_order
    }
}

fn clamp(value: f64, param: &PickParam) -> f64 {
    value.max(param.lo).min(param.hi)
}

/// A control's current value; Temperature reads as Kelvin.
#[must_use]
pub fn param_value(config: &WorkspaceConfig, param: &PickParam) -> f64 {
    if param.compound {
        if render_path(&config.process) != RenderPath::Print {
            return slide_wb_to_kelvin(config.exposure.wb_cyan, config.exposure.wb_yellow);
        }
        return wb_to_kelvin(config.exposure.wb_magenta, config.exposure.wb_yellow);
    }
    field_value(&config.exposure, param.field)
}

fn field_value(exposure: &ExposureConfig, field: &str) -> f64 {
    match field {
        "grade" => exposure.grade,
        "density" => exposure.density,
        "highlight_grade" => exposure.highlight_grade,
        "shadow_grade" => exposure.shadow_grade,
        "highlight_density" => exposure.highlight_density,
        "shadow_density" => exposure.shadow_density,
        "preflash" => exposure.preflash,
        "dye_separation" => exposure.dye_separation,
        "midtone_gamma" => exposure.midtone_gamma,
        "toe" => exposure.toe,
        _ => 0.0,
    }
}

fn set_field(exposure: &mut ExposureConfig, field: &str, value: f64) {
    match field {
        "grade" => exposure.grade = value,
        "density" => exposure.density = value,
        "highlight_grade" => exposure.highlight_grade = value,
        "shadow_grade" => exposure.shadow_grade = value,
        "highlight_density" => exposure.highlight_density = value,
        "shadow_density" => exposure.shadow_density = value,
        "preflash" => exposure.preflash = value,
        "dye_separation" => exposure.dye_separation = value,
        "midtone_gamma" => exposure.midtone_gamma = value,
        "toe" => exposure.toe = value,
        _ => {}
    }
}

/// A config with one control set, clamped to its range.
#[must_use]
pub fn set_param(config: &WorkspaceConfig, param: &PickParam, value: f64) -> WorkspaceConfig {
    let value = clamp(value, param);
    let mut next = config.clone();
    if param.compound {
        if render_path(&next.process) != RenderPath::Print {
            let (cyan, yellow) =
                slide_kelvin_to_wb(value, next.exposure.wb_cyan, next.exposure.wb_yellow);
            next.exposure.wb_cyan = cyan;
            next.exposure.wb_yellow = yellow;
        } else {
            let (magenta, yellow) =
                kelvin_to_wb(value, next.exposure.wb_magenta, next.exposure.wb_yellow);
            next.exposure.wb_magenta = magenta;
            next.exposure.wb_yellow = yellow;
        }
    } else {
        set_field(&mut next.exposure, param.field, value);
    }
    next
}

/// The step a round proposes.
#[must_use]
pub fn step_for(param: &PickParam, selections: usize) -> f64 {
    (param.strong_step * (2.0f64 / 3.0).powi(selections as i32)).max(param.floor)
}

/// The first round: bold Grade, fixed phase, default route.
#[must_use]
pub fn start_pick_better() -> PickBetterProgress {
    PickBetterProgress {
        phase: Phase::Fixed,
        param_index: 0,
        round_number: 1,
        selection_counts: [0; 11],
        param_order: DEFAULT_ROUTE,
        randomize: false,
        randomize_order: std::array::from_fn(|index| index),
    }
}

/// The two candidates to render, differing in one Exposure control.
#[must_use]
pub fn current_pair(
    config: &WorkspaceConfig,
    progress: &PickBetterProgress,
    scale: f64,
) -> (WorkspaceConfig, WorkspaceConfig) {
    let real = effective_order(progress)[progress.param_index];
    let param = &PICK_PARAMS[real];
    let current = param_value(config, param);
    let step = step_for(param, progress.selection_counts[real]) * scale;
    let mut left_val = clamp(current + step, param);
    let mut right_val = clamp(current - step, param);
    if left_val == current {
        left_val = clamp(current - step / 2.0, param);
    }
    if right_val == current {
        right_val = clamp(current + step / 2.0, param);
    }
    (
        set_param(config, param, left_val),
        set_param(config, param, right_val),
    )
}

/// Commit one side and advance the round.
#[must_use]
pub fn apply_pick(
    config: &WorkspaceConfig,
    progress: &PickBetterProgress,
    side: usize,
    scale: f64,
) -> (WorkspaceConfig, PickBetterProgress) {
    let (left, right) = current_pair(config, progress, scale);
    let chosen = if side == 0 { left } else { right };
    let real = effective_order(progress)[progress.param_index];
    let mut counts = progress.selection_counts;
    counts[real] += 1;
    (chosen, advance(progress, counts))
}

/// Reject both candidates and advance without counting a selection.
#[must_use]
pub fn reject_and_advance(
    config: &WorkspaceConfig,
    progress: &PickBetterProgress,
) -> (WorkspaceConfig, PickBetterProgress) {
    let (phase, param_index) = match progress.phase {
        Phase::Fixed if progress.param_index + 1 < PICK_PARAMS.len() => {
            (Phase::Fixed, progress.param_index + 1)
        }
        Phase::Fixed => (Phase::Refine, 0),
        Phase::Refine => (
            Phase::Refine,
            (progress.param_index + 1) % PICK_PARAMS.len(),
        ),
    };
    (
        config.clone(),
        carry(progress, progress.selection_counts, phase, param_index),
    )
}

fn advance(progress: &PickBetterProgress, counts: [usize; 11]) -> PickBetterProgress {
    match progress.phase {
        Phase::Fixed if progress.param_index + 1 < PICK_PARAMS.len() => {
            carry(progress, counts, Phase::Fixed, progress.param_index + 1)
        }
        Phase::Fixed => carry(progress, counts, Phase::Refine, 0),
        Phase::Refine => carry(
            progress,
            counts,
            Phase::Refine,
            (progress.param_index + 1) % PICK_PARAMS.len(),
        ),
    }
}

fn carry(
    progress: &PickBetterProgress,
    counts: [usize; 11],
    phase: Phase,
    param_index: usize,
) -> PickBetterProgress {
    PickBetterProgress {
        phase,
        param_index,
        round_number: progress.round_number + 1,
        selection_counts: counts,
        param_order: progress.param_order,
        randomize: progress.randomize,
        randomize_order: progress.randomize_order,
    }
}

/// The refinement is endless; the user ends it.
#[must_use]
pub fn is_finished(_progress: &PickBetterProgress) -> bool {
    false
}

/// The "Name +delta" line shown after a pick.
#[must_use]
pub fn move_summary(
    old: &WorkspaceConfig,
    new: &WorkspaceConfig,
    progress: &PickBetterProgress,
) -> String {
    let real = effective_order(progress)[progress.param_index];
    let param = &PICK_PARAMS[real];
    let delta = param_value(new, param) - param_value(old, param);
    if delta.abs() < 1e-9 {
        return param.name.to_string();
    }
    let text = format!("{delta:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    let sign = if delta >= 0.0 { "+" } else { "" };
    format!("{} {sign}{text}", param.name)
}
