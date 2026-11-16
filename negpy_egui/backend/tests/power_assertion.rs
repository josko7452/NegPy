//! Ports the platform selection and failure-degradation cases in
//! `tests/test_power_assertion.py`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use negpy_backend::session::power_assertion::{
    NoOpPowerAssertion, PowerBackend, UnattendedPowerAssertion, acquire_with_backend,
};

#[derive(Default)]
struct FakeBackend {
    unsupported: usize,
    windows: usize,
    process_info: usize,
    caffeinate: usize,
    fail_process_info: bool,
    fail_caffeinate: bool,
    releases: Arc<AtomicUsize>,
}

impl FakeBackend {
    fn tapped(&self) -> Box<dyn UnattendedPowerAssertion> {
        Box::new(Tap {
            releases: Arc::clone(&self.releases),
        })
    }
}

struct Tap {
    releases: Arc<AtomicUsize>,
}

impl UnattendedPowerAssertion for Tap {
    fn release(&mut self) {
        self.releases.fetch_add(1, Ordering::SeqCst);
    }
}

impl PowerBackend for FakeBackend {
    fn unsupported(&mut self) -> Box<dyn UnattendedPowerAssertion> {
        self.unsupported += 1;
        Box::new(NoOpPowerAssertion)
    }

    fn windows(&mut self) -> Box<dyn UnattendedPowerAssertion> {
        self.windows += 1;
        self.tapped()
    }

    fn macos_process_info(&mut self, _reason: &str) -> Option<Box<dyn UnattendedPowerAssertion>> {
        self.process_info += 1;
        if self.fail_process_info {
            None
        } else {
            Some(self.tapped())
        }
    }

    fn caffeinate(&mut self, _parent_pid: u32) -> Option<Box<dyn UnattendedPowerAssertion>> {
        self.caffeinate += 1;
        if self.fail_caffeinate {
            None
        } else {
            Some(self.tapped())
        }
    }
}

#[test]
fn an_empty_reason_is_rejected() {
    let error = negpy_backend::session::power_assertion::acquire_unattended_power_assertion("  ")
        .err()
        .expect("empty reason");
    assert!(error.contains("must not be empty"));
}

#[cfg(target_os = "linux")]
#[test]
fn an_unsupported_platform_uses_the_noop_backend() {
    let mut backend = FakeBackend::default();
    let _assertion = acquire_with_backend("test scan", &mut backend).expect("acquire");
    assert_eq!(backend.unsupported, 1);
    assert_eq!(backend.windows, 0);
    assert_eq!(backend.process_info, 0);
    assert_eq!(backend.caffeinate, 0);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_prefers_the_process_info_activity() {
    let mut backend = FakeBackend::default();
    let mut assertion = acquire_with_backend("NegPy scan batch", &mut backend).expect("acquire");
    assert_eq!(backend.process_info, 1);
    assert_eq!(backend.caffeinate, 0);
    assertion.release();
    assertion.release();
    assert_eq!(backend.releases.load(Ordering::SeqCst), 2);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_falls_back_to_caffeinate() {
    let mut backend = FakeBackend {
        fail_process_info: true,
        ..FakeBackend::default()
    };
    let _assertion = acquire_with_backend("NegPy scan batch", &mut backend).expect("acquire");
    assert_eq!(backend.process_info, 1);
    assert_eq!(backend.caffeinate, 1);
}

#[cfg(target_os = "macos")]
#[test]
fn failed_macos_backends_degrade_to_noop() {
    let mut backend = FakeBackend {
        fail_process_info: true,
        fail_caffeinate: true,
        ..FakeBackend::default()
    };
    let _assertion = acquire_with_backend("NegPy scan batch", &mut backend).expect("acquire");
    assert_eq!(backend.unsupported, 1);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_uses_the_thread_execution_state_backend() {
    let mut backend = FakeBackend::default();
    let _assertion = acquire_with_backend("NegPy scan batch", &mut backend).expect("acquire");
    assert_eq!(backend.windows, 1);
    assert_eq!(backend.unsupported, 0);
}
