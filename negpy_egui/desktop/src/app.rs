//! The NegFe window shell.
//!
//! The shell is intentionally small during the theme port: it paints the fixed regions
//! the golden comparator scores, so every capture has the same geometry as NegPy.

use eframe::egui::{self, Color32, Frame, Rect, Stroke, TextureHandle, TextureOptions, Vec2};

use crate::fixture::ReferenceFixture;
use crate::frame_source::FrameImage;
use crate::styles::icons::Icon;
use crate::styles::theme::Theme;
use crate::view::contact_sheet;
use crate::view::rail::{RAIL_WIDTH, RailState};

/// The visible tab of the right rail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    Library,
    #[default]
    Roll,
    Frame,
    Metadata,
    Export,
    Scan,
}

impl Tab {
    /// The tab labels in the golden tab-strip order.
    pub const ALL: [Self; 6] = [
        Self::Library,
        Self::Roll,
        Self::Frame,
        Self::Metadata,
        Self::Export,
        Self::Scan,
    ];

    /// The label under the tab icon.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Roll => "Roll",
            Self::Frame => "Frame",
            Self::Metadata => "Metadata",
            Self::Export => "Export",
            Self::Scan => "Scan",
        }
    }

    /// The icon above the tab label.
    #[must_use]
    pub fn icon(self) -> Icon {
        match self {
            Self::Library => Icon::Grid,
            Self::Roll => Icon::Film,
            Self::Frame => Icon::Image,
            Self::Metadata => Icon::Metadata,
            Self::Export => Icon::Export,
            Self::Scan => Icon::Scan,
        }
    }

    /// Select a tab by its manifest key.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "library" => Self::Library,
            "frame" => Self::Frame,
            "metadata" => Self::Metadata,
            "export" => Self::Export,
            "scan" => Self::Scan,
            _ => Self::Roll,
        }
    }
}

/// The whole-window view a manifest entry asks to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub tab: Tab,
    pub subtab: String,
    pub group: String,
    pub view: String,
    pub scroll: u32,
    pub analysis_open: bool,
    pub analysis_log: bool,
    pub strip_open: bool,
    pub texture_size: Option<Vec2>,
    pub contact_sheet: bool,
}

impl ViewState {
    /// The frame view in the reference fixture.
    #[must_use]
    pub fn from_fixture(fixture: &ReferenceFixture) -> Self {
        Self {
            tab: Tab::from_key(&fixture.fixture.tab),
            subtab: String::new(),
            group: "roll".to_string(),
            view: "canvas".to_string(),
            scroll: 0,
            analysis_open: fixture.fixture.analysis.open,
            analysis_log: fixture.fixture.analysis.log,
            strip_open: fixture.fixture.strip.open,
            texture_size: None,
            contact_sheet: false,
        }
    }

    /// Set the state named by one manifest entry.
    pub fn from_capture(group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) -> Self {
        Self {
            tab: Tab::from_key(tab),
            subtab: subtab.to_string(),
            group: group.to_string(),
            view: view.to_string(),
            scroll: (scroll.max(0.0) * 1000.0).round() as u32,
            analysis_open: true,
            analysis_log: true,
            strip_open: true,
            texture_size: None,
            contact_sheet: view == "contact_sheet_dialog",
        }
    }
}

/// The desktop application state.
pub struct NegFeApp {
    pub view: ViewState,
    pub rail: RailState,
    pub frame: Option<FrameImage>,
    texture: Option<TextureHandle>,
}

impl NegFeApp {
    /// An app showing the reference fixture.
    #[must_use]
    pub fn new(fixture: &ReferenceFixture) -> Self {
        let view = ViewState::from_fixture(fixture);
        Self {
            rail: RailState::for_view(&view),
            view,
            frame: None,
            texture: None,
        }
    }

    /// An app showing one manifest capture.
    #[must_use]
    pub fn for_capture(group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) -> Self {
        let view = ViewState::from_capture(group, tab, subtab, view, scroll);
        Self {
            rail: RailState::for_view(&view),
            view,
            frame: None,
            texture: None,
        }
    }

    /// Show a decoded frame on the canvas.
    pub fn set_frame(&mut self, frame: FrameImage) {
        self.view.texture_size = Some(Vec2::new(frame.width as f32, frame.height as f32));
        self.texture = None;
        self.frame = Some(frame);
    }

    /// Paint the shell regions for this frame.
    pub fn draw(&mut self, ctx: &egui::Context) {
        let theme = Theme::default();
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = theme.bg_panel;
        style.visuals.window_fill = theme.bg_panel;
        style.visuals.extreme_bg_color = theme.bg_input;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border_primary);
        ctx.set_style(style);
        let texture = self.texture_for(ctx).map(|texture| texture.id());
        let screen = ctx.screen_rect();
        if self.view.contact_sheet {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme.bg_panel))
                .show(ctx, |ui| {
                    ui.painter().rect_filled(screen, 0.0, theme.bg_panel);
                    contact_sheet::paint(ui.painter(), screen, texture);
                });
            return;
        }
        let root = Frame::NONE.fill(surface());
        egui::CentralPanel::default().frame(root).show(ctx, |ui| {
            ui.painter().rect_filled(screen, 0.0, surface());
            let rail_rect = paint_shell(ui.painter(), screen, &self.view, texture);
            let mut rail = std::mem::replace(&mut self.rail, RailState::for_view(&self.view));
            let switched = rail.show(ui, rail_rect);
            self.view.tab = rail.tab;
            self.rail = rail;
            if let Some(tab) = switched {
                self.view.tab = tab;
            }
        });
    }

    fn texture_for(&mut self, ctx: &egui::Context) -> Option<&TextureHandle> {
        if self.texture.is_none()
            && let Some(frame) = &self.frame
            && frame.is_rgb()
        {
            let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.pixels);
            self.texture = Some(ctx.load_texture("fixture-frame", image, TextureOptions::LINEAR));
        }
        self.texture.as_ref()
    }
}

