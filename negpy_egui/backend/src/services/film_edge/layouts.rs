//! Mirrors `negpy/services/film_edge/layouts.py`: maker edge print layouts in roll millimeters.
//!
//! A new maker style is one `layouts` entry.

use crate::services::film_edge::barcodes::{KEYKODE_MODULE, KEYKODE_MODULES};
use crate::services::film_edge::geometry::{PERF_PHASE, PERF_PITCH};
use crate::string_enum;

/// A mark sits this many cap heights clear of its text.
pub const MARK_GAP: f64 = 0.3;
/// SMPTE ST 254 zero-frame dot, 0.64-0.76 mm across.
pub const KEY_DOT: f64 = 0.7;

string_enum! {
    /// The strip edge a run prints on.
    pub enum Band {
        Top => "top",
        Bottom => "bottom",
    }
    default Top
}

/// How a run places its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOn {
    /// At each frame number's label.
    Number,
    /// At each A label, 135 only.
    Half,
    /// Every `pitch` mm from `at`.
    Pitch,
}

/// One repeating line of edge print.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRun {
    pub on: RunOn,
    pub band: Band,
    /// A template over `{n} {label} {maker} {text} {code} {roll} {batch} {lot}`.
    pub text: &'static str,
    pub cap: f64,
    pub offset: f64,
    pub at: f64,
    pub pitch: f64,
    /// 0: to the roll end.
    pub count: usize,
    /// `{n}` of the first pitch item.
    pub first: u32,
    pub align: &'static str,
    pub max_width: f64,
    pub lead: &'static str,
    pub trail: &'static str,
    pub under: &'static str,
    pub char_width: f64,
    pub barcode: &'static str,
    pub barcode_at: f64,
    pub every: u32,
    pub phase: u32,
}

impl EdgeRun {
    fn new(on: RunOn, band: Band, text: &'static str, cap: f64) -> Self {
        Self {
            on,
            band,
            text,
            cap,
            offset: 1.0,
            at: 0.0,
            pitch: 0.0,
            count: 0,
            first: 1,
            align: "center",
            max_width: 0.0,
            lead: "",
            trail: "",
            under: "",
            char_width: 0.0,
            barcode: "",
            barcode_at: 0.0,
            every: 1,
            phase: 0,
        }
    }

    fn offset(mut self, offset: f64) -> Self {
        self.offset = offset;
        self
    }

    fn at(mut self, at: f64) -> Self {
        self.at = at;
        self
    }

    fn pitch(mut self, pitch: f64) -> Self {
        self.pitch = pitch;
        self
    }

    fn count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    fn first(mut self, first: u32) -> Self {
        self.first = first;
        self
    }

    fn align(mut self, align: &'static str) -> Self {
        self.align = align;
        self
    }

    fn max_width(mut self, max_width: f64) -> Self {
        self.max_width = max_width;
        self
    }

    fn lead(mut self, lead: &'static str) -> Self {
        self.lead = lead;
        self
    }

    fn trail(mut self, trail: &'static str) -> Self {
        self.trail = trail;
        self
    }

    fn under(mut self, under: &'static str) -> Self {
        self.under = under;
        self
    }

    fn char_width(mut self, char_width: f64) -> Self {
        self.char_width = char_width;
        self
    }

    fn barcode(mut self, barcode: &'static str) -> Self {
        self.barcode = barcode;
        self
    }

    fn barcode_at(mut self, barcode_at: f64) -> Self {
        self.barcode_at = barcode_at;
        self
    }

    fn every(mut self, every: u32) -> Self {
        self.every = every;
        self
    }

    fn phase(mut self, phase: u32) -> Self {
        self.phase = phase;
        self
    }
}

/// One maker's runs for each strip format, and the palettes whose 135 edge carries DX.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeLayout {
    pub runs_135: Vec<EdgeRun>,
    pub runs_120: Vec<EdgeRun>,
    pub dx: &'static [&'static str],
}

const TOP: Band = Band::Top;
const BOTTOM: Band = Band::Bottom;
/// 120 numbers run at the maker's own pitch, not in step with the camera's frames.
const CAP_120: f64 = 1.8;
const OFFSET_120: f64 = 1.3;

fn numbers_120(lead: &'static str) -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Pitch, TOP, "{maker} {text}", CAP_120)
            .offset(OFFSET_120)
            .at(30.0)
            .pitch(86.0)
            .count(19)
            .max_width(60.0),
        EdgeRun::new(RunOn::Pitch, BOTTOM, "{n}", CAP_120)
            .offset(OFFSET_120)
            .at(8.0)
            .pitch(43.0)
            .count(19)
            .lead(lead),
    ]
}

/// A key every 64 perforations (one foot), and a smaller "+32" key half a foot on.
const FOOT: f64 = 64.0 * PERF_PITCH;
const KEY_DOT_X: f64 = PERF_PHASE + 6.0 * PERF_PITCH;
const DOT_TO_BAR: f64 = 6.0 * PERF_PITCH - KEYKODE_MODULES as f64 * KEYKODE_MODULE;

