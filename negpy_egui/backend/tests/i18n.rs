//! Ports of `tests/test_i18n.py` language resolution and formatter assertions.

use negpy_backend::kernel::system::i18n::{self, Arg, Field};

fn system(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn language_resolution_follows_the_python_cases() {
    let cases: &[(&str, &[&str], &str)] = &[
        ("en", &["zh-CN"], "en"),
        ("zh_CN", &["en-US"], "zh_CN"),
        ("system", &["zh-Hans-CN"], "zh_CN"),
        ("system", &["en-US"], "en"),
        ("system", &["fr-FR"], "en"),
        ("system", &["zh-Hant-TW"], "en"),
        ("ja", &["en-US"], "ja"),
        ("ja_JP", &["en-US"], "ja"),
        ("system", &["ja-Jpan-JP"], "ja"),
        ("ko", &["en-US"], "ko"),
        ("KO_KR", &["en-US"], "ko"),
        ("system", &["ko-KR"], "ko"),
        ("de", &["en-US"], "de"),
        ("DE_AT", &["en-US"], "de"),
        ("system", &["de-DE"], "de"),
        ("vi", &["en-US"], "vi"),
        ("VI_VN", &["en-US"], "vi"),
        ("system", &["vi-VN"], "vi"),
        ("nb", &["en-US"], "nb"),
        ("system", &["nb-NO"], "nb"),
        ("system", &["no"], "nb"),
        ("system", &["nn-NO"], "nb"),
        ("system", &[], "en"),
        ("invalid", &["zh-CN"], "en"),
    ];
    for (choice, sys, expected) in cases {
        let languages = system(sys);
        let explicit = i18n::resolve_language(choice, Some(&languages));
        assert_eq!(&explicit, expected, "choice {choice:?} system {sys:?}");
    }
}

#[test]
fn format_signature_keeps_repeated_fields_and_conversions() {
    let field = |name: &str, spec: &str| Field {
        name: name.to_string(),
        spec: spec.to_string(),
        conversion: None,
    };
    assert_eq!(
        i18n::fields("{name!r} {name}").unwrap(),
        i18n::fields("{name} {name!r}").unwrap()
    );
    assert_ne!(
        i18n::fields("{x:.1f}").unwrap(),
        i18n::fields("{x:.2f}").unwrap()
    );
    assert_ne!(
        i18n::fields("{x} {x}").unwrap(),
        i18n::fields("{x}").unwrap()
    );
    assert_eq!(i18n::fields("{v:.1f}").unwrap(), vec![field("v", ".1f")]);
}

#[test]
fn the_german_catalog_uses_its_singular_and_plural_forms() {
    i18n::configure("de", None);
    use negpy_backend::kernel::system::text::count_of;
    assert_eq!(count_of(1, "frame", ""), "1 Bild");
    assert_eq!(count_of(2, "frame", ""), "2 Bilder");
    assert_eq!(count_of(2, "strip", ""), "2 Streifen");
    assert_eq!(count_of(0, "file", ""), "0 Dateien");
    i18n::configure("en", None);
}

#[test]
fn a_template_inserts_its_values_without_reformatting_them() {
    let name = "Print {filename} 中文.raw";
    let out = i18n::tr(
        "Export failed for {full_name}: {e}",
        &[
            ("full_name", Arg::Str(name)),
            ("e", Arg::Str("{raw error}")),
        ],
    );
    assert_eq!(out, format!("Export failed for {name}: {{raw error}}"));
}

#[test]
fn numbers_take_the_python_format_specs() {
    assert_eq!(format_value(".0f", 12.3456), "12");
    assert_eq!(format_value(".1f", 0.123), "0.1");
    assert_eq!(format_value(".1%", 0.123), "12.3%");
    assert_eq!(format_value("+.2f", 12.3456), "+12.35");
    assert_eq!(format_value(".1%", 0.9995), "100.0%");
    assert_eq!(format_value("g", 30.0), "30");
    assert_eq!(format_value("g", 1234567.0), "1.23457e+06");
    assert_eq!(format_value("g", 0.5), "0.5");
}

fn format_value(spec: &str, value: f64) -> String {
    let template = format!("{{v:{spec}}}");
    i18n::tr(&template, &[("v", Arg::Float(value))])
}

#[test]
fn a_missing_catalog_keeps_the_application_usable() {
    i18n::configure("en", None);
    assert_eq!(i18n::tr("Preferences", &[]), "Preferences");
    assert_eq!(i18n::language(), "en");
}
