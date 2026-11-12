//! The `override.toml` escape hatch and the performance recommendations.
//!
//! Mirrors `negpy/kernel/system/override.py`. The file is read before the settings
//! database exists, so a wrong value there must not stop the app: every field falls back
//! to its default rather than raising.

use std::collections::HashSet;
use std::path::Path;

use log::LevelFilter;

use crate::domain::types::AppConfig;

/// A hand-edited preview size below this is not a usable canvas (`= true` coerces to 1,
/// because bool is an int), and above it no GPU can hold the frame.
pub const PREVIEW_SIZE_MIN: i64 = 512;
pub const PREVIEW_SIZE_MAX: i64 = 8192;
pub const PREVIEW_SIZE_DEFAULT: i64 = 1600;

/// The long-edge texture cap applied on integrated GPUs when `max_texture_size` stays at
/// "auto": shared VRAM cannot be queried reliably, so an uncapped full-resolution load can
/// exceed what the driver can submit.
pub const INTEGRATED_GPU_TEXTURE_CAP_DEFAULT: i64 = 6144;

/// The recommendations come from RAM alone: each cache pool gets this share of it.
pub const CACHE_RAM_SHARE: f64 = 0.5;
pub const DECODED_BYTES_PER_PX: i64 = 12;
pub const TEXTURE_BYTES_PER_PX: i64 = 16;

/// What the Preferences recommendations rest on. `total_ram_bytes` is `None` when RAM is
/// unknown; `frame_px` is 0 when no frame is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    pub total_ram_bytes: Option<i64>,
    pub frame_px: i64,
    pub preview_px: i64,
}

/// How many entries of this performance key's size fit in a share of RAM; `None` when the
/// key has no derived limit, RAM is unknown, or the HQ row has no open frame.
#[must_use]
pub fn cache_entry_limit(key: &str, machine: Machine, scale: i64) -> Option<i64> {
    let total_ram = machine.total_ram_bytes?;
    if total_ram == 0 {
        return None;
    }
    let budget = (total_ram as f64 * CACHE_RAM_SHARE) as i64;
    let per_entry = match key {
        "preview_cache_max_bytes" => scale,
        "preview_cache_max_full_res_entries" => {
            if machine.frame_px == 0 {
                return None;
            }
            machine.frame_px * (DECODED_BYTES_PER_PX + TEXTURE_BYTES_PER_PX)
        }
        "preview_cache_max_entries" => machine.preview_px.pow(2) * DECODED_BYTES_PER_PX,
        "render_memo_max_entries" => machine.preview_px.pow(2) * TEXTURE_BYTES_PER_PX,
        _ => return None,
    };
    Some(budget / per_entry.max(1))
}

const DEFAULT_TOML_LINUX_WIN: &str = r#"# NegPy Override Configuration
# Edit this file and restart the app to apply changes.
#
# The [performance] numbers are also in Preferences. A value set here wins over the one
# chosen there, so this file stays the escape hatch for a machine that cannot start.
#
# rendering.backend options:
#   "auto"   - platform default (Vulkan on Linux/Windows, Metal on macOS)
#   "vulkan" - force Vulkan (Linux, Windows)
#   "dx12"   - force Direct3D 12 (Windows only)
#   "metal"  - force Metal (macOS only)
#   "cpu"    - disable GPU acceleration entirely

[rendering]
backend = "vulkan"

[display]
# Qt scene-graph backend. Options: "auto", "vulkan", "d3d12", "metal", "opengl", "software"
qt_rhi_backend = "auto"

# Window system plugin (Linux only). Options: "auto", "xcb", "wayland"
qt_platform = "auto"

[performance]
# Override HQ preview on startup. Uncomment to force a value.
# force_hq_preview = false

# Smaller tiles and no readback pipelining during export, at some cost to export speed.
# Off by default. Turn on if exporting crashes on your GPU (typically an older or
# memory-constrained integrated one) -- also available in Preferences.
# low_vram_export_tiling = true

# Cap GPU texture dimensions in pixels, including the HQ/full-resolution preview load.
# "auto" lets wgpu/hardware decide the maximum -- except on an integrated GPU, where NegPy
# applies a conservative default (currently 6144px) since shared VRAM can't be queried
# reliably up front. Set a number (e.g. 4096) to override either default.
max_texture_size = "auto"