fn key(
    text: &'static str,
    cap: f64,
    char_width: f64,
    dot_x: f64,
    barcode: &'static str,
) -> EdgeRun {
    let text_to_dot = MARK_GAP * cap + KEY_DOT / 2.0;
    EdgeRun::new(RunOn::Pitch, BOTTOM, text, cap)
        .at(dot_x - text_to_dot)
        .pitch(FOOT)
        .first(1000)
        .align("right")
        .trail("dot")
        .char_width(char_width)
        .barcode(barcode)
        .barcode_at(text_to_dot + DOT_TO_BAR)
}

fn keykode_135() -> Vec<EdgeRun> {
    let mut runs = vec![
        key("{code} {roll} {n:04}", 1.52, 1.52, KEY_DOT_X, "keykode"),
        key(
            "{code} {roll} {n:04}+32",
            1.0,
            0.68,
            KEY_DOT_X + FOOT / 2.0,
            "keykode+32",
        ),
    ];
    runs.extend([8.0, 12.0, 16.0, 20.0, 24.0].map(|m| {
        EdgeRun::new(RunOn::Pitch, BOTTOM, "", 1.0)
            .at(KEY_DOT_X + m * PERF_PITCH)
            .pitch(FOOT / 2.0)
            .barcode("index")
    }));
    runs
}

fn kodak_labels_135() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Number, BOTTOM, "{label}", 1.4),
        EdgeRun::new(RunOn::Half, BOTTOM, "{label}", 0.9)
            .offset(1.4)
            .under("arrow"),
    ]
}

fn kodak_top_135() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Number, TOP, "{n}", 1.2),
        EdgeRun::new(RunOn::Number, TOP, "{maker} {text}", 1.25)
            .at(15.5)
            .max_width(26.0),
    ]
}

fn kodak_120() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Pitch, TOP, "{n}", CAP_120)
            .offset(OFFSET_120)
            .at(8.0)
            .pitch(44.5)
            .count(15)
            .first(41),
        EdgeRun::new(RunOn::Pitch, TOP, "{maker} {text}", CAP_120)
            .offset(OFFSET_120)
            .at(30.25)
            .pitch(89.0)
            .count(8)
            .max_width(36.0),
        EdgeRun::new(RunOn::Pitch, BOTTOM, "{n}", CAP_120)
            .offset(OFFSET_120)
            .at(20.0)
            .pitch(60.0)
            .count(12)
            .trail("triangle"),
    ]
}

/// The emulsion batch every third frame, the lot smaller.
fn ilford_batch() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Number, TOP, "{batch}", 1.0)
            .at(20.0)
            .align("right")
            .every(3)
            .phase(2),
        EdgeRun::new(RunOn::Number, TOP, "-{lot}", 0.7)
            .offset(1.15)
            .at(20.05)
            .align("left")
            .every(3)
            .phase(2),
    ]
}

fn ilford_135() -> Vec<EdgeRun> {
    let mut runs = vec![
        EdgeRun::new(RunOn::Number, BOTTOM, "{label}", 1.5),
        EdgeRun::new(RunOn::Half, BOTTOM, "{label}", 1.1).lead("triangle"),
        EdgeRun::new(RunOn::Number, TOP, "{maker} {text}", 1.5).at(2.5),
    ];
    runs.extend(ilford_batch());
    runs
}

/// A monospaced name, numbers near the film edge, a long wedge before each A number.
fn kentmere_135() -> Vec<EdgeRun> {
    let mut runs = vec![
        EdgeRun::new(RunOn::Number, BOTTOM, "{label}", 1.55).offset(0.85),
        EdgeRun::new(RunOn::Half, BOTTOM, "{label}", 1.05)
            .offset(0.85)
            .at(1.0)
            .lead("wedge"),
        EdgeRun::new(RunOn::Number, TOP, "{maker} {text}", 1.45)
            .offset(0.95)
            .at(2.5)
            .char_width(1.32),
    ];
    runs.extend(ilford_batch());
    runs
}

fn kodak_135() -> Vec<EdgeRun> {
    let mut runs = kodak_labels_135();
    runs.extend(kodak_top_135());
    runs
}

fn kodak_alternating_135() -> Vec<EdgeRun> {
    let mut runs = kodak_labels_135();
    runs.push(kodak_top_135()[0].clone());
    runs.push(
        EdgeRun::new(RunOn::Number, TOP, "{maker} {text}", 1.25)
            .at(15.5)
            .max_width(26.0)
            .every(2)
            .phase(0),
    );
    runs.push(
        EdgeRun::new(RunOn::Number, TOP, "{text} {maker}", 1.25)
            .at(15.5)
            .max_width(26.0)
            .every(2)
            .phase(1),
    );
    runs
}

