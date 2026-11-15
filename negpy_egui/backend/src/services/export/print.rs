//! Paper layout and the shared print properties in `negpy/services/export/print.py`.
//!
//! Pure geometry: integer sizes for one export, plus the toned paper white the carrier
//! and the mat both read.

use crate::domain::models::{ExportConfig, ExportResolutionMode, FinishConfig, ToningConfig};
use crate::features::exposure::analysis::working_oetf_decode;
use crate::features::toning::logic::{apply_chemical_toning, apply_split_toning};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The paper color in scene-linear: the toned white, or the picked hex decoded.
#[must_use]
pub fn effective_paper_linear(finish: &FinishConfig, toning: &ToningConfig) -> [f32; 3] {
    if finish.border_match_paper {
        return toned_paper_white(toning);
    }
    let hex = finish.border_color.trim_start_matches('#');
    let channel = |index: usize| -> f32 {
        hex.get(index * 2..index * 2 + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .map_or(1.0, |value| value as f32 / 255.0)
    };
    let srgb = [channel(0), channel(1), channel(2)];
    let linear = working_oetf_decode(&srgb);
    [linear[0], linear[1], linear[2]]
}

/// The toned paper white in scene-linear.
#[must_use]
pub fn toned_paper_white(toning: &ToningConfig) -> [f32; 3] {
    let white = ImageBuffer::from_rgb(1, 1, vec![1.0; 3]);
    let tinted = apply_chemical_toning(
        &white,
        toning.selenium_strength,
        toning.sepia_strength,
        toning.gold_strength,
        toning.blue_strength,
        toning.copper_strength,
        toning.vanadium_strength,
        false,
    );
    let tinted = apply_split_toning(
        &tinted,
        toning.shadow_tint_hue,
        toning.shadow_tint_strength,
        toning.highlight_tint_hue,
        toning.highlight_tint_strength,
    );
    [tinted.pixels[0], tinted.pixels[1], tinted.pixels[2]]
}

/// The mat color in display-encoded hex, the picked one or the toned paper white.
#[must_use]
pub fn effective_border_color(finish: &FinishConfig, toning: &ToningConfig) -> String {
    if !finish.border_match_paper {
        return finish.border_color.clone();
    }
    let white = toned_paper_white(toning);
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(white[0]),
        channel(white[1]),
        channel(white[2])
    )
}

/// The largest image inside `max_w` x `max_h`; integer math keeps an exact side exact.
#[must_use]
pub fn fit_content(img_w: usize, img_h: usize, max_w: usize, max_h: usize) -> (usize, usize) {
    if img_w * max_h > max_w * img_h {
        (max_w, max_w * img_h / img_w)
    } else {
        (max_h * img_w / img_h, max_h)
    }
}

/// Long-edge paper size in pixels for a non-ORIGINAL mode.
#[must_use]
pub fn paper_long_edge_px(export: &ExportConfig) -> usize {
    if export.export_resolution_mode == ExportResolutionMode::TargetPx {
        return (export.export_target_long_edge_px.max(1)) as usize;
    }
    ((export.export_print_size / 2.54) * export.export_dpi as f64) as usize
}

/// The physical DPI for PRINT/ORIGINAL, the derived virtual DPI for TARGET_PX.
#[must_use]
pub fn effective_dpi(export: &ExportConfig) -> i64 {
    if export.export_resolution_mode == ExportResolutionMode::TargetPx {
        let denom = export.export_print_size.max(0.1) / 2.54;
        return ((export.export_target_long_edge_px as f64 / denom) as i64).max(1);
    }
    export.export_dpi
}

/// An aspect string as a ratio; an unreadable one reads as square.
#[must_use]
pub fn fair_paper_ratio(aspect_ratio: &str) -> f64 {
    aspect_ratio
        .split_once(':')
        .and_then(|(w, h)| {
            let w: f64 = w.parse().ok()?;
            let h: f64 = h.parse().ok()?;
            (h != 0.0).then_some(w / h)
        })
        .unwrap_or(1.0)
}

/// Paper dimensions in pixels from a long edge and an aspect ratio.
#[must_use]
pub fn paper_dims_from_long_edge(
    long_edge_px: usize,
    aspect_ratio: &str,
    img_w: usize,
    img_h: usize,
) -> (usize, usize) {
    if aspect_ratio.eq_ignore_ascii_case("original") {
        if img_w >= img_h {
            return (long_edge_px, long_edge_px * img_h / img_w.max(1));
        }
        return (long_edge_px * img_w / img_h.max(1), long_edge_px);
    }
    let ratio = fair_paper_ratio(aspect_ratio);
    if ratio >= 1.0 {
        let paper_w = long_edge_px;
        (paper_w, (paper_w as f64 / ratio) as usize)
    } else {
        let paper_h = long_edge_px;
        ((paper_h as f64 * ratio) as usize, paper_h)
    }
}

/// A vertical offset that splits the padding top:bottom like the borders.
#[must_use]
pub fn weighted_offset_y(
    paper_h: usize,
    target_h: usize,
    border_px: usize,
    border_bottom_px: usize,
) -> usize {
    let pad_y = paper_h.saturating_sub(target_h);
    if border_px == 0 {
        return pad_y / 2;
    }
    (pad_y as f64 * border_px as f64 / (border_px + border_bottom_px) as f64).round() as usize
}