# Long edge of the interactive preview, in pixels. Higher is a sharper canvas at 100%
# zoom and more VRAM and CPU per frame. Clamped to 512-8192.
# preview_render_size = 1600

# Preview cache size. Larger keeps more recently-viewed photos in memory for instant
# navigation; lower it on machines with little RAM. Uncomment to override defaults.
# preview_cache_max_bytes = 1200000000
# preview_cache_max_entries = 8

# How many full-resolution (HQ preview) buffers to keep in memory. Each is large
# (a 60 MP scan is ~700 MB), but keeping the previous frame makes navigating back
# instant. Raise on high-RAM machines together with preview_cache_max_bytes.
# preview_cache_max_full_res_entries = 2

# How many rendered frames to keep for instant navigate-back (no spinner, no re-render).
# A preview-size frame is ~27 MB of VRAM; HQ frames follow the full-res budget above.
# render_memo_max_entries = 8

[logging]
# Verbosity: "debug", "info", "warning", "error"
level = "info"
"#;

fn default_toml_content() -> String {
    if cfg!(target_os = "macos") {
        DEFAULT_TOML_LINUX_WIN.replace("backend = \"vulkan\"", "backend = \"metal\"")
    } else {
        DEFAULT_TOML_LINUX_WIN.to_string()
    }
}

/// The parsed `override.toml`. A field stays `None` when the file does not set it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverrideConfig {
    pub backend: String,
    pub qt_rhi_backend: String,
    pub qt_platform: String,
    pub force_hq_preview: Option<bool>,
    pub low_vram_export_tiling: Option<bool>,
    pub max_texture_size: Option<i64>,
    pub preview_render_size: Option<i64>,
    pub preview_cache_max_bytes: Option<i64>,
    pub preview_cache_max_entries: Option<i64>,
    pub preview_cache_max_full_res_entries: Option<i64>,
    pub render_memo_max_entries: Option<i64>,
    pub log_level: String,
}

impl OverrideConfig {
    /// The field defaults the Python dataclass declares.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            backend: "auto".to_string(),
            qt_rhi_backend: "auto".to_string(),
            qt_platform: "auto".to_string(),
            log_level: "info".to_string(),
            ..Self::default()
        }
    }

    /// The `log` filter the saved level name maps to; an unknown name gives `INFO`.
    #[must_use]
    pub fn log_level_filter(&self) -> LevelFilter {
        match self.log_level.as_str() {
            "debug" => LevelFilter::Debug,
            "warning" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Info,
        }
    }
}

/// Load `override.toml`, creating it with platform defaults if absent, and parse it.
#[must_use]
pub fn load_or_create(path: &Path) -> OverrideConfig {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, default_toml_content());
        return platform_defaults();
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match text.parse::<toml::Table>() {
            Ok(data) => parse(&data),
            Err(_) => platform_defaults(),
        },
        Err(_) => platform_defaults(),
    }
}

/// The platform's default backend: Metal on macOS, Vulkan elsewhere.
#[must_use]
pub fn platform_defaults() -> OverrideConfig {
    OverrideConfig {
        backend: if cfg!(target_os = "macos") {
            "metal"
        } else {
            "vulkan"
        }
        .to_string(),
        ..OverrideConfig::with_defaults()
    }
}

