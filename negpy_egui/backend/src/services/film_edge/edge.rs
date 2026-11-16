//! Mirrors `negpy/services/film_edge/edge.py`: the edge print items of a strip.

use crate::features::metadata::gear::{FilmEdge, FilmStock, GearLibrary};
use crate::services::film_edge::barcodes::{
    DX_CLOCK_TRACK, DX_DATA_TRACK, DX_MODULE, FRAME_INDEX_LENGTH, FRAME_INDEX_TRACK,
    KEYKODE_MODULE, KEYKODE_TRACK, dx_bits, keykode_bits,
};
use crate::services::film_edge::geometry::{FilmGeometry, HALF_FRAME_PITCH, SheetFormat};
use crate::services::film_edge::layouts::{Band, EdgeRun, RunOn, layout};

/// The label pitch of a 135 strip, a half frame.
pub const LABEL_PITCH_135: f64 = HALF_FRAME_PITCH;
/// A DX label starts this far past its frame.
pub const DX_START: f64 = 1.9;
/// A DX label ends this far past its position, clear of the barcode's quiet zone.
pub const DX_LABEL_END: f64 = 1.5;
/// Longer than any item's half-width, so an item that straddles a cut is kept and clipped.
pub const REACH: f64 = 45.0;
/// The layout a stock with no edge data falls back to.
pub const GENERIC: &str = "generic";

/// A line of printed text, the mark names as `EdgeRun` takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeText {
    pub x: f64,
    pub band: Band,
    pub text: String,
    pub cap: f64,
    pub offset: f64,
    pub align: String,
    pub max_width: f64,
    pub lead: String,
    pub trail: String,
    pub under: String,
    pub char_width: f64,
}

/// One barcode track: `(near, far)` mm from the film edge and its module bits.
pub type Track = ((f64, f64), Vec<u8>);

/// A barcode in the bottom band.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeCode {
    pub x: f64,
    pub tracks: Vec<Track>,
    pub module: f64,
}

impl EdgeCode {
    /// The mark's length in millimeters.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.tracks
            .iter()
            .map(|(_, bits)| bits.len() as f64)
            .fold(0.0, f64::max)
            * self.module
    }
}

/// A mark or a barcode.
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeItem {
    Text(EdgeText),
    Code(EdgeCode),
}

impl EdgeItem {
    /// The item's position along the strip.
    #[must_use]
    pub fn x(&self) -> f64 {
        match self {
            EdgeItem::Text(text) => text.x,
            EdgeItem::Code(code) => code.x,
        }
    }
}

const CINE_WORDS: [&str; 11] = [
    "vision",
    "cinestill",
    "double-x",
    "eastman",
    "eterna",
    "5207",
    "5219",
    "5222",
    "5203",
    "5213",
    "7222",
];
const ILFORD_MAKERS: [&str; 3] = ["ILFORD", "KENTMERE", "HARMAN"];
const MAKER_WORDS: [&str; 9] = [
    "KODAK",
    "EASTMAN",
    "ILFORD",
    "KENTMERE",
    "HARMAN",
    "FUJIFILM",
    "FUJICOLOR",
    "FUJICHROME",
    "FUJI",
];

/// Folds accents so every glyph exists in the edge face, then upper-cases and collapses spaces.
#[must_use]
pub fn ascii_upper(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    let folded: String = text
        .nfkd()
        .filter(char::is_ascii)
        .flat_map(char::to_uppercase)
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The layout key guessed from a film's name, for a stock with no edge data.
#[must_use]
pub fn edge_family(stock: &str, manufacturer: &str) -> String {
    let text = format!("{manufacturer} {stock}").to_lowercase();
    if CINE_WORDS.iter().any(|word| text.contains(word)) {
        return "cine".to_string();
    }
    if text.contains("kodak") {
        return "kodak".to_string();
    }
    if text.contains("kentmere") {
        return "kentmere".to_string();
    }
    if ILFORD_MAKERS
        .iter()
        .any(|maker| text.contains(&maker.to_lowercase()))
    {
        return "ilford".to_string();
    }
    if text.contains("fuji") {
        return "fuji".to_string();
    }
    GENERIC.to_string()
}

/// `(maker, product)` of a film name in edge capitals; a leading maker word comes off the name.
#[must_use]
pub fn split_maker(name: &str, manufacturer: &str) -> (String, String) {
    let maker = ascii_upper(manufacturer);
    let words: Vec<String> = ascii_upper(name)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if let Some(first) = words.first()
        && (*first == maker || MAKER_WORDS.contains(&first.as_str()))
    {
        return (first.clone(), words[1..].join(" "));
    }
    (maker, words.join(" "))
}

/// The edge print of one stock.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeStyle {
    pub layout: String,
    pub maker: String,
    pub text: String,
    /// A short mark (Fuji's RVP); the layout prints `text` where it is empty.
    pub code: String,
    pub dx: bool,
    pub dx_number: Option<(u32, u32)>,
    /// False: rebate and perforations only.
    pub printed: bool,
    pub color_lines: Vec<crate::features::metadata::gear::EdgeColorLine>,
}

