//! Capture the golden matrix from the NegFe shell.
//!
//! The binary opens a fixed client area, steps every manifest entry, and writes one PNG
//! per entry plus the same manifest fields as the Python harness.

use std::path::PathBuf;

use eframe::egui::{self, ViewportBuilder};
use negpy_desktop::app::NegFeApp;
use negpy_desktop::capture::{self, CaptureSize};
use negpy_desktop::fixture::{CaptureEntry, GoldenManifest, ReferenceFixture};
use negpy_desktop::frame_source::{FrameImage, frame_path, load_display_frame, placeholder};

#[derive(Clone, Debug)]
struct Options {
    golden: PathBuf,
    output: PathBuf,
    fixture: PathBuf,
}

fn main() -> eframe::Result {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let manifest = GoldenManifest::read(&options.golden.join("manifest.json"))
        .and_then(|manifest| manifest.validate().map(|()| manifest))
        .unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2);
        });
    let reference = ReferenceFixture::read(&options.fixture).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    std::fs::create_dir_all(&options.output).unwrap();
    let frame = load_frame(&manifest, &reference);
    let viewport = ViewportBuilder::default()
        .with_inner_size([manifest.width as f32, manifest.height as f32])
        .with_decorations(false)
        .with_resizable(true);
    eframe::run_native(
        "NegFe Capture",
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Wgpu,
            persist_window: false,
            ..Default::default()
        },
        Box::new(move |cc| {
            let ppp = cc.egui_ctx.pixels_per_point();
            let zoom = (1.0 / ppp).clamp(0.25, 1.0);
            cc.egui_ctx.set_zoom_factor(zoom);
            negpy_desktop::styles::fonts::install(&cc.egui_ctx);
            Ok(Box::new(CaptureApp::new(
                manifest,
                reference,
                options.output,
                frame,
            )))
        }),
    )
}

fn load_frame(manifest: &GoldenManifest, reference: &ReferenceFixture) -> FrameImage {
    let path = frame_path(manifest.frame_folder(), &reference.fixture.frame);
    let profile = manifest.profile_dir();
    path.as_deref()
        .and_then(
            |path| match load_display_frame(path, profile.as_deref(), 1600) {
                Ok(frame) => Some(frame),
                Err(error) => {
                    eprintln!("{error}");
                    None
                }
            },
        )
        .unwrap_or_else(|| placeholder(1600, 1200))
}

struct CaptureApp {
    manifest: GoldenManifest,
    reference: ReferenceFixture,
    output: PathBuf,
    app: NegFeApp,
    index: usize,
    requested: bool,
    wrote_manifest: bool,
    frames: usize,
    frame: FrameImage,
    viewport: Option<CaptureSize>,
}

impl CaptureApp {
    fn new(
        manifest: GoldenManifest,
        reference: ReferenceFixture,
        output: PathBuf,
        frame: FrameImage,
    ) -> Self {
        let mut app = NegFeApp::for_capture(
            &manifest.captures[0].group,
            &manifest.captures[0].tab,
            &manifest.captures[0].subtab,
            &manifest.captures[0].view,
            manifest.captures[0].scroll,
        );
        app.set_frame(frame.clone());
        let viewport = CaptureSize {
            width: manifest.width,
            height: manifest.height,
        };
        Self {
            manifest,
            reference,
            output,
            app,
            frame,
            index: 0,
            requested: false,
            wrote_manifest: false,
            frames: 0,
            viewport: Some(viewport),
        }
    }

    fn entry(&self) -> &CaptureEntry {
        &self.manifest.captures[self.index]
    }

    fn advance(&mut self) {
        self.index += 1;
        self.requested = false;
        if let Some(entry) = self.manifest.captures.get(self.index) {
            let mut app = NegFeApp::for_capture(
                &entry.group,
                &entry.tab,
                &entry.subtab,
                &entry.view,
                entry.scroll,
            );
            app.set_frame(self.frame.clone());
            self.app = app;
        }
    }

    fn finish_manifest(&mut self) {
        if self.wrote_manifest {
            return;
        }
        let written = GoldenManifest {
            profile: self.output.display().to_string(),
            folder: self.reference.fixture.frame.clone(),
            frame: self.reference.fixture.frame.clone(),
            width: self.manifest.width,
            height: self.manifest.height,
            captures: self.manifest.captures.clone(),
            source_folder: self
                .manifest
                .frame_folder()
                .map_or_else(String::new, |path| path.display().to_string()),
            source_profile: self
                .manifest
                .profile_dir()
                .map_or_else(String::new, |path| path.display().to_string()),
            source: self.manifest.source.clone(),
        };
        if let Err(error) = written.write(&self.output.join("manifest.json")) {
            eprintln!("{error}");
        }
        self.wrote_manifest = true;
    }
}

impl eframe::App for CaptureApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if self.frames > 600 {
            eprintln!("Timed out waiting for the screenshot event.");
            std::process::exit(1);
        }
        if self.index >= self.manifest.captures.len() {
            self.finish_manifest();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let expected = self.entry().size();
        if self.viewport != Some(expected) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                expected.width as f32,
                expected.height as f32,
            )));
            self.viewport = Some(expected);
            self.requested = false;
            ctx.request_repaint();
            return;
        }
        self.app.draw(ctx);
        if let Some(image) = capture::take_screenshot(ctx) {
            let image = if expected.accepts(&image) {
                image
            } else {
                std::sync::Arc::new(capture::resize(&image, expected.width, expected.height))
            };
            let path = self.output.join(&self.entry().file);
            if let Err(error) = capture::write_png(&image, &path) {
                eprintln!("{error}");
                std::process::exit(1);
            }
            self.advance();
            println!("wrote {}", self.manifest.captures[self.index - 1].file);
            ctx.request_repaint();
            return;
        }
        if !self.requested {
            capture::request(ctx);
            self.requested = true;
        }
        ctx.request_repaint();
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut golden = None;
    let mut output = None;
    let mut fixture = None;
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--golden" => golden = args.next().map(PathBuf::from),
            "--output" => output = args.next().map(PathBuf::from),
            "--fixture" => fixture = args.next().map(PathBuf::from),
            other => return Err(format!("Unknown argument: {other}")),
        }
    }
    let golden = golden.ok_or_else(|| "Missing --golden <dir>.".to_string())?;
    let root = golden
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap_or_else(|| std::path::Path::new("."));
    let output = output.unwrap_or_else(|| root.join("actual/rust"));
    let fixture = fixture.unwrap_or_else(|| root.join("reference_roll.json"));
    Ok(Options {
        golden,
        output,
        fixture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_paths_sit_beside_the_golden_set() {
        let args = vec!["--golden".to_string(), "tests/gui/golden/tall".to_string()];
        let options = parse_args(args.into_iter()).unwrap();
        assert_eq!(options.output, PathBuf::from("tests/gui/actual/rust"));
        assert_eq!(
            options.fixture,
            PathBuf::from("tests/gui/reference_roll.json")
        );
    }
}
