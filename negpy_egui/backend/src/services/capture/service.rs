//! One R/G/B triplet from an injected light and camera.
//!
//! Mirrors `negpy/services/capture/service.py`. Every capture is staged beside the
//! destination, validated, and only then promoted, so a misfire cannot replace a good
//! channel of an earlier frame.

use std::path::{Path, PathBuf};

use crate::services::capture::blend::capture_validated_single;
use crate::services::capture::blend::{CAPTURE_ORDER, CaptureError, Channel, verify_raw_size};
use crate::services::capture::hardware::{
    BulbCamera, BulbRelease, Camera, CaptureResult, CaptureSettings, LightSource, ShutterRelease,
};

pub type ProgressCb<'a> = &'a mut dyn FnMut(f64);
pub type ChannelCb<'a> = &'a mut dyn FnMut(&str);

/// The raw suffix the camera writes.
pub const RAW_SUFFIX: &str = ".raw";

/// Dark time inside the Bulb exposure before the first LED, so the shutter's lag after the
/// press never shortens the first channel.
pub const BULB_LEAD_S: f64 = 0.35;
/// Dark time after the last LED, by cable only: the light's timer ends the pulse, so the
/// host's latency must fit inside it.
pub const BULB_TAIL_S: f64 = 0.2;
/// The firmware counts the pulse in 10 ms units, so a 3.5 mm jack holds at most 2.55 s.
pub const SHUTTER_PULSE_UNIT_S: f64 = 0.01;
pub const MAX_SHUTTER_PULSE_S: f64 = 255.0 * SHUTTER_PULSE_UNIT_S;

/// One camera exposure with no light control, for white-light scanning without the
/// Scanlight. One file, no R/G/B split.
pub fn capture_single(
    camera: &mut dyn Camera,
    roll_name: &str,
    frame_number: u32,
    output_folder: &Path,
    shutter: Option<&str>,
    min_raw_bytes: u64,
    max_raw_bytes: u64,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<PathBuf, CaptureError> {
    std::fs::create_dir_all(output_folder)
        .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
    let stem = output_folder.join(format!("{roll_name}_Frame{frame_number:03}"));
    let result = capture_validated_single(
        &stem.with_extension("raw"),
        |staged| {
            let path = camera
                .capture(staged, shutter, None, None)
                .map_err(CaptureError)?;
            if cancel.is_some_and(|cancel| cancel()) {
                return Err(CaptureError("capture cancelled".to_string()));
            }
            Ok(path)
        },
        min_raw_bytes,
        max_raw_bytes,
    );
    result
}

/// Promote a frame's three channel files as one recoverable operation.
///
/// Existing files are backed up inside the staging folder, so an I/O failure during
/// promotion rolls every channel back to the prior complete triplet.
pub fn promote_triplet(
    staged: &[(Channel, PathBuf)],
    output_folder: &Path,
) -> Result<Vec<(Channel, PathBuf)>, CaptureError> {
    let mut final_paths = Vec::with_capacity(staged.len());
    for (channel, path) in staged {
        let name = path
            .file_name()
            .ok_or_else(|| CaptureError("a staged capture has no file name".to_string()))?;
        final_paths.push((*channel, output_folder.join(name)));
    }
    let staging_dir = staged
        .first()
        .and_then(|(_, path)| path.parent())
        .ok_or_else(|| CaptureError("the staged captures have no folder".to_string()))?;
    let mut backups: Vec<(Channel, PathBuf)> = Vec::new();
    let mut promoted: Vec<(Channel, PathBuf)> = Vec::new();
    let rollback = |promoted: &[(Channel, PathBuf)], backups: &[(Channel, PathBuf)]| {
        for (channel, final_path) in promoted.iter().rev() {
            match backups.iter().find(|(backup, _)| backup == channel) {
                Some((_, backup_path)) if backup_path.exists() => {
                    let _ = std::fs::rename(backup_path, final_path);
                }
                _ => {
                    let _ = std::fs::remove_file(final_path);
                }
            }
        }
    };
    for (channel, final_path) in &final_paths {
        if final_path.exists() {
            let name = final_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let backup = staging_dir.join(format!(".previous-{name}"));
            if std::fs::hard_link(final_path, &backup).is_err()
                && std::fs::copy(final_path, &backup).is_err()
            {
                return Err(CaptureError(format!(
                    "could not back up the prior {}",
                    channel.letter()
                )));
            }
            backups.push((*channel, backup));
        }
    }
    for ((channel, staged_path), (_, final_path)) in staged.iter().zip(&final_paths) {
        if let Err(error) = std::fs::rename(staged_path, final_path) {
            rollback(&promoted, &backups);
            return Err(CaptureError(format!(
                "could not promote completed triplet: {error}"
            )));
        }
        promoted.push((*channel, final_path.clone()));
    }
    Ok(final_paths)
}

/// The capture service. It owns neither the light nor the camera.
pub struct CaptureService<'a> {
    light: &'a mut dyn LightSource,
    camera: &'a mut dyn Camera,
    sleep: Box<dyn FnMut(f64) + 'a>,
}

