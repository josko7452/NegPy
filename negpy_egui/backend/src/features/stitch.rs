//! Panorama registration, warping and blending for multi-part scans.
//!
//! Mirrors `negpy/features/stitch/logic.py`. All buffers are scene-linear, already
//! EXIF-oriented and flat-fielded; transforms are valid only on buffers decoded the same way.

use crate::domain::models::StitchConfig;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const PROXY_MAX_EDGE: usize = 1600;
pub const MIN_INLIERS: usize = 100;
pub const NO_OVERLAP_MSG: &str = "Could not find overlap between the selected frames";

/// Registration failure; the message is shown to the user verbatim.
#[derive(Clone, Debug, PartialEq)]
pub struct StitchError(pub &'static str);

/// Cancellation is a registration failure with its own message.
pub const CANCELLED_MSG: &str = "Cancelled";

/// A 2x3 affine, row-major.
pub type Affine = [f64; 6];

/// The registration proxy: green channel, at most 1600 px, contrast-stretched uint8.
#[must_use]
pub fn build_proxy(rgb: &ImageBuffer) -> (Vec<u8>, f64) {
    let mut gray = vec![0.0; rgb.width * rgb.height];
    negpy_core::convert::extract_channel(&rgb.pixels, 3, 1, &mut gray);
    let long = rgb.width.max(rgb.height);
    let scale = (PROXY_MAX_EDGE as f64 / long as f64).min(1.0);
    if scale < 1.0 {
        let dw = ((rgb.width as f64 * scale).round() as usize).max(1);
        let dh = ((rgb.height as f64 * scale).round() as usize).max(1);
        if let Some(resized) = negpy_core::resize::resize_area(&gray, rgb.height, rgb.width, 1, dh, dw) {
            gray = resized;
        }
    }
    let mut proxy = vec![0u8; gray.len()];
    negpy_core::stitch::proxy_stretch(&gray, &mut proxy);
    (proxy, scale)
}

/// The similarity transform mapping `mov` proxy coordinates to `ref` proxy coordinates.
pub fn register_pair(
    reference: &[u8],
    moving: &[u8],
    (height, width): (usize, usize),
    min_inliers: usize,
) -> Result<(Affine, usize), StitchError> {
    negpy_core::register::register_pair((reference, height, width), (moving, height, width), min_inliers)
        .map(|(matrix, inliers)| (matrix, inliers))
        .ok_or(StitchError(NO_OVERLAP_MSG))
}

/// Proxy-coordinate affine -> full-resolution affine.
#[must_use]
pub fn scale_affine(mut matrix: Affine, ref_scale: f64, mov_scale: f64) -> Affine {
    let ratio = mov_scale / ref_scale;
    matrix[0] *= ratio;
    matrix[1] *= ratio;
    matrix[3] *= ratio;
    matrix[4] *= ratio;
    matrix[2] /= ref_scale;
    matrix[5] /= ref_scale;
    matrix
}

/// Compose 2x3 affines: `(a ∘ b)(x) = a(b(x))`.
#[must_use]
pub fn chain_affine(a: Affine, b: Affine) -> Affine {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

fn transform_point(matrix: Affine, x: f64, y: f64) -> (f64, f64) {
    (matrix[0] * x + matrix[1] * y + matrix[2], matrix[3] * x + matrix[4] * y + matrix[5])
}

/// The union bounding box of the warped parts, and the transforms shifted into it.
#[must_use]
pub fn compute_canvas(sizes: &[(usize, usize)], transforms: &[Affine]) -> ((usize, usize), Vec<Affine>) {
    let mut lo = (f64::INFINITY, f64::INFINITY);
    let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (&(w, h), &matrix) in sizes.iter().zip(transforms) {
        for (x, y) in [(0, 0), (w, 0), (w, h), (0, h)] {
            let (px, py) = transform_point(matrix, x as f64, y as f64);
            lo = (lo.0.min(px), lo.1.min(py));
            hi = (hi.0.max(px), hi.1.max(py));
        }
    }
    let lo = (lo.0.floor(), lo.1.floor());
    let hi = (hi.0.ceil(), hi.1.ceil());
    let shift = [1.0, 0.0, -lo.0, 0.0, 1.0, -lo.1];
    let shifted = transforms.iter().map(|&m| chain_affine(shift, m)).collect();
    (
        ((hi.0 - lo.0).max(0.0) as usize, (hi.1 - lo.1).max(0.0) as usize),
        shifted,
    )
}

/// Chained pairwise registration, each part against the previous one.
#[must_use]
pub fn register_parts(
    parts: &[ImageBuffer],
    is_cancelled: impl Fn() -> bool,
    on_progress: impl Fn(usize, usize, usize),
) -> Result<(Vec<Affine>, (usize, usize)), StitchError> {
    let proxies: Vec<(Vec<u8>, f64)> = parts.iter().map(build_proxy).collect();
    let mut transforms = vec![[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]];
    for i in 1..parts.len() {
        if is_cancelled() {
            return Err(StitchError(CANCELLED_MSG));
        }
        on_progress(i, parts.len() - 1, i + 1);
        let (height, width) = (parts[i].height, parts[i].width);
        let (matrix, _) = register_pair(
            &proxies[i - 1].0,
            &proxies[i].0,
            (height, width),
            MIN_INLIERS,
        )?;
        let full = scale_affine(matrix, proxies[i - 1].1, proxies[i].1);
        transforms.push(chain_affine(transforms[i - 1], full));
    }
    let sizes: Vec<(usize, usize)> = parts.iter().map(|part| (part.width, part.height)).collect();
    let (canvas, shifted) = compute_canvas(&sizes, &transforms);
    Ok((shifted, canvas))
}

/// Stored full-resolution transforms -> the scale the parts were actually decoded at.
#[must_use]
pub fn scale_transforms(
    config: &StitchConfig,
    decoded_sizes: &[(usize, usize)],
) -> (Vec<Affine>, (usize, usize)) {
    let reference_scale = decoded_sizes[0].0 as f64 / config.stitch_sizes[0][0].max(1) as f64;
    let mut out = Vec::with_capacity(config.stitch_transforms.len());
    for ((flat, &(decoded_w, _)), &[full_w, _]) in config
        .stitch_transforms
        .iter()
        .zip(decoded_sizes)
        .zip(&config.stitch_sizes)
    {
        let mut matrix: Affine = std::array::from_fn(|index| flat.get(index).copied().unwrap_or(0.0));
        let part_scale = decoded_w as f64 / full_w.max(1) as f64;
        let ratio = reference_scale / part_scale;
        matrix[0] *= ratio;
        matrix[1] *= ratio;
        matrix[3] *= ratio;
        matrix[4] *= ratio;
        matrix[2] *= reference_scale;
        matrix[5] *= reference_scale;
        out.push(matrix);
    }
    (
        out,
        (
            (config.stitch_canvas[0] as f64 * reference_scale).round().max(0.0) as usize,
            (config.stitch_canvas[1] as f64 * reference_scale).round().max(0.0) as usize,
        ),
    )
}

/// Warp a part and its validity mask into the canvas.
#[must_use]
pub fn warp_into_canvas(
    img: &ImageBuffer,
    transform: Affine,
    canvas: (usize, usize),
) -> (ImageBuffer, Vec<bool>) {
    let source = negpy_core::warp::Image {
        data: &img.pixels,
        h: img.height,
        w: img.width,
        cn: 3,
    };
    let mut warped = vec![0.0; canvas.0 * canvas.1 * 3];
    negpy_core::warp::warp_affine(
        &source,
        (negpy_core::warp::Interp::Cubic, negpy_core::warp::Border::Constant),
        transform,
        false,
        canvas.0,
        &mut warped,
    );
    let ones = vec![1.0f32; img.width * img.height];
    let mask_source = negpy_core::warp::Image {
        data: &ones,
        h: img.height,
        w: img.width,
        cn: 1,
    };
    let mut mask_values = vec![0.0; canvas.0 * canvas.1];
    negpy_core::warp::warp_affine(
        &mask_source,
        (negpy_core::warp::Interp::Linear, negpy_core::warp::Border::Constant),
        transform,
        false,
        canvas.0,
        &mut mask_values,
    );
    (
        ImageBuffer::from_rgb(canvas.0, canvas.1, warped),
        mask_values.into_iter().map(|value| value > 0.999).collect(),
    )
}

const MIN_OVERLAP_PX: usize = 1000;
const SEAM_MAX_EDGE: usize = 1024;
const SEAM_FEATHER_PX: f64 = 48.0;
const LIT_RATIO: f32 = 0.25;
const LIT_FLOOR: f32 = 0.05;
const LIT_MARGIN_FEATHERS: f32 = 4.0;
const LIT_GATE_MARGINS: f32 = 1.5;
const LIT_SPECKLE_MARGINS: f32 = 0.25;

fn resize_mask(mask: &[bool], (h, w): (usize, usize), (dh, dw): (usize, usize)) -> Vec<bool> {
    let src: Vec<u8> = mask.iter().map(|value| u8::from(*value)).collect();
    let mut dst = vec![0u8; dh * dw];
    negpy_core::resize::resize_nearest(&src, (h, w, 1), (dh, dw), &mut dst);
    dst.into_iter().map(|value| value > 0).collect()
}

fn resize_green(img: &ImageBuffer, (dh, dw): (usize, usize)) -> Vec<f32> {
    let mut green = vec![0.0; img.width * img.height];
    negpy_core::convert::extract_channel(&img.pixels, 3, 1, &mut green);
    negpy_core::resize::resize_area(&green, img.height, img.width, 1, dh, dw).unwrap_or_default()
}

fn unlit_regions(unlit: &[bool], (h, w): (usize, usize)) -> (Vec<i32>, Vec<f32>) {
    // A region label starts at each unlit pixel; the distance transform gives its depth.
    let mut labels = vec![0i32; h * w];
    let mut next = 1i32;
    for (label, value) in labels.iter_mut().zip(unlit) {
        if *value {
            *label = next;
            next += 1;
        }
    }
    let u8s: Vec<u8> = unlit.iter().map(|value| u8::from(*value)).collect();
    let depth = negpy_core::distance::distance_transform(&u8s, h, w, negpy_core::distance::Metric::L2Mask3);
    (labels, depth)
}

fn lit_coverage(
    warped: &[ImageBuffer],
    masks: &[Vec<bool>],
    canvas: (usize, usize),
    decoded_scale: f64,
) -> Vec<Vec<bool>> {
    if warped.len() < 2 {
        return masks.to_vec();
    }
    let (w, h) = canvas;
    let scale = (SEAM_MAX_EDGE as f64 / w.max(h) as f64).min(1.0);
    let (sw, sh) = (
        (w as f64 * scale).round().max(1.0) as usize,
        (h as f64 * scale).round().max(1.0) as usize,
    );
    let near: Vec<Vec<bool>> = masks.iter().map(|mask| resize_mask(mask, (h, w), (sh, sw))).collect();
    let u8s: Vec<Vec<u8>> = near.iter().map(|mask| mask.iter().map(|v| u8::from(*v)).collect()).collect();
    let valid: Vec<Vec<bool>> = u8s
        .iter()
        .map(|mask| {
            negpy_core::distance::distance_transform(mask, sh, sw, negpy_core::distance::Metric::L2Mask3)
                .into_iter()
                .map(|value| value > 1.0)
                .collect()
        })
        .collect();
    if valid.iter().any(|mask| !mask.iter().any(|v| *v)) {
        return masks.to_vec();
    }
    let green: Vec<Vec<f32>> = warped
        .iter()
        .zip(&valid)
        .map(|(img, valid)| {
            let plane = resize_green(img, (sh, sw));
            plane
                .into_iter()
                .zip(valid)
                .map(|(value, valid)| if *valid { value } else { 0.0 })
                .collect()
        })
        .collect();
    let mut top = vec![0.0f32; sw * sh];
    for plane in &green {
        for (top, value) in top.iter_mut().zip(plane) {
            *top = top.max(*value);
        }
    }
    let mut peaks = Vec::new();
    for (plane, valid) in green.iter().zip(&valid) {
        let values: Vec<f32> = plane.iter().zip(valid).filter(|(_, v)| **v).map(|(v, _)| *v).collect();
        if values.is_empty() {
            return masks.to_vec();
        }
        let p99 = negpy_core::stats::np_percentiles(&values, &[99.0]).map_or(0.0, |p| p[0]);
        peaks.push(p99);
    }
    let floor = LIT_FLOOR * peaks.iter().copied().fold(0.0f32, f32::max);
    let mut shared = vec![0u8; sw * sh];
    for valid in &valid {
        for (shared, value) in shared.iter_mut().zip(valid) {
            *shared += u8::from(*value);
        }
    }
    let margin = (LIT_MARGIN_FEATHERS * SEAM_FEATHER_PX as f32 * decoded_scale as f32 * scale as f32) as f64;
    let mut regions = Vec::new();
    for (plane, valid) in green.iter().zip(&valid) {
        let unlit: Vec<bool> = (0..sw * sh)
            .map(|i| valid[i] && shared[i] >= 2 && top[i] > floor && plane[i] < LIT_RATIO * top[i])
            .collect();
        regions.push(unlit_regions(&unlit, (sh, sw)));
    }
    if !regions
        .iter()
        .any(|(_, depth)| depth.iter().copied().fold(0.0f32, f32::max) as f64 > LIT_GATE_MARGINS as f64 * margin)
    {
        return masks.to_vec();
    }
    let mut lit_masks = Vec::new();
    for (near, (labels, depth)) in near.iter().zip(&regions) {
        let mut keep = vec![false; depth.len()];
        for (i, value) in depth.iter().enumerate().skip(1) {
            keep[i] = *value > LIT_SPECKLE_MARGINS as f32 * margin as f32;
        }
        let unlit: Vec<bool> = labels.iter().map(|label| keep.get(*label as usize).copied().unwrap_or(false)).collect();
        if unlit.iter().any(|v| *v) {
            let clear: Vec<u8> = unlit.iter().map(|v| u8::from(!*v)).collect();
            let clear =
                negpy_core::distance::distance_transform(&clear, sh, sw, negpy_core::distance::Metric::L2Mask3);
            lit_masks.push(
                near.iter()
                    .zip(clear)
                    .map(|(covered, distance)| *covered && distance as f64 > margin)
                    .collect(),
            );
        } else {
            lit_masks.push(near.clone());
        }
    }
    let min_lit = (MIN_OVERLAP_PX as f64 * (decoded_scale * scale).powi(2)) as usize;
    let mut group: Vec<usize> = (0..lit_masks.len()).collect();
    fn root(group: &mut [usize], mut index: usize) -> usize {
        while group[index] != index {
            index = group[index];
        }
        index
    }
    for i in 0..lit_masks.len() {
        for j in i + 1..lit_masks.len() {
            let overlap = lit_masks[i]
                .iter()
                .zip(&lit_masks[j])
                .filter(|(a, b)| **a && **b)
                .count();
            if overlap >= min_lit {
                let (ri, rj) = (root(&mut group, j), root(&mut group, i));
                group[ri] = rj;
            }
        }
    }
    if (0..lit_masks.len()).map(|i| root(&mut group, i)).collect::<std::collections::HashSet<_>>().len() > 1 {
        return masks.to_vec();
    }
    let mut any_lit = vec![false; sw * sh];
    for lit in &lit_masks {
        for (any, value) in any_lit.iter_mut().zip(lit) {
            *any |= *value;
        }
    }
    let mut cover: Vec<Vec<bool>> = Vec::new();
    for ((mask, near), lit) in masks.iter().zip(&near).zip(&lit_masks) {
        let removed: Vec<bool> = (0..sw * sh)
            .map(|i| near[i] && !lit[i] && any_lit[i])
            .collect();
        let invert: Vec<u8> = removed.iter().map(|v| u8::from(!*v)).collect();
        let beside = negpy_core::distance::distance_transform(&invert, sh, sw, negpy_core::distance::Metric::L2Mask3);
        let removed: Vec<bool> = removed
            .iter()
            .zip(beside)
            .zip(near)
            .map(|((removed, distance), near)| *removed || (distance <= 1.0 && !near))
            .collect();
        let upscaled = resize_mask(&removed, (sh, sw), (h, w));
        cover.push(mask.iter().zip(upscaled).map(|(mask, removed)| *mask && !removed).collect());
    }
    let mut orphan: Vec<bool> = masks.iter().fold(vec![false; h * w], |mut acc: Vec<bool>, mask| {
        for (orphan, value) in acc.iter_mut().zip(mask) {
            *orphan |= *value;
        }
        acc
    });
    for c in &cover {
        for (orphan, value) in orphan.iter_mut().zip(c) {
            *orphan = *orphan && !*value;
        }
    }
    if orphan.iter().any(|value| *value) {
        for (c, mask) in cover.iter_mut().zip(masks) {
            for ((c, mask), orphan) in c.iter_mut().zip(mask).zip(&orphan) {
                if *orphan && *mask {
                    *c = true;
                }
            }
        }
    }
    cover
}

fn seam_cost(img: &ImageBuffer) -> Vec<f32> {
    img.pixels.iter().map(|value| value.clamp(0.0, 1.0).powf(1.0 / 2.2)).collect()
}

/// Partition the covered canvas between parts along a minimum-error path.
#[must_use]
pub fn find_seams(
    warped: &[ImageBuffer],
    masks: &[Vec<bool>],
    canvas: (usize, usize),
) -> Option<Vec<Vec<bool>>> {
    if warped.len() < 2 {
        return None;
    }
    let (w, h) = canvas;
    let scale = (SEAM_MAX_EDGE as f64 / w.max(h) as f64).min(1.0);
    let (sw, sh) = (
        (w as f64 * scale).round().max(1.0) as usize,
        (h as f64 * scale).round().max(1.0) as usize,
    );
    let small: Vec<Vec<f32>> = warped.iter().map(seam_cost).collect();
    let resized: Vec<Vec<f32>> = small
        .iter()
        .map(|img| {
            negpy_core::resize::resize_area(img, h, w, 3, sh, sw).unwrap_or_default()
        })
        .collect();
    let mut small_masks: Vec<Vec<u8>> = masks
        .iter()
        .map(|mask| resize_mask(mask, (h, w), (sh, sw)).into_iter().map(u8::from).collect())
        .collect();
    let images: Vec<&[f32]> = resized.iter().map(Vec::as_slice).collect();
    negpy_core::seam::dp_seams(&images, &mut small_masks, (sh, sw));
    let mut seams: Vec<Vec<bool>> = small_masks
        .iter()
        .map(|mask| {
            resize_mask(
                &mask.iter().map(|value| *value > 0).collect::<Vec<_>>(),
                (sh, sw),
                (h, w),
            )
        })
        .collect();
    for (seam, mask) in seams.iter_mut().zip(masks) {
        for (seam, mask) in seam.iter_mut().zip(mask) {
            *seam &= *mask;
        }
    }
    let mut orphan: Vec<bool> = masks
        .iter()
        .fold(vec![false; w * h], |mut acc, mask| {
            for (orphan, value) in acc.iter_mut().zip(mask) {
                *orphan |= *value;
            }
            acc
        });
    for seam in &seams {
        for (orphan, seam) in orphan.iter_mut().zip(seam) {
            *orphan &= !*seam;
        }
    }
    if orphan.iter().any(|value| *value) {
        for (seam, mask) in seams.iter_mut().zip(masks) {
            for ((seam, mask), orphan) in seam.iter_mut().zip(mask).zip(&mut orphan) {
                if *orphan && *mask {
                    *seam = true;
                    *orphan = false;
                }
            }
        }
    }
    Some(seams)
}

/// Cross-fade within `feather_px` of the cut; beyond it one part wins outright.
#[must_use]
pub fn seam_blend(
    warped: &[ImageBuffer],
    masks: &[Vec<bool>],
    seams: &[Vec<bool>],
    feather_px: f64,
) -> ImageBuffer {
    let images: Vec<&[f32]> = warped.iter().map(|img| img.pixels.as_slice()).collect();
    let masks: Vec<&[bool]> = masks.iter().map(Vec::as_slice).collect();
    let seams: Vec<&[bool]> = seams.iter().map(Vec::as_slice).collect();
    let (width, height) = (warped[0].width, warped[0].height);
    let out = negpy_core::stitch::seam_blend(&images, &masks, &seams, (height, width), feather_px.max(1.0));
    ImageBuffer::from_rgb(width, height, out)
}

/// The distance-weighted average, the fallback when no seam can be cut.
#[must_use]
pub fn feather_blend(warped: &[ImageBuffer], masks: &[Vec<bool>]) -> ImageBuffer {
    let images: Vec<&[f32]> = warped.iter().map(|img| img.pixels.as_slice()).collect();
    let masks: Vec<&[bool]> = masks.iter().map(Vec::as_slice).collect();
    let (width, height) = (warped[0].width, warped[0].height);
    let out = negpy_core::stitch::feather_blend(&images, &masks, (height, width));
    ImageBuffer::from_rgb(width, height, out)
}

/// Warp, blend and carry the IR plane for the decoded parts.
#[must_use]
pub fn stitch_composite(
    parts: &[ImageBuffer],
    irs: &[Option<Vec<f32>>],
    config: &StitchConfig,
) -> (ImageBuffer, Option<Vec<f32>>) {
    let decoded_sizes: Vec<(usize, usize)> = parts.iter().map(|part| (part.width, part.height)).collect();
    let (transforms, canvas) = scale_transforms(config, &decoded_sizes);
    let decoded_scale = canvas.0 as f64 / config.stitch_canvas[0].max(1) as f64;
    let feather_px = SEAM_FEATHER_PX * decoded_scale;
    let warped: Vec<ImageBuffer> = parts
        .iter()
        .zip(&transforms)
        .map(|(part, &transform)| warp_into_canvas(part, transform, canvas).0)
        .collect();
    let masks: Vec<Vec<bool>> = parts
        .iter()
        .zip(&transforms)
        .map(|(part, &transform)| warp_into_canvas(part, transform, canvas).1)
        .collect();
    let mut coverage = lit_coverage(&warped, &masks, canvas, decoded_scale);
    let mut warped = warped;
    for i in 1..warped.len() {
        for j in 0..i {
            let overlap = negpy_core::stitch::mask_overlap(&coverage[j], &coverage[i]);
            if overlap >= MIN_OVERLAP_PX {
                if let Some(gains) = negpy_core::stitch::compensation_gains(
                    &warped[j].pixels,
                    &coverage[j],
                    &warped[i].pixels,
                    &coverage[i],
                    MIN_OVERLAP_PX,
                ) {
                    negpy_core::stitch::scale_channels(&mut warped[i].pixels, gains);
                }
                break;
            }
        }
    }
    let seams = find_seams(&warped, &coverage, canvas);
    let mut blended = seams.as_ref().map_or_else(
        || feather_blend(&warped, &coverage),
        |seams| seam_blend(&warped, &coverage, seams, feather_px),
    );
    negpy_core::stitch::clip_unit(&mut blended.pixels);
    let _ = &mut coverage;
    let ir = if !irs.is_empty() && irs.iter().all(Option::is_some) {
        let mut acc = vec![-1.0; canvas.0 * canvas.1];
        for (i, plane) in irs.iter().enumerate() {
            let Some(plane) = plane else {
                continue;
            };
            let ir_img = ImageBuffer::from_rgb(canvas.0, canvas.1, plane.clone());
            let _ = ir_img;
            let source = ImageBuffer::from_rgb(parts[i].width, parts[i].height, plane.clone());
            let (warped_ir, mask) = warp_into_canvas(&source, transforms[i], canvas);
            let selected = seams.as_ref().map_or(&mask, |seams| &seams[i]);
            let gray: Vec<f32> = warped_ir.pixels.chunks_exact(3).map(|pixel| pixel[0]).collect();
            negpy_core::stitch::ir_blend_into(&mut acc, &gray, selected, seams.is_none());
        }
        negpy_core::stitch::ir_finish(&mut acc);
        Some(acc)
    } else {
        None
    };
    (blended, ir)
}