/// Parse a decoded TOML table, clamping or discarding every value it cannot use.
#[must_use]
pub fn parse(data: &toml::Table) -> OverrideConfig {
    let rendering = data.get("rendering").and_then(toml::Value::as_table);
    let display = data.get("display").and_then(toml::Value::as_table);
    let performance = data.get("performance").and_then(toml::Value::as_table);
    let logging_section = data.get("logging").and_then(toml::Value::as_table);

    let backend = section_string(rendering, "backend", "auto").to_lowercase();
    let backend = if matches!(
        backend.as_str(),
        "auto" | "vulkan" | "dx12" | "metal" | "cpu"
    ) {
        backend
    } else {
        "auto".to_string()
    };

    let qt_rhi = section_string(display, "qt_rhi_backend", "auto").to_lowercase();
    let qt_rhi = if matches!(
        qt_rhi.as_str(),
        "auto" | "vulkan" | "d3d12" | "metal" | "opengl" | "software"
    ) {
        qt_rhi
    } else {
        "auto".to_string()
    };

    let qt_platform = section_string(display, "qt_platform", "auto").to_lowercase();
    let qt_platform = if matches!(qt_platform.as_str(), "auto" | "xcb" | "wayland") {
        qt_platform
    } else {
        "auto".to_string()
    };

    let log_level = section_string(logging_section, "level", "info").to_lowercase();
    let log_level = if matches!(log_level.as_str(), "debug" | "info" | "warning" | "error") {
        log_level
    } else {
        "info".to_string()
    };

    OverrideConfig {
        backend,
        qt_rhi_backend: qt_rhi,
        qt_platform,
        force_hq_preview: performance
            .and_then(|table| table.get("force_hq_preview"))
            .and_then(toml::Value::as_bool),
        low_vram_export_tiling: performance
            .and_then(|table| table.get("low_vram_export_tiling"))
            .and_then(toml::Value::as_bool),
        max_texture_size: positive_i64(performance, "max_texture_size"),
        preview_render_size: positive_i64(performance, "preview_render_size"),
        preview_cache_max_bytes: positive_i64(performance, "preview_cache_max_bytes"),
        preview_cache_max_entries: positive_i64(performance, "preview_cache_max_entries"),
        preview_cache_max_full_res_entries: positive_i64(
            performance,
            "preview_cache_max_full_res_entries",
        ),
        render_memo_max_entries: positive_i64(performance, "render_memo_max_entries"),
        log_level,
    }
}

fn section_string(section: Option<&toml::Table>, key: &str, default: &str) -> String {
    section.and_then(|table| table.get(key)).map_or_else(
        || default.to_string(),
        |value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_string)
        },
    )
}

/// A positive integer from the `[performance]` table; a bool is not a number.
fn positive_i64(section: Option<&toml::Table>, key: &str) -> Option<i64> {
    let value = section?.get(key)?;
    let int = value.as_integer()?;
    (int > 0).then_some(int)
}

const WGPU_BACKEND: &[(&str, &str)] =
    &[("vulkan", "Vulkan"), ("dx12", "D3D12"), ("metal", "Metal")];
const QT_RHI: &[(&str, &str)] = &[
    ("vulkan", "vulkan"),
    ("dx12", "d3d12"),
    ("d3d12", "d3d12"),
    ("metal", "metal"),
    ("opengl", "opengl"),
    ("software", "software"),
];
const QT_PLATFORM: &[(&str, &str)] = &[("xcb", "xcb"), ("wayland", "wayland")];

/// Set the environment variables and update `app_config` from the override settings.
pub fn apply(cfg: &OverrideConfig, app_config: &mut AppConfig) {
    if cfg.backend == "cpu" {
        app_config.use_gpu = false;
    } else if cfg.backend != "auto" {
        if let Some(wgpu) = lookup(WGPU_BACKEND, &cfg.backend) {
            set_env("WGPU_BACKEND_TYPE", wgpu);
        }
        // Derive the Qt RHI from the backend unless it is overridden independently.
        if cfg.qt_rhi_backend == "auto"
            && let Some(rhi) = lookup(QT_RHI, &cfg.backend)
        {
            set_env("QSG_RHI_BACKEND", rhi);
        }
    }

    // An independent Qt RHI override takes precedence.
    if cfg.qt_rhi_backend != "auto"
        && let Some(rhi) = lookup(QT_RHI, &cfg.qt_rhi_backend)
    {
        set_env("QSG_RHI_BACKEND", rhi);
    }

    // The Qt platform plugin is Linux only.
    if cfg!(target_os = "linux")
        && cfg.qt_platform != "auto"
        && let Some(platform) = lookup(QT_PLATFORM, &cfg.qt_platform)
    {
        set_env("QT_QPA_PLATFORM", platform);
    }

    if let Some(value) = cfg.max_texture_size {
        app_config.max_texture_size = Some(value);
    }
    if let Some(value) = cfg.force_hq_preview {
        app_config.force_hq_preview = Some(value);
    }
    if let Some(value) = cfg.low_vram_export_tiling {
        app_config.low_vram_export_tiling = value;
    }
    if let Some(value) = cfg.preview_render_size {
        let clamped = value.clamp(PREVIEW_SIZE_MIN, PREVIEW_SIZE_MAX);
        if clamped != value {
            log::warn!("preview_render_size {value} is out of range, using {clamped}");
        }
        app_config.preview_render_size = clamped;
    }
    if let Some(value) = cfg.preview_cache_max_bytes {
        app_config.preview_cache_max_bytes = value;
    }
    if let Some(value) = cfg.preview_cache_max_entries {
        app_config.preview_cache_max_entries = value;
    }
    if let Some(value) = cfg.preview_cache_max_full_res_entries {
        app_config.preview_cache_max_full_res_entries = value;
    }
    if let Some(value) = cfg.render_memo_max_entries {
        app_config.render_memo_max_entries = value;
    }
}

fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| *value)
}

fn set_env(key: &str, value: &str) {
    // SAFETY: this runs during startup, before any worker thread reads the environment.
    unsafe { std::env::set_var(key, value) };
}

/// The texture-size budget to enforce for a full-resolution GPU load. An explicit
/// `max_texture_size` always wins; absent that, an integrated GPU gets the conservative
/// default and a discrete GPU stays uncapped.
#[must_use]
pub fn effective_max_texture_size(
    app_config: &AppConfig,
    gpu_is_integrated: Option<bool>,
) -> Option<i64> {
    if app_config.max_texture_size.is_some() {
        return app_config.max_texture_size;
    }
    if gpu_is_integrated == Some(true) {
        return Some(INTEGRATED_GPU_TEXTURE_CAP_DEFAULT);
    }
    None
}

/// The `[performance]` numbers a user can also set in Preferences, saved under these exact
/// key names. The bool and string keys are absent on purpose: they are read before the
/// settings database exists, or a wrong choice stops the app from starting.
pub const STORED_PERF_KEYS: [&str; 6] = [
    "preview_render_size",
    "preview_cache_max_bytes",
    "preview_cache_max_entries",
    "preview_cache_max_full_res_entries",
    "render_memo_max_entries",
    "max_texture_size",
];

/// Which stored keys `override.toml` has taken over, so a UI can say so and stand down.
#[must_use]
pub fn toml_pinned_keys(cfg: &OverrideConfig) -> HashSet<&'static str> {
    STORED_PERF_KEYS
        .iter()
        .filter(|key| match **key {
            "preview_render_size" => cfg.preview_render_size.is_some(),
            "preview_cache_max_bytes" => cfg.preview_cache_max_bytes.is_some(),
            "preview_cache_max_entries" => cfg.preview_cache_max_entries.is_some(),
            "preview_cache_max_full_res_entries" => {
                cfg.preview_cache_max_full_res_entries.is_some()
            }
            "render_memo_max_entries" => cfg.render_memo_max_entries.is_some(),
            "max_texture_size" => cfg.max_texture_size.is_some(),
            _ => false,
        })
        .copied()
        .collect()
}

/// Fill in the saved preferences that `override.toml` left alone. Runs after [`apply`], so
/// the file keeps the last word. `max_texture_size` stores 0 for "let the hardware decide",
/// which `AppConfig` spells as `None`.
pub fn apply_stored(
    cfg: &OverrideConfig,
    app_config: &mut AppConfig,
    get_setting: impl Fn(&str) -> Option<i64>,
) {
    let pinned = toml_pinned_keys(cfg);
    for key in STORED_PERF_KEYS {
        if pinned.contains(key) {
            continue;
        }
        let Some(raw) = get_setting(key) else {
            continue;
        };
        match key {
            "max_texture_size" => app_config.max_texture_size = (raw > 0).then_some(raw),
            "preview_render_size" => {
                app_config.preview_render_size = raw.clamp(PREVIEW_SIZE_MIN, PREVIEW_SIZE_MAX)
            }
            _ if raw > 0 => match key {
                "preview_cache_max_bytes" => app_config.preview_cache_max_bytes = raw,
                "preview_cache_max_entries" => app_config.preview_cache_max_entries = raw,
                "preview_cache_max_full_res_entries" => {
                    app_config.preview_cache_max_full_res_entries = raw
                }
                "render_memo_max_entries" => app_config.render_memo_max_entries = raw,
                _ => {}
            },
            _ => {}
        }
    }
}
