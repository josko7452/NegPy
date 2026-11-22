//! The Analysis card's measured curve and readouts.

use std::collections::BTreeMap;

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics;
use negpy_backend::features::exposure::normalization::LogNegativeBounds;
use negpy_backend::features::exposure::print::{PrintRenderSettings, sample_curve};
use negpy_backend::features::exposure::stats::{StatRow, negative_statistics};
use serde_json::Value;

/// One line of the Readouts card.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisRow {
    pub name: String,
    pub value: String,
    pub warn: bool,
}

/// The values one render measured, ready for the Analysis card.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnalysisMetrics {
    pub curve: Vec<[f64; 2]>,
    pub rows: Vec<AnalysisRow>,
}

const CURVE_SAMPLES: usize = 33;
const CHART_MIN: f64 = -0.1;
const CHART_MAX: f64 = 1.1;

impl AnalysisMetrics {
    /// Read the curve and readouts a render measured.
    #[must_use]
    pub fn from_values(config: &WorkspaceConfig, values: &BTreeMap<String, Value>) -> Self {
        let read = |name: &str| values.get(name).and_then(Value::as_f64);
        let bounds = read_bounds(values);
        let metrics = AutoSliderMetrics {
            metered_anchor: read("metered_anchor"),
            textural_range: read("textural_range"),
            norm_density_range: read("norm_density_range"),
            shadow_point: read("shadow_point"),
            highlight_point: read("highlight_point"),
            bounds,
            neutral_axis: None,
        };
        let settings = PrintRenderSettings {
            config: &config.exposure,
            process: &config.process,
            density_range: metrics.norm_density_range,
            textural_range: metrics.textural_range,
            shadow_point: metrics.shadow_point,
            highlight_point: metrics.highlight_point,
            anchor: config
                .exposure
                .auto_exposure
                .then_some(metrics.metered_anchor)
                .flatten(),
            bounds: metrics.bounds,
            neutral_axis: None,
            mode: config.process.process_mode,
            base_black_hold: 0.0,
            mask: None,
            diffusion: None,
            local_exposure: None,
            local_grade: None,
            local_flash: None,
            key_params: None,
            key_alpha: None,
            grade_deltas: None,
        };
        let rows = negative_statistics(
            metrics.norm_density_range,
            metrics.metered_anchor,
            None,
            None,
            read_triple(values, "scan_clip_fractions"),
            read_triple(values, "repair_fractions"),
            None,
            read_triple(values, "density_above_base"),
            values
                .get("base_source")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let mut rows: Vec<AnalysisRow> = rows.iter().map(row_from_stat).collect();
        if let Some(engine) = values.get("engine").and_then(Value::as_str) {
            let reason = values
                .get("engine_reason")
                .and_then(Value::as_str)
                .unwrap_or("");
            rows.push(AnalysisRow {
                name: "Engine".to_string(),
                value: if reason.is_empty() {
                    engine.to_string()
                } else {
                    format!("{engine} ({reason})")
                },
                warn: false,
            });
        }
        Self {
            curve: sample_curve(&settings, CHART_MIN, CHART_MAX, CURVE_SAMPLES),
            rows,
        }
    }
}

fn row_from_stat(row: &StatRow) -> AnalysisRow {
    AnalysisRow {
        name: row.name.to_string(),
        value: row.value.clone(),
        warn: row.warn,
    }
}

fn read_triple(values: &BTreeMap<String, Value>, name: &str) -> Option<[f64; 3]> {
    let items = values.get(name)?.as_array()?;
    if items.len() < 3 {
        return None;
    }
    Some([items[0].as_f64()?, items[1].as_f64()?, items[2].as_f64()?])
}

fn read_bounds(values: &BTreeMap<String, Value>) -> Option<LogNegativeBounds> {
    let bound = |name: &str| -> Option<[f64; 3]> {
        let items = values.get("final_bounds")?.get(name)?.as_array()?;
        if items.len() < 3 {
            return None;
        }
        Some([items[0].as_f64()?, items[1].as_f64()?, items[2].as_f64()?])
    };
    Some(LogNegativeBounds::new(bound("floors")?, bound("ceils")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_without_metrics_still_samples_the_curve() {
        let metrics = AnalysisMetrics::from_values(&WorkspaceConfig::default(), &BTreeMap::new());
        assert_eq!(metrics.curve.len(), CURVE_SAMPLES);
        assert_eq!(metrics.rows.len(), 6);
        assert_eq!(metrics.rows[2].value, "—");
    }

    #[test]
    fn the_readouts_name_the_engine_and_its_fallback() {
        let mut values = BTreeMap::new();
        values.insert("engine".to_string(), serde_json::json!("CPU"));
        values.insert("engine_reason".to_string(), serde_json::json!("carrier"));
        let metrics = AnalysisMetrics::from_values(&WorkspaceConfig::default(), &values);
        let row = metrics.rows.last().expect("the engine row is last");
        assert_eq!(row.name, "Engine");
        assert_eq!(row.value, "CPU (carrier)");
        assert!(!row.warn);
    }

    #[test]
    fn the_readouts_use_the_measured_density_range() {
        let mut values = BTreeMap::new();
        values.insert("norm_density_range".to_string(), serde_json::json!(1.1));
        values.insert("metered_anchor".to_string(), serde_json::json!(0.5));
        let metrics = AnalysisMetrics::from_values(&WorkspaceConfig::default(), &values);
        assert_eq!(metrics.rows[0].value, "Δ1.10");
        assert_ne!(metrics.rows[1].value, "—");
    }
}
