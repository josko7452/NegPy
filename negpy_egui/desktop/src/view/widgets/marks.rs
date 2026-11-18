//! A triage badge: a check for a keeper, a cross for a rejected frame.
//!
//! Every badge sits on the same neutral chip, because red on a frame would read as
//! the selection or a failure.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, Vec2};

use crate::styles::icons::Icon;
use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

/// The badge's radius.
pub const BADGE_RADIUS: f32 = 9.0;

/// The chip a badge sits on.
#[must_use]
pub fn chip_fill() -> Color32 {
    with_alpha(Theme::default().mark_shadow, 190)
}

/// The ring around a badge chip.
#[must_use]
pub fn chip_ring() -> Color32 {
    with_alpha(Theme::default().mark_photo, 90)
}

/// The badge's center, inset from the image's lower right corner.
#[must_use]
pub fn badge_center(image: Rect) -> egui::Pos2 {
    egui::pos2(
        image.right() - BADGE_RADIUS - 4.0,
        image.bottom() - BADGE_RADIUS - 4.0,
    )
}

/// Paint a triage badge on `image`.
pub fn draw_mark_badge(painter: &egui::Painter, image: Rect, check: bool) {
    let theme = Theme::default();
    let center = badge_center(image);
    painter.circle_filled(center, BADGE_RADIUS, chip_fill());
    painter.circle_stroke(center, BADGE_RADIUS, Stroke::new(1.0_f32, chip_ring()));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        if check {
            Icon::Check.glyph()
        } else {
            Icon::Close.glyph()
        },
        FontId::proportional(10.0),
        theme.text_primary,
    );
}

/// The badge's rectangle, for hit testing and tests.
#[must_use]
pub fn badge_rect(image: Rect) -> Rect {
    Rect::from_center_size(badge_center(image), Vec2::splat(BADGE_RADIUS * 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_badge_sits_inside_the_image() {
        let image = Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(200.0, 100.0));
        let badge = badge_rect(image);
        assert!(image.contains_rect(badge));
    }

    #[test]
    fn a_badge_chip_is_translucent_shadow() {
        assert_eq!(chip_fill().a(), 190);
        assert_eq!(chip_fill().r(), 0);
    }
}
