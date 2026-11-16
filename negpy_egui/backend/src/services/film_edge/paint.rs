//! Mirrors `negpy/services/film_edge/paint.py`: rebate, edge print and perforations.

use ab_glyph::Font;

use crate::features::metadata::gear::EdgeColorLine;
use crate::services::film_edge::barcodes::DX_CLOCK_TRACK;
use crate::services::film_edge::edge::{EdgeCode, EdgeItem, EdgeStyle, EdgeText, edge_items};
use crate::services::film_edge::geometry::{
    FilmGeometry, PERF_ACROSS, PERF_FROM_EDGE, PERF_PHASE, PERF_PITCH, PERF_RADIUS,
    perforation_centers,
};
use crate::services::film_edge::glyphs::{Canvas, TextRun};
use crate::services::film_edge::layouts::{Band, KEY_DOT, MARK_GAP};

pub type Rgb = [u8; 3];

/// The four colors a strip paints with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub no_film: Rgb,
    pub rebate: Rgb,
    pub rim: Option<Rgb>,
    pub ink: Rgb,
    pub label: Rgb,
}

/// A DX clock track runs past the rebate, up between the holes.
pub const DX_BAND: f64 = DX_CLOCK_TRACK.1 + 0.1;
pub const PERF_RIM: f64 = 0.08;
pub const COLOR_LINE_THICK_MM: f64 = 0.3;
pub const DASH_RUN_PITCHES: i32 = 5;

/// `px`: millimeters to whole pixels.
#[must_use]
pub fn px(mm: f64, s: f64) -> i64 {
    (mm * s).round() as i64
}

/// Paint one strip from `roll_start` for `length` mm; `s` is pixels per mm.
#[allow(clippy::too_many_arguments)]
pub fn paint_strip(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    geometry: &FilmGeometry,
    style: &EdgeStyle,
    frame_count: usize,
    roll_start: f64,
    length: f64,
    s: f64,
    palette: &Palette,
    draft: bool,
    numbers: Option<&[usize]>,
) {
    let x0 = px(x, s);
    let y0 = px(y, s);
    let x1 = px(x + length, s);
    let y1 = px(y + geometry.width, s);
    let (cx0, cy0, cx1, cy1) = clip_box(x0, y0, x1, y1, canvas.width, canvas.height);
    if cx1 <= cx0 || cy1 <= cy0 {
        return;
    }
    fill(canvas, cx0, cy0, cx1, cy1, palette.rebate);
    let strip_w = (x1 - x0).max(1) as usize;
    let items = edge_items(
        geometry,
        style,
        frame_count,
        roll_start,
        roll_start + length,
        numbers,
    );
    for band in [Band::Top, Band::Bottom] {
        let band_mm = if band == Band::Bottom && geometry.perforated() && style.dx {
            geometry.edge_band().max(DX_BAND)
        } else {
            geometry.edge_band()
        };
        let band_h = px(band_mm, s).max(1) as usize;
        let mut coverage = vec![0.0f32; strip_w * band_h];
        for item in &items {
            match item {
                EdgeItem::Text(text) if text.band == band => {
                    let at = ((text.x - roll_start) * s) as f32;
                    draw_text(&mut coverage, strip_w, band_h, band, band_mm, s, text, at);
                }
                EdgeItem::Code(code) if band == Band::Bottom => {
                    draw_code(
                        &mut coverage,
                        strip_w,
                        band_h,
                        band,
                        band_mm,
                        s,
                        code,
                        draft,
                    );
                }
                _ => {}
            }
        }
        let band_y = if band == Band::Top {
            y0
        } else {
            y1 - band_h as i64
        };
        let (bx0, by0, bx1, by1) = clip_box(
            x0,
            band_y,
            x0 + strip_w as i64,
            band_y + band_h as i64,
            canvas.width,
            canvas.height,
        );
        if bx1 > bx0 && by1 > by0 {
            let mut cropped = Vec::with_capacity(((bx1 - bx0) * (by1 - by0)) as usize);
            for row in by0..by1 {
                let source_row = (row - band_y) as usize;
                let source_start = source_row * strip_w + (bx0 - x0) as usize;
                cropped.extend_from_slice(
                    &coverage[source_start..source_start + (bx1 - bx0) as usize],
                );
            }
            canvas.blend(bx0, by0, bx1, by1, &cropped, palette.ink);
        }
    }
    for line in &style.color_lines {
        draw_color_line(canvas, line, x0, x1, y1, s, roll_start);
    }
    if geometry.perforated() {
        let width = (PERF_RADIUS * 2.0 * s).max(1.0);
        for roll_x in perforation_centers(roll_start, roll_start + length) {
            let center_x = x0 as f64 + (roll_x - roll_start) * s;
            for center_y in [
                PERF_FROM_EDGE + PERF_ACROSS / 2.0,
                geometry.width - PERF_FROM_EDGE - PERF_ACROSS / 2.0,
            ] {
                let center_y = (y + center_y) * s;
                let radius = (width / 2.0).max(0.5);
                draw_hole(canvas, center_x, center_y, radius, palette);
            }
        }
    }
}

