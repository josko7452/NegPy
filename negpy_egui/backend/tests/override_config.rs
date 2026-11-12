//! Ports of `tests/test_override_config.py`.

use std::sync::Mutex;

use negpy_backend::domain::types::AppConfig;
use negpy_backend::kernel::system::override_config::{
    self as oc, INTEGRATED_GPU_TEXTURE_CAP_DEFAULT, Machine, OverrideConfig, PREVIEW_SIZE_MAX,
    PREVIEW_SIZE_MIN,
};

/// `apply` writes process environment variables, which are global.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn sample_app() -> AppConfig {
    let mut app = AppConfig::with_defaults();
    app.edits_db_path = "/tmp/e.db".into();
    app.override_toml_path = "/tmp/override.toml".into();
    app
}

fn parse(text: &str) -> OverrideConfig {
    oc::parse(&text.parse::<toml::Table>().unwrap())
}

#[test]
fn the_defaults_match_the_python_dataclass() {
    let cfg = OverrideConfig::with_defaults();
    assert_eq!(cfg.backend, "auto");
    assert_eq!(cfg.qt_rhi_backend, "auto");
    assert_eq!(cfg.qt_platform, "auto");
    assert_eq!(cfg.force_hq_preview, None);
    assert_eq!(cfg.max_texture_size, None);
    assert_eq!(cfg.log_level, "info");
}

#[test]
fn the_log_level_maps_to_a_filter_and_an_unknown_name_falls_back() {
    assert_eq!(
        OverrideConfig {
            log_level: "debug".into(),
            ..OverrideConfig::with_defaults()
        }
        .log_level_filter(),
        log::LevelFilter::Debug
    );
    assert_eq!(
        OverrideConfig {
            log_level: "warning".into(),
            ..OverrideConfig::with_defaults()
        }
        .log_level_filter(),
        log::LevelFilter::Warn
    );
    assert_eq!(
        OverrideConfig {
            log_level: "verbose".into(),
            ..OverrideConfig::with_defaults()
        }
        .log_level_filter(),
        log::LevelFilter::Info
    );
}

#[test]
fn parse_reads_every_field() {
    let cfg = parse(
        r#"
        [rendering]
        backend = "dx12"
        [display]
        qt_rhi_backend = "d3d12"
        qt_platform = "xcb"
        [performance]
        force_hq_preview = true
        max_texture_size = 4096
        [logging]
        level = "debug"
        "#,
    );
    assert_eq!(cfg.backend, "dx12");
    assert_eq!(cfg.qt_rhi_backend, "d3d12");
    assert_eq!(cfg.qt_platform, "xcb");
    assert_eq!(cfg.force_hq_preview, Some(true));
    assert_eq!(cfg.max_texture_size, Some(4096));
    assert_eq!(cfg.log_level, "debug");
}

#[test]
fn parse_rejects_nonsense_numbers() {
    for value in ["0", "-1", "\"eight\""] {
        let cfg = parse(&format!(
            "[performance]\nrender_memo_max_entries = {value}\n"
        ));
        assert_eq!(cfg.render_memo_max_entries, None, "value {value}");
    }
}

#[test]
fn an_invalid_choice_falls_back_to_auto() {
    assert_eq!(parse("[rendering]\nbackend = \"directx9\"").backend, "auto");
    assert_eq!(
        parse("[display]\nqt_rhi_backend = \"dx11\"").qt_rhi_backend,
        "auto"
    );
    assert_eq!(
        parse("[display]\nqt_platform = \"windows\"").qt_platform,
        "auto"
    );
    assert_eq!(parse("[logging]\nlevel = \"trace\"").log_level, "info");
}

#[test]
fn max_texture_size_auto_and_zero_yield_none() {
    assert_eq!(
        parse("[performance]\nmax_texture_size = \"auto\"").max_texture_size,
        None
    );
    assert_eq!(
        parse("[performance]\nmax_texture_size = 0").max_texture_size,
        None
    );
}

#[test]
fn an_empty_table_gives_all_defaults() {
    let cfg = parse("");
    assert_eq!(cfg.backend, "auto");
    assert_eq!(cfg.qt_rhi_backend, "auto");
    assert_eq!(cfg.max_texture_size, None);
}

