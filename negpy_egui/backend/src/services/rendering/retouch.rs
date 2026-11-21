//! The retouch source bakes, run on the linear frame before either engine.
//!
//! Mirrors `ImageProcessor._ir_bake`, `_luma_bake`, `_manual_bake` and `_clone_bake`.
//! Every stage reads and writes source coordinates, so geometry and crop cannot move a
//! painted stroke. The IR plane is an input; a file without one skips the IR stage.

use crate::domain::models::{RetouchConfig, WorkspaceConfig};
use crate::features::retouch::logic::{
    self, apply_hair_inpaint, apply_ir_attenuation, apply_score_repair, clone_token, downsample_ir,
    exclusion_cover, ir_bake_token, ir_defect_score, ir_detect_cutoff, ir_detect_target,
    ir_ratio_and_gain, lines_to_score, manual_bake_token, repair_components, route_wide_defects,
    strokes_to_score,
};
use crate::features::retouch::models::CloneStrokeConfig;
use crate::features::retouch::processor::detect_dust;
use crate::features::retouch::processor::without_ir;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The IR measurements one bake keeps between stages.
pub struct IrPlane<'a> {
    pub samples: &'a [f32],
    pub width: usize,
    pub height: usize,
}

/// One painted heal stroke: points, brush diameter, and the unused clone offset.
fn heal_stroke(value: &serde_json::Value) -> Option<(Vec<[f64; 2]>, f64)> {
    let items = value.as_array()?;
    let points = items.first()?.as_array()?;
    let points = points
        .iter()
        .map(|point| {
            let pair = point.as_array()?;
            Some([pair.first()?.as_f64()?, pair.get(1)?.as_f64()?])
        })
        .collect::<Option<Vec<_>>>()?;
    Some((points, items.get(1)?.as_f64()?))
}

/// One exclusion stroke: points and brush diameter.
fn exclusion_stroke(value: &serde_json::Value) -> Option<(Vec<[f64; 2]>, f64)> {
    heal_stroke(value)
}

/// One scratch line: two normalized endpoints and the guide width.
fn scratch_line(value: &serde_json::Value) -> Option<[f64; 5]> {
    let items = value.as_array()?;
    Some([
        items.first()?.as_f64()?,
        items.get(1)?.as_f64()?,
        items.get(2)?.as_f64()?,
        items.get(3)?.as_f64()?,
        items.get(4)?.as_f64()?,
    ])
}

