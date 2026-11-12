//! Ports of `tests/test_text_plurals.py` and `tests/test_text_human_bytes.py`.

use negpy_backend::kernel::system::i18n;
use negpy_backend::kernel::system::text::{count_of, human_bytes, plural};

#[test]
fn a_count_and_its_noun_agree() {
    assert_eq!(count_of(0, "file", ""), "0 files");
    assert_eq!(count_of(1, "file", ""), "1 file");
    assert_eq!(count_of(2, "file", ""), "2 files");
    assert_eq!(count_of(17, "file", ""), "17 files");
}

#[test]
fn zero_takes_the_plural() {
    assert_eq!(count_of(0, "frame", ""), "0 frames");
}

#[test]
fn an_irregular_plural_can_be_given() {
    assert_eq!(count_of(1, "copy", "copies"), "1 copy");
    assert_eq!(count_of(3, "copy", "copies"), "3 copies");
}

#[test]
fn the_plural_alone_omits_the_number() {
    assert_eq!(plural(1, "frame", ""), "frame");
    assert_eq!(plural(2, "frame", ""), "frames");
}

#[test]
fn it_carries_verbs_too() {
    assert_eq!(plural(1, "is", "are"), "is");
    assert_eq!(plural(2, "is", "are"), "are");
}

#[test]
fn a_multi_word_noun_pluralizes_on_the_last_word() {
    assert_eq!(count_of(2, "edit sidecar", ""), "2 edit sidecars");
}

#[test]
fn negatives_are_treated_by_magnitude() {
    assert_eq!(count_of(-1, "file", ""), "-1 file");
}

#[test]
fn a_count_in_a_single_form_language_keeps_one_noun() {
    i18n::configure("ja", None);
    assert_eq!(count_of(1, "frame", ""), "1 フレーム");
    assert_eq!(count_of(2, "frame", ""), "2 フレーム");
    i18n::configure("en", None);
}

#[test]
fn each_size_unit_starts_at_its_own_threshold() {
    assert_eq!(human_bytes(0.0), "0 B");
    assert_eq!(human_bytes(1023.0), "1023 B");
    assert_eq!(human_bytes(1024.0), "1.0 KB");
    assert_eq!(human_bytes(1_048_576.0), "1.0 MB");
    assert_eq!(human_bytes(1_073_741_824.0), "1.0 GB");
}

#[test]
fn a_size_just_under_the_threshold_moves_up() {
    assert_eq!(human_bytes(1_048_575.0), "1.0 MB");
    assert_eq!(human_bytes(1_073_741_823.0), "1.0 GB");
}

#[test]
fn gb_is_the_last_unit() {
    assert_eq!(human_bytes(1024.0 * 1024.0 * 1024.0 * 1024.0), "1024.0 GB");
}
