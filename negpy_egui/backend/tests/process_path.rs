//! Ports tests/test_source_encoding.py's render-path and decode-token rules.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode, SourceEncoding};
use negpy_backend::features::process::path::{
    RenderPath, demosaic_token, effective_linear_raw, is_mono, linear_raw_token, positive_decode,
    render_path,
};

fn process(mode: ProcessMode, positive: bool, linear_raw: bool) -> ProcessConfig {
    ProcessConfig {
        process_mode: mode,
        positive_source: positive,
        linear_raw,
        ..ProcessConfig::default()
    }
}

#[test]
fn a_negative_prints() {
    assert_eq!(
        render_path(&process(ProcessMode::C41, false, false)),
        RenderPath::Print
    );
    assert_eq!(
        render_path(&process(ProcessMode::BW, false, false)),
        RenderPath::Print
    );
}

#[test]
fn a_slide_transfers_unless_marked_positive() {
    assert_eq!(
        render_path(&process(ProcessMode::E6, false, false)),
        RenderPath::Transfer
    );
    assert_eq!(
        render_path(&process(ProcessMode::E6, true, false)),
        RenderPath::Positive
    );
}

#[test]
fn only_a_positive_carries_a_positive_decode() {
    let mut negative = process(ProcessMode::C41, false, false);
    negative.source_encoding = SourceEncoding::Gamma22;
    assert_eq!(positive_decode(&negative), None);
    let mut slide = process(ProcessMode::E6, false, false);
    slide.source_encoding = SourceEncoding::Linear;
    assert_eq!(positive_decode(&slide), None);
    let mut positive = process(ProcessMode::E6, true, false);
    positive.source_encoding = SourceEncoding::Gamma22;
    assert_eq!(positive_decode(&positive), Some(SourceEncoding::Gamma22));
}

#[test]
fn linear_raw_is_forced_on_an_as_captured_slide() {
    assert!(!effective_linear_raw(&process(
        ProcessMode::C41,
        false,
        false
    )));
    assert!(effective_linear_raw(&process(
        ProcessMode::C41,
        false,
        true
    )));
    assert!(effective_linear_raw(&process(
        ProcessMode::E6,
        false,
        false
    )));
    assert!(!effective_linear_raw(&process(
        ProcessMode::E6,
        true,
        false
    )));
}

#[test]
fn the_decode_token_uses_the_effective_linear_raw_value() {
    assert_eq!(
        linear_raw_token(&process(ProcessMode::C41, false, false)),
        "|lr:0"
    );
    assert_eq!(
        linear_raw_token(&process(ProcessMode::C41, false, true)),
        "|lr:1"
    );
    assert_eq!(
        linear_raw_token(&process(ProcessMode::E6, false, false)),
        "|lr:1"
    );
    let mut positive = process(ProcessMode::E6, true, false);
    positive.source_encoding = SourceEncoding::Gamma22;
    assert_eq!(linear_raw_token(&positive), "|lr:0|pd:gamma22");
}

#[test]
fn bw_is_the_bw_negative_or_a_bw_slide() {
    assert!(is_mono(&process(ProcessMode::BW, false, false)));
    assert!(!is_mono(&process(ProcessMode::C41, false, false)));
    let mut slide = process(ProcessMode::E6, false, false);
    assert!(!is_mono(&slide));
    slide.slide_bw = true;
    assert!(is_mono(&slide));
}

#[test]
fn the_demosaic_token_names_the_mode() {
    assert_eq!(demosaic_token("AHD"), "|dm:AHD");
}
