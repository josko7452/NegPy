//! Semantic-search vectors: normalizing, ranking and batch persistence.
//!
//! Mirrors `negpy/services/assets/semantic_model.py` and
//! `negpy/services/assets/embeddings.py` for the parts that do not run a model: the
//! z-score cutoff that both rankings share, the L2 normal form, and the batch loop that
//! persists each vector as it lands.

use std::collections::BTreeMap;
use std::path::Path;

use crate::infrastructure::storage::repository::StorageRepository;

/// The model's embedding width.
pub const EMBEDDING_DIM: usize = 512;
/// Named once in the model cache directory, so a retired model never passes for the
/// current one under the same filenames.
pub const MODEL_VERSION: &str = "clip-vit-base-patch16-v1";
/// How many standard deviations above a query's own mean score a candidate must sit.
pub const SIMILARITY_Z_SCORE: f64 = 3.0;
/// The CLIP preprocessing edge, in pixels.
pub const IMAGE_SIZE: usize = 224;
/// The text tower's fixed sequence length.
pub const TEXT_CONTEXT_LENGTH: usize = 77;

/// The per-channel mean and standard deviation of the CLIP image preprocessing.
pub const IMAGE_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
pub const IMAGE_STD: [f32; 3] = [0.268_629_54, 0.261_302_58, 0.275_777_11];

/// Scale a vector to unit length; an all-zero vector stays as it is.
#[must_use]
pub fn l2_normalize(vector: &[f32]) -> Vec<f32> {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if norm > 1e-12 {
        vector
            .iter()
            .map(|value| (*value as f64 / norm) as f32)
            .collect()
    } else {
        vector.to_vec()
    }
}

/// One candidate's score against the query.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub key: String,
    pub score: f64,
}

/// Keys ranked by cosine similarity to `query`, most relevant first.
///
/// A candidate below the query's own z-score cutoff is left out rather than kept at the
/// bottom, so a search narrows instead of only reordering.
#[must_use]
pub fn rank_by_similarity(query: &[f32], candidates: &BTreeMap<String, Vec<f32>>) -> Vec<String> {
    if candidates.is_empty() {
        return Vec::new();
    }
    let scored: Vec<Ranked> = candidates
        .iter()
        .filter(|(_, vector)| vector.len() == query.len())
        .map(|(key, vector)| Ranked {
            key: key.clone(),
            score: vector
                .iter()
                .zip(query)
                .map(|(left, right)| f64::from(*left) * f64::from(*right))
                .sum(),
        })
        .collect();
    if scored.is_empty() {
        return Vec::new();
    }
    let count = scored.len() as f64;
    let mean = scored.iter().map(|item| item.score).sum::<f64>() / count;
    let variance = scored
        .iter()
        .map(|item| (item.score - mean) * (item.score - mean))
        .sum::<f64>()
        / count;
    let cutoff = mean + SIMILARITY_Z_SCORE * variance.sqrt();
    let mut ordered = scored;
    ordered.sort_by(|left, right| right.score.total_cmp(&left.score));
    ordered
        .into_iter()
        .filter(|item| item.score >= cutoff)
        .map(|item| item.key)
        .collect()
}

/// The CLIP image preprocessing: shortest-edge bicubic resize, centre crop, rescale and
/// normalize, as `(channels, height, width)` row-major.
///
/// `rgba` is a row-major RGB image of `width` by `height`.
#[must_use]
pub fn preprocess_image(rgba: &[f32], width: usize, height: usize) -> Option<Vec<f32>> {
    if width == 0 || height == 0 || rgba.len() != width * height * 3 {
        return None;
    }
    let short = width.min(height);
    let scale = IMAGE_SIZE as f64 / short as f64;
    let resized_w = IMAGE_SIZE.max((width as f64 * scale).round() as usize);
    let resized_h = IMAGE_SIZE.max((height as f64 * scale).round() as usize);
    let resized = resize_bicubic(rgba, width, height, resized_w, resized_h)?;
    let left = (resized_w - IMAGE_SIZE) / 2;
    let top = (resized_h - IMAGE_SIZE) / 2;

    let plane = IMAGE_SIZE * IMAGE_SIZE;
    let mut out = vec![0.0f32; 3 * plane];
    for row in 0..IMAGE_SIZE {
        for column in 0..IMAGE_SIZE {
            let source = ((top + row) * resized_w + left + column) * 3;
            for channel in 0..3 {
                let value = resized[source + channel];
                out[channel * plane + row * IMAGE_SIZE + column] =
                    (value / 255.0 - IMAGE_MEAN[channel]) / IMAGE_STD[channel];
            }
        }
    }
    Some(out)
}

