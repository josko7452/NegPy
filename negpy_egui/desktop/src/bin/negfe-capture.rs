//! Capture the golden matrix from the NegFe shell.
//!
//! The binary opens a fixed client area, steps every manifest entry, and writes one PNG
//! per entry plus the same manifest fields as the Python harness.

use std::path::PathBuf;

use eframe::egui::{self, ViewportBuilder};
use negpy_desktop::app::NegFeApp;
use negpy_desktop::capture::{self, CaptureSize};
use negpy_desktop::fixture::{CaptureEntry, GoldenManifest, ReferenceFixture};
use negpy_desktop::frame_source::FrameImage;
use negpy_desktop::launch::{load_manifest_edit, load_manifest_state};

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
    let (frame, strip, history, prints, roll_analysis) = load_manifest_state(&manifest, &reference);
    let edit = load_manifest_edit(&manifest, &reference);
    let viewport = ViewportBuilder::default()
        .with_inner_size([manifest.width as f32, manifest.height as f32])
        .with_decorations(false)
        .with_resizable(true)
        .with_clamp_size_to_monitor_size(false);
    eframe::run_native(
        "NegFe Capture",
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Wgpu,
            persist_window: false,
            ..Default::default()
        },
        Box::new(move |cc| {
            negpy_desktop::styles::fonts::install(&cc.egui_ctx);
            Ok(Box::new(CaptureApp::new(
                manifest,
                reference,
                options.output,
                frame,
                strip,
                history,
                prints,
                roll_analysis,
                edit,
            )))
        }),
    )
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
    viewport: Option<CaptureSize>,
}

impl CaptureApp {
    fn new(
        manifest: GoldenManifest,
        reference: ReferenceFixture,
        output: PathBuf,
        frame: FrameImage,
        strip: Vec<FrameImage>,
        history: Vec<negpy_backend::session::workspace::HistoryRow>,
        prints: Vec<String>,
        roll_analysis: negpy_desktop::view::rail::roll_analysis::RollAnalysisState,
        edit: negpy_desktop::edit::FrameEdit,
    ) -> Self {
        let mut app = NegFeApp::for_capture(
            &manifest.captures[0].group,
            &manifest.captures[0].tab,
            &manifest.captures[0].subtab,
            &manifest.captures[0].view,
            manifest.captures[0].scroll,
        );
        app.set_frame(frame.clone());
        app.set_strip(strip.clone());
        app.set_history(&history, &prints);
        app.set_roll_analysis(roll_analysis.clone());
        app.set_edit(edit.clone());
        Self {
            manifest,
            reference,
            output,
            app,
            index: 0,
            requested: false,
            wrote_manifest: false,
            frames: 0,
            viewport: None,
        }
    }

    fn entry(&self) -> &CaptureEntry {
        &self.manifest.captures[self.index]
    }

    fn advance(&mut self) {
        self.index += 1;
        self.requested = false;
        if let Some(entry) = self.manifest.captures.get(self.index) {
            self.app.select_view(
                &entry.group,
                &entry.tab,
                &entry.subtab,
                &entry.view,
                entry.scroll,
            );
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
            } else if let Some(factor) = capture::proportional_factor(&image, expected) {
                std::sync::Arc::new(capture::downscale(&image, factor))
            } else {
                eprintln!(
                    "{} captured at {}x{}, expected {}x{}",
                    self.entry().file,
                    image.size[0],
                    image.size[1],
                    expected.width,
                    expected.height
                );
                std::process::exit(1);
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
