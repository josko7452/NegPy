//! The CLIP session wrapper over a generated ONNX graph.

#![cfg(feature = "tract-onnx")]

use std::fs;
use std::path::Path;

use negpy_backend::services::assets::clip_model::{ClipModel, MERGES_FILE, TEXT_FILE, VISION_FILE};
use negpy_backend::services::assets::embeddings::EMBEDDING_DIM;
use prost::Message;
use tract_onnx::pb;

fn value(name: &str, dims: &[i64]) -> pb::ValueInfoProto {
    use pb::tensor_shape_proto::{Dimension, dimension};
    use pb::type_proto::{Tensor, Value};
    pb::ValueInfoProto {
        name: name.to_string(),
        r#type: Some(pb::TypeProto {
            denotation: String::new(),
            value: Some(Value::TensorType(Tensor {
                elem_type: pb::tensor_proto::DataType::Float as i32,
                shape: Some(pb::TensorShapeProto {
                    dim: dims
                        .iter()
                        .map(|dim| Dimension {
                            denotation: String::new(),
                            value: Some(dimension::Value::DimValue(*dim)),
                        })
                        .collect(),
                }),
            })),
        }),
        doc_string: String::new(),
    }
}

fn write_identity_model(path: &Path, input: &str, shape: &[i64]) {
    let output = format!("{input}_out");
    let graph = pb::GraphProto {
        node: vec![pb::NodeProto {
            input: vec![input.to_string()],
            output: vec![output.clone()],
            name: "identity".to_string(),
            op_type: "Identity".to_string(),
            domain: String::new(),
            attribute: vec![],
            doc_string: String::new(),
        }],
        name: input.to_string(),
        initializer: vec![],
        sparse_initializer: vec![],
        doc_string: String::new(),
        input: vec![value(input, shape)],
        output: vec![value(&output, shape)],
        value_info: vec![],
        quantization_annotation: vec![],
    };
    let model = pb::ModelProto {
        ir_version: 8,
        opset_import: vec![pb::OperatorSetIdProto {
            domain: String::new(),
            version: 17,
        }],
        producer_name: "negpy-test".to_string(),
        producer_version: String::new(),
        domain: String::new(),
        model_version: 1,
        doc_string: String::new(),
        graph: Some(graph),
        metadata_props: vec![],
        training_info: vec![],
        functions: vec![],
    };
    fs::write(path, model.encode_to_vec()).unwrap();
}

#[test]
fn the_vision_tower_runs_and_normalizes_its_vector() {
    let dir = tempfile::tempdir().unwrap();
    write_identity_model(
        &dir.path().join(VISION_FILE),
        "pixel_values",
        &[1, 3, 224, 224],
    );
    let model = ClipModel::from_dir(dir.path().to_path_buf());
    let image = vec![128.0f32; 16 * 16 * 3];
    let vector = match model.embed_image_result(&image, 16, 16) {
        Ok(Some(vector)) => vector,
        Ok(None) => panic!("no vector"),
        Err(error) => panic!("embedding failed: {error}"),
    };
    assert_eq!(vector.len(), EMBEDDING_DIM);
    let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
}

#[test]
fn a_text_model_without_its_second_input_is_a_miss() {
    let dir = tempfile::tempdir().unwrap();
    write_identity_model(&dir.path().join(TEXT_FILE), "input_ids", &[1, 77]);
    fs::write(dir.path().join(MERGES_FILE), "#version: 0.2\n").unwrap();
    let model = ClipModel::from_dir(dir.path().to_path_buf());
    assert!(model.embed_text("a cat").is_none());
}

#[test]
fn a_missing_model_file_reads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let model = ClipModel::from_dir(dir.path().to_path_buf());
    assert!(model.embed_text("a cat").is_none());
    assert!(model.embed_image(&[0.0; 48], 4, 4).is_none());
}
