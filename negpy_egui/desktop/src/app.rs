//! The NegFe window shell.
//!
//! The shell is intentionally small during the theme port: it paints the fixed regions
//! the golden comparator scores, so every capture has the same geometry as NegPy.

use eframe::egui::{self, Color32, Frame, Rect, Stroke, TextureHandle, TextureOptions, Vec2};

use crate::edit::FrameEdit;
use crate::fixture::ReferenceFixture;
use crate::frame_source::FrameImage;
use crate::styles::icons::Icon;
use crate::styles::layout::TOP_ROW_HEIGHT;
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
        let (tab, subtab) = if group == "frame" {
            (Tab::Frame, tab.to_string())
        } else {
            let tab = if tab.is_empty() {
                Tab::from_key(group)
            } else {
                Tab::from_key(tab)
            };
            (tab, subtab.to_string())
        };
        Self {
            tab,
            subtab,
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
    pub edit: FrameEdit,
    pub frame: Option<FrameImage>,
    pub strip: Vec<FrameImage>,
    texture: Option<TextureHandle>,
    strip_textures: Vec<TextureHandle>,
}

impl NegFeApp {
    /// An app showing the reference fixture.
    #[must_use]
    pub fn new(fixture: &ReferenceFixture) -> Self {
        let view = ViewState::from_fixture(fixture);
        Self {
            rail: RailState::for_view(&view),
            view,
            edit: FrameEdit::default(),
            frame: None,
            strip: Vec::new(),
            texture: None,
            strip_textures: Vec::new(),
        }
    }

    /// An app showing one manifest capture.
    #[must_use]
    pub fn for_capture(group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) -> Self {
        let view = ViewState::from_capture(group, tab, subtab, view, scroll);
        Self {
            rail: RailState::for_view(&view),
            view,
            edit: FrameEdit::default(),
            frame: None,
            strip: Vec::new(),
            texture: None,
            strip_textures: Vec::new(),
        }
    }

    /// Show a decoded frame on the canvas.
    pub fn set_frame(&mut self, frame: FrameImage) {
        self.view.texture_size = Some(Vec2::new(frame.width as f32, frame.height as f32));
        self.texture = None;
        self.frame = Some(frame);
    }

    /// Set the edit the rail reads and writes.
    pub fn set_edit(&mut self, edit: FrameEdit) {
        self.edit = edit;
    }

    /// Move to the tab and page a capture entry asks for, without rebuilding the rail.
    pub fn select_view(&mut self, group: &str, tab: &str, subtab: &str, view: &str, scroll: f64) {
        let texture_size = self.view.texture_size;
        self.view = ViewState::from_capture(group, tab, subtab, view, scroll);
        self.view.texture_size = texture_size;
        self.rail.select_view(&self.view);
    }

    /// Show the roll's thumbnails in the Film Strip.
    pub fn set_strip(&mut self, strip: Vec<FrameImage>) {
        self.strip_textures.clear();
        self.strip = strip;
    }

    /// Show the frame's stored History rows and work-print names.
    pub fn set_history(
        &mut self,
        rows: &[negpy_backend::session::workspace::HistoryRow],
        work_prints: &[String],
    ) {
        self.rail.history.load_session(rows, work_prints);
    }

    /// Show the roll and baseline labels the stored edit names.
    pub fn set_roll_analysis(
        &mut self,
        state: crate::view::rail::roll_analysis::RollAnalysisState,
    ) {
        self.rail.roll_analysis = state;
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
        self.ensure_strip_textures(ctx);
        let strip_textures: Vec<(egui::TextureId, Vec2)> = self
            .strip_textures
            .iter()
            .zip(self.strip.iter())
            .map(|(texture, frame)| {
                (
                    texture.id(),
                    Vec2::new(frame.width as f32, frame.height as f32),
                )
            })
            .collect();
        let screen = ctx.screen_rect();
        if self.view.view == "light_table" {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(theme.bg_panel))
                .show(ctx, |ui| {
                    crate::view::light_table::paint(ui, screen, &strip_textures, 0);
                    let rail_rect = rail_rect(screen);
                    let switched = self.rail.show(ui, rail_rect, &mut self.edit);
                    self.view.tab = self.rail.tab;
                    if let Some(tab) = switched {
                        self.view.tab = tab;
                    }
                    let _ = self.edit.save();
                });
            return;
        }
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
            let rail_rect = paint_shell(ui.painter(), screen, &self.view, texture, &strip_textures);
            let switched = self.rail.show(ui, rail_rect, &mut self.edit);
            self.view.tab = self.rail.tab;
            if let Some(tab) = switched {
                self.view.tab = tab;
            }
            let _ = self.edit.save();
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