fn clip_box(
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
    width: usize,
    height: usize,
) -> (i64, i64, i64, i64) {
    (
        x0.max(0),
        y0.max(0),
        x1.min(width as i64),
        y1.min(height as i64),
    )
}

fn fill(canvas: &mut Canvas, x0: i64, y0: i64, x1: i64, y1: i64, color: Rgb) {
    for y in y0..y1 {
        for x in x0..x1 {
            canvas.set(x as usize, y as usize, color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    band: Band,
    band_mm: f64,
    s: f64,
    item: &EdgeText,
    x_px: f32,
) {
    let cap = item.cap * s;
    let mut run = TextRun::capped(&item.text, cap);
    if item.char_width > 0.0 {
        run = run.monospaced((item.char_width * s) as f32);
    } else if item.max_width > 0.0 && run.width > (item.max_width * s) as f32 {
        run = run.narrowed((item.max_width * s) as f32);
    }
    let left = match item.align.as_str() {
        "right" => x_px - run.width,
        "left" => x_px,
        _ => x_px - run.width / 2.0,
    };
    let baseline = y_of(band, band_mm, item.offset, s) + cap / 2.0;
    rasterize(coverage, width, height, &run, left, baseline as f32);
    let right = left + run.width;
    marks(
        coverage,
        width,
        height,
        item,
        left,
        right,
        baseline as f32,
        cap as f32,
        s,
    );
}

fn y_of(band: Band, band_mm: f64, offset: f64, s: f64) -> f64 {
    let from_top = if band == Band::Top {
        offset
    } else {
        band_mm - offset
    };
    from_top * s
}

fn rasterize(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    run: &TextRun,
    left: f32,
    baseline: f32,
) {
    for (glyph, _) in &run.glyphs {
        if let Some(outline) = run.font.outline_glyph(glyph.clone()) {
            let bounds = outline.px_bounds();
            let x0 = (left + bounds.min.x).round() as i64;
            let y0 = (baseline + bounds.min.y).round() as i64;
            outline.draw(|x, y, value| {
                let px_x = x0 + x as i64;
                let px_y = y0 + y as i64;
                if px_x < 0 || px_y < 0 || px_x >= width as i64 || px_y >= height as i64 {
                    return;
                }
                let at = px_y as usize * width + px_x as usize;
                coverage[at] = coverage[at].max(value);
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn marks(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    item: &EdgeText,
    left: f32,
    right: f32,
    baseline: f32,
    cap: f32,
    s: f64,
) {
    let gap = MARK_GAP * cap as f64;
    if !item.lead.is_empty() {
        mark(
            coverage,
            width,
            height,
            &item.lead,
            (left as f64 - gap) as f32,
            baseline,
            cap,
            s,
        );
    }
    if !item.trail.is_empty() {
        let size = if item.trail == "dot" {
            KEY_DOT * s
        } else {
            0.9 * cap as f64
        };
        mark(
            coverage,
            width,
            height,
            &item.trail,
            (right as f64 + gap + size) as f32,
            baseline,
            cap,
            s,
        );
    }
}

fn mark(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    kind: &str,
    right: f32,
    center_y: f32,
    cap: f32,
    s: f64,
) {
    let size = 0.9 * cap;
    let half = size / 2.0;
    match kind {
        "triangle" | "wedge" => {
            let back = right - size * if kind == "wedge" { 1.75 } else { 1.0 };
            triangle(
                coverage,
                width,
                height,
                [
                    (right, center_y),
                    (back, center_y - half),
                    (back, center_y + half),
                ],
            );
        }
        "outline" => {
            triangle_stroke(
                coverage,
                width,
                height,
                [
                    (right, center_y),
                    (right - size, center_y - half),
                    (right - size, center_y + half),
                ],
                (0.1 * s) as f32,
            );
        }
        "arrow" => arrow(
            coverage,
            width,
            height,
            right - 1.4 * cap,
            right,
            center_y,
            s,
        ),
        "dots" => {
            let radius = (0.14 * cap).max(0.5);
            for k in 0..4 {
                let cx = right - radius - k as f32 * 0.5 * cap;
                disk(coverage, width, height, cx, center_y, radius);
            }
        }
        "dot" => {
            let radius = (KEY_DOT * s / 2.0) as f32;
            disk(coverage, width, height, right - radius, center_y, radius);
        }
        _ => {}
    }
}

fn arrow(coverage: &mut [f32], width: usize, height: usize, x0: f32, x1: f32, y: f32, s: f64) {
    let thickness = (0.12 * s).max(1.0) as f32;
    let head = (0.35 * s) as f32;
    line(
        coverage,
        width,
        height,
        (x0, y),
        (x1 - head * 0.5, y),
        thickness,
    );
    triangle(
        coverage,
        width,
        height,
        [
            (x1, y),
            (x1 - head, y - head * 0.6),
            (x1 - head, y + head * 0.6),
        ],
    );
}

fn line(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    a: (f32, f32),
    b: (f32, f32),
    thickness: f32,
) {
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0).ceil() as usize + 1;
    let radius = thickness / 2.0;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = a.0 + (b.0 - a.0) * t;
        let y = a.1 + (b.1 - a.1) * t;
        disk(coverage, width, height, x, y, radius);
    }
}

fn disk(coverage: &mut [f32], width: usize, height: usize, cx: f32, cy: f32, radius: f32) {
    let x0 = (cx - radius - 1.0).floor() as i64;
    let y0 = (cy - radius - 1.0).floor() as i64;
    let x1 = (cx + radius + 1.0).ceil() as i64;
    let y1 = (cy + radius + 1.0).ceil() as i64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
                continue;
            }
            let distance = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            if distance <= radius {
                let at = y as usize * width + x as usize;
                coverage[at] = coverage[at].max((radius - distance + 0.5).clamp(0.0, 1.0));
            }
        }
    }
}

fn triangle(coverage: &mut [f32], width: usize, height: usize, points: [(f32, f32); 3]) {
    let min_x = points
        .iter()
        .map(|p| p.0)
        .fold(f32::INFINITY, f32::min)
        .floor() as i64;
    let max_x = points
        .iter()
        .map(|p| p.0)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i64;
    let min_y = points
        .iter()
        .map(|p| p.1)
        .fold(f32::INFINITY, f32::min)
        .floor() as i64;
    let max_y = points
        .iter()
        .map(|p| p.1)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil() as i64;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
                continue;
            }
            if inside_triangle((x as f32 + 0.5, y as f32 + 0.5), points) {
                coverage[y as usize * width + x as usize] = 1.0;
            }
        }
    }
}