impl EdgeStyle {
    fn digest(&self) -> u32 {
        let text = format!("{} {}", self.maker, self.text);
        let ascii: Vec<u8> = text.bytes().filter(|byte| byte.is_ascii()).collect();
        crc32fast::hash(&ascii)
    }

    /// The stock's DX number, else a stand-in from its name; never 0, which readers reject.
    #[must_use]
    pub fn dx_product(&self) -> (u32, u32) {
        self.dx_number.unwrap_or_else(|| {
            let digest = self.digest();
            (1 + digest % 127, (digest >> 8) % 16)
        })
    }

    /// Stand-in 6-digit roll prefix of a KEYKODE key, as "07 6163".
    #[must_use]
    pub fn roll(&self) -> String {
        let digits = format!("{:06}", self.digest() % 1_000_000);
        format!("{} {}", &digits[..2], &digits[2..])
    }

    /// Stand-in emulsion batch and lot, as "5048" and "12".
    #[must_use]
    pub fn batch(&self) -> (String, String) {
        let digest = self.digest() >> 4;
        (
            format!("{}", 1000 + digest % 9000),
            format!("{}", 10 + (digest >> 14) % 90),
        )
    }
}

/// `(part1, part2)` of "95-13"; None unless both fit the edge code.
#[must_use]
pub fn parse_dx(text: &str) -> Option<(u32, u32)> {
    let (part1, part2) = text.split_once('-')?;
    let (part1, part2) = (part1.trim(), part2.trim());
    if !part1.chars().all(|ch| ch.is_ascii_digit()) || !part2.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let number = (part1.parse().ok()?, part2.parse().ok()?);
    ((1..=127).contains(&number.0) && number.1 <= 15).then_some(number)
}

fn edge_owner<'a>(stock: &'a FilmStock, library: Option<&'a GearLibrary>) -> &'a FilmStock {
    let mut owner = stock;
    if stock.edge.is_none()
        && let Some(library) = library
    {
        let key = (
            stock.manufacturer.to_lowercase(),
            stock.stock_name.to_lowercase(),
        );
        if let Some(twin) = library.film_stocks.iter().find(|twin| {
            twin.is_bundled
                && twin.edge.is_some()
                && (
                    twin.manufacturer.to_lowercase(),
                    twin.stock_name.to_lowercase(),
                ) == key
        }) {
            owner = twin;
        }
    }
    if let Some(edge) = &owner.edge
        && !edge.prints_as.is_empty()
        && let Some(library) = library
        && let Some(source) = library.get_film_stock(&edge.prints_as)
    {
        return source;
    }
    owner
}

