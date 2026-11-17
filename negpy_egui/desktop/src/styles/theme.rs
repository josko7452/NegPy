//! Theme tokens mirrored from `negpy/desktop/view/styles/theme.py`.
//!
//! Every surface, border, text and mark color lives here. A style module reads a
//! token; it never writes a literal color.

use eframe::egui::Color32;

/// The application palette and spacing scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub font_size_small: u8,
    pub font_size_base: u8,
    pub font_size_header: u8,
    pub font_size_title: u8,
    pub font_size_micro: u8,

    pub bg_dark: Color32,
    pub bg_panel: Color32,
    pub bg_input: Color32,
    pub bg_card: Color32,
    pub bg_header: Color32,
    pub bg_selected: Color32,
    pub bg_hover: Color32,
    pub bg_menu_selected: Color32,
    pub bg_checked: Color32,
    pub bg_checked_hover: Color32,
    pub border_checked: Color32,
    pub border_checked_hover: Color32,
    pub bg_primary: Color32,
    pub bg_primary_hover: Color32,
    pub border_scope_frame: Color32,
    pub mark_edited: Color32,
    pub progress_fill: Color32,
    pub bg_accent_muted: Color32,
    pub border_input: Color32,
    pub border_primary: Color32,
    pub border_color: Color32,
    pub border_hover: Color32,
    pub border_focus: Color32,
    pub border_indicator: Color32,
    pub slider_disabled: Color32,
    pub text_on_accent: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_hint: Color32,
    pub text_muted: Color32,
    pub text_unit: Color32,
    pub warn_amber: Color32,
    pub mark_photo: Color32,
    pub mark_shadow: Color32,
    pub error: Color32,
    pub error_text: Color32,

    pub space_xs: u8,
    pub space_sm: u8,
    pub space_md: u8,
    pub space_lg: u8,
    pub space_xl: u8,
    pub space_2xl: u8,
    pub radius_sm: u8,
    pub radius_md: u8,
    pub radius_lg: u8,

    pub weight_regular: u16,
    pub weight_medium: u16,
    pub weight_semibold: u16,
    pub weight_bold: u16,

    pub channel_red: Color32,
    pub channel_green: Color32,
    pub channel_blue: Color32,
    pub channel_red_text: Color32,
    pub channel_green_text: Color32,
    pub channel_blue_text: Color32,
    pub filter_cyan: Color32,
    pub filter_magenta: Color32,
    pub filter_yellow: Color32,
    pub dodge: Color32,
    pub burn: Color32,
    pub chart_mask: Color32,
    pub mode_c41: Color32,
    pub mode_bw: Color32,
    pub mode_e6: Color32,

    pub canvas_bg_black: Color32,
    pub canvas_bg_dark_grey: Color32,
    pub canvas_bg_mid_grey: Color32,
    pub canvas_bg_light_grey: Color32,
    pub canvas_bg_white: Color32,
    pub status_success: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            font_size_small: 12,
            font_size_base: 13,
            font_size_header: 14,
            font_size_title: 16,
            font_size_micro: 9,

            bg_dark: hex(0x32, 0x32, 0x32),
            bg_panel: hex(0x32, 0x32, 0x32),
            bg_input: hex(0x2f, 0x2f, 0x2f),
            bg_card: hex(0x37, 0x37, 0x37),
            bg_header: hex(0x3b, 0x3b, 0x3b),
            bg_selected: hex(0x48, 0x48, 0x48),
            bg_hover: hex(0x4c, 0x4c, 0x4c),
            bg_menu_selected: hex(0x54, 0x54, 0x54),
            bg_checked: hex(0x50, 0x42, 0x42),
            bg_checked_hover: hex(0x5b, 0x45, 0x44),
            border_checked: hex(0x71, 0x4a, 0x4a),
            border_checked_hover: hex(0x86, 0x50, 0x50),
            bg_primary: hex(0x72, 0x42, 0x41),
            bg_primary_hover: hex(0x82, 0x47, 0x47),
            border_scope_frame: hex(0x5d, 0x4b, 0x24),
            mark_edited: hex(0xc7, 0x9a, 0x3a),
            progress_fill: hex(0x9c, 0x3b, 0x3b),
            bg_accent_muted: hex(0x3f, 0x39, 0x39),
            border_input: hex(0x3f, 0x3f, 0x3f),
            border_primary: hex(0x4c, 0x4c, 0x4c),
            border_color: hex(0x59, 0x59, 0x59),
            border_hover: hex(0x60, 0x60, 0x60),
            border_focus: hex(0x89, 0x89, 0x89),
            border_indicator: hex(0x69, 0x69, 0x69),
            slider_disabled: hex(0x7c, 0x7c, 0x7c),
            text_on_accent: hex(0xff, 0xff, 0xff),
            text_primary: hex(0xe4, 0xe4, 0xe4),
            text_secondary: hex(0xbc, 0xbc, 0xbc),
            text_hint: hex(0xa4, 0xa4, 0xa4),
            text_muted: hex(0x7a, 0x7a, 0x7a),
            text_unit: hex(0x8b, 0x8b, 0x8b),
            warn_amber: hex(0xc7, 0x9a, 0x3a),
            mark_photo: hex(0xff, 0xff, 0xff),
            mark_shadow: hex(0x00, 0x00, 0x00),
            error: hex(0xd3, 0x2f, 0x2f),
            error_text: hex(0xff, 0x68, 0x68),

            space_xs: 2,
            space_sm: 4,
            space_md: 6,
            space_lg: 8,
            space_xl: 12,
            space_2xl: 16,
            radius_sm: 3,
            radius_md: 4,
            radius_lg: 6,

            weight_regular: 400,
            weight_medium: 500,
            weight_semibold: 600,
            weight_bold: 700,

            channel_red: hex(0xd3, 0x2f, 0x2f),
            channel_green: hex(0x38, 0x8e, 0x3c),
            channel_blue: hex(0x19, 0x76, 0xd2),
            channel_red_text: hex(0xff, 0x68, 0x68),
            channel_green_text: hex(0x5a, 0xdc, 0x78),
            channel_blue_text: hex(0x63, 0x98, 0xff),
            filter_cyan: hex(0x00, 0xb1, 0xb1),
            filter_magenta: hex(0xb1, 0x00, 0xb1),
            filter_yellow: hex(0xb1, 0xb1, 0x00),
            dodge: hex(0xe8, 0xc8, 0x4a),
            burn: hex(0x4a, 0x8f, 0xe8),
            chart_mask: hex(0xaf, 0x8c, 0xff),
            mode_c41: hex(0xe0, 0x8a, 0x3c),
            mode_bw: hex(0x8c, 0x8c, 0x8c),
            mode_e6: hex(0x4f, 0xb0, 0xd8),

            canvas_bg_black: hex(0x07, 0x07, 0x07),
            canvas_bg_dark_grey: hex(0x3a, 0x3a, 0x3a),
            canvas_bg_mid_grey: hex(0x6f, 0x6f, 0x6f),
            canvas_bg_light_grey: hex(0xaa, 0xaa, 0xaa),
            canvas_bg_white: hex(0xe8, 0xe8, 0xe8),
            status_success: hex(0x33, 0xad, 0x83),
        }
    }
}