/// One clone stroke: path, brush settings and the source offset.
fn clone_stroke(value: &serde_json::Value) -> Option<CloneStrokeConfig> {
    let items = value.as_array()?;
    let points = items.first()?.as_array()?;
    let points = points
        .iter()
        .map(|point| {
            let pair = point.as_array()?;
            Some([pair.first()?.as_f64()?, pair.get(1)?.as_f64()?])
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CloneStrokeConfig {
        points,
        size: items.get(1)?.as_f64()?,
        dx: items.get(2)?.as_f64()?,
        dy: items.get(3)?.as_f64()?,
        strength: items.get(4)?.as_f64()?,
        feather: items.get(5)?.as_f64()?,
        match_tone: items.get(6)?.as_bool()?,
    })
}

/// The source-hash tokens the bakes contribute.
#[must_use]
pub fn source_tokens(config: &RetouchConfig, has_ir: bool) -> String {
    crate::features::retouch::processor::source_tokens(config, has_ir)
}

/// Bake every source repair the config asks for.
#[must_use]
pub fn bake(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    ir: Option<IrPlane<'_>>,
    preview_size: f64,
) -> ImageBuffer {
    let (image, tokens) = bake_with_tokens(image, config, ir, preview_size);
    let _ = tokens;
    image
}

/// The baked image and the hash body its cache and engine keys read.
#[must_use]
pub fn bake_with_tokens(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    ir: Option<IrPlane<'_>>,
    preview_size: f64,
) -> (ImageBuffer, String) {
    let tokens = engine_tokens(&config.retouch, ir.is_some());
    if config.exposure.render_intent == crate::domain::models::RenderIntent::Flat {
        return (image.clone(), String::new());
    }
    (bake_stages(image, config, ir, preview_size), tokens)
}

fn bake_stages(
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    ir: Option<IrPlane<'_>>,
    preview_size: f64,
) -> ImageBuffer {
    let image = image;
    let retouch = &config.retouch;
    let mut current = image.clone();
    let mut routed: Vec<Option<Vec<u8>>> = Vec::new();

    let (ir_corrected, ir_routed) = bake_ir(&current, retouch, ir.as_ref(), preview_size);
    let ir_repaired = ir_corrected.is_some();
    if let Some((mask, _)) = ir_corrected {
        current = mask;
        routed.push(ir_routed);
    }

    let detected = detect_dust(&current, retouch);
    let (mut dust, mut hairs) = detected.map_or((None, Vec::new()), |found| {
        (
            found.score,
            found.hairs.into_iter().map(Some).collect::<Vec<_>>(),
        )
    });
    if ir_repaired && (dust.is_some() || !hairs.is_empty()) {
        if let Some(plane) = ir.as_ref() {
            let cover = ir_cover(plane, &current);
            let (new_dust, hair_masks) = without_ir(
                dust.as_deref(),
                &hairs,
                current.height,
                current.width,
                &cover,
                (current.height, current.width),
            );
            dust = new_dust;
            hairs = hair_masks;
        }
    }
    routed.extend(hairs);
    if let Some(score) = dust {
        let repaired = repair_components(&current, &score, current.height, current.width, true);
        if let Some(mask) = route_wide_defects(&score, current.height, current.width, None) {
            routed.push(Some(mask));
        }
        current = repaired;
    }

    let (manual, manual_routed) = bake_manual(&current, retouch);
    if manual_routed {
        current = manual;
    }
    if let Some(mask) = manual_routed_mask(retouch, &current) {
        routed.push(Some(mask));
    }

    let hair_masks: Vec<Option<Vec<u8>>> = routed.into_iter().flatten().map(Some).collect();
    if !hair_masks.is_empty() {
        current = apply_hair_inpaint(
            &current,
            &hair_masks,
            current.height,
            current.width,
            hair_radius(current.height, current.width),
        );
    }

    current = bake_clones(&current, retouch);
    current
}

fn ir_cover(plane: &IrPlane<'_>, image: &ImageBuffer) -> Vec<bool> {
    let target = ir_detect_target(image.height.max(image.width), 1600.0 as usize);
    let ir = downsample_ir(plane.samples, plane.height, plane.width, target, None);
    let (ratio, _gain, _degenerate, _gammas) =
        ir_ratio_and_gain(&ir, image, plane.height, plane.width);
    ratio.iter().map(|value| *value < 0.97).collect()
}

fn bake_ir(
    image: &ImageBuffer,
    config: &RetouchConfig,
    ir: Option<&IrPlane<'_>>,
    preview_size: f64,
) -> (Option<(ImageBuffer, bool)>, Option<Vec<u8>>) {
    let Some(plane) = ir.filter(|_| config.ir_dust_remove) else {
        return (None, None);
    };
    let target = ir_detect_target(
        image.height.max(image.width),
        preview_size.max(1.0) as usize,
    );
    let ir = downsample_ir(plane.samples, plane.height, plane.width, target, None);
    let (ratio, gain, degenerate, _gammas) =
        ir_ratio_and_gain(&ir, image, plane.height, plane.width);
    if degenerate {
        return (None, None);
    }
    let score = ir_defect_score(
        &ratio,
        plane.height,
        plane.width,
        ir_detect_cutoff(config.ir_threshold, config.ir_attenuation),
    );
    let attenuated = if config.ir_attenuation {
        apply_ir_attenuation(image, &gain, plane.height, plane.width)
    } else {
        image.clone()
    };
    let repaired = apply_score_repair(
        &attenuated,
        &score,
        plane.height,
        plane.width,
        true,
        Some(image.height.max(image.width)),
    );
    let routed = route_wide_defects(&score, plane.height, plane.width, Some(0.12));
    (Some((repaired, true)), routed)
}

fn bake_manual(image: &ImageBuffer, config: &RetouchConfig) -> (ImageBuffer, bool) {
    let strokes = config
        .manual_heal_strokes
        .iter()
        .filter_map(heal_stroke)
        .collect::<Vec<_>>();
    let spots = config
        .manual_dust_spots
        .iter()
        .filter_map(|spot| {
            let items = spot.as_array()?;
            Some([
                items.first()?.as_f64()?,
                items.get(1)?.as_f64()?,
                items.get(2)?.as_f64()?,
            ])
        })
        .collect::<Vec<_>>();
    let lines = config
        .scratch_lines
        .iter()
        .filter_map(scratch_line)
        .collect::<Vec<_>>();
    if strokes.is_empty() && spots.is_empty() && lines.is_empty() {
        return (image.clone(), false);
    }
    let parts = [
        strokes_to_score(image, &strokes, &spots),
        lines_to_score(image, &lines, config.scratch_threshold),
    ];
    let score = match (&parts[0], &parts[1]) {
        (Some(a), Some(b)) => Some(
            a.iter()
                .zip(b)
                .map(|(left, right)| left.min(*right))
                .collect::<Vec<f32>>(),
        ),
        (Some(score), None) | (None, Some(score)) => Some(score.clone()),
        (None, None) => None,
    };
    match score {
        Some(score) => (
            repair_components(image, &score, image.height, image.width, false),
            true,
        ),
        None => (image.clone(), false),
    }
}

fn manual_routed_mask(config: &RetouchConfig, image: &ImageBuffer) -> Option<Vec<u8>> {
    let strokes = config
        .manual_heal_strokes
        .iter()
        .filter_map(heal_stroke)
        .collect::<Vec<_>>();
    let lines = config
        .scratch_lines
        .iter()
        .filter_map(scratch_line)
        .collect::<Vec<_>>();
    let score = strokes_to_score(image, &strokes, &[])
        .or_else(|| lines_to_score(image, &lines, config.scratch_threshold))?;
    route_wide_defects(&score, image.height, image.width, None)
}

fn hair_radius(height: usize, width: usize) -> i64 {
    let scale = (height.max(width) as f64 / 1600.0).max(1.0);
    (logic::HAIR_DILATE_PX as f64 * scale).round().max(1.0) as i64
}

fn bake_clones(image: &ImageBuffer, config: &RetouchConfig) -> ImageBuffer {
    if clone_token(config).is_empty() {
        return image.clone();
    }
    let strokes: Vec<CloneStrokeConfig> = config
        .clone_strokes
        .iter()
        .filter_map(clone_stroke)
        .collect();
    logic::apply_clone_strokes(image, &strokes)
}

/// The exclusion mask the optical detector must respect.
#[must_use]
pub fn exclusion_mask(config: &RetouchConfig, height: usize, width: usize) -> Option<Vec<u8>> {
    let strokes: Vec<(Vec<[f64; 2]>, f64)> = config
        .dust_exclusion_strokes
        .iter()
        .filter_map(exclusion_stroke)
        .collect();
    if strokes.is_empty() {
        return None;
    }
    let cover = exclusion_cover(&strokes, height, width);
    cover.iter().any(|value| *value != 0).then_some(cover)
}

/// The IR, manual and clone tokens the engine cache reads.
#[must_use]
pub fn engine_tokens(config: &RetouchConfig, has_ir: bool) -> String {
    format!(
        "{}{}{}",
        ir_bake_token(config, has_ir),
        manual_bake_token(config),
        clone_token(config)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> ImageBuffer {
        ImageBuffer::from_rgb(32, 32, vec![0.2; 32 * 32 * 3])
    }

    #[test]
    fn a_config_without_repairs_keeps_the_pixels() {
        let image = frame();
        let config = WorkspaceConfig::default();
        assert_eq!(bake(&image, &config, None, 1600.0), image);
    }

    #[test]
    fn a_heal_stroke_changes_the_source() {
        let mut image = frame();
        for y in 14..18 {
            for x in 14..18 {
                let index = (y * 32 + x) * 3;
                image.pixels[index..index + 3].fill(1.0);
            }
        }
        let mut config = WorkspaceConfig::default();
        config
            .retouch
            .manual_heal_strokes
            .push(serde_json::json!([[[0.5, 0.5]], 12.0, 0.0, 0.0]));
        let baked = bake(&image, &config, None, 1600.0);
        assert_ne!(baked.pixels, image.pixels);
    }

    #[test]
    fn a_malformed_stroke_is_skipped() {
        let image = frame();
        let mut config = WorkspaceConfig::default();
        config
            .retouch
            .manual_heal_strokes
            .push(serde_json::json!([1]));
        assert_eq!(bake(&image, &config, None, 1600.0), image);
    }
}
