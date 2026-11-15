//! Mirrors `negpy/kernel/system/memory.py`: physical memory the OS can report.

use std::path::Path;

/// Available memory without paging, or `None` when the OS cannot report it.
#[must_use]
pub fn available_system_memory_bytes() -> Option<u64> {
    if cfg!(target_os = "linux")
        && let Some(available) = linux_available_memory()
    {
        return Some(available);
    }
    if cfg!(target_os = "macos")
        && let Some(available) = macos_available_memory()
    {
        return Some(available);
    }
    posix_available_memory()
}

/// Physical memory installed, or `None` when the OS cannot report it.
#[must_use]
pub fn total_system_memory_bytes() -> Option<u64> {
    posix_total_memory()
}

fn posix_total_memory() -> Option<u64> {
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    Some((pages as u64).saturating_mul(page_size as u64))
}

fn posix_available_memory() -> Option<u64> {
    let pages = unsafe { libc::sysconf(libc::_SC_AVPHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    Some((pages as u64).saturating_mul(page_size as u64))
}

#[cfg(target_os = "linux")]
fn linux_available_memory() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("MemAvailable:") {
            let kib = value.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(kib * 1024);
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn linux_available_memory() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn macos_available_memory() -> Option<u64> {
    let output = std::process::Command::new("/usr/bin/vm_stat")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_vm_stat(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
fn macos_available_memory() -> Option<u64> {
    None
}

/// The reclaimable pages `vm_stat` reports, as bytes.
#[must_use]
pub fn parse_vm_stat(output: &str) -> Option<u64> {
    let page_size = output
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    let mut pages = 0u64;
    for line in output.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !matches!(name, "Pages free" | "Pages inactive" | "Pages speculative") {
            continue;
        }
        let Ok(value) = value.trim().trim_end_matches('.').parse::<u64>() else {
            continue;
        };
        pages = pages.saturating_add(value);
    }
    (pages > 0).then(|| pages * page_size)
}

/// A cached directory read for callers that only need to know a path exists.
#[must_use]
pub fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}
