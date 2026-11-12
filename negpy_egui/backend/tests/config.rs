//! Ports of tests/test_shipped_defaults.py and the config half of
//! tests/test_sync_settings.py.

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::kernel::system::config::{app_config, default_workspace_config};
use negpy_backend::kernel::system::override_config::PREVIEW_SIZE_DEFAULT;

#[test]
fn the_app_config_has_the_shipped_paths_and_limits() {
    let cfg = app_config();
    assert_eq!(cfg.thumbnail_size, 256);
    assert!(cfg.max_workers >= 1);
    assert_eq!(cfg.preview_render_size, PREVIEW_SIZE_DEFAULT);
    assert_eq!(cfg.max_history_steps, 100);
    assert!(cfg.edits_db_path.ends_with("edits.db"));
    assert!(cfg.settings_db_path.ends_with("settings.db"));
    assert!(cfg.presets_dir.ends_with("presets"));
    assert!(cfg.cache_dir.ends_with("cache"));
    assert!(cfg.user_icc_dir.ends_with("icc"));
    assert!(cfg.crosstalk_dir.ends_with("crosstalk"));
    assert!(cfg.sensor_dir.ends_with("sensor"));
    assert!(cfg.flatfield_dir.ends_with("flatfield"));
    assert!(cfg.gear_dir.ends_with("gear"));
    assert!(cfg.default_export_dir.ends_with("export"));
    assert!(cfg.adobe_rgb_profile.ends_with("AdobeCompat-v4.icc"));
    assert!(cfg.override_toml_path.ends_with("override.toml"));
    assert!(cfg.use_gpu);
    assert_eq!(cfg.preview_cache_max_entries, 8);
    assert_eq!(cfg.preview_cache_max_bytes, 1_200_000_000);
    assert_eq!(cfg.preview_cache_max_full_res_entries, 2);
    assert_eq!(cfg.render_memo_max_entries, 8);
    assert_eq!(cfg.canvas_zoom_min, 0.25);
    assert_eq!(cfg.canvas_zoom_max, 8.0);
}

#[test]
fn the_shipped_edit_matches_the_dataclass_defaults() {
    let bare = WorkspaceConfig::default();
    let shipped = default_workspace_config("/tmp/export");
    assert_eq!(shipped.process, bare.process);
    assert_eq!(shipped.flatfield, bare.flatfield);
    assert_eq!(shipped.rgbscan, bare.rgbscan);
    assert_eq!(shipped.stitch, bare.stitch);
    assert_eq!(shipped.hdr, bare.hdr);
    assert_eq!(shipped.local, bare.local);
    assert_eq!(shipped.altproc, bare.altproc);
    assert_eq!(shipped.metadata, bare.metadata);
    assert_eq!(shipped.export.export_path, "/tmp/export");
}

#[test]
fn the_shipped_edit_carries_the_named_defaults() {
    let cfg = default_workspace_config("/tmp/export");
    assert_eq!(cfg.exposure.density, 1.0);
    assert_eq!(cfg.exposure.grade, 115.0);
    assert_eq!(cfg.exposure.toe_width, 2.5);
    assert_eq!(cfg.exposure.shoulder_width, 2.5);
    assert_eq!(cfg.geometry.rotation, 0);
    assert_eq!(cfg.geometry.fine_rotation, 0.0);
    assert_eq!(cfg.geometry.autocrop_offset, 0);
    assert_eq!(cfg.lab.clahe_strength, 0.0);
    assert_eq!(cfg.lab.saturation, 1.0);
    assert_eq!(cfg.lab.sharpen, 0.25);
    assert_eq!(cfg.toning.selenium_strength, 0.0);
    assert_eq!(cfg.toning.sepia_strength, 0.0);
    assert!(!cfg.retouch.dust_remove);
    assert_eq!(cfg.retouch.dust_threshold, 0.66);
    assert_eq!(cfg.retouch.dust_size, 4);
    assert_eq!(cfg.retouch.manual_dust_size, 6);
    assert_eq!(cfg.export.export_color_space, "sRGB");
    assert_eq!(cfg.export.export_print_size, 30.0);
    assert_eq!(cfg.export.export_dpi, 300);
    assert_eq!(cfg.finish.border_size, 0.0);
    assert_eq!(cfg.finish.border_color, "#ffffff");
}