impl<'a> CaptureService<'a> {
    #[must_use]
    pub fn new(
        light: &'a mut dyn LightSource,
        camera: &'a mut dyn Camera,
        sleep: impl FnMut(f64) + 'a,
    ) -> Self {
        Self {
            light,
            camera,
            sleep: Box::new(sleep),
        }
    }

    /// Live light control for the RGB sliders, with no capture.
    pub fn set_light(&mut self, r: u32, g: u32, b: u32) {
        self.light.set_color(r, g, b, 0, 0);
    }

    /// Capture red, green and blue exposures of the current frame.
    ///
    /// Each channel lights only its LED, settles, captures and size-checks. The light
    /// ends at `end_light` after a success, else off.
    pub fn capture_triplet(
        &mut self,
        settings: &CaptureSettings,
        progress: Option<ProgressCb<'_>>,
        cancel: Option<&dyn Fn() -> bool>,
        on_channel: Option<ChannelCb<'_>>,
        end_light: Option<[u32; 4]>,
    ) -> Result<CaptureResult, CaptureError> {
        std::fs::create_dir_all(&settings.output_folder)
            .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
        let frame_name = settings.frame_name();
        let staging_dir = settings
            .output_folder
            .join(format!(".{frame_name}-capture"));
        let _ = std::fs::remove_dir_all(&staging_dir);
        std::fs::create_dir_all(&staging_dir)
            .map_err(|error| CaptureError(format!("cannot create the staging folder: {error}")))?;
        let mut staged: Vec<(Channel, PathBuf)> = Vec::new();
        let mut ok = false;
        let mut progress = progress;
        let mut on_channel = on_channel;
        let result = (|| -> Result<CaptureResult, CaptureError> {
            for (index, channel) in CAPTURE_ORDER.into_iter().enumerate() {
                if cancel.is_some_and(|cancel| cancel()) {
                    return Err(CaptureError("capture cancelled".to_string()));
                }
                if let Some(callback) = on_channel.as_mut() {
                    callback(channel.letter());
                }
                let rgb = channel
                    .rgb(settings.levels[index])
                    .expect("the capture order is RGB");
                self.light.set_color(rgb[0], rgb[1], rgb[2], 0, 0);
                (self.sleep)(settings.settle_s);
                let stem = staging_dir.join(format!("{frame_name}_{}", channel.letter()));
                let shutter = settings
                    .shutters
                    .as_ref()
                    .and_then(|shutters| shutters[index].as_deref());
                let path = self
                    .camera
                    .capture(
                        &stem.with_extension("raw"),
                        shutter,
                        if index == 0 {
                            settings.iso.as_deref()
                        } else {
                            None
                        },
                        if index == 0 {
                            settings.aperture.as_deref()
                        } else {
                            None
                        },
                    )
                    .map_err(CaptureError)?;
                verify_raw_size(&path, settings.min_raw_bytes, settings.max_raw_bytes)?;
                staged.push((channel, path));
                if let Some(callback) = progress.as_mut() {
                    callback((index + 1) as f64 / CAPTURE_ORDER.len() as f64);
                }
                if index + 1 < CAPTURE_ORDER.len() {
                    (self.sleep)(settings.inter_exposure_delay_s);
                }
            }
            if cancel.is_some_and(|cancel| cancel()) {
                return Err(CaptureError("capture cancelled".to_string()));
            }
            let promoted = promote_triplet(&staged, &settings.output_folder)?;
            ok = true;
            let path_of = |channel: Channel| {
                promoted
                    .iter()
                    .find(|(found, _)| *found == channel)
                    .map(|(_, path)| path.clone())
                    .unwrap_or_default()
            };
            Ok(CaptureResult {
                frame_number: settings.frame_number,
                red_path: path_of(Channel::Red),
                green_path: path_of(Channel::Green),
                blue_path: path_of(Channel::Blue),
            })
        })();
        self.release_light(ok, end_light);
        let _ = std::fs::remove_dir_all(&staging_dir);
        result
    }