/// One export layout: the scaled content and its rectangle on the paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub image: ImageBuffer,
    /// `(x, y, width, height)` of the content inside the paper.
    pub content_rect: (usize, usize, usize, usize),
}

#[must_use]
pub fn apply_layout(
    img: &ImageBuffer,
    export: &ExportConfig,
    border_size: f64,
    border_color: &str,
    finish: &FinishConfig,
) -> Layout {
    let weight = finish.border_bottom_weight.max(1.0);
    let dpi = effective_dpi(export) as f64;
    let border_px = ((border_size / 2.54) * dpi) as usize;
    let border_bottom_px = (border_px as f64 * weight) as usize;
    let border_y_px = border_px + border_bottom_px;
    let (img_w, img_h) = (img.width, img.height);

    let (target_w, target_h, paper_w, paper_h) =
        if export.export_resolution_mode == ExportResolutionMode::Original {
            let (target_w, target_h) = (img_w, img_h);
            if export.paper_aspect_ratio.eq_ignore_ascii_case("original") {
                (
                    target_w,
                    target_h,
                    target_w + 2 * border_px,
                    target_h + border_y_px,
                )
            } else {
                let ratio = export
                    .paper_aspect_ratio
                    .split_once(':')
                    .and_then(|(w, h)| {
                        let w: f64 = w.parse().ok()?;
                        let h: f64 = h.parse().ok()?;
                        (h != 0.0).then_some(w / h)
                    })
                    .unwrap_or(img_w as f64 / img_h as f64);
                let min_w = target_w + 2 * border_px;
                let min_h = target_h + border_y_px;
                if min_w as f64 / min_h as f64 > ratio {
                    (target_w, target_h, min_w, (min_w as f64 / ratio) as usize)
                } else {
                    (target_w, target_h, (min_h as f64 * ratio) as usize, min_h)
                }
            }
        } else {
            let long_px = paper_long_edge_px(export);
            if export.paper_aspect_ratio.eq_ignore_ascii_case("original") {
                if img_w >= img_h {
                    let target_w = long_px.saturating_sub(2 * border_px).max(10);
                    let target_h = target_w * img_h / img_w.max(1);
                    (
                        target_w,
                        target_h,
                        target_w + 2 * border_px,
                        target_h + border_y_px,
                    )
                } else {
                    let target_h = long_px.saturating_sub(border_y_px).max(10);
                    let target_w = target_h * img_w / img_h.max(1);
                    (
                        target_w,
                        target_h,
                        target_w + 2 * border_px,
                        target_h + border_y_px,
                    )
                }
            } else {
                let (paper_w, paper_h) =
                    paper_dims_from_long_edge(long_px, &export.paper_aspect_ratio, img_w, img_h);
                let max_w = paper_w.saturating_sub(2 * border_px).max(10);
                let max_h = paper_h.saturating_sub(border_y_px).max(10);
                let (target_w, target_h) = fit_content(img_w, img_h, max_w, max_h);
                (target_w, target_h, paper_w, paper_h)
            }
        };

    let scaled = scale_to(img, target_w, target_h);
    let offset_x = (paper_w.saturating_sub(target_w)) / 2;
    let offset_y = weighted_offset_y(paper_h, target_h, border_px, border_bottom_px);
    if (paper_w, paper_h, offset_x, offset_y) == (target_w, target_h, 0, 0) {
        return Layout {
            image: scaled,
            content_rect: (0, 0, target_w, target_h),
        };
    }
    let hex = border_color.trim_start_matches('#');
    let channel = |index: usize| -> f32 {
        hex.get(index * 2..index * 2 + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .map_or(1.0, |value| value as f32 / 255.0)
    };
    let mut paper = vec![0.0f32; paper_w * paper_h * 3];
    for pixel in paper.chunks_exact_mut(3) {
        pixel.copy_from_slice(&[channel(0), channel(1), channel(2)]);
    }
    let copy_w = target_w.min(paper_w.saturating_sub(offset_x));
    let copy_h = target_h.min(paper_h.saturating_sub(offset_y));
    for y in 0..copy_h {
        let dst = ((offset_y + y) * paper_w + offset_x) * 3;
        let src = y * target_w * 3;
        paper[dst..dst + copy_w * 3].copy_from_slice(&scaled.pixels[src..src + copy_w * 3]);
    }
    Layout {
        image: ImageBuffer::from_rgb(paper_w, paper_h, paper),
        content_rect: (offset_x, offset_y, copy_w, copy_h),
    }
}

/// Area downscale or cubic upscale to the target size.
fn scale_to(img: &ImageBuffer, target_w: usize, target_h: usize) -> ImageBuffer {
    if target_w == img.width && target_h == img.height {
        return img.clone();
    }
    let shrinking = target_w < img.width || target_h < img.height;
    let pixels = if shrinking {
        negpy_core::resize::resize_area(&img.pixels, img.height, img.width, 3, target_h, target_w)
            .unwrap_or_default()
    } else {
        let mut out = vec![0.0; target_w * target_h * 3];
        negpy_core::resize::resize_kernel(
            &img.pixels,
            (img.height, img.width, 3),
            (target_h, target_w),
            negpy_core::resize::Kernel::Cubic,
            &mut out,
        );
        out
    };
    ImageBuffer::from_rgb(target_w, target_h, pixels)
}