/// Edge print of a catalog stock; with no stock, guessed from a free-text film name and maker.
#[must_use]
pub fn edge_style(
    stock: Option<&FilmStock>,
    library: Option<&GearLibrary>,
    name: &str,
    manufacturer: &str,
) -> EdgeStyle {
    let Some(stock) = stock else {
        let (maker, text) = split_maker(name, manufacturer);
        return EdgeStyle {
            layout: edge_family(name, manufacturer),
            maker,
            text,
            code: String::new(),
            dx: false,
            dx_number: None,
            printed: true,
            color_lines: Vec::new(),
        };
    };
    let owner = edge_owner(stock, library);
    let empty = FilmEdge::default();
    let edge = owner.edge.as_ref().unwrap_or(&empty);
    let (maker, text) = split_maker(&owner.stock_name, &owner.manufacturer);
    let guessed = edge_family(&owner.stock_name, &owner.manufacturer);
    let layout = if edge.style.is_empty() {
        guessed
    } else {
        edge.style.clone()
    };
    let mut color_lines = edge.color_lines.clone();
    if color_lines.is_empty()
        && let Some(library) = library
    {
        let key = (
            owner.manufacturer.to_lowercase(),
            owner.stock_name.to_lowercase(),
        );
        if let Some(twin) = library.film_stocks.iter().find(|twin| {
            twin.is_bundled
                && twin
                    .edge
                    .as_ref()
                    .is_some_and(|edge| !edge.color_lines.is_empty())
                && (
                    twin.manufacturer.to_lowercase(),
                    twin.stock_name.to_lowercase(),
                ) == key
        }) {
            color_lines = twin.edge.as_ref().expect("checked").color_lines.clone();
        }
    }
    EdgeStyle {
        layout,
        maker: if edge.maker.is_empty() {
            maker
        } else {
            edge.maker.clone()
        },
        text: if edge.text.is_empty() {
            text
        } else {
            edge.text.clone()
        },
        code: edge.code.clone(),
        dx: false,
        dx_number: parse_dx(&edge.dx),
        printed: true,
        color_lines,
    }
}

/// `1`, `1A`, `2`, `2A` … for a frame label index.
#[must_use]
pub fn label_text(index: u32) -> String {
    format!("{}{}", index / 2 + 1, if index % 2 == 1 { "A" } else { "" })
}

