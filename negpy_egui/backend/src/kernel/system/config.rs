//! The process-wide app configuration and the shipped edit.
//!
//! Mirrors `negpy/kernel/system/config.py`. Paths are resolved from the user directory
//! at startup and every field is public so later services can read it without a global.

use crate::domain::models::workspace::WorkspaceConfig;
use crate::domain::models::{
    ExposureConfig, FinishConfig, GeometryConfig, LabConfig, ProcessConfig, RetouchConfig,
    ToningConfig,
};
use crate::domain::types::AppConfig;
use crate::kernel::system::override_config::PREVIEW_SIZE_DEFAULT;
use crate::kernel::system::paths::{default_user_dir, resource_path};

/// The paths and limits assembled once at startup.
#[must_use]
pub fn app_config() -> AppConfig {
    let user_dir = default_user_dir();
    AppConfig {
        thumbnail_size: 256,
        max_workers: std::thread::available_parallelism().map_or(1, |count| count.get() as i64),
        preview_render_size: PREVIEW_SIZE_DEFAULT,
        edits_db_path: user_dir.join("edits.db").display().to_string(),
        settings_db_path: user_dir.join("settings.db").display().to_string(),
        presets_dir: user_dir.join("presets").display().to_string(),
        cache_dir: user_dir.join("cache").display().to_string(),
        user_icc_dir: user_dir.join("icc").display().to_string(),
        crosstalk_dir: user_dir.join("crosstalk").display().to_string(),
        sensor_dir: user_dir.join("sensor").display().to_string(),
        flatfield_dir: user_dir.join("flatfield").display().to_string(),
        gear_dir: user_dir.join("gear").display().to_string(),
        default_export_dir: user_dir.join("export").display().to_string(),
        adobe_rgb_profile: resource_path("icc/AdobeCompat-v4.icc")
            .display()
            .to_string(),
        override_toml_path: user_dir.join("override.toml").display().to_string(),
        max_history_steps: 100,
        ..AppConfig::with_defaults()
    }
}

/// The shipped defaults for a new edit.
#[must_use]
pub fn default_workspace_config(export_path: impl Into<String>) -> WorkspaceConfig {
    let mut config = WorkspaceConfig {
        process: ProcessConfig {
            crosstalk_strength: 0.0,
            ..ProcessConfig::default()
        },
        exposure: ExposureConfig {
            density: 1.0,
            grade: 115.0,
            toe_width: 2.5,
            shoulder_width: 2.5,
            ..ExposureConfig::default()
        },
        geometry: GeometryConfig {
            rotation: 0,
            fine_rotation: 0.0,
            autocrop_offset: 0,
            ..GeometryConfig::default()
        },
        lab: LabConfig {
            clahe_strength: 0.0,
            saturation: 1.0,
            sharpen: 0.25,
            ..LabConfig::default()
        },
        toning: ToningConfig {
            selenium_strength: 0.0,
            sepia_strength: 0.0,
            ..ToningConfig::default()
        },
        retouch: RetouchConfig {
            dust_remove: false,
            dust_threshold: 0.66,
            dust_size: 4,
            manual_dust_size: 6,
            ..RetouchConfig::default()
        },
        finish: FinishConfig {
            border_size: 0.0,
            border_color: "#ffffff".to_string(),
            ..FinishConfig::default()
        },
        ..WorkspaceConfig::default()
    };
    config.export.export_path = export_path.into();
    config
}
