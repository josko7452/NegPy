//! Ports of tests/test_ir_sidecar_discovery.py and the extension rows of
//! tests/test_jxl_loader.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::constants::{
    SUPPORTED_JXL_EXTENSIONS, SUPPORTED_RAW_EXTENSIONS, is_hidden_path, is_image_name,
    is_ir_sidecar_path,
};

fn touch(dir: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(dir.join(name), b"").unwrap();
    }
}

#[test]
fn an_ir_sidecar_is_hidden_only_when_its_main_file_is_present() {
    let dir = tempfile::tempdir().unwrap();
    touch(
        dir.path(),
        &[
            "frame1.tif",
            "frame1_IR.tif",
            "frame1_IR_VALID.tif",
            "frame1_ir.tiff",
            "frame1_IR.jpg",
            "lonely_IR.tif",
            "_IR.tif",
        ],
    );

    for name in ["frame1_IR.tif", "frame1_IR_VALID.tif", "frame1_ir.tiff"] {
        assert!(is_ir_sidecar_path(&dir.path().join(name)), "{name}");
    }
    for name in ["frame1.tif", "frame1_IR.jpg", "lonely_IR.tif", "_IR.tif"] {
        assert!(!is_ir_sidecar_path(&dir.path().join(name)), "{name}");
    }
}

#[test]
fn a_main_file_name_matches_the_sidecar_without_case() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), &["Frame2.TIF", "frame2_ir.tif"]);
    assert!(is_ir_sidecar_path(&dir.path().join("frame2_ir.tif")));
}

#[test]
fn dot_names_are_hidden_and_jxl_is_a_supported_image() {
    assert!(is_hidden_path(Path::new("/photos/.frame.dng")));
    assert!(!is_hidden_path(Path::new("/photos/frame.dng")));
    assert!(is_image_name("frame.jxl"));
    assert!(SUPPORTED_JXL_EXTENSIONS.contains(&".jxl"));
    assert!(SUPPORTED_RAW_EXTENSIONS.contains(&".jxl"));
}