fn format_text(template: &str, fields: &TextFields) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            out.push(ch);
            continue;
        }
        let mut key = String::new();
        for next in chars.by_ref() {
            if next == '}' {
                break;
            }
            key.push(next);
        }
        let (name, width) = match key.split_once(':') {
            Some((name, width)) => (name, width.trim_start_matches('0').parse::<usize>().ok()),
            None => (key.as_str(), None),
        };
        let value = match name {
            "n" => Some(fields.n.to_string()),
            "label" => Some(fields.label.clone()),
            "maker" => Some(fields.maker.clone()),
            "text" => Some(fields.text.clone()),
            "code" => Some(fields.code.clone()),
            "roll" => Some(fields.roll.clone()),
            "batch" => Some(fields.batch.clone()),
            "lot" => Some(fields.lot.clone()),
            _ => None,
        };
        if let Some(value) = value {
            match width {
                Some(width) => out.push_str(&format!("{value:0width$}")),
                None => out.push_str(&value),
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct TextFields {
    n: u32,
    label: String,
    maker: String,
    text: String,
    code: String,
    roll: String,
    batch: String,
    lot: String,
}

/// Items that can touch `x0..x1`; the strip clips the ones that straddle a cut.
///
/// `numbers` is each frame slot's place on the roll; None means 0, 1, 2 …
#[must_use]
pub fn edge_items(
    geometry: &FilmGeometry,
    style: &EdgeStyle,
    frame_count: usize,
    x0: f64,
    x1: f64,
    numbers: Option<&[usize]>,
) -> Vec<EdgeItem> {
    if !style.printed {
        return Vec::new();
    }
    let layout = layout(&style.layout);
    let (batch, lot) = style.batch();
    let fields = TextFields {
        n: 0,
        label: String::new(),
        maker: style.maker.clone(),
        text: style.text.clone(),
        code: if style.code.is_empty() {
            style.text.clone()
        } else {
            style.code.clone()
        },
        roll: style.roll(),
        batch,
        lot,
    };
    let mut items = Vec::new();
    let runs = if geometry.format == SheetFormat::Medium {
        &layout.runs_120
    } else {
        let runs = &layout.runs_135;
        let dx = style.dx && !layout.dx.is_empty();
        let slots: Vec<usize> = numbers
            .map(<[usize]>::to_vec)
            .unwrap_or_else(|| (0..frame_count).collect());
        for (x, j) in label_slots(geometry, &slots[..slots.len().min(frame_count)]) {
            let number = j as u32 / 2 + 1;
            let is_a = j % 2 == 1;
            for run in runs {
                let wanted = if is_a { RunOn::Half } else { RunOn::Number };
                if run.on != wanted || number % run.every != run.phase {
                    continue;
                }
                let (mut anchor, mut align) = (x + run.at, run.align);
                if dx && run.band == Band::Bottom {
                    anchor += DX_LABEL_END;
                    align = "right";
                }
                add_text(
                    &mut items,
                    run,
                    anchor,
                    align,
                    &TextFields {
                        n: number,
                        label: label_text(j as u32),
                        ..clone_fields(&fields)
                    },
                );
            }
            if dx {
                let (clock, data) = dx_bits(number, is_a, style.dx_product());
                items.push(EdgeItem::Code(EdgeCode {
                    x: x + DX_START,
                    tracks: vec![(DX_DATA_TRACK, data), (DX_CLOCK_TRACK, clock)],
                    module: DX_MODULE,
                }));
            }
        }
        runs
    };
    let roll_end = frame_count as f64 * geometry.pitch;
    for run in runs {
        if run.on != RunOn::Pitch {
            continue;
        }
        for (k, x) in pitch_positions(run, roll_end) {
            let foot = run.first.saturating_add(k);
            add_text(
                &mut items,
                run,
                x,
                run.align,
                &TextFields {
                    n: foot,
                    label: String::new(),
                    ..clone_fields(&fields)
                },
            );
            if run.barcode == "index" {
                items.push(EdgeItem::Code(EdgeCode {
                    x: x - FRAME_INDEX_LENGTH / 2.0,
                    tracks: vec![(FRAME_INDEX_TRACK, vec![1])],
                    module: FRAME_INDEX_LENGTH,
                }));
            } else if !run.barcode.is_empty() {
                let perforations = if run.barcode.ends_with("+32") { 32 } else { 0 };
                let bits = keykode_bits(
                    &fields.code,
                    &style.roll().replace(' ', ""),
                    foot,
                    perforations,
                );
                items.push(EdgeItem::Code(EdgeCode {
                    x: x + run.barcode_at,
                    tracks: vec![(KEYKODE_TRACK, bits)],
                    module: KEYKODE_MODULE,
                }));
            }
        }
    }
    items
        .into_iter()
        .filter(|item| item.x() >= x0 - REACH && item.x() <= x1 + REACH)
        .collect()
}

fn clone_fields(fields: &TextFields) -> TextFields {
    TextFields {
        n: fields.n,
        label: fields.label.clone(),
        maker: fields.maker.clone(),
        text: fields.text.clone(),
        code: fields.code.clone(),
        roll: fields.roll.clone(),
        batch: fields.batch.clone(),
        lot: fields.lot.clone(),
    }
}

fn add_text(items: &mut Vec<EdgeItem>, run: &EdgeRun, x: f64, align: &str, fields: &TextFields) {
    let text = format_text(run.text, fields);
    if text.is_empty() {
        return;
    }
    items.push(EdgeItem::Text(EdgeText {
        x,
        band: run.band,
        text,
        cap: run.cap,
        offset: run.offset,
        align: align.to_string(),
        max_width: run.max_width,
        lead: run.lead.to_string(),
        trail: run.trail.to_string(),
        under: run.under.to_string(),
        char_width: run.char_width,
    }));
}

fn pitch_positions(run: &EdgeRun, roll_end: f64) -> Vec<(u32, f64)> {
    let mut out = Vec::new();
    let mut k = 0u32;
    while run.count == 0 || (k as usize) < run.count {
        let x = run.at + run.pitch * f64::from(k);
        if x > roll_end + run.pitch {
            break;
        }
        out.push((k, x));
        k += 1;
    }
    out
}

fn label_slots(geometry: &FilmGeometry, slots: &[usize]) -> Vec<(f64, usize)> {
    if geometry.format == SheetFormat::HalfFrame {
        return slots
            .iter()
            .enumerate()
            .map(|(index, roll_index)| (geometry.frame_center(index), *roll_index))
            .collect();
    }
    let mut labels = Vec::new();
    for (index, roll_index) in slots.iter().enumerate() {
        let x = geometry.frame_center(index);
        labels.push((x, 2 * roll_index));
        labels.push((x + LABEL_PITCH_135, 2 * roll_index + 1));
    }
    labels
}

/// A stock with no catalog entry prints from its name alone.
#[must_use]
pub fn style_for_name(name: &str, manufacturer: &str) -> EdgeStyle {
    edge_style(None, None, name, manufacturer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::film_edge::geometry::film_geometry;

    fn full() -> FilmGeometry {
        film_geometry(SheetFormat::FullFrame, "")
    }

    fn half() -> FilmGeometry {
        film_geometry(SheetFormat::HalfFrame, "")
    }

    fn style(layout: &str, maker: &str, text: &str, dx: bool) -> EdgeStyle {
        EdgeStyle {
            layout: layout.into(),
            maker: maker.into(),
            text: text.into(),
            code: String::new(),
            dx,
            dx_number: None,
            printed: true,
            color_lines: Vec::new(),
        }
    }

    fn bottom_labels(
        geometry: &FilmGeometry,
        style: &EdgeStyle,
        frames: usize,
    ) -> Vec<(f64, String)> {
        edge_items(geometry, style, frames, 0.0, 1e9, None)
            .into_iter()
            .filter_map(|item| match item {
                EdgeItem::Text(text) if text.band == Band::Bottom => Some((text.x, text.text)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_film_name_keeps_its_edge_capitals() {
        assert_eq!(
            ascii_upper("Rollei  Ortho  25 Plus"),
            "ROLLEI ORTHO 25 PLUS"
        );
    }

    #[test]
    fn the_family_is_guessed_from_the_name() {
        assert_eq!(edge_family("Kodak Portra 400", ""), "kodak");
        assert_eq!(edge_family("HP5 Plus", "Ilford"), "ilford");
        assert_eq!(edge_family("400", "Kentmere"), "kentmere");
        assert_eq!(edge_family("Superia 400", "Fujifilm"), "fuji");
        assert_eq!(edge_family("CineStill 800T", ""), "cine");
        assert_eq!(edge_family("Fomapan 400", "Foma"), "generic");
    }

    #[test]
    fn the_maker_comes_off_a_leading_name_word() {
        assert_eq!(
            split_maker("Kodak Portra 400", ""),
            ("KODAK".into(), "PORTRA 400".into())
        );
        assert_eq!(
            split_maker("Portra 400", "Kodak"),
            ("KODAK".into(), "PORTRA 400".into())
        );
        assert_eq!(
            split_maker("Velvia 50", ""),
            (String::new(), "VELVIA 50".into())
        );
    }

    #[test]
    fn a_dx_text_only_reads_the_numbers_the_code_can_carry() {
        assert_eq!(parse_dx("95-13"), Some((95, 13)));
        assert_eq!(parse_dx(" 109 - 9 "), Some((109, 9)));
        for text in ["", "95", "0-1", "128-0", "95-16", "a-b", "1533"] {
            assert_eq!(parse_dx(text), None, "{text}");
        }
    }

    #[test]
    fn the_labels_run_1_1a_2_2a() {
        assert_eq!(
            (0..6).map(label_text).collect::<Vec<_>>(),
            ["1", "1A", "2", "2A", "3", "3A"]
        );
    }

    #[test]
    fn full_frame_numbers_sit_on_frames_and_a_numbers_in_the_gaps() {
        let labels = bottom_labels(&full(), &style("kodak", "KODAK", "TRI-X 400", false), 3);
        assert_eq!(
            labels,
            [
                (19.0, "1".into()),
                (38.0, "1A".into()),
                (57.0, "2".into()),
                (76.0, "2A".into()),
                (95.0, "3".into()),
                (114.0, "3A".into())
            ]
        );
    }

    #[test]
    fn half_frames_carry_one_label_each() {
        let labels = bottom_labels(&half(), &style("ilford", "ILFORD", "HP5 PLUS", false), 4);
        let expected: Vec<(f64, String)> = ["1", "1A", "2", "2A"]
            .iter()
            .enumerate()
            .map(|(i, label)| (half().frame_center(i), (*label).to_string()))
            .collect();
        assert_eq!(labels, expected);
    }

    #[test]
    fn numbering_continues_across_strips() {
        let items = edge_items(
            &full(),
            &style("kodak", "KODAK", "PORTRA 400", false),
            12,
            6.0 * 38.0,
            12.0 * 38.0,
            None,
        );
        let texts: Vec<(f64, String)> = items
            .into_iter()
            .filter_map(|item| match item {
                EdgeItem::Text(text) if text.band == Band::Bottom => Some((text.x, text.text)),
                _ => None,
            })
            .collect();
        let inside: Vec<&str> = texts
            .iter()
            .filter(|(x, _)| *x > 6.0 * 38.0 && *x < 12.0 * 38.0)
            .map(|(_, text)| text.as_str())
            .collect();
        let on_cuts: Vec<&str> = texts
            .iter()
            .filter(|(x, _)| *x == 6.0 * 38.0 || *x == 12.0 * 38.0)
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(inside.first(), Some(&"7"));
        assert_eq!(inside.last(), Some(&"12"));
        assert_eq!(on_cuts, ["6A", "12A"]);
    }

    #[test]
    fn frames_left_out_keep_the_other_frames_numbers() {
        let items = edge_items(
            &full(),
            &style("kodak", "KODAK", "GOLD 200", true),
            2,
            0.0,
            1e9,
            Some(&[0, 2]),
        );
        let labels: Vec<String> = items
            .iter()
            .filter_map(|item| match item {
                EdgeItem::Text(text) if text.band == Band::Bottom => Some(text.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["1", "1A", "3", "3A"]);
    }

    #[test]
    fn plain_film_prints_nothing_on_its_edges() {
        let mut plain = style("kodak", "KODAK", "GOLD 200", true);
        plain.printed = false;
        assert!(edge_items(&full(), &plain, 6, 0.0, 1e9, None).is_empty());
        assert!(
            edge_items(
                &film_geometry(SheetFormat::Medium, "6×6"),
                &plain,
                6,
                0.0,
                1e9,
                None
            )
            .is_empty()
        );
    }

    #[test]
    fn each_label_is_followed_by_its_own_code_clear_of_the_label() {
        let items = edge_items(
            &full(),
            &style("generic", "AGFA", "VISTA 200", true),
            2,
            0.0,
            1e9,
            None,
        );
        let labels: Vec<&EdgeText> = items
            .iter()
            .filter_map(|item| match item {
                EdgeItem::Text(text) if text.band == Band::Bottom => Some(text),
                _ => None,
            })
            .collect();
        let codes: Vec<&EdgeCode> = items
            .iter()
            .filter_map(|item| match item {
                EdgeItem::Code(code) => Some(code),
                _ => None,
            })
            .collect();
        assert_eq!(codes.len(), 4);
        assert_eq!(labels.len(), 4);
        for (label, code) in labels.iter().zip(codes) {
            assert_eq!(label.align, "right");
            assert!(code.x - label.x >= 0.38);
            assert!((code.length() - 31.0 * DX_MODULE).abs() < 1e-9);
        }
    }

    #[test]
    fn a_real_dx_number_reaches_the_barcode() {
        let mut style = style("ilford", "ILFORD", "HP5 PLUS", true);
        style.dx_number = Some((109, 9));
        let code = edge_items(&full(), &style, 1, 0.0, 1e9, None)
            .into_iter()
            .find_map(|item| match item {
                EdgeItem::Code(code) => Some(code),
                _ => None,
            })
            .expect("a barcode");
        let (_, data) = &code.tracks[0];
        let payload = &data[5..28];
        let bits = |slice: &[u8]| {
            slice
                .iter()
                .fold(0u32, |acc, bit| acc * 2 + u32::from(*bit))
        };
        assert_eq!(bits(&payload[1..8]), 109);
        assert_eq!(bits(&payload[9..13]), 9);
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use crate::features::metadata::gear::{EdgeColorLine, FilmEdge, FilmStock, GearLibrary};
    use crate::services::film_edge::geometry::film_geometry;

    fn stock(manufacturer: &str, name: &str, edge: Option<FilmEdge>) -> FilmStock {
        FilmStock {
            id: name.into(),
            manufacturer: manufacturer.into(),
            stock_name: name.into(),
            edge,
            ..FilmStock::default()
        }
    }

    #[test]
    fn catalog_edge_data_beats_the_name_guess() {
        let stock = stock(
            "Kodak",
            "Vision3 500T",
            Some(FilmEdge {
                style: "cine".into(),
                maker: "EASTMAN".into(),
                text: "5219".into(),
                code: "EJ".into(),
                ..FilmEdge::default()
            }),
        );
        let style = edge_style(Some(&stock), None, "", "");
        assert_eq!(
            (
                style.layout.as_str(),
                style.maker.as_str(),
                style.text.as_str(),
                style.code.as_str()
            ),
            ("cine", "EASTMAN", "5219", "EJ")
        );
        let first = edge_items(
            &film_geometry(SheetFormat::FullFrame, ""),
            &style,
            12,
            0.0,
            1e9,
            None,
        )
        .into_iter()
        .find_map(|item| match item {
            EdgeItem::Text(text) => Some(text.text),
            _ => None,
        })
        .expect("a key");
        assert!(first.starts_with("EJ "));
    }

    #[test]
    fn the_display_name_never_reaches_the_edge() {
        let mut stock = stock(
            "Kodak",
            "Ektacolor Pro 400",
            Some(FilmEdge {
                text: "PORTRA 400".into(),
                ..FilmEdge::default()
            }),
        );
        stock.display_name = "Kodak Ektacolor Pro 400 (formerly Portra 400)".into();
        let style = edge_style(Some(&stock), None, "", "");
        assert_eq!(style.layout, "kodak");
        assert_eq!(style.text, "PORTRA 400");
    }

    #[test]
    fn a_personal_stock_reads_the_bundled_edge_and_its_color_lines() {
        let bundled = FilmStock {
            is_bundled: true,
            edge: Some(FilmEdge {
                style: "color".into(),
                maker: "KODAK".into(),
                text: "PORTRA 400".into(),
                color_lines: vec![EdgeColorLine {
                    at: 1.2,
                    color: "green".into(),
                    thickness: None,
                    dashed: false,
                }],
                ..FilmEdge::default()
            }),
            ..stock("Kodak", "Portra 400", None)
        };
        let personal = stock("Kodak", "Portra 400", None);
        let library = GearLibrary {
            film_stocks: vec![bundled, personal.clone()],
            ..GearLibrary::default()
        };
        let style = edge_style(Some(&personal), Some(&library), "", "");
        assert_eq!(style.text, "PORTRA 400");
        assert_eq!(style.color_lines.len(), 1);
    }

    #[test]
    fn a_respooled_stock_prints_as_its_source() {
        let source = FilmStock {
            id: "5219".into(),
            manufacturer: "Kodak".into(),
            stock_name: "Vision3 500T".into(),
            is_bundled: true,
            edge: Some(FilmEdge {
                style: "cine".into(),
                maker: "EASTMAN".into(),
                text: "5219".into(),
                ..FilmEdge::default()
            }),
            ..FilmStock::default()
        };
        let respool = stock(
            "CineStill",
            "800T",
            Some(FilmEdge {
                prints_as: "Vision3 500T".into(),
                ..FilmEdge::default()
            }),
        );
        let library = GearLibrary {
            film_stocks: vec![source, respool.clone()],
            ..GearLibrary::default()
        };
        let style = edge_style(Some(&respool), Some(&library), "", "");
        assert_eq!(style.layout, "cine");
        assert_eq!(style.maker, "EASTMAN");
        assert_eq!(style.text, "5219");
    }

    #[test]
    fn no_catalog_entry_guesses_from_the_free_text_name() {
        let style = edge_style(None, None, "Kodak 400TX", "");
        assert_eq!(style.layout, "kodak");
        assert_eq!(style.maker, "KODAK");
        assert_eq!(style.text, "400TX");
        assert!(style.dx_number.is_none());
    }
}
