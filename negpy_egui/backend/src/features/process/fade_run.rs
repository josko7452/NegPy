//! Fade Restoration's Estimate as its card runs it: the survivals, white, Gray Mix,
//! Restore Range and crossover table one press writes, and the hints that go with them.
//!
//! Mirrors `negpy/features/process/fade_estimate.py`.

use crate::domain::models::ProcessConfig;
use crate::features::process::fade::{
    CROSSOVER_FLOOR, crossover_amount, crossover_space, fade_density,
};
use crate::features::process::fade_auto_red::{AUTO_RED_HINT_RATIO, auto_red, survival_points};
use crate::features::process::fade_estimate::{
    FadeEstimate, GrayReadings, blend_gray_readings, estimate_fade_with, gray_readings,
    pick_neutrals, stain_suggested, survival_crossover, tone_bands_only, toward_no_change,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The gray readings of one Estimate grid, so a Gray Mix drag reads them once.
#[derive(Clone, Debug, Default)]
pub struct GrayMemo {
    grid: Option<ImageBuffer>,
    readings: Option<GrayReadings>,
}

impl GrayMemo {
    /// A reader on `grid` at Gray Mix `mix`.
    pub fn reader(
        &mut self,
        grid: &ImageBuffer,
        white_anchor: bool,
        space: &str,
        mix: f64,
    ) -> impl Fn(
        &ImageBuffer,
    ) -> (
        Option<crate::features::process::fade_estimate::NeutralAxisRefs>,
        String,
    ) + '_ {
        let readings = if self.grid.as_ref() != Some(grid) {
            self.grid = Some(grid.clone());
            self.readings = Some(gray_readings(grid));
            self.readings.clone()
        } else {
            self.readings.clone()
        };
        let readings = readings.unwrap_or_default();
        let _ = (white_anchor, space);
        move |_grid: &ImageBuffer| blend_gray_readings(&readings, mix)
    }

    /// The readings last taken on `grid`.
    #[must_use]
    pub fn readings(&self, grid: &ImageBuffer) -> Option<&GrayReadings> {
        (self.grid.as_ref() == Some(grid)).then_some(self.readings.as_ref())?
    }

    /// Whether the last reading on `grid` found grays only in the tone bands.
    #[must_use]
    pub fn tone_bands_only(&self, grid: &ImageBuffer) -> bool {
        self.readings(grid).is_some_and(tone_bands_only)
    }
}

/// The changes to write into the roll's Fade card.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FadeChanges {
    pub fade_ratio_r: Option<f64>,
    pub fade_ratio_g: Option<f64>,
    pub fade_ratio_b: Option<f64>,
    pub fade_density_shift: Option<f64>,
    pub fade_white: Option<[f64; 3]>,
    pub fade_white_space: Option<String>,
    pub fade_neutrals_mix: Option<f64>,
    pub fade_crossover_table: Option<Vec<f64>>,
    pub fade_crossover_space: Option<String>,
}

/// What one Estimate writes and shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EstimateOutcome {
    pub hint: String,
    pub stain: String,
    pub changes: FadeChanges,
    pub fade_white: Option<[f64; 3]>,
    pub fade_white_space: String,
    pub survivals: Option<(f64, f64, f64)>,
    pub mix: Option<f64>,
    pub tone_bands: bool,
    pub red_hint: Option<String>,
    pub suggest_red: bool,
}

/// The result, or why there is none.
#[must_use]
pub fn estimate_hint(chosen: &FadeEstimate) -> String {
    if !chosen.reason.is_empty() && chosen.green == 1.0 && chosen.blue == 1.0 {
        return chosen.reason.clone();
    }
    let red = chosen.red.unwrap_or(1.0);
    let text = format!(
        "Estimated: red {red:.2}, green {:.2}, blue {:.2}.",
        chosen.green, chosen.blue
    );
    if chosen.reason.is_empty() {
        text
    } else {
        format!("{} {}", chosen.reason, text)
    }
}

