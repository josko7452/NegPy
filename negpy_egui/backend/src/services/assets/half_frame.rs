//! Mirrors the half-frame slice geometry of `negpy/services/assets/half_frame.py`: the
//! hash suffix, the slice bounds and the point remap between two split geometries.

use crate::domain::models::WorkspaceConfig;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The separator between a file hash and its half index.
pub const SEP: char = '#';

/// The global setting that lists the scans the user split.
pub const SPLIT_SCANS_KEY: &str = "half_frame_scans";

/// The filled band between two joined halves, in output space.
pub const GAP_FILL: f32 = 0.0;

/// The largest gutter the split editor allows, as a fraction of the cropped extent.
pub const MAX_GUTTER_THICKNESS: f64 = 0.1;

/// A half-frame asset's crop and split, as its normalized coordinates read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HalfGeometry {
    /// Normalized `x1, y1, x2, y2`; None is the full frame.
    pub crop_rect: Option<[f64; 4]>,
    pub split_x: f64,
    pub gutter_thickness: f64,
    pub split_axis: char,
}

impl Default for HalfGeometry {
    fn default() -> Self {
        Self {
            crop_rect: None,
            split_x: 0.5,
            gutter_thickness: 0.0,
            split_axis: 'x',
        }
    }
}

/// One `slice_half` argument tuple.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HalfSlice {
    pub half: i32,
    pub split_x: f64,
    pub crop_rect: Option<[f64; 4]>,
    pub gutter_thickness: f64,
    pub split_axis: char,
}

/// The file hash of one half.
#[must_use]
pub fn half_hash(file_hash: &str, half: i32) -> String {
    format!("{file_hash}{SEP}{half}")
}

/// The half index of a `#`-suffixed hash, or None when the suffix is not a number.
#[must_use]
pub fn half_of(file_hash: Option<&str>) -> Option<i32> {
    let (_, suffix) = file_hash?.rsplit_once(SEP)?;
    suffix.parse::<i32>().ok()
}

/// The unsuffixed file hash shared by both halves.
#[must_use]
pub fn base_hash(file_hash: Option<&str>) -> Option<String> {
    let hash = file_hash?;
    Some(
        hash.split_once(SEP)
            .map_or(hash, |(base, _)| base)
            .to_string(),
    )
}

/// A half's display name.
#[must_use]
pub fn half_name(name: &str, half: i32) -> String {
    format!("{name} [{half}]")
}

fn px(value: f64) -> i64 {
    value.round_ties_even() as i64
}

fn slice_axis_bounds(
    (height, width): (usize, usize),
    half: i32,
    split_x: f64,
    crop_rect: Option<[f64; 4]>,
    gutter_thickness: f64,
) -> (usize, usize, usize, usize) {
    let (mut x1, mut y1, mut x2, mut y2) = (0, 0, width as i64, height as i64);
    if let Some(rect) = crop_rect {
        let (rx1, ry1, rx2, ry2) = (rect[0], rect[1], rect[2], rect[3]);
        let cx1 = px(width as f64 * rx1).clamp(0, width as i64);
        let cx2 = px(width as f64 * rx2).clamp(0, width as i64);
        let cy1 = px(height as f64 * ry1).clamp(0, height as i64);
        let cy2 = px(height as f64 * ry2).clamp(0, height as i64);
        if cx2 <= cx1 || cy2 <= cy1 {
            return (0, height, 0, width.min(1));
        }
        (x1, y1, x2, y2) = (cx1, cy1, cx2, cy2);
    }
    if half == 0 {
        return (y1 as usize, y2 as usize, x1 as usize, x2 as usize);
    }
    let cropped_width = x2 - x1;
    let split = px(cropped_width as f64 * split_x).clamp(1, cropped_width - 1);
    if gutter_thickness > 0.0 {
        let gutter = px(cropped_width as f64 * gutter_thickness).max(1);
        let lo = (split - gutter / 2).max(1);
        let hi = (lo + gutter).min(cropped_width - 1);
        let lo = (hi - gutter).max(1);
        return if half == 1 {
            (y1 as usize, y2 as usize, x1 as usize, (x1 + lo) as usize)
        } else {
            (y1 as usize, y2 as usize, (x1 + hi) as usize, x2 as usize)
        };
    }
    if half == 1 {
        (y1 as usize, y2 as usize, x1 as usize, (x1 + split) as usize)
    } else {
        (y1 as usize, y2 as usize, (x1 + split) as usize, x2 as usize)
    }
}