fn cubic(value: f64) -> f64 {
    // PIL's bicubic kernel coefficient a = -0.5.
    let value = value.abs();
    if value <= 1.0 {
        (1.5 * value - 2.5) * value * value + 1.0
    } else if value < 2.0 {
        ((-0.5 * value + 2.5) * value - 4.0) * value + 2.0
    } else {
        0.0
    }
}

fn resize_bicubic(
    src: &[f32],
    width: usize,
    height: usize,
    out_w: usize,
    out_h: usize,
) -> Option<Vec<f32>> {
    let mut out = vec![0.0f32; out_w * out_h * 3];
    let scale_x = width as f64 / out_w as f64;
    let scale_y = height as f64 / out_h as f64;
    for row in 0..out_h {
        let source_y = (row as f64 + 0.5) * scale_y - 0.5;
        let y0 = source_y.floor() as i64;
        for column in 0..out_w {
            let source_x = (column as f64 + 0.5) * scale_x - 0.5;
            let x0 = source_x.floor() as i64;
            let mut sum = [0.0f64; 3];
            let mut weight_sum = 0.0f64;
            for step_y in -1..=2i64 {
                let weight_y = cubic(source_y - (y0 + step_y) as f64);
                let sample_y = (y0 + step_y).clamp(0, height as i64 - 1) as usize;
                for step_x in -1..=2i64 {
                    let weight = weight_y * cubic(source_x - (x0 + step_x) as f64);
                    let sample_x = (x0 + step_x).clamp(0, width as i64 - 1) as usize;
                    let at = (sample_y * width + sample_x) * 3;
                    for channel in 0..3 {
                        sum[channel] += weight * f64::from(src[at + channel]);
                    }
                    weight_sum += weight;
                }
            }
            let destination = (row * out_w + column) * 3;
            for channel in 0..3 {
                let value = if weight_sum.abs() > 1e-12 {
                    sum[channel] / weight_sum
                } else {
                    0.0
                };
                out[destination + channel] = value.clamp(0.0, 255.0) as f32;
            }
        }
    }
    Some(out)
}

/// One file the batch loop embeds.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbedFile {
    pub path: String,
    pub hash: String,
    pub name: String,
}

/// Progress reported after each attempted file.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbedProgress {
    pub completed: usize,
    pub name: String,
}

/// Embed every file, persisting each vector as it lands.
///
/// `embed` returns the vector for one file's thumbnail, or `None` when the file has no
/// decodable thumbnail. `is_cancelled` is checked once per file, before the expensive
/// decode, so a library-wide batch can stop mid-way.
pub fn generate_batch_embeddings<F, C, P, R>(
    files: &[EmbedFile],
    repo: &StorageRepository,
    mut embed: F,
    mut is_cancelled: C,
    mut progress: P,
    mut ready: R,
) -> BTreeMap<String, Vec<f32>>
where
    F: FnMut(&EmbedFile) -> Option<Vec<f32>>,
    C: FnMut() -> bool,
    P: FnMut(&EmbedProgress),
    R: FnMut(&str, &[f32]),
{
    let mut out = BTreeMap::new();
    let mut completed = 0usize;
    for file in files {
        if is_cancelled() {
            continue;
        }
        let vector = embed(file);
        completed += 1;
        progress(&EmbedProgress {
            completed,
            name: file.name.clone(),
        });
        if let Some(vector) = vector {
            if repo
                .save_embedding(&file.hash, &vector, MODEL_VERSION, &file.path)
                .is_ok()
            {
                ready(&file.hash, &vector);
                out.insert(file.hash.clone(), vector);
            }
        }
    }
    out
}

/// The cache directory a model version's files live under.
#[must_use]
pub fn model_dir(cache_dir: &Path, version: &str) -> std::path::PathBuf {
    cache_dir.join("clip_model").join(version)
}

/// The model files a download must place beside each other.
pub const MODEL_FILES: &[(&str, &str)] = &[
    (
        "onnx/vision_model_quantized.onnx",
        "vision_model_quantized.onnx",
    ),
    (
        "onnx/text_model_quantized.onnx",
        "text_model_quantized.onnx",
    ),
    ("merges.txt", "merges.txt"),
];

