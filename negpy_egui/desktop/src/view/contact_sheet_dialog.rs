//! The live Contact Sheet dialog: a rendered page, its settings, and Export.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, Vec2};
use negpy_backend::services::export::contact_sheet_layout::{ContactSheetSettings, DPI_CHOICES};

use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::labeled_toggle;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::hint;
use crate::view::widgets::field_row::{RowColumns, field_row};

const PANEL_WIDTH: f32 = 340.0;
const MARGIN: f32 = 16.0;
const GAP: f32 = 16.0;

/// What the dialog asks the app to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SheetActions {
    pub close: bool,
    pub export: bool,
    pub refresh: bool,
    /// The open frame a tile click asked for.
    pub tile: Option<usize>,
}

/// The live dialog's state.
#[derive(Clone)]
pub struct ContactSheetDialog {
    pub settings: ContactSheetSettings,
    pub page: usize,
    pub page_count: usize,
    pub preview: Option<egui::TextureHandle>,
    pub status: String,
    /// A settings change that needs a new render.
    pub changed: bool,
    /// Whether the frame picker lists every frame.
    pub picking: bool,
    /// The entry indices the user left off the sheet.
    pub left_out: std::collections::BTreeSet<usize>,
    /// The frame names the picker lists, filled from the strip.
    pub frames: Vec<String>,
}

impl Default for ContactSheetDialog {
    fn default() -> Self {
        Self {
            settings: ContactSheetSettings::default(),
            page: 0,
            page_count: 1,
            preview: None,
            status: String::new(),
            changed: true,
            picking: false,
            left_out: std::collections::BTreeSet::new(),
            frames: Vec::new(),
        }
    }
}

impl ContactSheetDialog {
    /// Show a freshly rendered page.
    pub fn set_preview(&mut self, ctx: &egui::Context, width: usize, height: usize, pixels: &[u8]) {
        let image = egui::ColorImage::from_rgb([width, height], pixels);
        self.preview = Some(ctx.load_texture(
            format!("contact-sheet-{}", self.page),
            image,
            egui::TextureOptions::LINEAR,
        ));
        self.changed = false;
    }

