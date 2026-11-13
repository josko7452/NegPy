//! Trichromatic capture: classify three narrowband exposures and assemble one frame.
//!
//! Mirrors `negpy/features/rgbscan/logic.py`.

use std::collections::HashMap;
use std::path::Path;

use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// Red, green and blue, in the demosaiced RGB axis order.
pub const RED: usize = 0;
pub const GREEN: usize = 1;
pub const BLUE: usize = 2;

const EST_WIDTH: usize = 1024;
const SIG_WIDTH: usize = 96;
const SIG_INTERIOR: f32 = 0.6;
const SIG_FLOOR: f32 = 1.0;
/// The weakest real triplet, under the strongest confusable one.
pub const MIN_TRIPLET_AFFINITY: f64 = 0.25;
const NARROWBAND_DOMINANCE: f64 = 2.0;

/// One RGB-scan frame: the three exposures assigned to R/G/B channels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Triplet {
    pub red: String,
    pub green: String,
    pub blue: String,
    pub ok: bool,
}

/// One file's classification evidence, from a single raw read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameProbe {
    pub means: [f64; 3],
    pub signature: Vec<f32>,
}

/// The dominant channel of an (R, G, B) mean triple.
#[must_use]
pub fn classify_channel(means: &[f64]) -> usize {
    let mut best = 0;
    for channel in 1..3 {
        if means[channel] > means[best] {
            best = channel;
        }
    }
    best
}

/// Black-subtracted per-Bayer-color means.
#[must_use]
pub fn channel_means(mosaic: &[f32], colors: &[u8], black: f64) -> [f64; 3] {
    negpy_core::rgbscan::channel_means(mosaic, colors, black)
}

/// A scene signature from the log density of a black-subtracted mosaic.
#[must_use]
pub fn scene_signature(mosaic: &[f32], height: usize, width: usize, black: f64) -> Vec<f32> {
    let block = ((width / SIG_WIDTH) & !1).max(2);
    let hh = height / block * block;
    let ww = width / block * block;
    if hh < block || ww < block {
        return vec![0.0];
    }
    let gh = hh / block;
    let gw = ww / block;
    let mut small = vec![0.0f32; gh * gw];
    for gy in 0..gh {
        for gx in 0..gw {
            let mut sum = 0.0f32;
            for y in 0..block {
                for x in 0..block {
                    sum += mosaic[(gy * block + y) * width + gx * block + x];
                }
            }
            small[gy * gw + gx] = sum / (block * block) as f32 - black as f32;
        }
    }
    let inset_y = ((1.0 - SIG_INTERIOR) * 0.5 * gh as f32) as usize;
    let inset_x = ((1.0 - SIG_INTERIOR) * 0.5 * gw as f32) as usize;
    if gh <= 2 * inset_y || gw <= 2 * inset_x {
        return vec![0.0];
    }
    let ih = gh - 2 * inset_y;
    let iw = gw - 2 * inset_x;
    let mut interior = vec![0.0f32; ih * iw];
    for y in 0..ih {
        for x in 0..iw {
            interior[y * iw + x] = small[(y + inset_y) * gw + x + inset_x].max(SIG_FLOOR).ln();
        }
    }
    let mut grad = vec![0.0f32; ih * iw];
    let at = |y: isize, x: isize| -> f32 {
        let yy = y.clamp(0, ih as isize - 1) as usize;
        let xx = x.clamp(0, iw as isize - 1) as usize;
        interior[yy * iw + xx]
    };
    for y in 0..ih {
        for x in 0..iw {
            let (yi, xi) = (y as isize, x as isize);
            let gx = if iw < 3 {
                0.0
            } else {
                (at(yi, xi + 1) - at(yi, xi - 1)) * 0.5
            };
            let gy = if ih < 3 {
                0.0
            } else {
                (at(yi + 1, xi) - at(yi - 1, xi)) * 0.5
            };
            grad[y * iw + x] = (gx * gx + gy * gy).sqrt();
        }
    }
    let mean = grad.iter().sum::<f32>() / grad.len() as f32;
    let centered: Vec<f32> = grad.iter().map(|value| value - mean).collect();
    let sd = (centered.iter().map(|value| value * value).sum::<f32>() / centered.len() as f32).sqrt();
    if sd > 0.0 {
        centered.iter().map(|value| value / sd).collect()
    } else {
        centered
    }
}

