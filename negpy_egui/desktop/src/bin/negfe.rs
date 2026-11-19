//! Open the NegFe shell for manual use.

use std::path::PathBuf;

use eframe::egui::ViewportBuilder;
use negpy_desktop::app::NegFeApp;
use negpy_desktop::fixture::ReferenceFixture;
use negpy_desktop::launch::{self, LaunchOptions};

fn main() -> eframe::Result {
    let options = match LaunchOptions::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/gui/reference_roll.json");
    let reference = ReferenceFixture::read(&fixture_path).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let (frame, strip, history, prints, roll_analysis) =
        launch::load_manual_state(&options, &reference);
    let mut app = NegFeApp::new(&reference);
    app.set_frame(frame);
    app.set_strip(strip);
    app.set_history(&history, &prints);
    app.set_roll_analysis(roll_analysis);
    let viewport = ViewportBuilder::default()
        .with_inner_size([1920.0, 1600.0])
        .with_resizable(true);
    eframe::run_native(
        "NegFe",
        eframe::NativeOptions {
            viewport,
            renderer: eframe::Renderer::Wgpu,
            persist_window: false,
            ..Default::default()
        },
        Box::new(move |cc| {
            negpy_desktop::styles::fonts::install(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