#[test]
fn low_vram_export_tiling_reads_both_booleans_and_absent() {
    assert_eq!(
        parse("[performance]\nlow_vram_export_tiling = true").low_vram_export_tiling,
        Some(true)
    );
    assert_eq!(
        parse("[performance]\nlow_vram_export_tiling = false").low_vram_export_tiling,
        Some(false)
    );
    assert_eq!(parse("").low_vram_export_tiling, None);
}

#[test]
fn a_missing_file_is_created_with_a_valid_backend_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("override.toml");
    let _ = oc::load_or_create(&path);
    assert!(path.exists());
    let data: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    let rendering = data
        .get("rendering")
        .and_then(toml::Value::as_table)
        .unwrap();
    assert!(rendering.contains_key("backend"));
}

#[test]
fn creating_the_file_makes_its_parent_and_uses_the_platform_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir").join("override.toml");
    let _ = oc::load_or_create(&path);
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    let expected = if cfg!(target_os = "macos") {
        "backend = \"metal\""
    } else {
        "backend = \"vulkan\""
    };
    assert!(content.contains(expected));
}

#[test]
fn an_existing_file_is_loaded_and_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("override.toml");
    std::fs::write(&path, "[rendering]\nbackend = \"cpu\"\n").unwrap();
    let cfg = oc::load_or_create(&path);
    assert_eq!(cfg.backend, "cpu");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("backend = \"cpu\"")
    );
}

#[test]
fn a_corrupt_file_falls_back_to_the_platform_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("override.toml");
    std::fs::write(&path, "this is not valid toml ][[\n").unwrap();
    let cfg = oc::load_or_create(&path);
    let expected = if cfg!(target_os = "macos") {
        "metal"
    } else {
        "vulkan"
    };
    assert_eq!(cfg.backend, expected);
}

#[test]
fn cpu_disables_the_gpu() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mut app = sample_app();
    oc::apply(
        &OverrideConfig {
            backend: "cpu".into(),
            ..OverrideConfig::with_defaults()
        },
        &mut app,
    );
    assert!(!app.use_gpu);
}

#[test]
fn a_backend_forces_the_wgpu_and_rhi_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mut app = sample_app();
    oc::apply(
        &OverrideConfig {
            backend: "dx12".into(),
            ..OverrideConfig::with_defaults()
        },
        &mut app,
    );
    assert_eq!(std::env::var("WGPU_BACKEND_TYPE").unwrap(), "D3D12");
    assert_eq!(std::env::var("QSG_RHI_BACKEND").unwrap(), "d3d12");
    unsafe { std::env::remove_var("WGPU_BACKEND_TYPE") };
    unsafe { std::env::remove_var("QSG_RHI_BACKEND") };
}

#[test]
fn the_performance_values_reach_the_app_config() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mut app = sample_app();
    let cfg = OverrideConfig {
        max_texture_size: Some(2048),
        force_hq_preview: Some(true),
        low_vram_export_tiling: Some(false),
        preview_render_size: Some(3000),
        render_memo_max_entries: Some(16),
        ..OverrideConfig::with_defaults()
    };
    oc::apply(&cfg, &mut app);
    assert_eq!(app.max_texture_size, Some(2048));
    assert_eq!(app.force_hq_preview, Some(true));
    assert!(!app.low_vram_export_tiling);
    assert_eq!(app.preview_render_size, 3000);
    assert_eq!(app.render_memo_max_entries, 16);
}

#[test]
fn an_out_of_range_preview_size_is_clamped() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mut app = sample_app();
    oc::apply(
        &OverrideConfig {
            preview_render_size: Some(100),
            ..OverrideConfig::with_defaults()
        },
        &mut app,
    );
    assert_eq!(app.preview_render_size, PREVIEW_SIZE_MIN);
    oc::apply(
        &OverrideConfig {
            preview_render_size: Some(99999),
            ..OverrideConfig::with_defaults()
        },
        &mut app,
    );
    assert_eq!(app.preview_render_size, PREVIEW_SIZE_MAX);
}

#[test]
fn the_texture_cap_precedence_is_override_then_integrated_then_uncapped() {
    let mut app = sample_app();
    app.max_texture_size = Some(4096);
    assert_eq!(oc::effective_max_texture_size(&app, Some(true)), Some(4096));
    app.max_texture_size = None;
    assert_eq!(
        oc::effective_max_texture_size(&app, Some(true)),
        Some(INTEGRATED_GPU_TEXTURE_CAP_DEFAULT)
    );
    assert_eq!(oc::effective_max_texture_size(&app, Some(false)), None);
    assert_eq!(oc::effective_max_texture_size(&app, None), None);
}

