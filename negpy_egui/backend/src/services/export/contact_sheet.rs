//! Mirrors `negpy/services/export/contact_sheet.py`: frames and film strips on paper.

use ab_glyph::Font;

use crate::services::export::contact_sheet_layout::{
    STRIP_GAP, SheetPage, SheetPlan, StripPlacement,
};
use crate::services::export::contact_sheet_roll::SheetLook;
use crate::services::film_edge::edge::ascii_upper;
use crate::services::film_edge::glyphs::{Canvas, TextRun};
use crate::services::film_edge::paint::{Palette, paint_strip, px};

pub const COVER_TOLERANCE: f64 = 0.03;
pub const LABEL_CAP: f64 = 2.0;
pub const LABEL_ABOVE_STRIP: f64 = 3.0;

/// One frame's pixels, indexed by the plan's slots.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Tile {
    #[must_use]
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }
}

fn grey(level: i32) -> [u8; 3] {
    let value = level.clamp(0, 255) as u8;
    [value, value, value]
}

/// `palette_for`: the paper and ink colors of one sheet look.
#[must_use]
pub fn palette_for(look: &SheetLook) -> Palette {
    let mut palette = film_palette(look);
    if look.white_paper {
        palette.no_film = [255, 255, 255];
        palette.rim = None;
        palette.label = grey(look.black);
    }
    palette
}

fn film_palette(look: &SheetLook) -> Palette {
    let black = look.black;
    if look.palette == "slide" {
        let white = [241, 240, 236];
        return Palette {
            no_film: white,
            rebate: grey(black),
            rim: None,
            ink: white,
            label: grey(black),
        };
    }
    if look.palette == "color" {
        let warm = [236, 222, 188];
        return Palette {
            no_film: [
                (black + 8).clamp(0, 255) as u8,
                (black + 2).clamp(0, 255) as u8,
                (black + 2).clamp(0, 255) as u8,
            ],
            rebate: grey(black + 5),
            rim: Some([
                (black + 15).clamp(0, 255) as u8,
                (black + 7).clamp(0, 255) as u8,
                (black + 6).clamp(0, 255) as u8,
            ]),
            ink: warm,
            label: warm,
        };
    }
    Palette {
        no_film: grey(black),
        rebate: grey(black + 5),
        rim: Some(grey(black + 10)),
        ink: grey(224),
        label: grey(224),
    }
}

fn place_tile(canvas: &mut Canvas, tile: &Tile, box_: (i64, i64, i64, i64)) {
    let (x0, y0, x1, y1) = box_;
    let ww = (x1 - x0).max(0) as usize;
    let wh = (y1 - y0).max(0) as usize;
    if ww == 0 || wh == 0 || tile.width == 0 || tile.height == 0 {
        return;
    }
    let tile_aspect = tile.width as f64 / tile.height as f64;
    let window_aspect = ww as f64 / wh as f64;
    let cover = (tile_aspect / window_aspect).ln().abs() <= (1.0 + COVER_TOLERANCE).ln();
    let scale = if cover {
        (ww as f64 / tile.width as f64).max(wh as f64 / tile.height as f64)
    } else {
        (ww as f64 / tile.width as f64).min(wh as f64 / tile.height as f64)
    };
    let rw = ((tile.width as f64 * scale).round() as usize).max(1);
    let rh = ((tile.height as f64 * scale).round() as usize).max(1);
    let source: Vec<f32> = tile.pixels.iter().map(|value| f32::from(*value)).collect();
    let resized = if scale >= 1.0 {
        let mut dst = vec![0.0f32; rw * rh * 3];
        negpy_core::resize::resize_kernel(
            &source,
            (tile.height, tile.width, 3),
            (rh, rw),
            negpy_core::resize::Kernel::Cubic,
            &mut dst,
        )
        .map(|()| dst)
    } else {
        negpy_core::resize::resize_area(&source, tile.height, tile.width, 3, rh, rw)
    };
    let Some(resized) = resized else {
        return;
    };
    let (offset_x, offset_y, width, height) = if cover {
        let cx = (rw.saturating_sub(ww)) / 2;
        let cy = (rh.saturating_sub(wh)) / 2;
        (x0 - cx as i64, y0 - cy as i64, ww, wh)
    } else {
        (
            x0 + ((ww.saturating_sub(rw)) / 2) as i64,
            y0 + ((wh.saturating_sub(rh)) / 2) as i64,
            rw,
            rh,
        )
    };
    for row in 0..height {
        for column in 0..width {
            let x = offset_x + column as i64;
            let y = offset_y + row as i64;
            if x < 0 || y < 0 || x >= canvas.width as i64 || y >= canvas.height as i64 {
                continue;
            }
            let sx = if cover {
                column + (rw.saturating_sub(ww)) / 2
            } else {
                column
            };
            let sy = if cover {
                row + (rh.saturating_sub(wh)) / 2
            } else {
                row
            };
            if sx >= rw || sy >= rh {
                continue;
            }
            let at = (sy * rw + sx) * 3;
            canvas.set(
                x as usize,
                y as usize,
                [
                    resized[at].clamp(0.0, 255.0).round() as u8,
                    resized[at + 1].clamp(0.0, 255.0).round() as u8,
                    resized[at + 2].clamp(0.0, 255.0).round() as u8,
                ],
            );
        }
    }
}