/// How much two signatures look like the same frame. 1 is identical, 0 unrelated.
#[must_use]
pub fn frame_affinity(a: Option<&[f32]>, b: Option<&[f32]>) -> f64 {
    let (Some(a), Some(b)) = (a, b) else {
        return 0.0;
    };
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let product: f64 = a
        .iter()
        .zip(b)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    product / a.len() as f64
}

/// How far the strongest channel stands above the next.
#[must_use]
pub fn channel_dominance(means: &[f64]) -> f64 {
    let mut ordered = [means[0], means[1], means[2]];
    ordered.sort_by(f64::total_cmp);
    if ordered[1] > 0.0 {
        ordered[2] / ordered[1]
    } else {
        f64::INFINITY
    }
}

/// Whether a folder was shot one color at a time.
#[must_use]
pub fn looks_narrowband(all_means: &[[f64; 3]]) -> bool {
    if all_means.is_empty() {
        return false;
    }
    let mut ratios: Vec<f64> = all_means.iter().map(|means| channel_dominance(means)).collect();
    ratios.sort_by(f64::total_cmp);
    ratios[ratios.len() / 2] >= NARROWBAND_DOMINANCE
}

/// Paths in capture order, or unchanged when any file states no capture time.
#[must_use]
pub fn capture_ordered(paths: &[String], times: &HashMap<String, String>) -> Vec<String> {
    if paths.is_empty() || paths.iter().any(|path| times.get(path).is_none_or(String::is_empty)) {
        return paths.to_vec();
    }
    let mut ordered = paths.to_vec();
    ordered.sort_by(|a, b| {
        let time = times.get(a).cmp(&times.get(b));
        time.then_with(|| {
            let name = |path: &str| {
                Path::new(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_lowercase()
            };
            name(a).cmp(&name(b))
        })
    });
    ordered
}

fn mutual_best(
    triplet: &Triplet,
    pools: &HashMap<usize, Vec<String>>,
    affinity: &impl Fn(&str, &str) -> f64,
) -> bool {
    let members = [&triplet.red, &triplet.green, &triplet.blue];
    for (channel, member) in members.iter().enumerate() {
        for want in [RED, GREEN, BLUE] {
            if want == channel {
                continue;
            }
            let Some(pool) = pools.get(&want) else {
                return false;
            };
            if pool.is_empty() {
                return false;
            }
            let best = pool
                .iter()
                .max_by(|a, b| affinity(member, a).total_cmp(&affinity(member, b)));
            if best != Some(members[want]) {
                return false;
            }
        }
    }
    true
}

/// Weakest pairwise agreement among a triplet's three exposures.
#[must_use]
pub fn triplet_affinity(triplet: &Triplet, signatures: &HashMap<String, Vec<f32>>) -> Option<f64> {
    let red = signatures.get(&triplet.red)?;
    let green = signatures.get(&triplet.green)?;
    let blue = signatures.get(&triplet.blue)?;
    Some(
        frame_affinity(Some(red), Some(green))
            .min(frame_affinity(Some(red), Some(blue)))
            .min(frame_affinity(Some(green), Some(blue))),
    )
}

fn affinity_lookup<'a>(
    signatures: &'a HashMap<String, Vec<f32>>,
) -> impl Fn(&str, &str) -> f64 + 'a {
    move |a, b| {
        frame_affinity(
            signatures.get(a).map(Vec::as_slice),
            signatures.get(b).map(Vec::as_slice),
        )
    }
}

