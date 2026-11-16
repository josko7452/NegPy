//! The CLIP inference session over `tract-onnx`.
//!
//! Mirrors `negpy/services/assets/semantic_model.py`: sessions build lazily, each
//! embedder l2-normalizes its vector, and model files stay in the versioned cache.

#[cfg(feature = "tract-onnx")]
use std::path::{Path, PathBuf};
#[cfg(feature = "tract-onnx")]
use std::sync::OnceLock;

#[cfg(feature = "tract-onnx")]
use crate::services::assets::clip_tokenizer::ClipTokenizer;
#[cfg(feature = "tract-onnx")]
use crate::services::assets::embeddings::{
    EMBEDDING_DIM, TEXT_CONTEXT_LENGTH, l2_normalize, model_dir, preprocess_image,
};

#[cfg(feature = "tract-onnx")]
/// Every file a model download must leave in place.
pub const VISION_FILE: &str = "vision_model_quantized.onnx";
#[cfg(feature = "tract-onnx")]
pub const TEXT_FILE: &str = "text_model_quantized.onnx";
#[cfg(feature = "tract-onnx")]
pub const MERGES_FILE: &str = "merges.txt";

#[cfg(feature = "tract-onnx")]
/// One loaded tower.
struct Session {
    plan: std::sync::Arc<
        tract_onnx::prelude::RunnableModel<
            tract_onnx::prelude::TypedFact,
            Box<dyn tract_onnx::prelude::TypedOp>,
        >,
    >,
    inputs: Vec<String>,
}

#[cfg(feature = "tract-onnx")]
fn load_session(path: &Path) -> Result<Session, String> {
    use tract_onnx::prelude::*;
    let model = tract_onnx::onnx()
        .model_for_path(path)
        .map_err(|error| format!("Cannot load {}: {error}", path.display()))?;
    let inputs = model
        .input_outlets()
        .map_err(|error| error.to_string())?
        .iter()
        .map(|outlet| model.node(outlet.node).name.clone())
        .collect();
    let plan = model
        .into_typed()
        .map_err(|error| error.to_string())?
        .into_optimized()
        .map_err(|error| error.to_string())?
        .into_runnable()
        .map_err(|error| error.to_string())?;
    Ok(Session { plan, inputs })
}

#[cfg(feature = "tract-onnx")]
impl Session {
    fn run(&self, named: &[(&str, tract_onnx::prelude::Tensor)]) -> Result<Vec<f32>, String> {
        use tract_onnx::prelude::*;
        let mut inputs = TVec::new();
        for name in &self.inputs {
            let tensor = named
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, tensor)| tensor.clone())
                .ok_or_else(|| {
                    let have: Vec<&str> = named.iter().map(|(field, _)| *field).collect();
                    format!("CLIP ONNX model expects input '{name}', not provided (have: {have:?})")
                })?;
            inputs.push(tensor.into());
        }
        let outputs = self.plan.run(inputs).map_err(|error| error.to_string())?;
        let tensor = outputs
            .first()
            .ok_or_else(|| "CLIP model returned no output.".to_string())?;
        let values = tensor
            .to_plain_array_view::<f32>()
            .map_err(|error| error.to_string())?;
        values
            .as_slice()
            .ok_or_else(|| "CLIP model returned a non-contiguous vector.".to_string())?
            .get(..EMBEDDING_DIM)
            .map(<[f32]>::to_vec)
            .ok_or_else(|| "CLIP model returned a short vector.".to_string())
    }
}

/// Lazily loaded text and vision towers for semantic search.
#[cfg(feature = "tract-onnx")]
#[derive(Default)]
pub struct ClipModel {
    cache_dir: Option<PathBuf>,
    vision: OnceLock<Option<Session>>,
    text: OnceLock<Option<Session>>,
    tokenizer: OnceLock<Option<ClipTokenizer>>,
}

#[cfg(feature = "tract-onnx")]
impl ClipModel {
    /// A model over the default versioned cache directory.
    #[must_use]
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            cache_dir: Some(model_dir(
                cache_dir,
                crate::services::assets::embeddings::MODEL_VERSION,
            )),
            ..Self::default()
        }
    }

    /// A model over one exact directory.
    #[must_use]
    pub fn from_dir(dir: PathBuf) -> Self {
        Self {
            cache_dir: Some(dir),
            ..Self::default()
        }
    }

    fn dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }

    fn vision(&self) -> Option<&Session> {
        self.vision
            .get_or_init(|| {
                self.dir()
                    .and_then(|dir| load_session(&dir.join(VISION_FILE)).ok())
            })
            .as_ref()
    }

    fn text(&self) -> Option<&Session> {
        self.text
            .get_or_init(|| {
                self.dir()
                    .and_then(|dir| load_session(&dir.join(TEXT_FILE)).ok())
            })
            .as_ref()
    }

    fn tokenizer(&self) -> Option<&ClipTokenizer> {
        self.tokenizer
            .get_or_init(|| {
                self.dir()
                    .and_then(|dir| ClipTokenizer::from_file(&dir.join(MERGES_FILE)).ok())
            })
            .as_ref()
    }

    /// An L2-normalized embedding of a row-major RGB image in the 0..255 range.
    pub fn embed_image(&self, rgb: &[f32], width: usize, height: usize) -> Option<Vec<f32>> {
        self.embed_image_result(rgb, width, height).ok().flatten()
    }

    /// `embed_image` with the reason it produced no vector.
    pub fn embed_image_result(
        &self,
        rgb: &[f32],
        width: usize,
        height: usize,
    ) -> Result<Option<Vec<f32>>, String> {
        use tract_onnx::prelude::*;
        let Some(pixels) = preprocess_image(rgb, width, height) else {
            return Ok(None);
        };
        let tensor =
            Tensor::from_shape(&[1, 3, 224, 224], &pixels).map_err(|error| error.to_string())?;
        let Some(session) = self.vision() else {
            return Ok(None);
        };
        let vector = session.run(&[("pixel_values", tensor)])?;
        Ok(Some(l2_normalize(&vector)))
    }

    /// An L2-normalized embedding of a text query.
    pub fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        self.embed_text_result(text).ok().flatten()
    }

    /// `embed_text` with the reason it produced no vector.
    pub fn embed_text_result(&self, text: &str) -> Result<Option<Vec<f32>>, String> {
        use tract_onnx::prelude::*;
        let Some(tokenizer) = self.tokenizer() else {
            return Ok(None);
        };
        let mut tokenizer = tokenizer.clone();
        let (ids, mask) = tokenizer.encode(text, TEXT_CONTEXT_LENGTH);
        let shape = [1, ids.len()];
        let ids = Tensor::from_shape(&shape, &ids).map_err(|error| error.to_string())?;
        let mask = Tensor::from_shape(&shape, &mask).map_err(|error| error.to_string())?;
        let Some(session) = self.text() else {
            return Ok(None);
        };
        let vector = session.run(&[("input_ids", ids), ("attention_mask", mask)])?;
        Ok(Some(l2_normalize(&vector)))
    }
}
