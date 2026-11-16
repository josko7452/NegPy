//! In-place self update.
//!
//! Mirrors `negpy/kernel/system/updater.py`. A detached script performs the swap after
//! NegPy exits; the running process only downloads and stages it.

use std::path::{Path, PathBuf};

use crate::kernel::system::version::{
    RELEASES_PAGE, USER_AGENT, app_version, fetch_latest_release, is_newer,
};
use serde_json::{Map, Value};

pub const APPIMAGE: &str = "appimage";
pub const NSIS: &str = "nsis";
pub const APP_BUNDLE: &str = "app_bundle";
const CHUNK: usize = 256 * 1024;

/// A self update that cannot proceed, with a message fit for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateError(pub String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UpdateError {}

/// A newer release and the asset that suits this install, if one exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub size: u64,
}

impl UpdateInfo {
    /// Whether this install has a download it can stage.
    #[must_use]
    pub fn can_self_install(&self, kind: &str) -> bool {
        !self.download_url.is_empty() && !kind.is_empty()
    }
}

/// Which installer produced the running copy, or an empty string when it cannot update.
#[must_use]
pub fn install_kind(
    frozen: bool,
    platform: &str,
    appimage: Option<&str>,
    bundle: Option<&Path>,
) -> &'static str {
    if !frozen {
        return "";
    }
    if platform == "win32" {
        return NSIS;
    }
    if platform == "darwin" {
        return if bundle.is_some() { APP_BUNDLE } else { "" };
    }
    if platform.starts_with("linux") {
        return if appimage.is_some() { APPIMAGE } else { "" };
    }
    ""
}

/// The `.app` a running executable lives in, or `None` outside a bundle.
#[must_use]
pub fn app_bundle_path(executable: &Path) -> Option<PathBuf> {
    let executable = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf());
    executable
        .ancestors()
        .find(|parent| {
            parent
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

/// The machine architecture a macOS asset names.
#[must_use]
pub fn macos_arch(machine: &str) -> &'static str {
    if matches!(machine, "arm64" | "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// The release asset matching `kind`.
#[must_use]
pub fn select_asset(assets: &[Value], kind: &str, machine: &str) -> Option<Map<String, Value>> {
    let named: Vec<&Map<String, Value>> = assets
        .iter()
        .filter_map(Value::as_object)
        .filter(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| !name.is_empty())
                && asset
                    .get("browser_download_url")
                    .and_then(Value::as_str)
                    .is_some_and(|url| !url.is_empty())
        })
        .collect();
    let named_end = |suffix: &str| {
        named.iter().copied().find(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.ends_with(suffix))
        })
    };
    if kind == APPIMAGE {
        return named_end(".AppImage").cloned();
    }
    if kind == NSIS {
        return named_end("-Setup.exe").cloned();
    }
    if kind == APP_BUNDLE {
        let dmgs: Vec<&Map<String, Value>> = named
            .iter()
            .copied()
            .filter(|asset| {
                asset
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name.ends_with(".dmg"))
            })
            .collect();
        let suffix = format!("-{}.dmg", macos_arch(machine));
        if let Some(exact) = dmgs.iter().find(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.ends_with(&suffix))
        }) {
            return Some((*exact).clone());
        }
        if dmgs.len() == 1 {
            return Some(dmgs[0].clone());
        }
    }
    None
}