/// Group classified files into consecutive triplets.
#[must_use]
pub fn group_triplets(
    items: &[(String, usize)],
    signatures: Option<&HashMap<String, Vec<f32>>>,
    min_affinity: f64,
) -> Vec<Triplet> {
    let mut pools: HashMap<usize, Vec<String>> = HashMap::new();
    if let Some(signatures) = signatures {
        for (path, channel) in items {
            if signatures.contains_key(path) {
                pools.entry(*channel).or_default().push(path.clone());
            }
        }
    }
    let affinity = signatures.map(|sigs| affinity_lookup(sigs));
    let mut triplets = Vec::new();
    for chunk in items.chunks(3) {
        let mut by_channel: HashMap<usize, &str> = HashMap::new();
        for (path, channel) in chunk {
            by_channel.insert(*channel, path);
        }
        let mut triplet = Triplet {
            red: by_channel.get(&RED).map_or_else(
                || chunk.first().map_or_else(String::new, |(path, _)| path.clone()),
                |path| (*path).to_string(),
            ),
            green: by_channel.get(&GREEN).map_or_else(
                || chunk.get(1).map_or_else(String::new, |(path, _)| path.clone()),
                |path| (*path).to_string(),
            ),
            blue: by_channel.get(&BLUE).map_or_else(
                || chunk.get(2).map_or_else(String::new, |(path, _)| path.clone()),
                |path| (*path).to_string(),
            ),
            ok: chunk.len() == 3 && by_channel.len() == 3,
        };
        if triplet.ok && let (Some(signatures), Some(affinity)) = (signatures, affinity.as_ref()) {
            let score = triplet_affinity(&triplet, signatures);
            if score.is_some_and(|score| {
                score < min_affinity || !mutual_best(&triplet, &pools, affinity)
            }) {
                triplet.ok = false;
            }
        }
        triplets.push(triplet);
    }
    triplets
}

/// Assign three hand-picked files to red, green and blue by their dominant channel.
#[must_use]
pub fn place_triplet(
    probes: &HashMap<String, Option<FrameProbe>>,
    min_affinity: f64,
) -> (Option<Triplet>, &'static str) {
    if probes.len() != 3 || probes.values().any(Option::is_none) {
        return (None, "channels");
    }
    let mut by_channel: HashMap<usize, String> = HashMap::new();
    let mut signatures = HashMap::new();
    for (path, probe) in probes {
        let Some(probe) = probe else {
            return (None, "channels");
        };
        by_channel.insert(classify_channel(&probe.means), path.clone());
        signatures.insert(path.clone(), probe.signature.clone());
    }
    if by_channel.len() != 3 {
        return (None, "channels");
    }
    let triplet = Triplet {
        red: by_channel.get(&RED).cloned().unwrap_or_default(),
        green: by_channel.get(&GREEN).cloned().unwrap_or_default(),
        blue: by_channel.get(&BLUE).cloned().unwrap_or_default(),
        ok: true,
    };
    if triplet_affinity(&triplet, &signatures).is_some_and(|score| score < min_affinity) {
        return (None, "picture");
    }
    (Some(triplet), "")
}

/// Sub-pixel translation of `mov` relative to `ref`, from phase correlation.
#[must_use]
pub fn estimate_shift(ref_gray: &[f32], mov_gray: &[f32], height: usize, width: usize) -> (f64, f64) {
    if width <= EST_WIDTH {
        let (dx, dy, _) = negpy_core::phase::phase_correlate(ref_gray, mov_gray, height, width);
        return (dx, dy);
    }
    let scale = width as f64 / EST_WIDTH as f64;
    let dh = ((height as f64 / scale).round() as usize).max(1);
    let Some(r) = negpy_core::resize::resize_area(ref_gray, height, width, 1, dh, EST_WIDTH) else {
        return (0.0, 0.0);
    };
    let Some(m) = negpy_core::resize::resize_area(mov_gray, height, width, 1, dh, EST_WIDTH) else {
        return (0.0, 0.0);
    };
    let (dx, dy, _) = negpy_core::phase::phase_correlate(&r, &m, dh, EST_WIDTH);
    (dx * scale, dy * scale)
}