fn triangle_stroke(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    points: [(f32, f32); 3],
    thickness: f32,
) {
    for index in 0..3 {
        line(
            coverage,
            width,
            height,
            points[index],
            points[(index + 1) % 3],
            thickness,
        );
    }
}

fn inside_triangle(p: (f32, f32), triangle: [(f32, f32); 3]) -> bool {
    let sign = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
        (a.0 - c.0) * (b.1 - c.1) - (b.0 - c.0) * (a.1 - c.1)
    };
    let d1 = sign(p, triangle[0], triangle[1]);
    let d2 = sign(p, triangle[1], triangle[2]);
    let d3 = sign(p, triangle[2], triangle[0]);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

fn draw_code(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    band: Band,
    band_mm: f64,
    s: f64,
    code: &EdgeCode,
    draft: bool,
) {
    let x0 = (code.x * s).floor() as i64;
    let x1 = ((code.x + code.length()) * s).ceil() as i64;
    for (track, bits) in &code.tracks {
        let first = y_of(band, band_mm, track.0, s).round() as i64;
        let second = y_of(band, band_mm, track.1, s).round() as i64;
        let (top, bottom) = (
            first.min(second).max(0),
            first.max(second).min(height as i64),
        );
        if bottom <= top || x1 <= x0 {
            continue;
        }
        let start = x0.max(0);
        let end = x1.min(width as i64);
        for x in start..end {
            let mm = x as f64 / s;
            let module = ((mm - code.x) / code.module).floor() as i64;
            let on = if draft {
                bits.iter().copied().sum::<u8>() as f32 / bits.len().max(1) as f32
            } else if module >= 0 && (module as usize) < bits.len() {
                f32::from(bits[module as usize])
            } else {
                0.0
            };
            for y in top..bottom {
                let at = y as usize * width + x as usize;
                coverage[at] = coverage[at].max(on);
            }
        }
    }
}