/// The pixel bounds one half reads. A `y` split is the transposed `x` problem.
#[must_use]
pub fn slice_half_bounds(
    (height, width): (usize, usize),
    half: i32,
    split_x: f64,
    crop_rect: Option<[f64; 4]>,
    gutter_thickness: f64,
    split_axis: char,
) -> (usize, usize, usize, usize) {
    if split_axis == 'y' {
        let transposed = crop_rect.map(|rect| [rect[1], rect[0], rect[3], rect[2]]);
        let (ty1, ty2, tx1, tx2) =
            slice_axis_bounds((width, height), half, split_x, transposed, gutter_thickness);
        return (tx1, tx2, ty1, ty2);
    }
    slice_axis_bounds((height, width), half, split_x, crop_rect, gutter_thickness)
}

/// The full-resolution dimensions of a half slice.
#[must_use]
pub fn slice_half_dimensions(dimensions: (usize, usize), slice: &HalfSlice) -> (usize, usize) {
    let (y1, y2, x1, x2) = slice_half_bounds(
        dimensions,
        slice.half,
        slice.split_x,
        slice.crop_rect,
        slice.gutter_thickness,
        slice.split_axis,
    );
    (y2 - y1, x2 - x1)
}

/// One half of a decoded buffer; `half = 0` crops without splitting.
#[must_use]
pub fn slice_half(buf: &ImageBuffer, slice: &HalfSlice) -> ImageBuffer {
    let (y1, y2, x1, x2) = slice_half_bounds(
        (buf.height, buf.width),
        slice.half,
        slice.split_x,
        slice.crop_rect,
        slice.gutter_thickness,
        slice.split_axis,
    );
    let mut pixels = Vec::with_capacity((y2 - y1) * (x2 - x1) * 3);
    for y in y1..y2 {
        let start = (y * buf.width + x1) * 3;
        pixels.extend_from_slice(&buf.pixels[start..start + (x2 - x1) * 3]);
    }
    ImageBuffer::from_rgb(x2 - x1, y2 - y1, pixels)
}

/// The buffer cut by each slice in turn.
#[must_use]
pub fn slice_chain(buf: &ImageBuffer, slices: &[HalfSlice]) -> ImageBuffer {
    let mut out = buf.clone();
    for slice in slices {
        out = slice_half(&out, slice);
    }
    out
}

/// Half-local normalized point to full-scan normalized point.
#[must_use]
pub fn to_scan(x: f64, y: f64, half: i32, geometry: &HalfGeometry) -> (f64, f64) {
    let [x1, y1, x2, y2] = geometry.crop_rect.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let (cw, ch) = ((x2 - x1).max(1e-9), (y2 - y1).max(1e-9));
    let (cx, cy) = if half == 0 {
        (x, y)
    } else {
        let lo = (geometry.split_x - geometry.gutter_thickness / 2.0).max(0.0);
        let hi = (geometry.split_x + geometry.gutter_thickness / 2.0).min(1.0);
        if geometry.split_axis == 'y' {
            if half == 1 {
                (x, y * lo)
            } else {
                (x, hi + y * (1.0 - hi))
            }
        } else if half == 1 {
            (x * lo, y)
        } else {
            (hi + x * (1.0 - hi), y)
        }
    };
    (x1 + cx * cw, y1 + cy * ch)
}

/// Full-scan normalized point to half-local normalized point, the inverse of [`to_scan`].
#[must_use]
pub fn from_scan(fx: f64, fy: f64, half: i32, geometry: &HalfGeometry) -> (f64, f64) {
    let [x1, y1, x2, y2] = geometry.crop_rect.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let (cw, ch) = ((x2 - x1).max(1e-9), (y2 - y1).max(1e-9));
    let (cx, cy) = ((fx - x1) / cw, (fy - y1) / ch);
    if half == 0 {
        return (cx, cy);
    }
    let lo = (geometry.split_x - geometry.gutter_thickness / 2.0).max(0.0);
    let hi = (geometry.split_x + geometry.gutter_thickness / 2.0).min(1.0);
    if geometry.split_axis == 'y' {
        let y = if half == 1 {
            cy / lo.max(1e-9)
        } else {
            (cy - hi) / (1.0 - hi).max(1e-9)
        };
        (cx, y)
    } else {
        let x = if half == 1 {
            cx / lo.max(1e-9)
        } else {
            (cx - hi) / (1.0 - hi).max(1e-9)
        };
        (x, cy)
    }
}