    /// Paint the dialog into `screen` and return the buttons it asked for.
    pub fn show(&mut self, ui: &mut egui::Ui, screen: Rect, frames: &[String]) -> SheetActions {
        self.frames = frames.to_vec();
        self.left_out.retain(|index| *index < frames.len());
        let theme = Theme::default();
        let panel = Rect::from_min_size(
            egui::pos2(screen.right() - MARGIN - PANEL_WIDTH, screen.top() + MARGIN),
            Vec2::new(PANEL_WIDTH, screen.height() - 2.0 * MARGIN),
        );
        let canvas = Rect::from_min_max(
            egui::pos2(screen.left() + MARGIN, screen.top() + MARGIN),
            egui::pos2(panel.left() - GAP, screen.bottom() - MARGIN),
        );
        ui.painter().rect_filled(screen, 0.0, theme.bg_panel);
        ui.painter()
            .rect_filled(canvas, 0.0, theme.canvas_bg_dark_grey);
        let paper = paper_rect(
            canvas,
            self.settings.paper_width as f32,
            self.settings.paper_height as f32,
        );
        ui.painter().rect_filled(paper, 0.0, Color32::BLACK);
        if let Some(texture) = &self.preview {
            ui.painter().image(
                texture.id(),
                paper,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter().rect_stroke(
            paper,
            0.0,
            Stroke::new(1.0_f32, theme.border_primary),
            egui::StrokeKind::Inside,
        );
        self.show_navigation(ui, canvas, &theme);
        let mut actions = self.show_controls(ui, panel, &theme);
        actions.tile = self.tile_click(ui, canvas);
        actions
    }

    /// Start the picker with the scan's rejected frames left out.
    pub fn seed_excluded(&mut self, excluded: &[bool]) {
        for (index, set) in excluded.iter().enumerate() {
            if *set {
                self.left_out.insert(index);
            } else {
                self.left_out.remove(&index);
            }
        }
    }

    /// The included frames, in strip order.
    fn visible_frame_indices(&self) -> Vec<usize> {
        (0..self.frames.len())
            .filter(|index| !self.left_out.contains(index))
            .collect()
    }

    /// Read a click on one of the preview's tiles as a frame index.
    fn tile_click(&self, ui: &mut egui::Ui, canvas: Rect) -> Option<usize> {
        let paper = paper_rect(
            canvas,
            self.settings.paper_width as f32,
            self.settings.paper_height as f32,
        );
        let inner = paper.shrink(14.0);
        let columns = 3;
        let gap = 8.0;
        let tile_w = (inner.width() - gap * (columns as f32 - 1.0)) / columns as f32;
        let tile_h = (inner.height() * 0.45 - gap) / 2.0;
        let mut picked = None;
        for index in self.visible_frame_indices().into_iter().take(5) {
            let column = index % columns;
            let row = index / columns;
            let tile = Rect::from_min_size(
                egui::pos2(
                    inner.left() + column as f32 * (tile_w + gap),
                    inner.top() + row as f32 * (tile_h + gap),
                ),
                Vec2::new(tile_w, tile_h),
            );
            if ui
                .interact(
                    tile,
                    ui.id().with(("sheet-tile", index)),
                    egui::Sense::click(),
                )
                .clicked()
            {
                picked = Some(index);
            }
        }
        picked
    }

    fn show_navigation(&mut self, ui: &mut egui::Ui, canvas: Rect, theme: &Theme) {
        let y = canvas.bottom() - 20.0;
        let previous = button(
            ui,
            Rect::from_center_size(
                egui::pos2(canvas.center().x - 56.0, y),
                Vec2::new(36.0, 28.0),
            ),
            Icon::ChevronLeft.glyph(),
        );
        if previous {
            self.page = self.page.saturating_sub(1);
            self.changed = true;
        }
        ui.painter().text(
            egui::pos2(canvas.center().x, y),
            Align2::CENTER_CENTER,
            format!("Sheet {} of {}", self.page + 1, self.page_count.max(1)),
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        let next = button(
            ui,
            Rect::from_center_size(
                egui::pos2(canvas.center().x + 56.0, y),
                Vec2::new(36.0, 28.0),
            ),
            Icon::ChevronRight.glyph(),
        );
        if next {
            if self.page + 1 < self.page_count {
                self.page += 1;
            }
            self.changed = true;
        }
    }

    fn show_controls(&mut self, ui: &mut egui::Ui, panel: Rect, theme: &Theme) -> SheetActions {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(panel)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_width(panel.width());
        let columns = RowColumns::new();
        field_row(&mut child, columns, "Paper", |ui| {
            ui.label(format!(
                "{:.1} × {:.1} cm",
                self.settings.paper_width / 10.0,
                self.settings.paper_height / 10.0
            ));
        });
        field_row(&mut child, columns, "DPI", |ui| {
            if ui
                .add(egui::Button::new(self.settings.dpi.to_string()))
                .clicked()
            {
                let index = DPI_CHOICES
                    .iter()
                    .position(|dpi| *dpi == self.settings.dpi)
                    .unwrap_or(0);
                self.settings.dpi = DPI_CHOICES[(index + 1) % DPI_CHOICES.len()];
                self.changed = true;
            }
        });
        child.add_space(f32::from(theme.space_md));
        let mut changed = false;
        changed |=
            labeled_toggle(&mut child, "Film Edges", &mut self.settings.film_edges).clicked();
        changed |=
            labeled_toggle(&mut child, "Edge Print", &mut self.settings.edge_print).clicked();
        changed |=
            labeled_toggle(&mut child, "White Paper", &mut self.settings.white_paper).clicked();
        changed |=
            labeled_toggle(&mut child, "Roll Label", &mut self.settings.roll_label).clicked();
        if changed {
            self.changed = true;
        }
        child.add_space(f32::from(theme.space_md));
        if labeled_toggle(&mut child, "Pick Frames", &mut self.picking).clicked() {
            self.changed = true;
        }
        if self.picking {
            let _ = hint(
                &mut child,
                "Clear a frame to leave it off the sheet; select it to put it back.",
            );
            for (index, name) in self.frames.clone().iter().enumerate() {
                let mut included = !self.left_out.contains(&index);
                if labeled_toggle(&mut child, name, &mut included).clicked() {
                    if included {
                        self.left_out.remove(&index);
                    } else {
                        self.left_out.insert(index);
                    }
                    self.changed = true;
                }
            }
            if !self.left_out.is_empty() {
                let left_out = format!(
                    "{} left out",
                    crate::view::rail::count_of(self.left_out.len(), "frame")
                );
                let _ = hint(&mut child, &left_out);
            }
        }
        if !self.status.is_empty() {
            child.label(&self.status);
        }

        let footer = Rect::from_min_size(
            egui::pos2(panel.left(), panel.bottom() - DEFAULT_BUTTON_HEIGHT),
            Vec2::new(panel.width(), DEFAULT_BUTTON_HEIGHT),
        );
        let cancel = Rect::from_min_size(
            egui::pos2(
                footer.right() - 2.0 * 72.0 - f32::from(theme.space_md),
                footer.top(),
            ),
            Vec2::new(72.0, DEFAULT_BUTTON_HEIGHT),
        );
        let export = Rect::from_min_size(
            egui::pos2(footer.right() - 72.0, footer.top()),
            Vec2::new(72.0, DEFAULT_BUTTON_HEIGHT),
        );
        SheetActions {
            close: button(ui, cancel, "Cancel"),
            export: primary_button(ui, export, "Export"),
            refresh: self.changed,
            tile: None,
        }
    }
}

fn paper_rect(canvas: Rect, width_mm: f32, height_mm: f32) -> Rect {
    let pad = 28.0;
    let scale = ((canvas.width() - 2.0 * pad) / width_mm)
        .min((canvas.height() - 2.0 * pad) / height_mm)
        .max(0.05);
    Rect::from_center_size(
        canvas.center(),
        Vec2::new(width_mm * scale, height_mm * scale),
    )
}

fn button(ui: &mut egui::Ui, rect: Rect, label: &str) -> bool {
    let theme = Theme::default();
    ui.painter().rect_filled(rect, 4.0, theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
    ui.allocate_rect(rect, egui::Sense::click()).clicked()
}

fn primary_button(ui: &mut egui::Ui, rect: Rect, label: &str) -> bool {
    let theme = Theme::default();
    ui.painter().rect_filled(rect, 4.0, theme.bg_primary);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_checked_hover),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_on_accent,
    );
    ui.allocate_rect(rect, egui::Sense::click()).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ContactSheetDialog::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let actions = state.show(ui, ui.max_rect(), &[]);
                assert!(!actions.export && !actions.close);
            });
        });
    }

    #[test]
    fn a_page_change_marks_the_dialog_for_a_render() {
        let mut state = ContactSheetDialog::default();
        state.changed = false;
        state.page_count = 2;
        state.page = 0;
        state.changed = true;
        assert!(state.changed);
    }

    #[test]
    fn next_advances_until_the_last_page() {
        let ctx = egui::Context::default();
        let mut state = ContactSheetDialog::default();
        state.page_count = 2;
        state.page = 0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, ui.max_rect(), &[]);
            });
        });
        assert_eq!(state.page, 0);
        state.page = 1;
        state.changed = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, ui.max_rect(), &[]);
            });
        });
        assert_eq!(state.page, 1);
    }

    #[test]
    fn a_left_out_frame_is_not_a_visible_tile() {
        let mut state = ContactSheetDialog::default();
        state.frames = vec!["a".into(), "b".into(), "c".into()];
        state.left_out.insert(1);
        assert_eq!(state.visible_frame_indices(), vec![0, 2]);
    }

    #[test]
    fn left_out_starts_from_the_rejected_marks() {
        let mut state = ContactSheetDialog::default();
        state.left_out.extend([0, 2]);
        state.seed_excluded(&[true, false, true]);
        assert_eq!(
            state.left_out.iter().copied().collect::<Vec<_>>(),
            vec![0, 2]
        );
    }

    #[test]
    fn a_left_out_frame_stays_out_of_a_shorter_pick_list() {
        let mut state = ContactSheetDialog::default();
        state.left_out.extend([1, 4]);
        let frames = ["a".to_string(), "b".to_string(), "c".to_string()];
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, ui.max_rect(), &frames);
            });
        });
        assert!(state.left_out.contains(&1));
        assert!(!state.left_out.contains(&4));
    }
}