fn fuji_135() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Number, BOTTOM, "{label}", 1.5),
        EdgeRun::new(RunOn::Half, BOTTOM, "{label}", 1.5).lead("arrow"),
        EdgeRun::new(RunOn::Number, TOP, "{n}", 1.5),
        EdgeRun::new(RunOn::Number, TOP, "{code}", 1.0)
            .at(9.5)
            .max_width(7.0),
        EdgeRun::new(RunOn::Number, TOP, "{n}A", 1.5).at(19.0),
        EdgeRun::new(RunOn::Number, TOP, "FUJI", 1.0).at(28.5),
    ]
}

fn fuji_120() -> Vec<EdgeRun> {
    vec![
        EdgeRun::new(RunOn::Pitch, TOP, "FUJI {code}", CAP_120)
            .offset(OFFSET_120)
            .at(30.0)
            .pitch(84.0)
            .count(19)
            .max_width(50.0),
        EdgeRun::new(RunOn::Pitch, BOTTOM, "{code}", CAP_120)
            .offset(OFFSET_120)
            .at(6.4)
            .pitch(42.0)
            .count(19)
            .align("right")
            .max_width(24.0),
        EdgeRun::new(RunOn::Pitch, BOTTOM, "{n}", CAP_120)
            .offset(OFFSET_120)
            .at(8.0)
            .pitch(42.0)
            .count(19)
            .align("left")
            .lead("outline"),
    ]
}

fn generic_135() -> Vec<EdgeRun> {
    let mut runs = vec![
        EdgeRun::new(RunOn::Number, BOTTOM, "{label}", 1.4),
        EdgeRun::new(RunOn::Half, BOTTOM, "{label}", 1.1).lead("triangle"),
    ];
    runs.extend(kodak_top_135());
    runs
}

/// Every maker layout, keyed by `EdgeStyle::layout`.
#[must_use]
pub fn layouts() -> Vec<(&'static str, EdgeLayout)> {
    vec![
        (
            "kodak",
            EdgeLayout {
                runs_135: kodak_135(),
                runs_120: kodak_120(),
                dx: &["color"],
            },
        ),
        (
            "kodak-alternating",
            EdgeLayout {
                runs_135: kodak_alternating_135(),
                runs_120: kodak_120(),
                dx: &["color"],
            },
        ),
        (
            "fuji",
            EdgeLayout {
                runs_135: fuji_135(),
                runs_120: fuji_120(),
                dx: &["color"],
            },
        ),
        (
            "ilford",
            EdgeLayout {
                runs_135: ilford_135(),
                runs_120: numbers_120("dots"),
                dx: &["color", "bw"],
            },
        ),
        (
            "kentmere",
            EdgeLayout {
                runs_135: kentmere_135(),
                runs_120: numbers_120("dots"),
                dx: &[],
            },
        ),
        (
            "cine",
            EdgeLayout {
                runs_135: keykode_135(),
                runs_120: numbers_120("triangle"),
                dx: &[],
            },
        ),
        (
            "generic",
            EdgeLayout {
                runs_135: generic_135(),
                runs_120: numbers_120("triangle"),
                dx: &["color"],
            },
        ),
    ]
}

/// One layout by its key, falling back to `generic`.
#[must_use]
pub fn layout(key: &str) -> EdgeLayout {
    layouts()
        .into_iter()
        .find(|(name, _)| *name == key)
        .map(|(_, layout)| layout)
        .unwrap_or_else(|| EdgeLayout {
            runs_135: generic_135(),
            runs_120: numbers_120("triangle"),
            dx: &["color"],
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKS: [&str; 7] = ["", "triangle", "wedge", "outline", "arrow", "dots", "dot"];

    #[test]
    fn every_layout_is_well_formed() {
        for (name, layout) in layouts() {
            assert!(
                layout
                    .dx
                    .iter()
                    .all(|value| ["color", "bw"].contains(value)),
                "{name}"
            );
            for (format, runs) in [("135", &layout.runs_135), ("120", &layout.runs_120)] {
                for run in runs {
                    assert!(run.pitch > 0.0 || run.on != RunOn::Pitch, "{name} {format}");
                    assert!(run.every >= 1 && run.phase < run.every, "{name} {format}");
                    for mark in [run.lead, run.trail, run.under] {
                        assert!(MARKS.contains(&mark), "{name} {format} {mark}");
                    }
                    assert!(
                        ["", "keykode", "keykode+32", "index"].contains(&run.barcode),
                        "{name} {format}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_keykode_key_sits_a_dot_before_its_barcode() {
        let cine = layout("cine");
        let keys: Vec<&EdgeRun> = cine
            .runs_135
            .iter()
            .filter(|run| run.barcode.starts_with("keykode"))
            .collect();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].first, 1000);
        assert_eq!(keys[0].pitch, 64.0 * PERF_PITCH);
        assert!(keys[1].at - keys[0].at > 0.0);
    }
}