fn draw_strip(
    canvas: &mut Canvas,
    strip: &StripPlacement,
    plan: &SheetPlan,
    tiles: &[Option<Tile>],
    turns: &[i64],
    s: f64,
    look: &SheetLook,
    palette: &Palette,
    draft: bool,
    numbers: Option<&[usize]>,
) {
    paint_strip(
        canvas,
        strip.x,
        strip.y,
        &plan.geometry,
        &look.edge,
        plan.frame_count,
        strip.roll_start,
        strip.length,
        s,
        palette,
        draft,
        numbers,
    );
    let top = strip.y + plan.geometry.image_top();
    for index in strip.first..strip.first + strip.count {
        let Some(tile) = tiles.get(index).and_then(Option::as_ref) else {
            continue;
        };
        let turn = turns.get(index).copied().unwrap_or(0).rem_euclid(4);
        let turned = if turn == 0 {
            tile.clone()
        } else {
            rotate_tile(tile, turn)
        };
        let left = strip.x + plan.geometry.frame_center(index)
            - strip.roll_start
            - plan.geometry.frame_along / 2.0;
        place_tile(
            canvas,
            &turned,
            (
                px(left, s),
                px(top, s),
                px(left + plan.geometry.frame_along, s),
                px(top + plan.geometry.frame_across, s),
            ),
        );
    }
}

fn rotate_tile(tile: &Tile, turn: i64) -> Tile {
    let mut out = vec![0; tile.pixels.len()];
    let (w, h) = (tile.width, tile.height);
    let (dw, dh) = if turn % 2 == 0 { (w, h) } else { (h, w) };
    for y in 0..dh {
        for x in 0..dw {
            let (sx, sy) = match turn {
                1 => (y, h - 1 - x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (w - 1 - y, x),
            };
            let source = (sy * w + sx) * 3;
            let target = (y * dw + x) * 3;
            out[target..target + 3].copy_from_slice(&tile.pixels[source..source + 3]);
        }
    }
    Tile::new(dw, dh, out)
}

fn draw_row(
    canvas: &mut Canvas,
    page: &SheetPage,
    tiles: &[Option<Tile>],
    s: f64,
    look: &SheetLook,
) {
    for row in &page.rows {
        let (x0, y0, x1, y1) = (
            px(row.x, s).max(0),
            px(row.y, s).max(0),
            px(row.x + row.width, s).min(canvas.width as i64),
            px(row.y + row.height, s).min(canvas.height as i64),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                canvas.set(x as usize, y as usize, grey(look.black));
            }
        }
        for frame in &row.frames {
            let Some(tile) = tiles.get(frame.slot).and_then(Option::as_ref) else {
                continue;
            };
            place_tile(
                canvas,
                tile,
                (
                    px(frame.x, s),
                    px(frame.y, s),
                    px(frame.x + frame.width, s),
                    px(frame.y + frame.height, s),
                ),
            );
        }
    }
}