impl eframe::App for NegFeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

fn surface() -> Color32 {
    Theme::default().bg_panel
}

fn canvas() -> Color32 {
    Theme::default().canvas_bg_mid_grey
}

fn line() -> Stroke {
    Stroke::new(1.0_f32, Theme::default().border_primary)
}

fn paint_shell(
    painter: &egui::Painter,
    screen: Rect,
    view: &ViewState,
    frame: Option<egui::TextureId>,
) -> Rect {
    let margin = 8.0;
    let top = Rect::from_min_max(
        screen.min + Vec2::new(margin, margin),
        egui::pos2(screen.right() - margin, screen.top() + 44.0),
    );
    painter.rect_filled(top, 0.0, surface());
    painter.text(
        top.left_center(),
        egui::Align2::LEFT_CENTER,
        "P5080635.ORF · 7792 x 10388 px",
        egui::FontId::proportional(13.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );

    let body_top = top.bottom() + margin;
    let rail_width = RAIL_WIDTH.min((screen.width() * 0.27).max(420.0));
    let rail = Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    );
    let strip_height = if view.strip_open {
        (screen.height() * 0.155).clamp(180.0, 248.0)
    } else {
        0.0
    };
    let toolbar_height = 52.0;
    let canvas_width = rail.left() - margin - (screen.left() + margin);
    let canvas_bottom = screen.bottom() - margin - toolbar_height - strip_height;
    let canvas_rect = Rect::from_min_size(
        egui::pos2(screen.left() + margin, body_top),
        Vec2::new(canvas_width, canvas_bottom - body_top),
    );
    painter.rect_filled(canvas_rect, 0.0, canvas());

    if let Some(texture) = frame {
        let size = view
            .texture_size
            .unwrap_or_else(|| Vec2::new(canvas_rect.width() * 0.92, canvas_rect.height() * 0.93));
        let photo = fitted_rect(canvas_rect, size);
        painter.image(
            texture,
            photo,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let photo = fitted_rect(
            canvas_rect,
            Vec2::new(canvas_rect.width() * 0.92, canvas_rect.height() * 0.93),
        );
        painter.rect_filled(photo, 0.0, Color32::from_rgb(0xa8, 0xa0, 0x90));
    }

    if strip_height > 0.0 {
        let strip = Rect::from_min_size(
            egui::pos2(screen.left() + margin, canvas_bottom),
            Vec2::new(canvas_width, strip_height),
        );
        painter.rect_filled(strip, 0.0, Color32::from_rgb(0x2f, 0x2f, 0x2f));
        let tile_width = (strip.width() - 2.0 * margin) / 6.0;
        for index in 0..6 {
            let tile = Rect::from_min_size(
                egui::pos2(
                    strip.left() + margin + index as f32 * tile_width,
                    strip.top() + margin,
                ),
                Vec2::new(tile_width - margin, strip.height() - 2.0 * margin),
            );
            painter.rect_filled(tile, 0.0, Color32::from_rgb(0x80, 0x80, 0x80));
            painter.rect_stroke(tile, 0.0, line(), egui::StrokeKind::Inside);
        }
    }

    let toolbar = Rect::from_min_size(
        egui::pos2(
            screen.left() + margin,
            screen.bottom() - margin - toolbar_height,
        ),
        Vec2::new(canvas_width, toolbar_height),
    );
    painter.rect_filled(toolbar, 0.0, surface());
    painter.line_segment([toolbar.left_top(), toolbar.right_top()], line());

    painter.rect_filled(rail, 0.0, surface());
    painter.rect_stroke(rail, 0.0, line(), egui::StrokeKind::Outside);
    paint_toolbar(painter, toolbar);
    rail
}

fn fitted_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).min(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn paint_toolbar(painter: &egui::Painter, toolbar: Rect) {
    painter.rect_filled(toolbar, 0.0, surface());
    let labels = ["Photo", "Roll", "1 / 13", "16%", "Print", "Roll"];
    let mut x = toolbar.left() + 8.0;
    for label in labels {
        let width = if label == "Print" { 82.0 } else { 58.0 };
        let button =
            Rect::from_min_size(egui::pos2(x, toolbar.top() + 8.0), Vec2::new(width, 36.0));
        painter.rect_filled(button, 4.0, Color32::from_rgb(0x2f, 0x2f, 0x2f));
        painter.rect_stroke(button, 4.0, line(), egui::StrokeKind::Inside);
        painter.text(
            button.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            Color32::from_rgb(0xe4, 0xe4, 0xe4),
        );
        x += width + 6.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_key_selects_its_tab() {
        for tab in Tab::ALL {
            assert_eq!(Tab::from_key(&tab.label().to_lowercase()), tab);
        }
        assert_eq!(Tab::from_key("unknown"), Tab::Roll);
    }

    #[test]
    fn every_tab_has_a_distinct_icon_role() {
        let icons: Vec<_> = Tab::ALL.iter().map(|tab| tab.icon()).collect();
        for (index, icon) in icons.iter().enumerate() {
            assert!(!icons[..index].contains(icon));
        }
    }

    #[test]
    fn a_capture_keeps_its_scroll_in_thousandths() {
        let view = ViewState::from_capture("roll", "frame", "tone", "canvas", 0.25);
        assert_eq!(view.tab, Tab::Frame);
        assert_eq!(view.subtab, "tone");
        assert_eq!(view.scroll, 250);
    }
}