    /// One exposure with R, G and B lit together, at the preset's levels.
    pub fn capture_rgb_single(
        &mut self,
        settings: &CaptureSettings,
        cancel: Option<&dyn Fn() -> bool>,
        end_light: Option<[u32; 4]>,
    ) -> Result<PathBuf, CaptureError> {
        std::fs::create_dir_all(&settings.output_folder)
            .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
        self.light.set_color(
            settings.levels[0],
            settings.levels[1],
            settings.levels[2],
            0,
            0,
        );
        (self.sleep)(settings.settle_s);
        let stem = settings.output_folder.join(settings.frame_name());
        let shutter = settings
            .shutters
            .as_ref()
            .and_then(|shutters| shutters[0].as_deref());
        let result = crate::services::capture::blend::capture_validated_single(
            &stem.with_extension("raw"),
            |staged| {
                self.camera
                    .capture(
                        staged,
                        shutter,
                        settings.iso.as_deref(),
                        settings.aperture.as_deref(),
                    )
                    .map_err(CaptureError)
            },
            settings.min_raw_bytes,
            settings.max_raw_bytes,
        );
        let _ = &cancel;
        self.release_light(result.is_ok(), end_light);
        result
    }

    /// One exposure under one LED, written as the frame's `_<letter>` file.
    pub fn capture_channel(
        &mut self,
        roll_name: &str,
        frame_number: u32,
        output_folder: &Path,
        channel: Channel,
        level: u32,
        shutter: Option<&str>,
        iso: Option<&str>,
        aperture: Option<&str>,
        settle_s: f64,
        min_raw_bytes: u64,
        max_raw_bytes: u64,
        end_light: Option<[u32; 4]>,
    ) -> Result<PathBuf, CaptureError> {
        std::fs::create_dir_all(output_folder)
            .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
        match channel.rgb(level) {
            Some([r, g, b]) => self.light.set_color(r, g, b, 0, 0),
            None => self.light.set_color(0, 0, 0, 0, level),
        }
        (self.sleep)(settle_s);
        let stem = output_folder.join(format!(
            "{roll_name}_Frame{frame_number:03}_{}",
            channel.letter()
        ));
        let result = crate::services::capture::blend::capture_validated_single(
            &stem.with_extension("raw"),
            |staged| {
                self.camera
                    .capture(staged, shutter, iso, aperture)
                    .map_err(CaptureError)
            },
            min_raw_bytes,
            max_raw_bytes,
        );
        self.release_light(result.is_ok(), end_light);
        result
    }

    /// Single white-light exposure for slide or E-6 film.
    pub fn capture_white(
        &mut self,
        roll_name: &str,
        frame_number: u32,
        output_folder: &Path,
        w_level: u32,
        shutter: Option<&str>,
        settle_s: f64,
        min_raw_bytes: u64,
        max_raw_bytes: u64,
        end_light: Option<[u32; 4]>,
    ) -> Result<PathBuf, CaptureError> {
        std::fs::create_dir_all(output_folder)
            .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
        self.light.set_color(0, 0, 0, w_level, 0);
        (self.sleep)(settle_s);
        let stem = output_folder.join(format!("{roll_name}_Frame{frame_number:03}"));
        let result = crate::services::capture::blend::capture_validated_single(
            &stem.with_extension("raw"),
            |staged| {
                self.camera
                    .capture(staged, shutter, None, None)
                    .map_err(CaptureError)
            },
            min_raw_bytes,
            max_raw_bytes,
        );
        self.release_light(result.is_ok(), end_light);
        result
    }

    /// Leave the light at `end_light` after a success, with no off step between. A failed
    /// or cancelled capture turns it off.
    fn release_light(&mut self, ok: bool, end_light: Option<[u32; 4]>) {
        match end_light {
            Some([r, g, b, w]) if ok && (r != 0 || g != 0 || b != 0 || w != 0) => {
                self.light.set_color(r, g, b, w, 0);
            }
            _ => self.light.off(),
        }
    }

