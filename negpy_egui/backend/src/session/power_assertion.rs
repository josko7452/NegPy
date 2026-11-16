//! Best-effort idle-sleep protection for unattended desktop operations.
//!
//! Mirrors `negpy/desktop/power_assertion.py`. The scanner still owns its USB lifecycle;
//! this asks the OS to keep the computer awake while a user-requested scan runs.

use std::process::{Child, Command, Stdio};

/// One idempotently releasable OS power assertion.
pub trait UnattendedPowerAssertion: Send {
    /// Release the assertion. A second call is a no-op.
    fn release(&mut self);
}

/// An assertion with no OS backend, used off macOS and Windows and after failures.
#[derive(Default)]
pub struct NoOpPowerAssertion;

impl UnattendedPowerAssertion for NoOpPowerAssertion {
    fn release(&mut self) {}
}

/// Picks the backend for one acquisition. Separate from the assertion so tests can inject
/// a fake OS boundary and a fake helper process.
pub trait PowerBackend {
    fn unsupported(&mut self) -> Box<dyn UnattendedPowerAssertion>;
    fn windows(&mut self) -> Box<dyn UnattendedPowerAssertion>;
    fn macos_process_info(&mut self, reason: &str) -> Option<Box<dyn UnattendedPowerAssertion>>;
    fn caffeinate(&mut self, parent_pid: u32) -> Option<Box<dyn UnattendedPowerAssertion>>;
}

/// Acquire an idle-sleep assertion for `reason` on the current platform.
///
/// An empty reason is a caller error. A backend that cannot acquire degrades to the
/// no-op assertion, because a scan must not fail over an OS hint.
pub fn acquire_unattended_power_assertion(
    reason: &str,
) -> Result<Box<dyn UnattendedPowerAssertion>, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("power assertion reason must not be empty".to_string());
    }
    acquire_with_backend(reason, &mut SystemPowerBackend)
}

pub fn acquire_with_backend(
    reason: &str,
    backend: &mut dyn PowerBackend,
) -> Result<Box<dyn UnattendedPowerAssertion>, String> {
    if cfg!(target_os = "windows") {
        return Ok(backend.windows());
    }
    if cfg!(target_os = "macos") {
        if let Some(assertion) = backend.macos_process_info(reason) {
            return Ok(assertion);
        }
        if let Some(assertion) = backend.caffeinate(std::process::id()) {
            return Ok(assertion);
        }
    }
    Ok(backend.unsupported())
}

/// The real platform boundaries.
pub struct SystemPowerBackend;

impl PowerBackend for SystemPowerBackend {
    fn unsupported(&mut self) -> Box<dyn UnattendedPowerAssertion> {
        Box::new(NoOpPowerAssertion)
    }

    #[cfg(target_os = "windows")]
    fn windows(&mut self) -> Box<dyn UnattendedPowerAssertion> {
        use windows_sys::Win32::System::Threading::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        // SAFETY: SetThreadExecutionState has no pointer arguments and only changes the
        // calling thread's execution state.
        let result = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        if result == 0 {
            log::warn!("could not create a Windows idle-sleep assertion");
            Box::new(NoOpPowerAssertion)
        } else {
            Box::new(WindowsPowerAssertion)
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn windows(&mut self) -> Box<dyn UnattendedPowerAssertion> {
        Box::new(NoOpPowerAssertion)
    }

    #[cfg(target_os = "macos")]
    fn macos_process_info(&mut self, reason: &str) -> Option<Box<dyn UnattendedPowerAssertion>> {
        use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
        let process_info = NSProcessInfo::processInfo();
        let options = NSActivityOptions::UserInitiated | NSActivityOptions::IdleSystemSleepDisabled;
        let reason = NSString::from_str(reason);
        let token = process_info.beginActivityWithOptions_reason(options, &reason);
        Some(Box::new(NsProcessInfoPowerAssertion {
            process_info,
            token: Some(token),
        }))
    }

    #[cfg(not(target_os = "macos"))]
    fn macos_process_info(&mut self, _reason: &str) -> Option<Box<dyn UnattendedPowerAssertion>> {
        None
    }

    fn caffeinate(&mut self, parent_pid: u32) -> Option<Box<dyn UnattendedPowerAssertion>> {
        let child = Command::new("/usr/bin/caffeinate")
            .args(["-i", "-w", &parent_pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        Some(Box::new(CaffeinatePowerAssertion { child: Some(child) }))
    }
}

#[cfg(target_os = "windows")]
struct WindowsPowerAssertion;

#[cfg(target_os = "windows")]
impl UnattendedPowerAssertion for WindowsPowerAssertion {
    fn release(&mut self) {
        use windows_sys::Win32::System::Threading::{ES_CONTINUOUS, SetThreadExecutionState};
        // SAFETY: The call has no pointer arguments and clears only this thread's state.
        let _ = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    }
}

#[cfg(target_os = "macos")]
struct NsProcessInfoPowerAssertion {
    process_info: objc2_foundation::NSProcessInfo,
    token: Option<
        objc2_foundation::Retained<
            objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>,
        >,
    >,
}

#[cfg(target_os = "macos")]
impl UnattendedPowerAssertion for NsProcessInfoPowerAssertion {
    fn release(&mut self) {
        if let Some(token) = self.token.take() {
            // SAFETY: The token came from this processInfo's own beginActivity call.
            unsafe { self.process_info.endActivity(&token) };
        }
    }
}

struct CaffeinatePowerAssertion {
    child: Option<Child>,
}

impl UnattendedPowerAssertion for CaffeinatePowerAssertion {
    fn release(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}
