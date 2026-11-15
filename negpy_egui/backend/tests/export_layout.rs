//! Ports the layout rules in tests/test_preview_layout_scale.py and
//! `negpy/services/export/print.py`.

use negpy_backend::domain::models::{
    ExportConfig, ExportResolutionMode, FinishConfig, ToningConfig,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::export::print::{
    apply_layout, effective_border_color, effective_dpi, fair_paper_ratio, fit_content,
    paper_dims_from_long_edge, paper_long_edge_px, weighted_offset_y,
};

fn image(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

#[test]
fn fit_content_returns_the_image_at_its_own_size() {
    for (w, h) in [(1355, 908), (500, 390), (1792, 1216), (1353, 911)] {
        assert_eq!(fit_content(w, h, w, h + 50), (w, h));
        assert_eq!(fit_content(h, w, h + 50, w), (h, w));
    }
}

#[test]
fn original_paper_matches_the_image() {
    for (w, h) in [(1355, 908), (500, 390), (1792, 1216)] {
        assert_eq!(paper_dims_from_long_edge(w, "original", w, h), (w, h));
        assert_eq!(paper_dims_from_long_edge(w, "original", h, w), (h, w));
    }
}

#[test]
fn a_fixed_ratio_derives_the_short_side() {
    assert_eq!(
        paper_dims_from_long_edge(2000, "3:2", 100, 100),
        (2000, 1333)
    );
    assert_eq!(
        paper_dims_from_long_edge(2000, "2:3", 100, 100),
        (1333, 2000)
    );
}

#[test]
fn apply_layout_keeps_an_unscaled_image_whole() {
    let mut export = ExportConfig::default();
    export.export_resolution_mode = ExportResolutionMode::TargetPx;
    export.export_target_long_edge_px = 1355;
    let layout = apply_layout(
        &image(1355, 908, 0.5),
        &export,
        0.0,
        "#ffffff",
        &FinishConfig::default(),
    );
    assert_eq!((layout.image.width, layout.image.height), (1355, 908));
    assert_eq!(layout.content_rect, (0, 0, 1355, 908));
}

#[test]
fn a_border_pads_the_paper_and_centers_the_content() {
    let mut export = ExportConfig::default();
    export.export_resolution_mode = ExportResolutionMode::TargetPx;
    export.export_target_long_edge_px = 1000;
    export.export_dpi = 254;
    export.paper_aspect_ratio = "original".to_string();
    let layout = apply_layout(
        &image(100, 100, 0.5),
        &export,
        2.54,
        "#000000",
        &FinishConfig::default(),
    );
    // The paper stays 1000 px square, and the content fills what the borders leave.
    assert_eq!((layout.image.width, layout.image.height), (1000, 1000));
    assert_eq!(layout.content_rect, (84, 84, 832, 832));
    assert_eq!(layout.image.pixels[0], 0.0);
}

#[test]
fn a_weighted_bottom_border_pushes_the_content_up() {
    let mut finish = FinishConfig::default();
    finish.border_bottom_weight = 3.0;
    let offset = weighted_offset_y(400, 100, 20, 60);
    assert_eq!(offset, 75);
    assert_eq!(weighted_offset_y(400, 100, 0, 0), 150);
}

#[test]
fn the_paper_long_edge_follows_the_resolution_mode() {
    let mut export = ExportConfig::default();
    export.export_resolution_mode = ExportResolutionMode::Print;
    export.export_print_size = 25.4;
    export.export_dpi = 300;
    assert_eq!(paper_long_edge_px(&export), 3000);
    assert_eq!(effective_dpi(&export), 300);

    export.export_resolution_mode = ExportResolutionMode::TargetPx;
    export.export_target_long_edge_px = 4000;
    assert_eq!(paper_long_edge_px(&export), 4000);
    assert_eq!(effective_dpi(&export), 400);
}

#[test]
fn the_border_color_is_the_picked_hex_or_the_toned_paper() {
    let mut finish = FinishConfig::default();
    finish.border_color = "#102030".to_string();
    let toning = ToningConfig::default();
    assert_eq!(effective_border_color(&finish, &toning), "#102030");
    finish.border_match_paper = true;
    assert_eq!(effective_border_color(&finish, &toning), "#ffffff");
}

#[test]
fn a_ratio_parse_falls_back_to_square() {
    assert_eq!(fair_paper_ratio("nonsense"), 1.0);
    assert_eq!(fair_paper_ratio("3:2"), 1.5);
}