fn align_to(reference: &[f32], mov: &ImageBuffer, channel: usize, max_shift: f64) -> ImageBuffer {
    let mut plane = vec![0.0f32; mov.width * mov.height];
    negpy_core::convert::extract_channel(&mov.pixels, 3, channel, &mut plane);
    let (dx, dy) = estimate_shift(reference, &plane, mov.height, mov.width);
    if dx.abs().max(dy.abs()) > max_shift {
        return mov.clone();
    }
    let source = negpy_core::warp::Image {
        data: &mov.pixels,
        h: mov.height,
        w: mov.width,
        cn: 3,
    };
    let matrix = [1.0, 0.0, -dx, 0.0, 1.0, -dy];
    let mut out = vec![0.0; mov.pixels.len()];
    negpy_core::warp::warp_affine(
        &source,
        (negpy_core::warp::Interp::Cubic, negpy_core::warp::Border::Replicate),
        matrix,
        false,
        mov.width,
        &mut out,
    );
    ImageBuffer::from_rgb(mov.width, mov.height, out)
}

/// Assemble one frame: red from red, green from green, blue from blue.
#[must_use]
pub fn assemble_rgb(
    red: &ImageBuffer,
    green: &ImageBuffer,
    blue: &ImageBuffer,
    align: bool,
) -> Option<ImageBuffer> {
    if red.width != green.width
        || red.width != blue.width
        || red.height != green.height
        || red.height != blue.height
    {
        return None;
    }
    let (mut red, mut blue) = (red.clone(), blue.clone());
    if align {
        let mut ref_green = vec![0.0f32; green.width * green.height];
        negpy_core::convert::extract_channel(&green.pixels, 3, GREEN, &mut ref_green);
        let max_shift = 16f64.max(0.02 * red.width as f64);
        red = align_to(&ref_green, &red, RED, max_shift);
        blue = align_to(&ref_green, &blue, BLUE, max_shift);
    }
    let mut out = vec![0.0f32; red.pixels.len()];
    negpy_core::convert::pick_channels(
        [&red.pixels, &green.pixels, &blue.pixels],
        &mut out,
    );
    Some(ImageBuffer::from_rgb(red.width, red.height, out))
}

/// The identity of the active triplet, folded into the render source hash.
#[must_use]
pub fn rgbscan_token(config: &crate::domain::models::RgbScanConfig) -> String {
    crate::domain::models::rgbscan_token(config)
}