/// Whether every model file is already on disk.
#[must_use]
pub fn clip_model_ready(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|(_, name)| dir.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cos(similarity: f32) -> [f32; 2] {
        [(1.0 - similarity * similarity).sqrt(), similarity]
    }

    #[test]
    fn the_ranking_keeps_only_the_outliers() {
        let mut candidates = BTreeMap::new();
        for (index, value) in (0..28)
            .map(|index| 0.08 + f64::from(index) * 0.003)
            .enumerate()
        {
            candidates.insert(format!("h{index}"), cos(value as f32).to_vec());
        }
        candidates.insert("h28".to_string(), cos(0.32).to_vec());
        candidates.insert("h29".to_string(), cos(0.35).to_vec());

        let ranked = rank_by_similarity(&[0.0, 1.0], &candidates);

        assert_eq!(ranked, ["h29", "h28"]);
    }

    #[test]
    fn an_empty_candidate_set_ranks_nothing() {
        assert!(rank_by_similarity(&[1.0, 0.0], &BTreeMap::new()).is_empty());
    }

    #[test]
    fn a_vector_that_does_not_match_the_query_width_is_skipped() {
        let mut candidates = BTreeMap::new();
        for (index, value) in (0..28)
            .map(|index| 0.08 + f64::from(index) * 0.003)
            .enumerate()
        {
            candidates.insert(format!("h{index}"), cos(value as f32).to_vec());
        }
        candidates.insert("match".to_string(), cos(0.35).to_vec());
        candidates.insert("long".to_string(), vec![1.0, 0.0, 0.0]);

        let ranked = rank_by_similarity(&[0.0, 1.0], &candidates);

        assert_eq!(ranked, ["match"]);
    }

    #[test]
    fn normalization_makes_a_unit_vector_and_leaves_zero() {
        let normalized = l2_normalize(&[3.0, 4.0]);
        assert!((normalized[0] - 0.6).abs() < 1e-6);
        assert!((normalized[1] - 0.8).abs() < 1e-6);
        assert_eq!(l2_normalize(&[0.0, 0.0]), [0.0, 0.0]);
    }

    #[test]
    fn preprocessing_returns_three_normalized_planes() {
        let image = vec![128.0f32; 8 * 6 * 3];
        let out = preprocess_image(&image, 8, 6).expect("preprocess");
        assert_eq!(out.len(), 3 * IMAGE_SIZE * IMAGE_SIZE);
        let plane = IMAGE_SIZE * IMAGE_SIZE;
        assert!((out[0] - (128.0 / 255.0 - IMAGE_MEAN[0]) / IMAGE_STD[0]).abs() < 1e-5);
        assert!(out[plane] < out[2 * plane]);
    }

    #[test]
    fn a_bad_image_shape_is_a_miss() {
        assert!(preprocess_image(&[0.0; 3], 8, 6).is_none());
    }

    #[test]
    fn the_model_is_ready_only_when_every_file_exists() {
        let temp = std::env::temp_dir().join(format!("negpy-clip-{}", std::process::id()));
        let dir = model_dir(&temp, MODEL_VERSION);
        std::fs::create_dir_all(&dir).expect("model dir");
        assert!(!clip_model_ready(&dir));
        for (_, name) in MODEL_FILES {
            std::fs::write(dir.join(name), b"x").expect("model file");
        }
        assert!(clip_model_ready(&dir));
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn a_batch_persists_a_vector_per_embedded_file() {
        let temp = std::env::temp_dir().join(format!("negpy-embed-{}", std::process::id()));
        std::fs::create_dir_all(&temp).expect("temp dir");
        let repo = StorageRepository::new(
            temp.join("edits.db").to_string_lossy().into_owned(),
            temp.join("settings.db").to_string_lossy().into_owned(),
        );
        repo.initialize().expect("schema");
        let files = [
            EmbedFile {
                path: "/roll/a.nef".to_string(),
                hash: "h1".to_string(),
                name: "a.nef".to_string(),
            },
            EmbedFile {
                path: "/roll/b.nef".to_string(),
                hash: "h2".to_string(),
                name: "b.nef".to_string(),
            },
        ];
        let mut seen = Vec::new();
        let out = generate_batch_embeddings(
            &files,
            &repo,
            |file| (file.hash == "h1").then(|| vec![1.0, 0.0]),
            || false,
            |progress| seen.push(progress.completed),
            |_, _| {},
        );
        assert_eq!(out.len(), 1);
        assert!(out.contains_key("h1"));
        assert_eq!(seen, [1, 2]);
        assert_eq!(
            repo.embeddings_for(&["h1".to_string()], MODEL_VERSION)
                .expect("load")
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&temp);
    }
}