    /// One Bulb exposure with red, green and blue lit in turn.
    ///
    /// `keep_bulb` leaves the camera in Bulb for the next frame of a strip. `seconds` is
    /// the Trichrome shutter's true time; see `blend_plan`. The shutter opens and closes in
    /// the dark, and a cancel between channels ends the light and discards the file.
    #[allow(clippy::too_many_arguments)]
    pub fn capture_blend(
        &mut self,
        bulb: &mut dyn BulbCamera,
        settings: &CaptureSettings,
        seconds: f64,
        release: BulbRelease,
        cancel: Option<&dyn Fn() -> bool>,
        on_channel: Option<ChannelCb<'_>>,
        end_light: Option<[u32; 4]>,
        keep_bulb: bool,
        cable_release: Option<&mut dyn ShutterRelease>,
    ) -> Result<PathBuf, CaptureError> {
        let cable = release == BulbRelease::Cable;
        if cable && cable_release.is_none() {
            return Err(CaptureError(
                "this light has no shutter release output for a cable".to_string(),
            ));
        }
        let mut pulse = cable_release;
        let budget = cable.then(|| MAX_SHUTTER_PULSE_S - BULB_LEAD_S - BULB_TAIL_S);
        let plan = crate::services::capture::blend::blend_plan(settings.levels, seconds, budget)?;
        let light_s: f64 = plan.iter().map(|step| step.seconds).sum();
        std::fs::create_dir_all(&settings.output_folder)
            .map_err(|error| CaptureError(format!("cannot create the capture folder: {error}")))?;
        let stem = settings.output_folder.join(settings.frame_name());
        let mut on_channel = on_channel;
        let result = crate::services::capture::blend::capture_validated_single(
            &stem.with_extension("raw"),
            |staged| {
                let mut expose = || {
                    self.light.off();
                    if let Some(pulse) = pulse.as_deref_mut() {
                        pulse.shutter_pulse(BULB_LEAD_S + light_s + BULB_TAIL_S);
                    }
                    (self.sleep)(BULB_LEAD_S);
                    for step in &plan {
                        if cancel.is_some_and(|cancel| cancel()) {
                            break;
                        }
                        if let Some(callback) = on_channel.as_mut() {
                            callback(step.channel.letter());
                        }
                        match step.channel.rgb(step.level) {
                            Some([r, g, b]) => self.light.set_color_now(r, g, b, 0, 0),
                            None => self.light.set_color_now(0, 0, 0, 0, step.level),
                        }
                        (self.sleep)(step.seconds);
                    }
                    self.light.off();
                    if pulse.is_some() {
                        (self.sleep)(BULB_TAIL_S);
                    }
                };
                bulb.capture_bulb(
                    staged,
                    &mut expose,
                    !cable,
                    settings.iso.as_deref(),
                    settings.aperture.as_deref(),
                    keep_bulb,
                )
                .map_err(CaptureError)
            },
            settings.min_raw_bytes,
            settings.max_raw_bytes,
        );
        self.release_light(result.is_ok(), end_light);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::capture::hardware::CaptureSettings;

    struct FakeLight {
        calls: Vec<[u32; 5]>,
    }

    impl LightSource for FakeLight {
        fn set_color(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32) {
            self.calls.push([r, g, b, w, ir]);
        }
        fn off(&mut self) {
            self.calls.push([0, 0, 0, 0, 0]);
        }
        fn close(&mut self) {}
    }

    struct FakeCamera {
        bytes: u64,
        count: usize,
    }

    impl Camera for FakeCamera {
        fn capture(
            &mut self,
            out_path: &Path,
            _shutter: Option<&str>,
            _iso: Option<&str>,
            _aperture: Option<&str>,
        ) -> Result<PathBuf, String> {
            self.count += 1;
            std::fs::write(out_path, vec![0u8; self.bytes as usize]).map_err(|e| e.to_string())?;
            Ok(out_path.to_path_buf())
        }
        fn close(&mut self) {}
    }

    fn settings(folder: &Path) -> CaptureSettings {
        CaptureSettings {
            roll_name: "R1".to_string(),
            frame_number: 1,
            output_folder: folder.to_path_buf(),
            levels: [255, 128, 64],
            settle_s: 0.0,
            min_raw_bytes: 12,
            max_raw_bytes: 4096,
            ..CaptureSettings::default()
        }
    }

    #[test]
    fn a_plain_single_writes_one_file_and_never_touches_the_light() {
        let dir = tempfile::tempdir().unwrap();
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        let path = capture_single(&mut camera, "R1", 4, dir.path(), None, 12, 4096, None).unwrap();
        assert_eq!(camera.count, 1);
        assert_eq!(path.file_name().unwrap(), "R1_Frame004.raw");
        assert!(path.exists());
    }

    #[test]
    fn a_cancelled_single_never_reaches_its_final_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        let error = capture_single(
            &mut camera,
            "R1",
            5,
            dir.path(),
            None,
            12,
            4096,
            Some(&|| true),
        )
        .unwrap_err();
        assert!(error.0.contains("capture cancelled"));
        assert!(!dir.path().join("R1_Frame005.raw").exists());
    }