    fn ensure_strip_textures(&mut self, ctx: &egui::Context) {
        if self.strip_textures.len() == self.strip.len() {
            return;
        }
        self.strip_textures = self
            .strip
            .iter()
            .filter(|frame| frame.is_rgb())
            .map(|frame| {
                let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.pixels);
                ctx.load_texture(frame.name.clone(), image, TextureOptions::LINEAR)
            })
            .collect();
    }
}

/// The right rail's bounds for a window size.
#[must_use]
pub fn rail_rect(screen: Rect) -> Rect {
    let margin = 8.0;
    let body_top = screen.top() + TOP_ROW_HEIGHT + 8.0;
    let rail_width = if screen.width() >= 1800.0 {
        RAIL_WIDTH
    } else {
        RAIL_WIDTH.min((screen.width() * 0.27).max(420.0))
    };
    Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    )
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
    thumbnails: &[(egui::TextureId, Vec2)],
) -> Rect {
    let margin = 8.0;
    paint_top_bar(painter, screen);

    let body_top = screen.top() + crate::styles::layout::TOP_ROW_HEIGHT + 8.0;
    let rail_width = RAIL_WIDTH.min((screen.width() * 0.27).max(420.0));
    let rail_width = if screen.width() >= 1800.0 {
        RAIL_WIDTH
    } else {
        rail_width
    };
    let search = Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, screen.top() + margin),
        egui::pos2(
            screen.right() - margin,
            screen.top() + margin + crate::view::rail::SEARCH_HEIGHT,
        ),
    );
    let mut search_ui = egui::Ui::new(
        painter.ctx().clone(),
        "top-bar-search".into(),
        egui::UiBuilder::new()
            .max_rect(search)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    search_ui.set_width(search.width());
    crate::view::rail::paint_search(&mut search_ui, &Theme::default());
    let rail = Rect::from_min_max(
        egui::pos2(screen.right() - rail_width - margin, body_top),
        egui::pos2(screen.right() - margin, screen.bottom() - margin),
    );
    let strip_height = if view.strip_open { 239.0 } else { 0.0 };
    let toolbar_height = 52.0;
    let canvas_width = rail.left() - margin - (screen.left() + margin);
    let canvas_bottom = screen.bottom() - margin - toolbar_height - strip_height;
    let canvas_rect = Rect::from_min_size(
        egui::pos2(screen.left() + margin, body_top),
        Vec2::new(canvas_width, canvas_bottom - body_top),
    );
    painter.rect_filled(canvas_rect, 0.0, canvas());

    let photo = fitted_rect(canvas_rect, Vec2::new(1208.0, 1181.0));
    if let Some(texture) = frame {
        painter.image(
            texture,
            photo,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        painter.rect_filled(photo, 0.0, Color32::from_rgb(0xa8, 0xa0, 0x90));
    }

    if strip_height > 0.0 {
        let strip = Rect::from_min_size(
            egui::pos2(screen.left() + margin, canvas_bottom),
            Vec2::new(canvas_width, strip_height),
        );
        let theme = Theme::default();
        painter.rect_filled(strip, 0.0, Color32::from_rgb(0x6f, 0x6f, 0x6f));
        let scrollbar = 12.0;
        let tiles = Rect::from_min_max(
            strip.min + Vec2::splat(0.0),
            egui::pos2(strip.right(), strip.bottom() - scrollbar),
        );
        let tile_width = 210.0;
        let gap = 4.0;
        let inset = 4.0;
        let count = ((tiles.width() - inset) / (tile_width + gap)).ceil() as usize + 1;
        for index in 0..count {
            let left = tiles.left() + inset + index as f32 * (tile_width + gap);
            let tile = Rect::from_min_size(
                egui::pos2(left, tiles.top() + inset),
                Vec2::new(tile_width, tiles.height() - 2.0 * inset),
            );
            painter.rect_filled(tile, 0.0, Color32::from_rgb(0x17, 0x15, 0x16));
            if let Some((texture, size)) = thumbnails.get(index) {
                let content = cover_rect(tile, *size);
                painter.with_clip_rect(tile).image(
                    *texture,
                    content,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let border = if index == 0 {
                theme.error
            } else {
                theme.border_color
            };
            painter.rect_stroke(
                tile,
                0.0,
                Stroke::new(if index == 0 { 4.0 } else { 1.0_f32 }, border),
                egui::StrokeKind::Inside,
            );
            let dot = egui::pos2(tile.right() - 7.0, tile.top() + 7.0);
            painter.circle_filled(
                dot,
                4.0,
                if index < 2 {
                    theme.warn_amber
                } else {
                    Color32::from_rgb(0x8b, 0x8b, 0x8b)
                },
            );
            painter.circle_stroke(dot, 4.0, Stroke::new(1.0_f32, theme.mark_photo));
        }
        let bar = Rect::from_min_max(
            egui::pos2(strip.left() + inset, tiles.bottom() + 2.0),
            egui::pos2(strip.left() + strip.width() * 0.5, strip.bottom() - 2.0),
        );
        painter.rect_filled(bar, bar.height() / 2.0, theme.border_color);
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

/// The top bar's bounds, from the window edge to the rail.
#[must_use]
pub fn top_bar_rect(screen: Rect) -> Rect {
    let margin = 8.0;
    Rect::from_min_max(
        screen.min + Vec2::new(margin, margin),
        egui::pos2(
            screen.right() - margin,
            screen.top() + TOP_ROW_HEIGHT + margin,
        ),
    )
}

/// Paint the file name and the current recipe over `screen`.
pub fn paint_top_bar(painter: &egui::Painter, screen: Rect) {
    let top = top_bar_rect(screen);
    painter.rect_filled(top, 0.0, surface());
    painter.text(
        top.left_center() + Vec2::new(f32::from(Theme::default().space_xl), 0.0),
        egui::Align2::LEFT_CENTER,
        "P5080635.ORF · 7792 x 10388 px",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );
    painter.text(
        egui::pos2(screen.right() - RAIL_WIDTH - 26.0, top.center().y),
        egui::Align2::RIGHT_CENTER,
        "Digital (Adobe RGB) · Print Density 1.08 (auto) · Grade ISO-R 83 (auto)",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(0xbc, 0xbc, 0xbc),
    );
}

fn cover_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).max(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn fitted_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).min(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn paint_toolbar(painter: &egui::Painter, toolbar: Rect) {
    use crate::styles::icons::Icon;
    use crate::styles::layout::{
        BAR_BUTTON_HEIGHT, BAR_ICON_SIZE, BAR_TRACK_BUTTON_HEIGHT, BAR_TRACK_PAD,
    };

    let theme = Theme::default();
    painter.rect_filled(toolbar, 0.0, surface());
    let row_height = BAR_BUTTON_HEIGHT;
    let top = toolbar.top() + (toolbar.height() - row_height) / 2.0;
    let mut x = toolbar.left() + 8.0;

    let flat = |painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32| {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon.glyph(),
            egui::FontId::proportional(BAR_ICON_SIZE),
            color,
        );
    };
    let track = |painter: &egui::Painter, rect: Rect| {
        painter.rect_filled(rect, 4.0, theme.bg_input);
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0_f32, theme.border_primary),
            egui::StrokeKind::Inside,
        );
    };

    let mode = Rect::from_min_size(egui::pos2(x, top), Vec2::new(146.0, row_height));
    track(painter, mode);
    let segment = mode.shrink2(Vec2::new(BAR_TRACK_PAD, BAR_TRACK_PAD));
    let photo = Rect::from_min_size(
        segment.min,
        Vec2::new(segment.width() / 2.0, BAR_TRACK_BUTTON_HEIGHT),
    );
    let roll = Rect::from_min_size(
        egui::pos2(photo.right(), photo.top()),
        Vec2::new(segment.width() / 2.0, BAR_TRACK_BUTTON_HEIGHT),
    );
    painter.rect_filled(photo, 4.0, theme.bg_checked);
    painter.rect_stroke(
        photo,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_checked),
        egui::StrokeKind::Inside,
    );
    flat(
        painter,
        Rect::from_min_size(photo.min, Vec2::new(24.0, photo.height())),
        Icon::Image,
        theme.text_primary,
    );
    painter.text(
        photo.left_center() + Vec2::new(26.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Photo",
        egui::FontId::proportional(13.0),
        theme.text_primary,
    );
    flat(
        painter,
        Rect::from_min_size(roll.min, Vec2::new(24.0, roll.height())),
        Icon::Film,
        theme.text_hint,
    );
    painter.text(
        roll.left_center() + Vec2::new(26.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Roll",
        egui::FontId::proportional(13.0),
        theme.text_hint,
    );
    x = mode.right() + 8.0;

    let navigation = Rect::from_min_size(egui::pos2(x, top), Vec2::new(172.0, row_height));
    track(painter, navigation);
    let inner = navigation.shrink2(Vec2::new(BAR_TRACK_PAD, BAR_TRACK_PAD));
    flat(
        painter,
        Rect::from_min_size(inner.min, Vec2::splat(BAR_TRACK_BUTTON_HEIGHT)),
        Icon::ChevronLeft,
        theme.text_secondary,
    );
    painter.text(
        inner.center(),
        egui::Align2::CENTER_CENTER,
        "1 / 13",
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(inner.right() - BAR_TRACK_BUTTON_HEIGHT, inner.top()),
            Vec2::splat(BAR_TRACK_BUTTON_HEIGHT),
        ),
        Icon::ChevronRight,
        theme.text_secondary,
    );
    x = navigation.right() + 8.0;

    let zoom = Rect::from_min_size(egui::pos2(x, top), Vec2::new(76.0, row_height));
    track(painter, zoom);
    painter.text(
        zoom.center() - Vec2::new(6.0, 0.0),
        egui::Align2::CENTER_CENTER,
        "16%",
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(zoom.right() - 26.0, zoom.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_secondary,
    );
    x = zoom.right() + 8.0;

    let geometry = Rect::from_min_size(egui::pos2(x, top), Vec2::new(160.0, row_height));
    track(painter, geometry);
    for (index, icon) in [
        Icon::RotateLeft,
        Icon::RotateRight,
        Icon::LockSimple,
        Icon::LockOpen,
    ]
    .iter()
    .enumerate()
    {
        let button = Rect::from_min_size(
            egui::pos2(
                geometry.left()
                    + BAR_TRACK_PAD
                    + index as f32 * (geometry.width() - 2.0 * BAR_TRACK_PAD) / 4.0,
                geometry.top() + BAR_TRACK_PAD,
            ),
            Vec2::new(
                (geometry.width() - 2.0 * BAR_TRACK_PAD) / 4.0,
                BAR_TRACK_BUTTON_HEIGHT,
            ),
        );
        if index == 3 {
            painter.rect_filled(button, 4.0, theme.bg_checked);
        }
        flat(
            painter,
            button,
            *icon,
            if index == 3 {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
    }
    x = geometry.right() + 8.0;

    for (icon, width) in [(Icon::Wrench, 94.0), (Icon::Undo, 36.0), (Icon::Redo, 36.0)] {
        let button = Rect::from_min_size(egui::pos2(x, top), Vec2::new(width, row_height));
        track(painter, button);
        if icon == Icon::Wrench {
            flat(
                painter,
                Rect::from_min_size(button.min, Vec2::new(30.0, row_height)),
                icon,
                theme.text_primary,
            );
            painter.text(
                button.left_center() + Vec2::new(34.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "Tools",
                egui::FontId::proportional(13.0),
                theme.text_primary,
            );
            flat(
                painter,
                Rect::from_min_size(
                    egui::pos2(button.right() - 26.0, button.top()),
                    Vec2::new(24.0, row_height),
                ),
                Icon::ChevronDown,
                theme.text_secondary,
            );
            x = button.right() + 8.0;
            continue;
        }
        flat(
            painter,
            button,
            icon,
            if icon == Icon::Undo {
                theme.text_secondary
            } else {
                theme.text_muted
            },
        );
        x = button.right() + 6.0;
    }

    let print = Rect::from_min_size(egui::pos2(x, top), Vec2::new(112.0, row_height));
    track(painter, print);
    flat(
        painter,
        Rect::from_min_size(print.min, Vec2::new(34.0, row_height)),
        Icon::Print,
        theme.text_secondary,
    );
    painter.text(
        print.left_center() + Vec2::new(38.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Print",
        egui::FontId::proportional(13.0),
        theme.text_secondary,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(print.right() - 26.0, print.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_secondary,
    );
    x = print.right() + 6.0;

    let roll_average = Rect::from_min_size(egui::pos2(x, top), Vec2::new(66.0, row_height));
    track(painter, roll_average);
    flat(painter, roll_average, Icon::Film, theme.text_secondary);
    x = roll_average.right() + 6.0;

    let trailing = Rect::from_min_size(
        egui::pos2(toolbar.right() - 8.0 - 108.0, top),
        Vec2::new(108.0, row_height),
    );
    let primary = Rect::from_min_size(egui::pos2(x, top), Vec2::new(154.0, row_height));
    painter.rect_filled(primary, 4.0, theme.bg_primary);
    painter.rect_stroke(
        primary,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_checked_hover),
        egui::StrokeKind::Inside,
    );
    flat(
        painter,
        Rect::from_min_size(primary.min, Vec2::new(36.0, row_height)),
        Icon::Export,
        theme.text_on_accent,
    );
    painter.text(
        primary.left_center() + Vec2::new(40.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Roll",
        egui::FontId::proportional(13.0),
        theme.text_on_accent,
    );
    flat(
        painter,
        Rect::from_min_size(
            egui::pos2(primary.right() - 26.0, primary.top()),
            Vec2::new(24.0, row_height),
        ),
        Icon::ChevronDown,
        theme.text_on_accent,
    );

    track(painter, trailing);
    for (index, icon) in [
        Icon::Grid,
        Icon::ListDashes,
        Icon::Sliders,
        Icon::ChevronDown,
    ]
    .iter()
    .enumerate()
    {
        let width = 26.0;
        let button = Rect::from_min_size(
            egui::pos2(
                trailing.left() + BAR_TRACK_PAD + index as f32 * width,
                trailing.top() + BAR_TRACK_PAD,
            ),
            Vec2::new(width, BAR_TRACK_BUTTON_HEIGHT),
        );
        flat(
            painter,
            button,
            *icon,
            if index == 0 {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
        if index == 1 {
            painter.circle_filled(
                egui::pos2(button.center().x, button.center().y - 6.0),
                2.5,
                theme.status_success,
            );
        }
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

    #[test]
    fn selecting_a_capture_keeps_the_rail_state() {
        let fixture = ReferenceFixture::bundled().unwrap();
        let mut app = NegFeApp::new(&fixture);
        app.rail.library.filter = "kept".to_string();
        app.select_view("frame", "tone", "", "canvas", 0.0);
        assert_eq!(app.view.tab, Tab::Frame);
        assert_eq!(app.rail.frame_subtab.current_key(), "tone");
        assert_eq!(app.rail.library.filter, "kept");
    }

    #[test]
    fn a_capture_without_a_tab_key_reads_the_group() {
        let view = ViewState::from_capture("metadata", "", "shot", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Metadata);
        let view = ViewState::from_capture("scan", "", "", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Scan);
    }

    #[test]
    fn a_frame_capture_opens_the_page_in_its_tab_key() {
        let view = ViewState::from_capture("frame", "tone", "", "canvas", 0.0);
        assert_eq!(view.tab, Tab::Frame);
        assert_eq!(view.subtab, "tone");
        assert_eq!(
            crate::view::rail::RailState::for_view(&view)
                .frame_subtab
                .current_key(),
            "tone"
        );
    }
}