/// The newest release when it is later than the running one.
#[must_use]
pub fn update_from_payload(
    payload: &Map<String, Value>,
    current: &str,
    kind: &str,
    machine: &str,
) -> Option<UpdateInfo> {
    if current == "unknown" {
        return None;
    }
    let latest = payload
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    if !is_newer(&latest, current) {
        return None;
    }
    let assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let asset = select_asset(&assets, kind, machine);
    Some(UpdateInfo {
        version: latest,
        notes: payload
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        page_url: payload
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or(RELEASES_PAGE)
            .to_string(),
        asset_name: asset
            .as_ref()
            .and_then(|asset| asset.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        download_url: asset
            .as_ref()
            .and_then(|asset| asset.get("browser_download_url"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        size: asset
            .as_ref()
            .and_then(|asset| asset.get("size"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

/// Find the newest release and the asset that suits the current install.
#[must_use]
pub fn find_update(
    timeout_seconds: u64,
    current: &str,
    kind: &str,
    machine: &str,
) -> Option<UpdateInfo> {
    if current == "unknown" {
        return None;
    }
    let payload = fetch_latest_release(timeout_seconds)?;
    update_from_payload(&payload, current, kind, machine)
}

/// The running app's update check.
#[must_use]
pub fn check_for_update(timeout_seconds: u64) -> Option<UpdateInfo> {
    let frozen = std::env::var_os("NEGPY_FROZEN").is_some();
    let appimage = std::env::var_os("APPIMAGE").map(|value| value.to_string_lossy().to_string());
    let executable = std::env::current_exe().unwrap_or_default();
    let kind = install_kind(
        frozen,
        std::env::consts::OS,
        appimage.as_deref(),
        app_bundle_path(&executable).as_deref(),
    );
    let machine = std::env::consts::ARCH;
    find_update(timeout_seconds, &app_version(), kind, machine)
}

/// Where a downloaded asset lands. An AppImage is staged beside its target.
#[must_use]
pub fn staging_dir(appimage: Option<&Path>) -> PathBuf {
    if let Some(target) = appimage
        && target.parent().is_some_and(writable)
    {
        return target.parent().unwrap_or(Path::new(".")).to_path_buf();
    }
    std::env::temp_dir().join(format!("negpy-update-{}", std::process::id()))
}

fn writable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o222 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_dir()
    }
}

/// Stream an asset into `dest_dir`, reporting bytes and total.
pub fn download_asset(
    info: &UpdateInfo,
    dest_dir: &Path,
    mut on_progress: Option<&mut dyn FnMut(u64, u64)>,
    is_cancelled: Option<&dyn Fn() -> bool>,
) -> Result<PathBuf, UpdateError> {
    if info.download_url.is_empty() {
        return Err(UpdateError(
            "This release has no download for your platform.".to_string(),
        ));
    }
    std::fs::create_dir_all(dest_dir)
        .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
    let target = dest_dir.join(format!("{}.part", info.asset_name));
    let result = download_to(
        &info.download_url,
        &target,
        info.size,
        &mut on_progress,
        is_cancelled,
    );
    if let Err(error) = result {
        let _ = std::fs::remove_file(&target);
        return Err(error);
    }
    let final_path = dest_dir.join(&info.asset_name);
    std::fs::rename(&target, &final_path)
        .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
    Ok(final_path)
}

fn download_to(
    url: &str,
    target: &Path,
    declared_size: u64,
    on_progress: &mut Option<&mut dyn FnMut(u64, u64)>,
    is_cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), UpdateError> {
    use std::io::{Read, Write};
    let mut response = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .new_agent()
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
    let total = response
        .headers()
        .get("Content-Length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(declared_size);
    let mut out = std::fs::File::create(target)
        .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
    let mut reader = response.body_mut().as_reader();
    let mut done = 0u64;
    loop {
        if is_cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(UpdateError("Download cancelled.".to_string()));
        }
        let mut chunk = vec![0u8; CHUNK];
        let read = reader
            .read(&mut chunk)
            .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
        if read == 0 {
            break;
        }
        out.write_all(&chunk[..read])
            .map_err(|error| UpdateError(format!("Download failed: {error}")))?;
        done += read as u64;
        if let Some(callback) = on_progress {
            callback(done, total);
        }
    }
    Ok(())
}

/// A script that starts once NegPy is gone. The stdin read is the death signal;
/// the PID poll that follows is a bounded grace period for a zombie.
#[must_use]
pub fn wait_and_run_sh(pid: u32, body: &str) -> String {
    format!(
        "#!/bin/sh\ncat >/dev/null 2>&1\nwaited=0\nwhile kill -0 {pid} 2>/dev/null && [ $waited -lt 20 ]; do\n    sleep 0.5\n    waited=$((waited + 1))\ndone\n{body}"
    )
}

fn quote(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-".contains(ch))
    {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', "'\"'\"'"))
    }
}

/// Replace the running AppImage with the downloaded one and start it.
#[must_use]
pub fn appimage_script(downloaded: &Path, target: &Path, pid: u32) -> String {
    let new = quote(downloaded);
    let old = quote(target);
    wait_and_run_sh(
        pid,
        &format!("set -e\nchmod +x {new}\nmv -f {new} {old}\nrm -f \"$0\"\nexec {old}\n"),
    )
}

/// Mount the DMG, swap the bundle, and reopen the app.
#[must_use]
pub fn macos_script(
    dmg: &Path,
    bundle: &Path,
    pid: u32,
    mountpoint: &Path,
    stage: &Path,
) -> String {
    let inner = mountpoint.join(bundle.file_name().unwrap_or_default());
    wait_and_run_sh(
        pid,
        &format!(
            "set -e\nmkdir -p {}\nhdiutil attach {} -nobrowse -readonly -mountpoint {}\nrm -rf {}\nditto {} {}\nhdiutil detach {} -quiet || true\nrm -rf {}\nmv {} {}\nxattr -dr com.apple.quarantine {} 2>/dev/null || true\nrm -f {} \"$0\"\nopen {}\n",
            quote(mountpoint),
            quote(dmg),
            quote(mountpoint),
            quote(stage),
            quote(&inner),
            quote(stage),
            quote(mountpoint),
            quote(bundle),
            quote(stage),
            quote(bundle),
            quote(bundle),
            quote(dmg),
            quote(bundle)
        ),
    )
}

/// Run the NSIS installer silently over the current install, then restart.
#[must_use]
pub fn nsis_script(setup: &Path, install_dir: &Path, exe: &Path, pid: u32) -> String {
    format!(
        "@echo off\r\nchcp 65001 >nul\r\npowershell -NoProfile -Command \"try {{ Wait-Process -Id {pid} -Timeout 240 }} catch {{ }}\"\r\nstart \"\" /wait \"{}\" /S /D={}\r\ndel /q \"{}\"\r\nexplorer.exe \"{}\"\r\ndel \"%~f0\"\r\n",
        setup.display(),
        install_dir.display(),
        setup.display(),
        exe.display()
    )
}

/// Start the swap and return; the caller must then exit.
pub fn apply_update(
    downloaded: &Path,
    info: &UpdateInfo,
    kind: &str,
    target: &Path,
    pid: u32,
) -> Result<(), UpdateError> {
    match kind {
        APPIMAGE => {
            let parent = target.parent().unwrap_or(Path::new("."));
            if !writable(parent) {
                return Err(UpdateError(format!(
                    "{} is not writable. Move NegPy somewhere you own, or install the new AppImage yourself.",
                    parent.display()
                )));
            }
            let script = appimage_script(downloaded, target, pid);
            spawn_posix(&script, downloaded.parent().unwrap_or(Path::new(".")))
                .map_err(|error| UpdateError(format!("Could not start the update: {error}")))?;
        }
        APP_BUNDLE => {
            let parent = target.parent().unwrap_or(Path::new("."));
            if !writable(parent) {
                return Err(UpdateError(format!(
                    "{} is not writable. Install the update from the disk image yourself.",
                    parent.display()
                )));
            }
            let work = downloaded.parent().unwrap_or(Path::new("."));
            let script = macos_script(
                downloaded,
                target,
                pid,
                &work.join("mnt"),
                &work.join(target.file_name().unwrap_or_default()),
            );
            spawn_posix(&script, work)
                .map_err(|error| UpdateError(format!("Could not start the update: {error}")))?;
        }
        NSIS => {
            let script = nsis_script(
                downloaded,
                target.parent().unwrap_or(Path::new(".")),
                target,
                pid,
            );
            spawn_windows(&script, downloaded.parent().unwrap_or(Path::new(".")))?;
        }
        _ => {
            return Err(UpdateError(
                "This copy of NegPy cannot update itself.".to_string(),
            ));
        }
    }
    log::info!(
        "Update to {} staged from {}",
        info.version,
        downloaded.display()
    );
    Ok(())
}

fn spawn_posix(script: &str, workdir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let path = workdir.join("negpy-update.sh");
    std::fs::write(&path, script)?;
    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    std::fs::set_permissions(&path, permissions)?;
    spawn_detached("/bin/sh", &path)
}

fn spawn_windows(script: &str, workdir: &Path) -> Result<(), UpdateError> {
    let path = workdir.join("negpy-update.bat");
    std::fs::write(&path, script)
        .map_err(|error| UpdateError(format!("Could not stage the installer: {error}")))?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let status = std::process::Command::new("cmd")
            .args(["/C", &path.display().to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| {
                UpdateError(format!("Windows refused to start the installer: {error}"))
            })?;
        drop(status);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (script, workdir);
        Err(UpdateError(
            "This copy of NegPy cannot update itself.".to_string(),
        ))
    }
    #[cfg(target_os = "windows")]
    Ok(())
}

fn spawn_detached(shell: &str, script: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    std::process::Command::new(shell)
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_child| ())
}