/// `label_caps`: the sheet's label in capitals, one part per dot.
#[must_use]
pub fn label_caps(text: &str) -> String {
    text.split('·')
        .filter(|part| !part.trim().is_empty())
        .map(ascii_upper)
        .collect::<Vec<_>>()
        .join(" · ")
}

fn draw_label(
    canvas: &mut Canvas,
    plan: &SheetPlan,
    page: usize,
    s: f64,
    look: &SheetLook,
    palette: &Palette,
) {
    if !plan.label || look.label.is_empty() {
        return;
    }
    let (block_x, block_y, block_w, _) = plan.pages[page].block;
    let text = label_caps(&look.label);
    let page_text = if plan.pages.len() > 1 {
        format!("{}/{}", page + 1, plan.pages.len())
    } else {
        String::new()
    };
    let mut run = TextRun::capped(&text, LABEL_CAP * s);
    let mut page_run = TextRun::capped(&page_text, LABEL_CAP * s);
    let left = (block_x * s) as f32;
    let right = ((block_x + block_w) * s) as f32;
    let reserved = page_run.width + STRIP_GAP as f32 * s as f32;
    run = fit_text(run, right - left - reserved);
    let baseline = ((block_y - LABEL_ABOVE_STRIP) * s) as f32;
    let mut coverage = vec![0.0f32; canvas.width * canvas.height];
    rasterize_run(
        &mut coverage,
        canvas.width,
        canvas.height,
        &run,
        left,
        baseline,
    );
    if !page_text.is_empty() {
        page_run = fit_text(page_run, right - left);
        rasterize_run(
            &mut coverage,
            canvas.width,
            canvas.height,
            &page_run,
            right - page_run.width,
            baseline,
        );
    }
    canvas.blend(
        0,
        0,
        canvas.width as i64,
        canvas.height as i64,
        &coverage,
        palette.label,
    );
}

fn fit_text(mut run: TextRun, max_width: f32) -> TextRun {
    if max_width <= 0.0 || run.width <= max_width {
        return run;
    }
    let ellipsis = "...";
    let mut text: String = run.text.chars().collect();
    while !text.is_empty() {
        let candidate = format!("{}{ellipsis}", text.trim_end());
        let candidate_run = TextRun::new(run.font.clone(), &candidate, run.size);
        if candidate_run.width <= max_width {
            return candidate_run;
        }
        text.pop();
    }
    run = TextRun::new(run.font.clone(), ellipsis, run.size);
    run
}

fn rasterize_run(
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

/// `render_sheet`: one uint8 sRGB page.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn render_sheet(
    plan: &SheetPlan,
    page: usize,
    tiles: &[Option<Tile>],
    turns: &[i64],
    px_per_mm: f64,
    look: &SheetLook,
    draft: bool,
    numbers: Option<&[usize]>,
) -> Canvas {
    let palette = palette_for(look);
    let width = px(plan.paper_width, px_per_mm).max(1) as usize;
    let height = px(plan.paper_height, px_per_mm).max(1) as usize;
    let mut canvas = Canvas::new(width, height);
    for pixel in canvas.pixels.chunks_mut(3) {
        pixel.copy_from_slice(&palette.no_film);
    }
    let Some(page_data) = plan.pages.get(page) else {
        return canvas;
    };
    for strip in &page_data.strips {
        draw_strip(
            &mut canvas,
            strip,
            plan,
            tiles,
            turns,
            px_per_mm,
            look,
            &palette,
            draft,
            numbers,
        );
    }
    draw_row(&mut canvas, page_data, tiles, px_per_mm, look);
    draw_label(&mut canvas, plan, page, px_per_mm, look, &palette);
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_folded_to_ascii_capitals() {
        assert_eq!(label_caps("Portra 400 · 35 mm"), "PORTRA 400 · 35 MM");
    }

    #[test]
    fn a_rotated_tile_swaps_its_axes() {
        let tile = Tile::new(2, 1, vec![1, 2, 3, 4, 5, 6]);
        let turned = rotate_tile(&tile, 1);
        assert_eq!((turned.width, turned.height), (1, 2));
    }
}
