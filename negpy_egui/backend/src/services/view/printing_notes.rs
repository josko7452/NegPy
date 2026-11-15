//! Printer's notes: the print recipe and the dodge/burn map as a darkroom printer writes them.
//!
//! Mirrors `negpy/services/view/printing_notes.py`. Stops are in the exposure domain: a
//! burn adds exposure and reads `+`, a dodge withholds it and reads `−`.

use crate::domain::models::{
    ExposureConfig, FinishConfig, LocalAdjustmentsConfig, LocalMask, MaskKey,
};
use crate::features::exposure::constants::{ISO_R_MAX, ISO_R_MIN};
use crate::features::exposure::densitometer::zone_roman;
use crate::features::exposure::papers::resolve_paper;
use crate::features::local::logic::limited_indices;

const FRACTIONS: [(f64, &str); 5] = [
    (0.25, "¼"),
    (1.0 / 3.0, "⅓"),
    (0.5, "½"),
    (2.0 / 3.0, "⅔"),
    (0.75, "¾"),
];
const FRACTION_TOLERANCE: f64 = 0.02;

/// Exposure difference in stops, darkroom-signed.
#[must_use]
pub fn stops_label(stops: f64) -> String {
    let magnitude = stops.abs();
    if magnitude < 0.005 {
        return "0".to_string();
    }
    let sign = if stops < 0.0 { "−" } else { "+" };
    let whole = magnitude.trunc();
    let fraction = magnitude - whole;
    for (value, glyph) in FRACTIONS {
        if (fraction - value).abs() <= FRACTION_TOLERANCE {
            let whole = if whole == 0.0 {
                String::new()
            } else {
                format!("{whole:.0}")
            };
            return format!("{sign}{whole}{glyph}");
        }
    }
    if fraction <= FRACTION_TOLERANCE {
        return format!("{sign}{whole:.0}");
    }
    format!("{sign}{magnitude:.2}")
}

/// The grade a mask prints at, as `"R95"`; empty when it prints at the frame's grade.
#[must_use]
pub fn local_grade_label(grade: f64, delta: f64) -> String {
    if delta == 0.0 || grade <= 0.0 {
        return String::new();
    }
    let value = (grade + delta).clamp(ISO_R_MIN, ISO_R_MAX);
    format!("R{value:.0}")
}

/// The tones a limited mask acts on, as `"≥VI"`; empty when it acts on all.
#[must_use]
pub fn tone_limit_label(mask: &LocalMask) -> String {
    match mask.key {
        MaskKey::Off => String::new(),
        MaskKey::Highlights => format!("≥{}", zone_roman(mask.key_zone)),
        MaskKey::Shadows => format!("≤{}", zone_roman(mask.key_zone)),
    }
}

/// One enabled mask's printing note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaskNote {
    pub number: usize,
    pub is_burn: bool,
    pub stops: String,
    pub local_r: String,
    pub key: String,
    pub flash: String,
}

impl MaskNote {
    /// Burn, Dodge, Grade or Flash.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        if self.stops == "0" {
            if !self.flash.is_empty() && self.local_r.is_empty() {
                "Flash"
            } else {
                "Grade"
            }
        } else if self.is_burn {
            "Burn"
        } else {
            "Dodge"
        }
    }

    /// Short form for the map drawn on the print.
    #[must_use]
    pub fn badge(&self) -> String {
        let mut parts = vec![self.number.to_string()];
        if self.stops != "0" {
            parts.push(self.stops.clone());
        }
        if !self.local_r.is_empty() {
            parts.push(self.local_r.clone());
        }
        if !self.flash.is_empty() {
            parts.push(self.flash.clone());
        }
        if !self.key.is_empty() {
            parts.push(self.key.clone());
        }
        parts.join(" ")
    }

    /// Long form for the record card.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut text = format!("{} {}", self.number, self.kind());
        if self.stops != "0" {
            text.push_str(&format!(" {}", self.stops));
        }
        if !self.local_r.is_empty() {
            text.push_str(&format!(" @ {}", self.local_r));
        }
        if !self.flash.is_empty() {
            text.push_str(&format!(" {}", self.flash));
        }
        if !self.key.is_empty() {
            text.push_str(&format!(" on {}", self.key));
        }
        text
    }
}

/// One note per enabled mask, numbered by its position in the mask list.
#[must_use]
pub fn mask_notes(local: &LocalAdjustmentsConfig, grade: f64) -> Vec<MaskNote> {
    let limited = limited_indices(local);
    local
        .masks
        .iter()
        .enumerate()
        .filter(|(_, mask)| mask.enabled)
        .map(|(index, mask)| MaskNote {
            number: index + 1,
            is_burn: mask.stops > 0.0,
            stops: stops_label(mask.stops),
            local_r: local_grade_label(grade, mask.grade),
            flash: if mask.flash != 0.0 && !limited.contains(&index) {
                format!("flash {:.2}", mask.flash)
            } else {
                String::new()
            },
            key: if limited.contains(&index) {
                tone_limit_label(mask)
            } else {
                String::new()
            },
        })
        .collect()
}

