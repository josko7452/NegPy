//! Negative statistics for the Analysis panel.

use crate::features::exposure::constants::{ASSUMED_ANCHOR, SCAN_CLIP_LEVEL};
use crate::features::exposure::curve::default_grade_range;

const CLIP_WARN: f64 = 0.01;
const REPAIR_WARN: f64 = 0.05;
const GAMUT_WARN: f64 = 0.02;
const DIAG_FLAT: f64 = 0.80;
const DIAG_CONTRASTY: f64 = 1.25;
const EMPTY: &str = "—";
const LOG10_2: f64 = 0.30103;

/// One labelled read-out row.
#[derive(Clone, Debug, PartialEq)]
pub struct StatRow {
    pub name: &'static str,
    pub value: String,
    pub warn: bool,
    pub detail: String,
    pub per_channel: bool,
}

impl StatRow {
    fn new(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
            warn: false,
            detail: String::new(),
            per_channel: false,
        }
    }
}

fn density_detail(density_above_base: Option<[f64; 3]>, base_source: &str) -> String {
    let Some([dmin, dmed, dmax]) = density_above_base else {
        return "Dmax, Dmed and Dmin need a film base. Click the clear rebate with the Film Base tool on the Metering card; without one, the base is read from the rebate when the scan shows a film edge.".to_string();
    };
    let source = if base_source == "picked" {
        "Above the film base set with the Film Base tool"
    } else {
        "Above the clear base read from the rebate"
    };
    let caveat = if base_source == "picked" {
        ""
    } else {
        " The rebate reader can take bare light or a neighbor frame for the base; the Film Base tool pins it."
    };
    format!(
        "{source}: Dmax {dmax:.2} · Dmed {dmed:.2} · Dmin {dmin:.2}. Dmin and Dmax are the bounds the frame is normalized with, before the White and Black Point trims (the roll's under Roll Analysis); Dmed is the metered median. A Dmin below zero means the metered area holds something thinner than the base: bare light through a perforation or past the film's edge. An estimate from the scan: flare reads dense areas low.{caveat}"
    )
}

fn negative_row(
    norm_density_range: Option<f64>,
    density_above_base: Option<[f64; 3]>,
    base_source: &str,
) -> StatRow {
    let detail = density_detail(density_above_base, base_source);
    let Some(density_range) = norm_density_range.filter(|value| *value != 0.0) else {
        let mut row = StatRow::new("Negative", EMPTY);
        row.detail = detail;
        return row;
    };
    let ratio = density_range / default_grade_range();
    let word = if ratio < DIAG_FLAT {
        "Flat"
    } else if ratio > DIAG_CONTRASTY {
        "Contrasty"
    } else {
        "Normal"
    };
    let mut row = StatRow::new("Negative", format!("Δ{density_range:.2}"));
    row.detail = format!("{word} negative. {detail}");
    row
}

fn exposure_row(metered_anchor: Option<f64>, norm_density_range: Option<f64>) -> StatRow {
    let (Some(anchor), Some(density_range)) = (metered_anchor, norm_density_range) else {
        return StatRow::new("Exposure", EMPTY);
    };
    let deviation = anchor - ASSUMED_ANCHOR;
    let ev = deviation * density_range / LOG10_2;
    StatRow::new("Exposure", format!("{ev:+.1} EV"))
}

/// The Clipping row: worst-channel shares at the black and white ends.
#[must_use]
pub fn clipping_row(clip_low: Option<f64>, clip_high: Option<f64>, compact: bool) -> StatRow {
    let (Some(low), Some(high)) = (clip_low, clip_high) else {
        return StatRow::new("Clipping", EMPTY);
    };
    let warn = low > CLIP_WARN || high > CLIP_WARN;
    let mut row = if compact {
        StatRow::new(
            "Clipping",
            format!("{:.1} · {:.1}%", low * 100.0, high * 100.0),
        )
    } else {
        StatRow::new(
            "Clipping",
            format!("Sh {:.1}% · Hi {:.1}%", low * 100.0, high * 100.0),
        )
    };
    row.warn = warn;
    if compact {
        row.detail = format!(
            "Shadows {:.1}% · highlights {:.1}%",
            low * 100.0,
            high * 100.0
        );
    }
    row
}

fn scan_clip_row(scan_clip: Option<[f64; 3]>) -> StatRow {
    let Some([red, green, blue]) = scan_clip else {
        return StatRow::new("Scan clip", EMPTY);
    };
    let mut row = StatRow::new(
        "Scan clip",
        format!(
            "{:.1} · {:.1} · {:.1}%",
            red * 100.0,
            green * 100.0,
            blue * 100.0
        ),
    );
    row.warn = red.max(green).max(blue) > SCAN_CLIP_LEVEL as f64;
    row.detail = format!(
        "Red {:.1}% · green {:.1}% · blue {:.1}%",
        red * 100.0,
        green * 100.0,
        blue * 100.0
    );
    row.per_channel = true;
    row
}

fn repair_row(repair: Option<[f64; 3]>) -> StatRow {
    let Some(parts) = repair else {
        return StatRow::new("Repair", EMPTY);
    };
    let labels = ["IR", "dust", "painted"];
    let shown: Vec<String> = labels
        .iter()
        .zip(parts)
        .filter(|(_, fraction)| *fraction > 0.0)
        .map(|(label, fraction)| format!("{label} {:.2}%", fraction * 100.0))
        .collect();
    if shown.is_empty() {
        return StatRow::new("Repair", "none");
    }
    let mut row = StatRow::new("Repair", shown.join(" · "));
    row.warn = parts.iter().copied().fold(0.0, f64::max) > REPAIR_WARN;
    row
}

fn gamut_row(gamut: Option<f64>) -> StatRow {
    let Some(gamut) = gamut else {
        return StatRow::new("Gamut", EMPTY);
    };
    let mut row = StatRow::new("Gamut", format!("{:.1}%", gamut * 100.0));
    row.warn = gamut > GAMUT_WARN;
    row
}

/// Negative read-out, in display order.
#[must_use]
pub fn negative_statistics(
    norm_density_range: Option<f64>,
    metered_anchor: Option<f64>,
    clip_low: Option<f64>,
    clip_high: Option<f64>,
    scan_clip: Option<[f64; 3]>,
    repair: Option<[f64; 3]>,
    gamut: Option<f64>,
    density_above_base: Option<[f64; 3]>,
    base_source: &str,
) -> Vec<StatRow> {
    vec![
        negative_row(norm_density_range, density_above_base, base_source),
        exposure_row(metered_anchor, norm_density_range),
        clipping_row(clip_low, clip_high, true),
        scan_clip_row(scan_clip),
        repair_row(repair),
        gamut_row(gamut),
    ]
}
