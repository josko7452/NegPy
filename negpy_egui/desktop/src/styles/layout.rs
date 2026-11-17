//! Control geometry and gradient sweeps from `templates.py`.
//!
//! Panels share these sizes, so a control in one card lines up with the same control
//! in another. A private width or height needs the user's agreement first.

use eframe::egui::Color32;

use super::theme::Theme;

/// One width for every icon-only button.
pub const ICON_BUTTON_WIDTH: f32 = 36.0;
/// Label column beside a combo or entry.
pub const FIELD_LABEL_WIDTH: f32 = 90.0;
/// The taller Scan buttons.
pub const SCAN_BUTTON_HEIGHT: f32 = 40.0;
/// Section-header button and icon sizes.
pub const HEADER_BUTTON_SIZE: f32 = 24.0;
pub const HEADER_ICON_SIZE: f32 = 13.0;
pub const HEADER_HEIGHT: f32 = 40.0;
/// Section toolbars: Film Strip and Library.
pub const TOOLBAR_BUTTON_HEIGHT: f32 = 28.0;
pub const TOOLBAR_ICON_SIZE: f32 = 16.0;
/// The canvas bar's control height.
pub const BAR_BUTTON_HEIGHT: f32 = 36.0;
/// A track's edge beyond its buttons.
pub const BAR_TRACK_PAD: f32 = 2.5;
/// The controls inside a canvas-bar track.
pub const BAR_TRACK_BUTTON_HEIGHT: f32 = BAR_BUTTON_HEIGHT - 5.0;
/// The canvas strip and the Find row.
pub const TOP_ROW_HEIGHT: f32 = BAR_BUTTON_HEIGHT + 2.0 * 4.0;
/// Canvas-bar icons.
pub const BAR_ICON_SIZE: f32 = 18.0;

/// The default row-button height, matching the Python font and padding.
pub const DEFAULT_BUTTON_HEIGHT: f32 = 28.0;

/// A gradient icon's fill: stops evenly spaced along an angle in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    colors: [Color32; 3],
    count: usize,
    pub angle: f32,
}

impl Sweep {
    /// A left-to-right sweep.
    #[must_use]
    pub const fn horizontal<const N: usize>(colors: [Color32; N]) -> Self {
        let mut stops = [Color32::TRANSPARENT; 3];
        let mut index = 0;
        while index < N {
            stops[index] = colors[index];
            index += 1;
        }
        Self {
            colors: stops,
            count: N,
            angle: 0.0,
        }
    }

    /// A top-to-bottom sweep.
    #[must_use]
    pub const fn vertical<const N: usize>(colors: [Color32; N]) -> Self {
        let mut stops = [Color32::TRANSPARENT; 3];
        let mut index = 0;
        while index < N {
            stops[index] = colors[index];
            index += 1;
        }
        Self {
            colors: stops,
            count: N,
            angle: 90.0,
        }
    }

    /// The stops this sweep paints, in order.
    #[must_use]
    pub fn stops(&self) -> &[Color32] {
        &self.colors[..self.count]
    }
}

const RGB_STOPS: [Color32; 3] = [
    Theme::CHANNEL_RED_TEXT,
    Theme::CHANNEL_GREEN_TEXT,
    Theme::CHANNEL_BLUE_TEXT,
];
const RING_AROUND_STOPS: [Color32; 2] = [Theme::FILTER_MAGENTA, Theme::FILTER_YELLOW];
const TEST_STRIP_STOPS: [Color32; 2] = [Theme::CANVAS_BG_WHITE, Theme::MODE_BW];
const PAPER_STRIP_STOPS: [Color32; 2] = [Theme::CANVAS_BG_WHITE, Theme::MODE_C41];

/// Red, green and blue: a tool that works on the whole image.
pub const RGB_SWEEP: Sweep = Sweep::horizontal(RGB_STOPS);
/// The two filters a ring-around varies.
pub const RING_AROUND_SWEEP: Sweep = Sweep::horizontal(RING_AROUND_STOPS);
/// A test strip, light to dark across the diagonal.
pub const TEST_STRIP_SWEEP: Sweep = Sweep {
    colors: [
        TEST_STRIP_STOPS[0],
        TEST_STRIP_STOPS[1],
        Color32::TRANSPARENT,
    ],
    count: 2,
    angle: 45.0,
};
/// A paper test strip, paper white to a warm print.
pub const PAPER_STRIP_SWEEP: Sweep = Sweep::horizontal(PAPER_STRIP_STOPS);

/// The Clipping Overlay's highlight, then shadow, colors.
#[must_use]
pub fn clipping_sweep(pair: (Color32, Color32)) -> Sweep {
    let shadow = pair.1;
    let highlight = pair.0;
    Sweep {
        colors: [highlight, shadow, Color32::TRANSPARENT],
        count: 2,
        angle: 0.0,
    }
}

impl Theme {
    /// The red channel's text tier.
    pub const CHANNEL_RED_TEXT: Color32 = Color32::from_rgb(0xff, 0x68, 0x68);
    /// The green channel's text tier.
    pub const CHANNEL_GREEN_TEXT: Color32 = Color32::from_rgb(0x5a, 0xdc, 0x78);
    /// The blue channel's text tier.
    pub const CHANNEL_BLUE_TEXT: Color32 = Color32::from_rgb(0x63, 0x98, 0xff);
    /// The subtractive cyan filter.
    pub const FILTER_MAGENTA: Color32 = Color32::from_rgb(0xb1, 0x00, 0xb1);
    /// The subtractive yellow filter.
    pub const FILTER_YELLOW: Color32 = Color32::from_rgb(0xb1, 0xb1, 0x00);
    /// The light canvas swatch.
    pub const CANVAS_BG_WHITE: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
    /// The neutral silver of black-and-white film.
    pub const MODE_BW: Color32 = Color32::from_rgb(0x8c, 0x8c, 0x8c);
    /// The orange mask of color negative film.
    pub const MODE_C41: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x3c);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canvas_bar_keeps_its_track_inside_its_height() {
        let track = BAR_TRACK_BUTTON_HEIGHT;
        let bar = BAR_BUTTON_HEIGHT;
        assert!(track < bar);
        assert_eq!(TOP_ROW_HEIGHT, 44.0);
    }

    #[test]
    fn named_sweeps_keep_their_stops_and_direction() {
        assert_eq!(RGB_SWEEP.stops().len(), 3);
        assert_eq!(TEST_STRIP_SWEEP.angle, 45.0);
        assert_eq!(RING_AROUND_SWEEP.stops()[0], Theme::FILTER_MAGENTA);
    }

    #[test]
    fn a_clipping_sweep_puts_the_highlight_first() {
        let sweep = clipping_sweep((Color32::WHITE, Color32::BLACK));
        assert_eq!(sweep.stops()[0], Color32::WHITE);
        assert_eq!(sweep.stops()[1], Color32::BLACK);
    }
}
