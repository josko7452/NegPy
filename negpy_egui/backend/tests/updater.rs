//! Ports the asset, release-payload and script cases in `tests/test_updater.py`.

use std::path::Path;

use serde_json::{Map, Value, json};

use negpy_backend::kernel::system::updater::{
    APP_BUNDLE, APPIMAGE, NSIS, UpdateInfo, app_bundle_path, appimage_script, download_asset,
    install_kind, macos_script, nsis_script, select_asset, update_from_payload,
};

fn asset(name: &str) -> Value {
    json!({
        "name": name,
        "browser_download_url": format!("https://example.test/{name}"),
        "size": 10,
    })
}

fn assets() -> Vec<Value> {
    [
        "NegPy-0.9.5-x86_64.AppImage",
        "NegPy-0.9.5-Win64-Setup.exe",
        "NegPy-0.9.5-macOS-arm64.dmg",
        "NegPy-0.9.5-macOS-x86_64.dmg",
    ]
    .into_iter()
    .map(asset)
    .collect()
}

fn release(tag: &str, assets: Vec<Value>) -> Map<String, Value> {
    Map::from_iter([
        ("tag_name".to_string(), json!(tag)),
        ("body".to_string(), json!("## Notes")),
        (
            "html_url".to_string(),
            json!("https://example.test/release"),
        ),
        ("assets".to_string(), Value::Array(assets)),
    ])
}

#[test]
fn each_install_kind_picks_its_own_asset() {
    let assets = assets();
    let appimage = select_asset(&assets, APPIMAGE, "x86_64").expect("appimage");
    assert!(appimage["name"].as_str().unwrap().ends_with(".AppImage"));
    let nsis = select_asset(&assets, NSIS, "x86_64").expect("nsis");
    assert!(nsis["name"].as_str().unwrap().ends_with("-Setup.exe"));
}

#[test]
fn the_mac_build_follows_the_machine_architecture() {
    for (machine, expected) in [
        ("arm64", "arm64"),
        ("aarch64", "arm64"),
        ("x86_64", "x86_64"),
    ] {
        let selected = select_asset(&assets(), APP_BUNDLE, machine).expect("dmg");
        assert_eq!(
            selected["name"].as_str().unwrap(),
            format!("NegPy-0.9.5-macOS-{expected}.dmg")
        );
    }
}

#[test]
fn a_single_dmg_release_serves_any_mac() {
    let only = vec![asset("NegPy-0.9.5-macOS-x86_64.dmg")];
    assert!(select_asset(&only, APP_BUNDLE, "arm64").is_some());
}

#[test]
fn no_asset_for_an_install_kind_that_cannot_update_itself() {
    assert!(select_asset(&assets(), "", "x86_64").is_none());
    assert!(select_asset(&[asset("source.zip")], APPIMAGE, "x86_64").is_none());
}

#[test]
fn a_newer_release_reports_the_matching_asset() {
    let info = update_from_payload(&release("v0.9.5", assets()), "0.9.0", APPIMAGE, "x86_64")
        .expect("update");
    assert_eq!(info.version, "0.9.5");
    assert_eq!(info.asset_name, "NegPy-0.9.5-x86_64.AppImage");
    assert!(info.can_self_install(APPIMAGE));
}

#[test]
fn the_current_release_is_no_update() {
    assert!(
        update_from_payload(&release("v0.9.5", assets()), "0.9.5", APPIMAGE, "x86_64").is_none()
    );
}

#[test]
fn an_unknown_version_never_offers_an_update() {
    assert!(
        update_from_payload(&release("v0.9.5", assets()), "unknown", APPIMAGE, "x86_64").is_none()
    );
}

#[test]
fn a_release_without_a_build_still_points_at_the_page() {
    let info = update_from_payload(
        &release("v0.9.5", vec![asset("source.zip")]),
        "0.9.0",
        APPIMAGE,
        "x86_64",
    )
    .expect("release");
    assert!(!info.can_self_install(APPIMAGE));
    assert_eq!(info.page_url, "https://example.test/release");
}