impl crate::domain::models::LensWarp {
    /// The inverse map of one channel's rows, or `None` when this warp has no selected work.
    fn maps(
        &self,
        lens: &crate::domain::models::LensMetadata,
        (height, width): (usize, usize),
        rows: (usize, usize),
        channel: usize,
        corrections: crate::domain::models::LensCorrections,
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        let (start, stop) = rows;
        let count = (stop.saturating_sub(start)) * width;
        match self {
            crate::domain::models::LensWarp::Rectilinear { coefficients, center } => {
                use crate::domain::models::lens::IDENTITY;
                let common = coefficients[if coefficients.len() == 1 { 0 } else { 1 }];
                let selected = if corrections.ca && coefficients.len() == 3 {
                    coefficients[channel]
                } else {
                    common
                };
                let frame = lens_frame(lens, (height, width), *center);
                let mut mx = vec![0.0; count];
                let mut my = vec![0.0; count];
                if corrections.distortion {
                    negpy_core::lens::rectilinear_map(
                        width,
                        start..stop,
                        selected,
                        frame,
                        &mut mx,
                        &mut my,
                    );
                } else if selected == common {
                    for row in 0..stop - start {
                        for col in 0..width {
                            mx[row * width + col] = col as f32;
                            my[row * width + col] = (start + row) as f32;
                        }
                    }
                } else {
                    negpy_core::lens::rectilinear_ca_map(
                        width,
                        start..stop,
                        (selected, common),
                        frame,
                        &mut mx,
                        &mut my,
                    );
                }
                let _ = IDENTITY;
                Some((mx, my))
            }
            crate::domain::models::LensWarp::Panasonic { scale, a, b, c } => {
                if !corrections.distortion {
                    return None;
                }
                let mut mx = vec![0.0; count];
                let mut my = vec![0.0; count];
                negpy_core::lens::panasonic_map(
                    (height, width),
                    start..stop,
                    (*scale, *a, *b, *c),
                    lens.fill_scale,
                    &mut mx,
                    &mut my,
                );
                Some((mx, my))
            }
            crate::domain::models::LensWarp::Sony {
                distortion,
                ca_red,
                ca_blue,
            } => {
                let knot_count = if distortion.is_empty() {
                    ca_red.len().max(ca_blue.len())
                } else {
                    distortion.len()
                };
                if knot_count < 2 {
                    return None;
                }
                let mut factors = vec![1.0; knot_count];
                if corrections.distortion && !distortion.is_empty() {
                    for (factor, value) in factors.iter_mut().zip(distortion) {
                        *factor += value / 16384.0;
                    }
                }
                let ca = if channel == 0 { ca_red } else if channel == 2 { ca_blue } else { &Vec::new() };
                if corrections.ca && !ca.is_empty() {
                    for (factor, value) in factors.iter_mut().zip(ca) {
                        *factor *= 1.0 + value / 2_097_152.0;
                    }
                }
                let mut mx = vec![0.0; count];
                let mut my = vec![0.0; count];
                negpy_core::lens::sony_map(
                    (height, width),
                    start..stop,
                    &factors,
                    lens.fill_scale,
                    &mut mx,
                    &mut my,
                );
                Some((mx, my))
            }
        }
    }
}

fn lens_frame(
    lens: &crate::domain::models::LensMetadata,
    (height, width): (usize, usize),
    center: (f64, f64),
) -> negpy_core::lens::Frame {
    let active = lens
        .active_area
        .unwrap_or([0, 0, height as i64, width as i64]);
    let buffer = lens.buffer_area.unwrap_or(active);
    let (top, left, bottom, right) = (active[0], active[1], active[2], active[3]);
    let (buffer_top, buffer_left, buffer_bottom, buffer_right) =
        (buffer[0], buffer[1], buffer[2], buffer[3]);
    let sx = (buffer_right - buffer_left) as f64 / width as f64;
    let sy = (buffer_bottom - buffer_top) as f64 / height as f64;
    let cx = left as f64 + center.0 * (right - left - 1) as f64;
    let cy = top as f64 + center.1 * (bottom - top - 1) as f64;
    let radius = (cx - left as f64)
        .max(right as f64 - 1.0 - cx)
        .hypot((cy - top as f64).max(bottom as f64 - 1.0 - cy));
    negpy_core::lens::Frame {
        origin: (buffer_left as f64, buffer_top as f64),
        scale: (sx, sy),
        center: (cx, cy),
        radius,
        fill: lens.fill_scale,
    }
}

fn remap_plane(
    plane: &[f32],
    mx: &[f32],
    my: &[f32],
    width: usize,
    height: usize,
    start: usize,
    out: &mut [f32],
) {
    let at = |x: f32, y: f32| -> f32 {
        let xi = x.round_ties_even().clamp(0.0, (width - 1) as f32) as usize;
        let yi = y.round_ties_even().clamp(0.0, (height - 1) as f32) as usize;
        plane[yi * width + xi]
    };
    for row in 0..(out.len() / width) {
        for col in 0..width {
            out[row * width + col] = at(mx[row * width + col], my[row * width + col]);
        }
    }
    let _ = start;
}