    #[test]
    fn a_triplet_lights_each_channel_and_promotes_three_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut light = FakeLight { calls: Vec::new() };
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        let result = {
            let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
            service
                .capture_triplet(&settings(dir.path()), None, None, None, None)
                .unwrap()
        };
        assert_eq!(camera.count, 3);
        assert!(
            result.red_path.exists() && result.green_path.exists() && result.blue_path.exists()
        );
        assert_eq!(result.red_path.file_name().unwrap(), "R1_Frame001_R.raw");
        assert_eq!(light.calls[0], [255, 0, 0, 0, 0]);
        assert_eq!(light.calls[1], [0, 128, 0, 0, 0]);
        assert_eq!(light.calls[2], [0, 0, 64, 0, 0]);
    }

    #[test]
    fn a_capture_that_fails_a_size_check_keeps_the_old_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("R1_Frame001_G.raw"), vec![0u8; 64]).unwrap();
        let mut light = FakeLight { calls: Vec::new() };
        let mut camera = FakeCamera { bytes: 4, count: 0 };
        let error = {
            let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
            service
                .capture_triplet(&settings(dir.path()), None, None, None, None)
                .unwrap_err()
        };
        assert!(error.0.contains("plausible RAW range"));
        assert!(dir.path().join("R1_Frame001_G.raw").exists());
    }

    #[test]
    fn a_cancel_before_the_second_channel_turns_the_light_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut light = FakeLight { calls: Vec::new() };
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        let result = {
            let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
            let cancelled = std::cell::Cell::new(false);
            let result = service.capture_triplet(
                &settings(dir.path()),
                None,
                Some(&|| cancelled.get()),
                Some(&mut |_letter| cancelled.set(true)),
                None,
            );
            result
        };
        assert!(result.is_err());
        assert_eq!(light.calls.last(), Some(&[0, 0, 0, 0, 0]));
    }

    #[test]
    fn a_channel_capture_writes_the_lettered_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut light = FakeLight { calls: Vec::new() };
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        let path = {
            let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
            service
                .capture_channel(
                    "R1",
                    2,
                    dir.path(),
                    Channel::Ir,
                    90,
                    None,
                    None,
                    None,
                    0.0,
                    12,
                    4096,
                    None,
                )
                .unwrap()
        };
        assert_eq!(path.file_name().unwrap(), "R1_Frame002_IR.raw");
        assert_eq!(light.calls[0], [0, 0, 0, 0, 90]);
    }

    #[test]
    fn a_success_leaves_the_framing_light_on() {
        let dir = tempfile::tempdir().unwrap();
        let mut light = FakeLight { calls: Vec::new() };
        let mut camera = FakeCamera {
            bytes: 64,
            count: 0,
        };
        {
            let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
            service
                .capture_white(
                    "R1",
                    3,
                    dir.path(),
                    200,
                    None,
                    0.0,
                    12,
                    4096,
                    Some([10, 20, 30, 40]),
                )
                .unwrap();
        }
        assert_eq!(light.calls.last(), Some(&[10, 20, 30, 40, 0]));
    }

    struct SharedBulb {
        log: std::sync::Arc<std::sync::Mutex<Vec<[u32; 5]>>>,
        bulbs: std::sync::Arc<std::sync::Mutex<Vec<(bool, Option<String>, Option<String>)>>>,
        bytes: u64,
    }

    impl Camera for SharedBulb {
        fn capture(
            &mut self,
            _out_path: &Path,
            _shutter: Option<&str>,
            _iso: Option<&str>,
            _aperture: Option<&str>,
        ) -> Result<PathBuf, String> {
            Err("this camera takes only Bulb exposures".to_string())
        }
        fn close(&mut self) {}
    }

    impl BulbCamera for SharedBulb {
        fn capture_bulb(
            &mut self,
            out_path: &Path,
            expose: &mut dyn FnMut(),
            usb_release: bool,
            iso: Option<&str>,
            aperture: Option<&str>,
            _keep_bulb: bool,
        ) -> Result<PathBuf, String> {
            expose();
            std::fs::write(out_path, vec![0u8; self.bytes as usize]).map_err(|e| e.to_string())?;
            self.bulbs.lock().unwrap().push((
                usb_release,
                iso.map(str::to_string),
                aperture.map(str::to_string),
            ));
            Ok(out_path.to_path_buf())
        }
    }

    struct SharedLight {
        log: std::sync::Arc<std::sync::Mutex<Vec<[u32; 5]>>>,
    }

    impl LightSource for SharedLight {
        fn set_color(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32) {
            self.log.lock().unwrap().push([r, g, b, w, ir]);
        }
        fn off(&mut self) {
            self.log.lock().unwrap().push([0, 0, 0, 0, 0]);
        }
        fn close(&mut self) {}
    }

    #[test]
    fn a_blend_lights_each_channel_alone_in_one_bulb_exposure() {
        let dir = tempfile::tempdir().unwrap();
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let bulbs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut light = SharedLight {
            log: std::sync::Arc::clone(&log),
        };
        let mut camera = SharedBulb {
            log: std::sync::Arc::clone(&log),
            bulbs: std::sync::Arc::clone(&bulbs),
            bytes: 64,
        };
        let mut waits = Vec::new();
        let mut settings = settings(dir.path());
        settings.levels = [200, 180, 255];
        settings.iso = Some("100".to_string());
        settings.aperture = Some("8".to_string());
        {
            let mut service = CaptureService::new(&mut light, &mut camera, |seconds| {
                waits.push(seconds);
            });
            service
                .capture_blend(
                    &mut SharedBulb {
                        log: std::sync::Arc::clone(&log),
                        bulbs: std::sync::Arc::clone(&bulbs),
                        bytes: 64,
                    },
                    &settings,
                    0.5,
                    BulbRelease::Usb,
                    None,
                    None,
                    None,
                    false,
                    None,
                )
                .unwrap();
        }
        let lit: Vec<[u32; 5]> = log.lock().unwrap().clone();
        assert_eq!(lit[0], [0, 0, 0, 0, 0]);
        assert_eq!(lit[1], [200, 0, 0, 0, 0]);
        assert_eq!(lit[2], [0, 180, 0, 0, 0]);
        assert_eq!(lit[3], [0, 0, 255, 0, 0]);
        assert_eq!(lit[4], [0, 0, 0, 0, 0]);
        assert_eq!(waits, vec![0.35, 0.5, 0.5, 0.5]);
        let bulbs = bulbs.lock().unwrap();
        assert!(bulbs[0].0, "USB release");
        assert_eq!(bulbs[0].1.as_deref(), Some("100"));
        assert_eq!(bulbs[0].2.as_deref(), Some("8"));
    }

    struct CableLight {
        log: std::sync::Arc<std::sync::Mutex<Vec<[u32; 5]>>>,
        pulses: std::sync::Arc<std::sync::Mutex<Vec<f64>>>,
    }

    impl LightSource for CableLight {
        fn set_color(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32) {
            self.log.lock().unwrap().push([r, g, b, w, ir]);
        }
        fn off(&mut self) {
            self.log.lock().unwrap().push([0, 0, 0, 0, 0]);
        }
        fn close(&mut self) {}
    }

    impl ShutterRelease for CableLight {
        fn shutter_pulse(&mut self, seconds: f64) {
            self.pulses.lock().unwrap().push(seconds);
        }
    }

    #[test]
    fn a_cable_release_pulses_the_shutter_and_waits_for_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let bulbs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let pulses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut light = CableLight {
            log: std::sync::Arc::clone(&log),
            pulses: std::sync::Arc::clone(&pulses),
        };
        let mut camera = SharedBulb {
            log: std::sync::Arc::clone(&log),
            bulbs: std::sync::Arc::clone(&bulbs),
            bytes: 64,
        };
        let mut waits = Vec::new();
        let mut settings = settings(dir.path());
        settings.levels = [200, 180, 255];
        let mut pulse = CableLight {
            log: std::sync::Arc::clone(&log),
            pulses: std::sync::Arc::clone(&pulses),
        };
        {
            let mut service = CaptureService::new(&mut light, &mut camera, |seconds| {
                waits.push(seconds);
            });
            service
                .capture_blend(
                    &mut SharedBulb {
                        log: std::sync::Arc::clone(&log),
                        bulbs: std::sync::Arc::clone(&bulbs),
                        bytes: 64,
                    },
                    &settings,
                    0.4,
                    BulbRelease::Cable,
                    None,
                    None,
                    None,
                    false,
                    Some(&mut pulse),
                )
                .unwrap();
        }
        assert!(!bulbs.lock().unwrap()[0].0, "camera waits for the cable");
        assert_eq!(pulses.lock().unwrap()[0], 0.35 + 3.0 * 0.4 + 0.2);
        assert_eq!(waits[0], 0.35);
        assert_eq!(&waits[1..4], &[0.4, 0.4, 0.4]);
        assert_eq!(waits[4], 0.2);
    }

    struct RecordingLight {
        calls: Vec<[u32; 5]>,
        bulbs: Vec<Vec<[u32; 5]>>,
    }

    impl LightSource for RecordingLight {
        fn set_color(&mut self, r: u32, g: u32, b: u32, w: u32, ir: u32) {
            self.calls.push([r, g, b, w, ir]);
        }
        fn off(&mut self) {
            self.calls.push([0, 0, 0, 0, 0]);
        }
        fn close(&mut self) {}
    }

    struct FakeBulb;

    impl Camera for FakeBulb {
        fn capture(
            &mut self,
            _out_path: &Path,
            _shutter: Option<&str>,
            _iso: Option<&str>,
            _aperture: Option<&str>,
        ) -> Result<PathBuf, String> {
            Err("this camera takes only Bulb exposures".to_string())
        }
        fn close(&mut self) {}
    }

    impl BulbCamera for FakeBulb {
        fn capture_bulb(
            &mut self,
            _out_path: &Path,
            _expose: &mut dyn FnMut(),
            _usb_release: bool,
            _iso: Option<&str>,
            _aperture: Option<&str>,
            _keep_bulb: bool,
        ) -> Result<PathBuf, String> {
            Err("not used".to_string())
        }
    }

    #[test]
    fn a_cable_release_needs_a_shutter_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut light = RecordingLight {
            calls: Vec::new(),
            bulbs: Vec::new(),
        };
        let mut camera = FakeBulb;
        let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
        let error = service
            .capture_blend(
                &mut FakeBulb,
                &settings(dir.path()),
                0.5,
                BulbRelease::Cable,
                None,
                None,
                None,
                false,
                None,
            )
            .unwrap_err();
        assert!(error.0.contains("shutter release"));
    }

    #[test]
    fn a_cable_release_refuses_an_exposure_longer_than_the_pulse() {
        let dir = tempfile::tempdir().unwrap();
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let pulses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut light = CableLight {
            log: std::sync::Arc::clone(&log),
            pulses: std::sync::Arc::clone(&pulses),
        };
        let mut camera = SharedBulb {
            log: std::sync::Arc::clone(&log),
            bulbs: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            bytes: 64,
        };
        let mut pulse = CableLight {
            log: std::sync::Arc::clone(&log),
            pulses: std::sync::Arc::clone(&pulses),
        };
        let mut service = CaptureService::new(&mut light, &mut camera, |_| {});
        let error = service
            .capture_blend(
                &mut SharedBulb {
                    log: std::sync::Arc::clone(&log),
                    bulbs: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                    bytes: 64,
                },
                &settings(dir.path()),
                2.0,
                BulbRelease::Cable,
                None,
                None,
                None,
                false,
                Some(&mut pulse),
            )
            .unwrap_err();
        assert!(error.0.contains("release cable"));
        assert!(pulses.lock().unwrap().is_empty());
    }
}