fn cells(grid: &ImageBuffer) -> Vec<[f32; 3]> {
    grid.pixels
        .chunks_exact(3)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

/// `auto_red_changes`: the survivals Restore Range sets, and its shift.
#[must_use]
pub fn auto_red_changes(
    config: &ProcessConfig,
    space: &str,
    grid: &ImageBuffer,
) -> Option<(f64, f64)> {
    let green = config.fade_ratio_g;
    let blue = config.fade_ratio_b;
    let shift = config.fade_density_shift;
    let anchor = config.fade_white_anchor;
    let camera_raw = !space.is_empty();
    let points = |red: f64| {
        survival_points(
            &cells(grid),
            config,
            [red, green, blue],
            anchor,
            camera_raw,
            space,
            shift,
        )
    };
    auto_red(points, config.fade_ratio_r, shift).map(|fit| (fit.red, fit.shift))
}

/// Whether Restore Range would set Red Survival at `AUTO_RED_HINT_RATIO` of the current one.
#[must_use]
pub fn short_range(config: &ProcessConfig, space: &str, grid: &ImageBuffer) -> bool {
    auto_red_changes(config, space, grid)
        .is_some_and(|(red, _)| red <= config.fade_ratio_r * AUTO_RED_HINT_RATIO)
}

/// The crossover table and its tag after `new`'s survivals.
fn measured_crossover(
    config: &ProcessConfig,
    new: &ProcessConfig,
    space: &str,
    grid: &ImageBuffer,
    memo: &mut GrayMemo,
    mix: f64,
) -> Option<(Vec<f64>, String)> {
    let anchor = config.fade_white_anchor;
    let rgb = [new.fade_ratio_r, new.fade_ratio_g, new.fade_ratio_b];
    let table = survival_crossover(
        grid,
        memo.reader(grid, anchor, space, mix),
        rgb,
        anchor,
        !space.is_empty(),
        space,
        new.fade_density_shift,
    )?;
    let tag = crossover_space(&fade_density(new, space), space, new);
    Some((table, tag))
}

fn with_survivals(config: &ProcessConfig, values: &FadeChanges) -> ProcessConfig {
    let mut out = config.clone();
    if let Some(value) = values.fade_ratio_r {
        out.fade_ratio_r = value;
    }
    if let Some(value) = values.fade_ratio_g {
        out.fade_ratio_g = value;
    }
    if let Some(value) = values.fade_ratio_b {
        out.fade_ratio_b = value;
    }
    if let Some(value) = values.fade_density_shift {
        out.fade_density_shift = value;
    }
    if let Some(value) = values.fade_neutrals_mix {
        out.fade_neutrals_mix = value;
    }
    out
}

/// Estimate on `grid` with Gray Mix `mix`; `live` is a Gray Mix drag.
#[must_use]
pub fn run_estimate(
    config: &ProcessConfig,
    grid: &ImageBuffer,
    space: &str,
    memo: &mut GrayMemo,
    mix: Option<f64>,
    live: bool,
    keep_table: bool,
) -> EstimateOutcome {
    let mix_changes = mix.is_some();
    let mix_given = mix;
    let mix = mix.unwrap_or(config.fade_neutrals_mix);
    let anchor = config.fade_white_anchor;
    let camera_raw = !space.is_empty();
    let pick = !live && !mix_changes;
    let mut chosen = estimate_fade_with(
        grid,
        memo.reader(grid, anchor, space, mix),
        anchor,
        camera_raw,
        space,
        !live && !pick && !keep_table,
    );
    let tone_bands = memo.tone_bands_only(grid);
    let mut picked = None;
    let mut slider_mix = None;
    if pick && chosen.red.is_some() {
        let readings = memo.readings(grid).cloned();
        let found = if tone_bands {
            None
        } else {
            readings.and_then(|readings| pick_neutrals(grid, &readings, anchor, camera_raw, space))
        };
        match found {
            Some((found_mix, found_estimate)) => {
                picked = Some(found_mix);
                slider_mix = Some(found_mix);
                chosen = found_estimate;
            }
            None => {
                chosen = estimate_fade_with(
                    grid,
                    memo.reader(grid, anchor, space, mix),
                    anchor,
                    camera_raw,
                    space,
                    true,
                );
            }
        }
    }
    let mut mix_out = None;
    if tone_bands {
        if !mix_changes {
            slider_mix = Some(1.0);
            mix_out = Some(1.0);
        }
        chosen = toward_no_change(chosen, mix);
    }
    let failed = !chosen.reason.is_empty() && chosen.green == 1.0 && chosen.blue == 1.0;
    let mut changes = FadeChanges::default();
    let mut fade_white = None;
    let mut fade_white_space = String::new();
    if (anchor || camera_raw)
        && chosen.white.is_some()
        && (!failed || camera_raw || stain_suggested(chosen.white.unwrap()))
    {
        let white = chosen.white.unwrap();
        if !camera_raw {
            changes.fade_white = Some(white);
            changes.fade_white_space = Some(space.to_string());
        }
        fade_white = Some(white);
        fade_white_space = space.to_string();
    }
    if let Some(found) = picked {
        changes.fade_neutrals_mix = Some(found);
        mix_out = Some(found);
    }
    let mut stain = String::new();
    if !anchor && chosen.white.is_some_and(|white| stain_suggested(white)) {
        stain = "The clear areas have a yellow stain: turn on Remove Stain.".to_string();
    }
    let mut hint = estimate_hint(&chosen);
    if let Some(found) = picked {
        hint = format!("Gray Mix set to {:.0} %. {}", found * 100.0, hint);
    }
    if tone_bands {
        hint = format!(
            "This frame has few clear grays. Drag Gray Mix to set how much Estimate changes. {}",
            hint
        );
    }
    if anchor
        && fade_white.is_none()
        && (
            config.fade_white_r,
            config.fade_white_g,
            config.fade_white_b,
        ) == (0.0, 0.0, 0.0)
    {
        hint.push_str(" No stain found to remove.");
    }
    if failed {
        return EstimateOutcome {
            hint,
            stain,
            changes,
            fade_white: if camera_raw { None } else { fade_white },
            fade_white_space,
            survivals: None,
            mix: slider_mix,
            tone_bands,
            red_hint: None,
            suggest_red: false,
        };
    }
    let red = chosen.red.unwrap_or(config.fade_ratio_r);
    let mut survivals = FadeChanges {
        fade_ratio_r: Some(red),
        fade_ratio_g: Some(chosen.green),
        fade_ratio_b: Some(chosen.blue),
        fade_density_shift: Some(0.0),
        fade_neutrals_mix: mix_given.or(mix_out),
        ..FadeChanges::default()
    };
    let trial = with_survivals(config, &survivals);
    let short = !live && !config.fade_auto_red && short_range(&trial, space, grid);
    let mut auto = None;
    if config.fade_auto_red && live {
        survivals.fade_ratio_r = Some(config.fade_ratio_r);
        survivals.fade_density_shift = Some(config.fade_density_shift);
    } else if config.fade_auto_red {
        auto = auto_red_changes(&trial, space, grid);
    }
    if let Some((auto_red_value, auto_shift)) = auto {
        survivals.fade_ratio_r = Some(auto_red_value);
        survivals.fade_density_shift = Some(auto_shift);
    }
    let measured =
        if !keep_table && (auto.is_some() || (tone_bands && chosen.crossover.is_none())) && !live {
            measured_crossover(config, &trial, space, grid, memo, mix)
        } else {
            None
        };
    if !keep_table && measured.is_none() && chosen.crossover.is_some() {
        let table_config = with_survivals(config, &survivals);
        survivals.fade_crossover_table = chosen.crossover.clone();
        survivals.fade_crossover_space = Some(crossover_space(
            &fade_density(&table_config, space),
            space,
            &table_config,
        ));
    }
    let mut red_hint = None;
    let mut suggest_red = false;
    if !live {
        let table = measured
            .as_ref()
            .map(|(table, _)| table.as_slice())
            .or(chosen.crossover.as_deref());
        if config.fade_crossover == 0.0 && crossover_amount(table) >= CROSSOVER_FLOOR {
            hint.push_str(" The colors shift from shadows to highlights: raise Crossover.");
        }
        suggest_red = short && !config.fade_auto_red;
        red_hint = Some(if suggest_red {
            "This frame has a short range: turn on Restore Range.".to_string()
        } else {
            String::new()
        });
    }
    if let Some((table, _)) = measured {
        survivals.fade_crossover_table = Some(table);
    }
    let white = changes.fade_white;
    let white_space = changes.fade_white_space.clone();
    let mix_value = changes.fade_neutrals_mix;
    changes = survivals;
    if white.is_some() {
        changes.fade_white = white;
        changes.fade_white_space = white_space;
    }
    changes.fade_neutrals_mix = changes.fade_neutrals_mix.or(mix_value);
    EstimateOutcome {
        hint,
        stain,
        changes,
        fade_white,
        fade_white_space,
        survivals: Some((red, chosen.green, chosen.blue)),
        mix: slider_mix,
        tone_bands,
        red_hint,
        suggest_red,
    }
}