#[test]
fn a_source_checkout_has_no_install_kind() {
    assert_eq!(
        install_kind(false, "linux", Some("/tmp/NegPy.AppImage"), None),
        ""
    );
    assert_eq!(
        install_kind(true, "linux", Some("/tmp/NegPy.AppImage"), None),
        APPIMAGE
    );
    assert_eq!(install_kind(true, "win32", None, None), NSIS);
    assert_eq!(
        install_kind(
            true,
            "darwin",
            None,
            Some(Path::new("/Applications/NegPy.app"))
        ),
        APP_BUNDLE
    );
}

#[test]
fn an_app_bundle_is_found_from_its_executable() {
    let bundle = app_bundle_path(Path::new("/Applications/NegPy.app/Contents/MacOS/NegPy"));
    assert_eq!(
        bundle.as_deref(),
        Some(Path::new("/Applications/NegPy.app"))
    );
    assert!(app_bundle_path(Path::new("/usr/local/bin/negpy")).is_none());
}

#[test]
fn the_appimage_script_waits_then_replaces_and_relaunches() {
    let script = appimage_script(
        Path::new("/tmp/new.AppImage"),
        Path::new("/home/u/NegPy.AppImage"),
        4242,
    );
    assert!(script.contains("kill -0 4242"));
    assert!(script.contains("mv -f /tmp/new.AppImage /home/u/NegPy.AppImage"));
    assert!(script.trim_end().ends_with("exec /home/u/NegPy.AppImage"));
}

#[test]
fn the_posix_scripts_wait_on_the_pipe_not_only_the_pid() {
    let script = appimage_script(Path::new("/tmp/new"), Path::new("/tmp/old"), 1);
    assert!(script.find("cat >/dev/null").unwrap() < script.find("kill -0 1").unwrap());
    assert!(script.contains("waited -lt 20"));
}

#[test]
fn the_windows_script_does_not_wait_on_a_stuck_pid_forever() {
    assert!(
        nsis_script(Path::new("s.exe"), Path::new("d"), Path::new("e.exe"), 1)
            .contains("Wait-Process -Id 1 -Timeout 240")
    );
}

#[test]
fn the_macos_script_mounts_swaps_and_reopens() {
    let script = macos_script(
        Path::new("/tmp/n.dmg"),
        Path::new("/Applications/NegPy.app"),
        7,
        Path::new("/tmp/mnt"),
        Path::new("/tmp/NegPy.app"),
    );
    assert!(script.contains("kill -0 7"));
    assert!(script.contains("hdiutil attach /tmp/n.dmg"));
    assert!(script.contains("ditto /tmp/mnt/NegPy.app /tmp/NegPy.app"));
    assert!(script.contains("rm -rf /Applications/NegPy.app"));
    assert!(script.contains("open /Applications/NegPy.app"));
}

#[test]
fn the_windows_script_installs_silently_over_the_current_install() {
    let script = nsis_script(
        Path::new(r"C:\Temp\Setup.exe"),
        Path::new(r"C:\Program Files\NegPy"),
        Path::new(r"C:\Program Files\NegPy\NegPy.exe"),
        9,
    );
    assert!(script.contains("Wait-Process -Id 9"));
    assert!(script.contains(r#"start "" /wait "C:\Temp\Setup.exe" /S /D=C:\Program Files\NegPy"#));
    assert!(script.contains(r#"explorer.exe "C:\Program Files\NegPy\NegPy.exe""#));
}

#[test]
fn the_windows_script_switches_cmd_to_utf8_before_the_paths() {
    let script = nsis_script(
        Path::new("C:\\Users\\José\\Temp\\Setup.exe"),
        Path::new(r"C:\Program Files\NegPy"),
        Path::new(r"C:\Program Files\NegPy\NegPy.exe"),
        3,
    );
    assert!(script.contains("chcp 65001 >nul"));
    assert!(script.find("chcp 65001").unwrap() < script.find("José").unwrap());
}

#[test]
fn a_release_without_a_download_is_refused() {
    let info = UpdateInfo {
        version: "0.9.5".to_string(),
        notes: String::new(),
        page_url: "https://example.test/release".to_string(),
        asset_name: String::new(),
        download_url: String::new(),
        size: 0,
    };
    let dir = tempfile::TempDir::new().unwrap();
    assert!(download_asset(&info, dir.path(), None, None).is_err());
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}