/// The printing record: one line per decision that is not at its default.
#[must_use]
pub fn recipe_lines(
    exposure: &ExposureConfig,
    local: &LocalAdjustmentsConfig,
    finish: &FinishConfig,
    frame: &str,
    brief: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if !frame.is_empty() {
        lines.push(frame.to_string());
    }
    let paper = resolve_paper(&exposure.paper_profile).label;
    let mut flags = vec![paper.to_string()];
    if exposure.paper_dmin {
        flags.push("Paper White".to_string());
    }
    if exposure.paper_black {
        flags.push("Paper Black".to_string());
    }
    lines.push(flags.join(" · "));

    let mut density = format!("Print Density {:.2}", exposure.density);
    if exposure.auto_exposure {
        density.push_str(" (auto)");
    }
    lines.push(density);

    if !brief && (exposure.shadow_density != 0.0 || exposure.highlight_density != 0.0) {
        lines.push(format!(
            "Zone density: shadows {:+.2} · highlights {:+.2}",
            exposure.shadow_density, exposure.highlight_density
        ));
    }
    let mut grade = format!("Grade ISO-R {:.0}", exposure.grade);
    if exposure.auto_normalize_contrast {
        grade.push_str(" (auto)");
    }
    if exposure.shadow_grade != 0.0 || exposure.highlight_grade != 0.0 {
        grade.push_str(&format!(
            " · split {:+.0}/{:.0}",
            exposure.shadow_grade, exposure.highlight_grade
        ));
    }
    lines.push(grade);
    if brief {
        return lines;
    }
    if exposure.wb_cyan != 0.0 || exposure.wb_magenta != 0.0 || exposure.wb_yellow != 0.0 {
        lines.push(format!(
            "Filtration C{:+.2} M{:+.2} Y{:+.2}",
            exposure.wb_cyan, exposure.wb_magenta, exposure.wb_yellow
        ));
    }
    if exposure.toe != 0.0 || exposure.shoulder != 0.0 {
        lines.push(format!(
            "Toe {:+.2} · Shoulder {:+.2}",
            exposure.toe, exposure.shoulder
        ));
    }
    if exposure.midtone_gamma != 0.0 {
        lines.push(format!("Snap {:+.2}", exposure.midtone_gamma));
    }
    if exposure.preflash != 0.0 {
        lines.push(format!("Preflash {:.2} of threshold", exposure.preflash));
    }
    if finish.vignette_stops != 0.0 {
        lines.push(format!(
            "Edge burn {} stop",
            stops_label(finish.vignette_stops)
        ));
    }
    let notes = mask_notes(local, exposure.grade);
    if !notes.is_empty() {
        let summaries: Vec<String> = notes.iter().map(MaskNote::summary).collect();
        lines.push(format!("Dodge & burn: {}", summaries.join(" · ")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(stops: f64) -> LocalMask {
        LocalMask {
            vertices: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]],
            stops,
            ..LocalMask::default()
        }
    }

    #[test]
    fn stops_are_written_as_darkroom_fractions() {
        assert_eq!(stops_label(1.0), "+1");
        assert_eq!(stops_label(0.5), "+½");
        assert_eq!(stops_label(-1.0 / 3.0), "−⅓");
        assert_eq!(stops_label(1.25), "+1¼");
        assert_eq!(stops_label(0.2), "+0.20");
        assert_eq!(stops_label(0.0), "0");
    }

    #[test]
    fn a_burn_reads_plus_and_a_dodge_reads_minus() {
        let notes = mask_notes(
            &LocalAdjustmentsConfig {
                masks: vec![mask(0.5), mask(-0.5)],
            },
            0.0,
        );
        assert_eq!(notes[0].kind(), "Burn");
        assert_eq!(notes[1].kind(), "Dodge");
        assert_eq!(notes[0].badge(), "1 +½");
    }

    #[test]
    fn a_grade_only_mask_names_the_grade_it_prints_at() {
        let note = &mask_notes(
            &LocalAdjustmentsConfig {
                masks: vec![LocalMask {
                    grade: 30.0,
                    ..mask(0.0)
                }],
            },
            115.0,
        )[0];
        assert_eq!(note.kind(), "Grade");
        assert_eq!(note.local_r, "R145");
        assert_eq!(note.summary(), "1 Grade @ R145");
    }

    #[test]
    fn a_tone_limited_mask_names_its_zone() {
        let note = &mask_notes(
            &LocalAdjustmentsConfig {
                masks: vec![LocalMask {
                    key: MaskKey::Highlights,
                    key_zone: 6.0,
                    ..mask(1.0)
                }],
            },
            0.0,
        )[0];
        assert_eq!(note.badge(), "1 +1 ≥VI");
        assert_eq!(note.summary(), "1 Burn +1 on ≥VI");
    }

    #[test]
    fn the_recipe_keeps_quiet_about_defaults() {
        let lines = recipe_lines(
            &ExposureConfig::default(),
            &LocalAdjustmentsConfig::default(),
            &FinishConfig::default(),
            "",
            false,
        );
        assert!(lines[0].contains("Digital (Adobe RGB)"));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Print Density 1.00"))
        );
        assert!(lines.iter().any(|line| line.starts_with("Grade ISO-R 115")));
    }

    #[test]
    fn a_brief_recipe_keeps_three_lines() {
        let exposure = ExposureConfig {
            shadow_density: 0.1,
            toe: 0.2,
            midtone_gamma: 0.3,
            ..ExposureConfig::default()
        };
        let full = recipe_lines(
            &exposure,
            &LocalAdjustmentsConfig::default(),
            &FinishConfig::default(),
            "",
            false,
        );
        let brief = recipe_lines(
            &exposure,
            &LocalAdjustmentsConfig::default(),
            &FinishConfig::default(),
            "",
            true,
        );
        assert_eq!(brief.len(), 3);
        assert!(full.len() > brief.len());
    }
}
