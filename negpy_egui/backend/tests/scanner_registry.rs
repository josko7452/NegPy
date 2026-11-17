//! Ports the observable rules of `tests/scanners/test_capabilities.py` and
//! `infrastructure/scanners/registry.py`: a saved backend id resolves, and an unknown
//! one falls back to the platform default.

use negpy_backend::services::scanning::registry::{
    backend_choices, create_backend, default_choice, label_for, unavailable_hint,
};
use negpy_backend::services::scanning::settings::default_backend_id;

#[test]
fn the_choices_are_in_dropdown_order() {
    let ids: Vec<&str> = backend_choices().iter().map(|choice| choice.id).collect();
    assert!(ids.contains(&"plustek"));
    assert!(ids.contains(&"pieusb"));
    assert!(ids.contains(&"nkscan"));
    if !cfg!(target_os = "windows") {
        assert_eq!(ids.first(), Some(&"sane"));
    }
}

#[test]
fn the_platform_default_names_a_registered_backend() {
    assert!(
        backend_choices()
            .iter()
            .any(|choice| choice.id == default_choice())
    );
    assert_eq!(default_backend_id(), default_choice());
}

#[test]
fn an_unknown_id_falls_back_to_the_default() {
    let fallback = label_for("removed-backend");
    let default = backend_choices()
        .into_iter()
        .find(|choice| choice.id == default_choice())
        .expect("a default backend")
        .label;
    assert_eq!(fallback, default);
    assert!(create_backend("removed-backend").is_err());
}

#[test]
fn a_hardware_transport_reports_its_install_hint() {
    let error = create_backend("pieusb").err().expect("a hint").0;
    assert!(error.contains(unavailable_hint("pieusb")));
}

#[test]
fn the_simulated_rig_is_opt_in() {
    assert_eq!(
        default_choice() == "sim",
        std::env::var("NEGPY_SIMULATE_HARDWARE").is_ok_and(|value| value == "1")
    );
}
