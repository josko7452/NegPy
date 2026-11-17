//! The scanner backends this build can offer.
//!
//! Mirrors `negpy/infrastructure/scanners/registry.py`: the dropdown lists one entry per
//! backend in registration order, the platform picks the default, and an unknown or
//! retired id falls back instead of failing a scan. A transport that needs a system
//! driver reports the install hint.

use crate::services::scanning::settings::default_backend_id;
use crate::services::scanning::simulated::SimulatedBackend;
use crate::services::scanning::transport::{ScannerBackend, ScannerUnavailable};

/// One selectable backend: its saved id and the label the dropdown shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendChoice {
    pub id: &'static str,
    pub label: &'static str,
}

/// Every backend this build registers, in dropdown order.
pub fn backend_choices() -> Vec<BackendChoice> {
    let mut choices = Vec::new();
    if !cfg!(target_os = "windows") {
        choices.push(BackendChoice {
            id: "sane",
            label: "SANE",
        });
    }
    choices.extend([
        BackendChoice {
            id: "nkscan",
            label: "Nikon Coolscan (nkscan)",
        },
        BackendChoice {
            id: "plustek",
            label: "pyOpticfilm (Plustek)",
        },
        BackendChoice {
            id: "pieusb",
            label: "PIEUSB",
        },
    ]);
    if crate::services::scanning::simulated::enabled() {
        choices.insert(
            0,
            BackendChoice {
                id: "sim",
                label: "Simulated",
            },
        );
    }
    choices
}

/// The default backend id for this platform and build.
#[must_use]
pub fn default_choice() -> &'static str {
    if crate::services::scanning::simulated::enabled() {
        return "sim";
    }
    default_backend_id()
}

/// The label for a saved id, falling back to the default backend's label.
#[must_use]
pub fn label_for(id: &str) -> &'static str {
    if let Some(choice) = backend_choices().into_iter().find(|choice| choice.id == id) {
        return choice.label;
    }
    backend_choices()
        .into_iter()
        .find(|choice| choice.id == default_choice())
        .map_or("SANE", |choice| choice.label)
}

/// The install hint a transport reports when its driver is absent.
#[must_use]
pub fn unavailable_hint(id: &str) -> &'static str {
    match id {
        "plustek" => {
            "The Plustek USB backend is not available. Install it with: uv sync --group plustek. On Windows, bind the scanner's USB id to WinUSB with Zadig (see docs/PLUSTEK_WINDOWS.md)."
        }
        "sane" => {
            "The SANE backend is not available on this system. Install the SANE scanner driver."
        }
        "pieusb" => "The PIEUSB backend is not available. Install the PIE USB driver.",
        "nkscan" => "The nkscan backend is not available. Install the nkscan library.",
        _ => "This scanner backend is not available in this build.",
    }
}

/// Instantiate the backend for `id`.
///
/// `sim` is the only transport this build drives. The hardware transports report their
/// install hint, which the panel shows in place of a device list.
pub fn create_backend(id: &str) -> Result<Box<dyn ScannerBackend>, ScannerUnavailable> {
    let resolved = if backend_choices().iter().any(|choice| choice.id == id) {
        id
    } else {
        default_choice()
    };
    match resolved {
        "sim" => Ok(Box::new(SimulatedBackend::new())),
        other => Err(ScannerUnavailable(format!(
            "{} {}",
            label_for(other),
            unavailable_hint(other)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_has_a_label() {
        for choice in backend_choices() {
            assert!(!choice.label.is_empty());
            assert_eq!(label_for(choice.id), choice.label);
        }
    }

    #[test]
    fn an_unknown_id_falls_back_to_the_default_label() {
        let fallback = label_for("removed-backend");
        let default = backend_choices()
            .into_iter()
            .find(|choice| choice.id == default_choice())
            .map(|choice| choice.label)
            .unwrap_or("SANE");
        assert_eq!(fallback, default);
    }

    #[test]
    fn a_simulated_build_drives_the_simulated_backend() {
        if crate::services::scanning::simulated::enabled() {
            assert!(create_backend("sim").is_ok());
        }
    }

    #[test]
    fn a_hardware_transport_names_its_install_hint() {
        let error = create_backend("plustek").err().expect("a hint").0;
        assert!(error.contains("plustek"));
    }
}