/// Re-anchor a half-local point to the same film location under a changed geometry.
#[must_use]
pub fn remap_point(
    x: f64,
    y: f64,
    half: i32,
    old_geometry: &HalfGeometry,
    new_geometry: &HalfGeometry,
) -> (f64, f64) {
    let (fx, fy) = to_scan(x, y, half, old_geometry);
    from_scan(fx, fy, half, new_geometry)
}

/// The width of the diptych gap, from the two rendered halves' extents along the split.
#[must_use]
pub fn gap_px(left_width: usize, right_width: usize, gutter_thickness: f64) -> usize {
    if gutter_thickness <= 0.0 || gutter_thickness >= 1.0 {
        return 0;
    }
    px((left_width + right_width) as f64 * gutter_thickness / (1.0 - gutter_thickness)).max(0)
        as usize
}

/// Join two rendered halves with a filled `gap`-wide band. Unequal cross sizes are
/// centre-padded, never resampled.
#[must_use]
pub fn join_halves(left: &ImageBuffer, right: &ImageBuffer, gap: usize, axis: char) -> ImageBuffer {
    let (join_axis_x, pad_height) = if axis == 'x' {
        (true, left.height.max(right.height))
    } else {
        (false, left.width.max(right.width))
    };
    let width = if join_axis_x {
        left.width + gap + right.width
    } else {
        pad_height
    };
    let height = if join_axis_x {
        pad_height
    } else {
        left.height + gap + right.height
    };
    let mut pixels = vec![GAP_FILL; width * height * 3];
    let blit = |source: &ImageBuffer, pixels: &mut [f32], offset_x: usize, offset_y: usize| {
        let (pad_x, pad_y) = if join_axis_x {
            (0, pad_height.saturating_sub(source.height) / 2)
        } else {
            (pad_height.saturating_sub(source.width) / 2, 0)
        };
        for y in 0..source.height {
            for x in 0..source.width {
                let dst = ((offset_y + pad_y + y) * width + offset_x + pad_x + x) * 3;
                let src = (y * source.width + x) * 3;
                pixels[dst..dst + 3].copy_from_slice(&source.pixels[src..src + 3]);
            }
        }
    };
    if join_axis_x {
        blit(left, &mut pixels, 0, 0);
        blit(right, &mut pixels, left.width + gap, 0);
    } else {
        blit(left, &mut pixels, 0, 0);
        blit(right, &mut pixels, 0, left.height + gap);
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

/// Re-anchor every position-based manual edit in a half's config after its crop or
/// split changes. The crop and analysis rects have no place on the new boundary.
#[must_use]
pub fn remap_workspace_config(
    config: &WorkspaceConfig,
    half: i32,
    old_geometry: &HalfGeometry,
    new_geometry: &HalfGeometry,
) -> WorkspaceConfig {
    if old_geometry == new_geometry {
        return config.clone();
    }
    let mut out = config.clone();
    let point = |x: f64, y: f64| remap_point(x, y, half, old_geometry, new_geometry);
    let point_value = |value: &serde_json::Value| -> Option<[f64; 2]> {
        let x = value.get(0)?.as_f64()?;
        let y = value.get(1)?.as_f64()?;
        let (x, y) = point(x, y);
        Some([x, y])
    };
    let remap_stroke = |stroke: &serde_json::Value| -> serde_json::Value {
        let Some(items) = stroke.as_array() else {
            return stroke.clone();
        };
        let Some(points) = items.first().and_then(serde_json::Value::as_array) else {
            return stroke.clone();
        };
        let mapped: Vec<serde_json::Value> = points
            .iter()
            .map(|point| serde_json::json!(point_value(point).unwrap_or([0.0, 0.0])))
            .collect();
        let mut out = items.clone();
        out[0] = serde_json::Value::Array(mapped);
        serde_json::Value::Array(out)
    };
    let remap_spot = |spot: &serde_json::Value| -> serde_json::Value {
        let Some(items) = spot.as_array() else {
            return spot.clone();
        };
        let Some([x, y]) = point_value(spot).map(|point| point) else {
            return spot.clone();
        };
        let mut out = items.clone();
        out[0] = serde_json::json!(x);
        out[1] = serde_json::json!(y);
        serde_json::Value::Array(out)
    };
    let remap_line = |line: &serde_json::Value| -> serde_json::Value {
        let Some(items) = line.as_array() else {
            return line.clone();
        };
        if items.len() < 4 {
            return line.clone();
        }
        let start = point(
            items[0].as_f64().unwrap_or(0.0),
            items[1].as_f64().unwrap_or(0.0),
        );
        let end = point(
            items[2].as_f64().unwrap_or(0.0),
            items[3].as_f64().unwrap_or(0.0),
        );
        let mut out = items.clone();
        out[0] = serde_json::json!(start.0);
        out[1] = serde_json::json!(start.1);
        out[2] = serde_json::json!(end.0);
        out[3] = serde_json::json!(end.1);
        serde_json::Value::Array(out)
    };
    out.retouch.manual_heal_strokes = out
        .retouch
        .manual_heal_strokes
        .iter()
        .map(remap_stroke)
        .collect();
    out.retouch.scratch_lines = out.retouch.scratch_lines.iter().map(remap_line).collect();
    out.retouch.clone_strokes = out.retouch.clone_strokes.iter().map(remap_stroke).collect();
    out.retouch.manual_dust_spots = out
        .retouch
        .manual_dust_spots
        .iter()
        .map(remap_spot)
        .collect();
    out.retouch.dust_exclusion_strokes = out
        .retouch
        .dust_exclusion_strokes
        .iter()
        .map(remap_stroke)
        .collect();
    for mask in &mut out.local.masks {
        for vertex in &mut mask.vertices {
            let (x, y) = point(vertex[0], vertex[1]);
            *vertex = [x, y];
        }
    }
    out.geometry.crop_rect = None;
    out.process.analysis_rect = None;
    out.process.meter_rect = None;
    out.process.meter_key.clear();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_suffix_names_the_half() {
        assert_eq!(half_hash("abc", 2), "abc#2");
        assert_eq!(half_of(Some("abc#2")), Some(2));
        assert_eq!(half_of(Some("abc#stitch")), None);
        assert_eq!(base_hash(Some("abc#2")), Some("abc".to_string()));
    }

    #[test]
    fn a_left_split_reads_the_first_half() {
        let bounds = slice_half_bounds((100, 200), 1, 0.5, None, 0.0, 'x');
        assert_eq!(bounds, (0, 100, 0, 100));
        let bounds = slice_half_bounds((100, 200), 2, 0.5, None, 0.0, 'x');
        assert_eq!(bounds, (0, 100, 100, 200));
    }

    #[test]
    fn the_gutter_takes_its_width_off_the_split() {
        let bounds = slice_half_bounds((100, 200), 1, 0.5, None, 0.1, 'x');
        assert_eq!(bounds.3, 90);
        let bounds = slice_half_bounds((100, 200), 2, 0.5, None, 0.1, 'x');
        assert_eq!(bounds.2, 110);
    }

    #[test]
    fn the_point_maps_out_and_back() {
        let geometry = HalfGeometry {
            crop_rect: Some([0.1, 0.1, 0.9, 0.9]),
            split_x: 0.5,
            gutter_thickness: 0.0,
            split_axis: 'x',
        };
        let (fx, fy) = to_scan(0.25, 0.75, 1, &geometry);
        let (x, y) = from_scan(fx, fy, 1, &geometry);
        assert!((x - 0.25).abs() < 1e-12);
        assert!((y - 0.75).abs() < 1e-12);
    }

    #[test]
    fn a_join_fills_the_gap_and_centres_a_shorter_half() {
        let left = ImageBuffer::from_rgb(2, 4, vec![1.0; 24]);
        let right = ImageBuffer::from_rgb(2, 2, vec![0.5; 12]);
        let joined = join_halves(&left, &right, 1, 'x');
        assert_eq!((joined.width, joined.height), (5, 4));
        assert_eq!(joined.pixels[0], 1.0);
        assert_eq!(joined.pixels[(2 * 5 + 2) * 3], GAP_FILL);
    }

    #[test]
    fn a_remap_keeps_the_film_location() {
        let old = HalfGeometry::default();
        let new = HalfGeometry {
            crop_rect: Some([0.2, 0.0, 1.0, 1.0]),
            ..HalfGeometry::default()
        };
        let (fx, fy) = to_scan(0.5, 0.5, 1, &old);
        let moved = remap_point(0.5, 0.5, 1, &old, &new);
        let (nx, ny) = to_scan(moved.0, moved.1, 1, &new);
        assert!((nx - fx).abs() < 1e-12);
        assert!((ny - fy).abs() < 1e-12);
    }
}