impl Theme {
    /// A surface color with `alpha` applied.
    #[must_use]
    pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    }

    /// The relative luminance of a color, with the sRGB transfer.
    #[must_use]
    pub fn luminance(color: Color32) -> f32 {
        fn linear(channel: u8) -> f32 {
            let value = f32::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    /// The contrast ratio between two colors.
    #[must_use]
    pub fn contrast(a: Color32, b: Color32) -> f32 {
        let left = Self::luminance(a);
        let right = Self::luminance(b);
        (left.max(right) + 0.05) / (left.min(right) + 0.05)
    }
}

const fn hex(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contrast_helper_matches_the_srgb_reference() {
        let theme = Theme::default();
        assert!(Theme::contrast(theme.text_on_accent, theme.bg_dark) > 10.0);
    }

    #[test]
    fn readable_text_keeps_its_contrast() {
        let theme = Theme::default();
        for text in [theme.text_primary, theme.text_secondary, theme.text_hint] {
            assert!(Theme::contrast(text, theme.bg_panel) >= 4.5);
        }
    }

    #[test]
    fn the_disabled_gray_stays_below_the_reading_floor() {
        let theme = Theme::default();
        assert!(Theme::contrast(theme.text_muted, theme.bg_panel) < 4.5);
    }

    #[test]
    fn alpha_builds_a_translucent_token() {
        let color = Theme::with_alpha(Theme::default().mark_shadow, 140);
        assert_eq!(color.r(), 0);
        assert_eq!(color.a(), 140);
    }
}