const GIB: i64 = 1024 * 1024 * 1024;

#[test]
fn cache_entry_limits_derive_from_the_machine() {
    // The four keys a Preferences recommendation derives, in the Python test's order.
    let keys = [
        "preview_cache_max_bytes",
        "preview_cache_max_full_res_entries",
        "preview_cache_max_entries",
        "render_memo_max_entries",
    ];
    let limits = |machine| {
        keys.iter()
            .map(|key| {
                oc::cache_entry_limit(
                    key,
                    machine,
                    if *key == "preview_cache_max_bytes" {
                        1024 * 1024
                    } else {
                        1
                    },
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        limits(Machine {
            total_ram_bytes: Some(4 * GIB),
            frame_px: 8_000_000,
            preview_px: 1600
        }),
        vec![Some(2048), Some(9), Some(69), Some(52)]
    );
    assert_eq!(
        limits(Machine {
            total_ram_bytes: Some(512 * GIB),
            frame_px: 400_000_000,
            preview_px: 8192
        }),
        vec![Some(262144), Some(24), Some(341), Some(256)]
    );
}

#[test]
fn an_unknown_ram_yields_no_limit() {
    let machine = Machine {
        total_ram_bytes: None,
        frame_px: 61_005_568,
        preview_px: 1600,
    };
    for key in oc::STORED_PERF_KEYS {
        assert_eq!(oc::cache_entry_limit(key, machine, 1), None, "key {key}");
    }
    let four = oc::cache_entry_limit(
        "preview_cache_max_full_res_entries",
        Machine {
            total_ram_bytes: Some(128 * GIB),
            frame_px: 0,
            preview_px: 1600,
        },
        1,
    );
    assert_eq!(four, None);
    let none = oc::cache_entry_limit(
        "preview_render_size",
        Machine {
            total_ram_bytes: Some(128 * GIB),
            frame_px: 61_005_568,
            preview_px: 1600,
        },
        1,
    );
    assert_eq!(none, None);
}

#[test]
fn a_zero_preview_size_does_not_divide_by_zero() {
    let machine = Machine {
        total_ram_bytes: Some(8 * GIB),
        frame_px: 0,
        preview_px: 0,
    };
    assert!(oc::cache_entry_limit("render_memo_max_entries", machine, 1).unwrap() > 0);
}

#[test]
fn a_saved_value_fills_a_gap_and_the_toml_wins_over_it() {
    let mut app = sample_app();
    let get = |key: &str| match key {
        "preview_render_size" => Some(2048),
        "render_memo_max_entries" => Some(12),
        _ => None,
    };
    oc::apply_stored(&OverrideConfig::with_defaults(), &mut app, get);
    assert_eq!(app.preview_render_size, 2048);
    assert_eq!(app.render_memo_max_entries, 12);

    let cfg = OverrideConfig {
        preview_render_size: Some(1024),
        ..OverrideConfig::with_defaults()
    };
    oc::apply(&cfg, &mut app);
    oc::apply_stored(&cfg, &mut app, |_| Some(4096));
    assert_eq!(app.preview_render_size, 1024);
}

#[test]
fn a_zero_texture_cap_means_the_hardware_decides() {
    let mut app = sample_app();
    app.max_texture_size = Some(4096);
    oc::apply_stored(&OverrideConfig::with_defaults(), &mut app, |key| {
        (key == "max_texture_size").then_some(0)
    });
    assert_eq!(app.max_texture_size, None);
}

#[test]
fn pinned_keys_name_only_what_the_file_set() {
    assert!(oc::toml_pinned_keys(&OverrideConfig::with_defaults()).is_empty());
    let cfg = OverrideConfig {
        preview_render_size: Some(1024),
        max_texture_size: Some(2048),
        ..OverrideConfig::with_defaults()
    };
    let pinned = oc::toml_pinned_keys(&cfg);
    assert_eq!(pinned.len(), 2);
    assert!(pinned.contains("preview_render_size"));
    assert!(pinned.contains("max_texture_size"));
}