fn apply_exif_orientation(img: &ImageBuffer, orientation: u8) -> ImageBuffer {
    if orientation < 2 {
        return img.clone();
    }
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let (oh, ow) = negpy_core::orient::oriented_dims(img.height, img.width, orientation);
    let mut dst = vec![0u8; oh * ow * 12];
    negpy_core::orient::orient(&bytes, img.height, img.width, 12, orientation, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    ImageBuffer::from_rgb(ow, oh, pixels)
}

impl crate::domain::models::LensMetadata {
    /// The largest scale up to 1 at which every output edge reads inside the source.
    #[must_use]
    pub fn fill_scale(&self, (height, width): (usize, usize), corrections: crate::domain::models::LensCorrections) -> f64 {
        let applied: Vec<&crate::domain::models::LensWarp> = self
            .warps
            .iter()
            .filter(|warp| {
                (corrections.distortion && warp.has_distortion())
                    || (corrections.ca && warp.has_ca())
            })
            .collect();
        let Some(last) = applied.last() else {
            return 1.0;
        };
        if !corrections.distortion || !applied.iter().any(|warp| warp.has_distortion()) {
            return 1.0;
        }
        let edge_reads = |scale: f64| {
            let mut lens = self.clone();
            lens.fill_scale = scale;
            let rows = [0, height.saturating_sub(1)];
            for channel in if corrections.ca { [0, 1, 2] } else { [0, 0, 0] } {
                for row in rows {
                    let Some((mx, my)) = last.maps(&lens, (height, width), (row, row + 1), channel, corrections) else {
                        continue;
                    };
                    for col in [0, width.saturating_sub(1)] {
                        let (x, y) = (mx[col], my[col]);
                        if x < 0.0 || x > (width - 1) as f32 || y < 0.0 || y > (height - 1) as f32 {
                            return false;
                        }
                    }
                }
            }
            true
        };
        if edge_reads(1.0) {
            return 1.0;
        }
        if !edge_reads(0.5) {
            return 1.0;
        }
        let (mut lo, mut hi) = (0.5, 1.0);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if edge_reads(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Apply the selected warps, with bounded map memory.
    #[must_use]
    pub fn apply(
        &self,
        img: &ImageBuffer,
        orientation: u8,
        corrections: crate::domain::models::LensCorrections,
    ) -> ImageBuffer {
        if !corrections.any() || !self.available() || img.width.min(img.height) < 2 {
            return img.clone();
        }
        let inverse = match orientation {
            6 => 8,
            8 => 6,
            other => other,
        };
        let mut source = apply_exif_orientation(img, inverse);
        let applied: Vec<&crate::domain::models::LensWarp> = self
            .warps
            .iter()
            .filter(|warp| {
                (corrections.distortion && warp.has_distortion())
                    || (corrections.ca && warp.has_ca())
            })
            .collect();
        let scale = self.fill_scale((source.height, source.width), corrections);
        let mut filled = self.clone();
        filled.fill_scale = scale;
        for (index, warp) in applied.iter().enumerate() {
            let lens = if index == applied.len() - 1 {
                &filled
            } else {
                self
            };
            let mut result = vec![0.0; source.pixels.len()];
            for channel in 0..3 {
                let mut plane = vec![0.0; source.width * source.height];
                negpy_core::convert::extract_channel(&source.pixels, 3, channel, &mut plane);
                for start in (0..source.height).step_by(256) {
                    let stop = (start + 256).min(source.height);
                    let Some((mx, my)) = warp.maps(lens, (source.height, source.width), (start, stop), channel, corrections) else {
                        continue;
                    };
                    let mut out = vec![0.0; (stop - start) * source.width];
                    remap_plane(&plane, &mx, &my, source.width, source.height, start, &mut out);
                    for row in 0..stop - start {
                        for col in 0..source.width {
                            result[((start + row) * source.width + col) * 3 + channel] =
                                out[row * source.width + col].max(0.0);
                        }
                    }
                }
            }
            source = ImageBuffer::from_rgb(source.width, source.height, result);
        }
        apply_exif_orientation(&source, orientation)
    }
}