fn draw_color_line(
    canvas: &mut Canvas,
    line: &EdgeColorLine,
    x0: i64,
    x1: i64,
    edge_y: i64,
    s: f64,
    roll_start: f64,
) {
    let color = match line.color.to_lowercase().as_str() {
        "green" => [52, 150, 67],
        "red" => [239, 27, 4],
        _ => return,
    };
    let center = edge_y - px(line.at, s);
    let h = px(line.thickness.unwrap_or(COLOR_LINE_THICK_MM), s).max(1) as i64;
    let (y0, y1) = ((center - h / 2).max(0), center - h / 2 + h);
    if y1 <= y0 || y0 >= canvas.height as i64 {
        return;
    }
    let y1 = y1.min(canvas.height as i64);
    if !line.dashed {
        let (cx0, cx1) = (x0.max(0), x1.min(canvas.width as i64));
        for y in y0..y1 {
            for x in cx0..cx1 {
                canvas.set(x as usize, y as usize, color);
            }
        }
        return;
    }
    let roll_left = roll_start;
    let roll_right = roll_start + (x1 - x0) as f64 / s;
    let first = ((roll_left - PERF_PHASE) / PERF_PITCH).floor() as i64;
    let last = ((roll_right - PERF_PHASE) / PERF_PITCH).ceil() as i64;
    for j in first..=last {
        if ((j + 4) % 10) >= DASH_RUN_PITCHES as i64 {
            continue;
        }
        let run_start = PERF_PHASE + j as f64 * PERF_PITCH;
        let run_end = PERF_PHASE + (j + 1) as f64 * PERF_PITCH;
        let cx0 = (x0 as f64 + (run_start - roll_start) * s).round() as i64;
        let cx1 = (x0 as f64 + (run_end - roll_start) * s).round() as i64;
        for y in y0..y1 {
            for x in cx0.max(0)..cx1.min(canvas.width as i64) {
                canvas.set(x as usize, y as usize, color);
            }
        }
    }
}

fn draw_hole(canvas: &mut Canvas, cx: f64, cy: f64, radius: f64, palette: &Palette) {
    let x0 = (cx - radius - 1.0).floor() as i64;
    let y0 = (cy - radius - 1.0).floor() as i64;
    let x1 = (cx + radius + 1.0).ceil() as i64;
    let y1 = (cy + radius + 1.0).ceil() as i64;
    let w = (x1 - x0).max(1) as usize;
    let h = (y1 - y0).max(1) as usize;
    let mut hole = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let px_x = x0 + x as i64;
            let px_y = y0 + y as i64;
            let distance =
                ((px_x as f64 + 0.5 - cx).powi(2) + (px_y as f64 + 0.5 - cy).powi(2)).sqrt();
            hole[y * w + x] = (radius - distance + 0.5).clamp(0.0, 1.0) as f32;
        }
    }
    canvas.blend(x0, y0, x1, y1, &hole, palette.no_film);
    if let Some(rim) = palette.rim {
        let inner = (radius - PERF_RIM * radius.max(1.0)).max(0.0);
        let mut rim_cov = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let px_x = x0 + x as i64;
                let px_y = y0 + y as i64;
                let distance =
                    ((px_x as f64 + 0.5 - cx).powi(2) + (px_y as f64 + 0.5 - cy).powi(2)).sqrt();
                rim_cov[y * w + x] = (hole[y * w + x]
                    - ((inner - distance + 0.5).clamp(0.0, 1.0) as f32))
                    .clamp(0.0, 1.0);
            }
        }
        canvas.blend(x0, y0, x1, y1, &rim_cov, rim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::film_edge::edge::edge_style;
    use crate::services::film_edge::geometry::{SheetFormat, film_geometry};

    #[test]
    fn a_strip_fills_its_rebate_and_punches_holes() {
        let geometry = film_geometry(SheetFormat::FullFrame, "6x6");
        let mut canvas = Canvas::new(400, 140);
        let style = edge_style(None, None, "Kodak Portra 400", "Kodak");
        paint_strip(
            &mut canvas,
            0.0,
            0.0,
            &geometry,
            &style,
            6,
            0.0,
            38.0,
            4.0,
            &Palette {
                no_film: [255; 3],
                rebate: [10, 10, 10],
                rim: None,
                ink: [230; 3],
                label: [230; 3],
            },
            false,
            None,
        );
        assert!(canvas.pixels.chunks(3).any(|pixel| pixel == [255; 3]));
        assert!(canvas.pixels.chunks(3).any(|pixel| pixel == [10, 10, 10]));
    }
}
