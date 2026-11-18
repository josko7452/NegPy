//! A tab strip that keeps one tab visible when the rest spill.
//!
//! The strip places its tabs by hand so its minimum width is one tab, not the sum of
//! them. A pinned tab stays in view; the rest pass into the » menu.

use eframe::egui::{self, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::theme::Theme;

/// The overflow button's width.
pub const OVERFLOW_WIDTH: f32 = 30.0;

/// The tabs a strip shows at a width, with the pinned tab kept in view.
#[must_use]
pub fn visible_tabs(count: usize, pinned: Option<usize>, room: f32, min_item: f32) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    if room >= min_item * count as f32 {
        return (0..count).collect();
    }
    let fits = (((room - OVERFLOW_WIDTH) / min_item).floor() as usize).clamp(1, count);
    if let Some(pinned) = pinned
        && pinned >= fits
    {
        let mut visible: Vec<usize> = (0..fits - 1).collect();
        visible.push(pinned);
        return visible;
    }
    (0..fits).collect()
}

/// The indices a strip hides at a width.
#[must_use]
pub fn hidden_tabs(count: usize, pinned: Option<usize>, room: f32, min_item: f32) -> Vec<usize> {
    let visible = visible_tabs(count, pinned, room, min_item);
    (0..count)
        .filter(|index| !visible.contains(index))
        .collect()
}

/// What a strip did this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverflowResult {
    pub picked: Option<usize>,
    pub overflow: bool,
    pub hidden: Vec<usize>,
}

/// Paint a strip and return the clicked tab, or the overflow button.
pub fn overflow_bar(
    ui: &mut Ui,
    tabs: &[crate::view::widgets::panel_tab::PanelTab],
    current: usize,
    height: f32,
) -> OverflowResult {
    let theme = Theme::default();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(rect, f32::from(theme.radius_lg), theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        f32::from(theme.radius_lg),
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    let visible = visible_tabs(tabs.len(), Some(current), rect.width(), 36.0);
    let hidden = hidden_tabs(tabs.len(), Some(current), rect.width(), 36.0);
    let room = rect.width()
        - if hidden.is_empty() {
            0.0
        } else {
            OVERFLOW_WIDTH
        };
    let width = if visible.is_empty() {
        0.0
    } else {
        room / visible.len() as f32
    };
    let mut x = rect.left();
    let mut picked = None;
    for index in visible {
        let tab_rect =
            Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(width, rect.height()));
        if tabs[index]
            .show_in(ui, tab_rect, index == current)
            .clicked()
        {
            picked = Some(index);
        }
        x += width;
    }
    let mut overflow = false;
    if !hidden.is_empty() {
        let more = Rect::from_min_size(
            egui::pos2(rect.right() - OVERFLOW_WIDTH, rect.top()),
            Vec2::new(OVERFLOW_WIDTH, rect.height()),
        );
        let response = ui.interact(more, ui.id().with("overflow"), Sense::click());
        ui.painter().text(
            more.center(),
            egui::Align2::CENTER_CENTER,
            Icon::ChevronRight.glyph(),
            egui::FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_secondary,
        );
        overflow = response.clicked();
    }
    OverflowResult {
        picked,
        overflow,
        hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_shows_when_the_strip_has_room() {
        assert_eq!(visible_tabs(4, Some(0), 400.0, 36.0), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_pinned_tab_stays_when_the_strip_is_narrow() {
        let visible = visible_tabs(6, Some(5), 140.0, 36.0);
        assert!(visible.contains(&5));
        assert!(!hidden_tabs(6, Some(5), 140.0, 36.0).contains(&5));
    }

    #[test]
    fn a_narrow_room_keeps_one_tab_and_the_overflow_button() {
        assert_eq!(visible_tabs(3, None, 40.0, 36.0).len(), 1);
        assert_eq!(hidden_tabs(3, None, 40.0, 36.0).len(), 2);
    }
}
