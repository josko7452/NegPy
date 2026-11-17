//! Color-vision palettes for views that tell marks apart by hue.
//!
//! A view asks for a role, never a fixed hue. Standard vision uses the neon pair;
//! the other choices use Okabe-Ito colors on an axis that vision keeps.

use eframe::egui::Color32;

use super::theme::Theme;

/// One color-vision choice and the roles it fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisionPalette {
    pub key: &'static str,
    pub label: &'static str,
    pub pair: (Color32, Color32),
    pub dodge_burn: (Color32, Color32),
    pub zone_ramp: [Color32; 9],
}

/// Every choice in Preferences order.
pub const PALETTES: [VisionPalette; 4] = [
    VisionPalette {
        key: "standard",
        label: "Standard",
        pair: (rgb(0x39, 0xff, 0x14), rgb(0xff, 0x00, 0xff)),
        dodge_burn: (Theme::DODGE, Theme::BURN),
        zone_ramp: [
            rgb(0x2d, 0x2d, 0xaf),
            rgb(0x1e, 0x69, 0xd7),
            rgb(0x00, 0xa5, 0xc3),
            rgb(0x28, 0xaf, 0x6e),
            rgb(0x8c, 0x8c, 0x8c),
            rgb(0xaf, 0xcd, 0x3c),
            rgb(0xf5, 0xd7, 0x28),
            rgb(0xfa, 0x96, 0x1e),
            rgb(0xf0, 0x5f, 0x82),
        ],
    },
    VisionPalette {
        key: "protan_deutan",
        label: "Protanopia / deuteranopia (red-green)",
        pair: (rgb(0x56, 0xb4, 0xe9), rgb(0xd5, 0x5e, 0x00)),
        dodge_burn: (rgb(0xe6, 0x9f, 0x00), rgb(0x56, 0xb4, 0xe9)),
        zone_ramp: [
            rgb(0x1b, 0x3a, 0x6b),
            rgb(0x00, 0x72, 0xb2),
            rgb(0x3f, 0x98, 0xd0),
            rgb(0xa6, 0xce, 0xe8),
            rgb(0x8c, 0x8c, 0x8c),
            rgb(0xf7, 0xe8, 0xa0),
            rgb(0xf0, 0xe4, 0x42),
            rgb(0xe6, 0x9f, 0x00),
            rgb(0xa8, 0x64, 0x00),
        ],
    },
    VisionPalette {
        key: "tritan",
        label: "Tritanopia (blue-yellow)",
        pair: (rgb(0xd5, 0x5e, 0x00), rgb(0x00, 0x9e, 0x73)),
        dodge_burn: (rgb(0xd5, 0x5e, 0x00), rgb(0x00, 0x9e, 0x73)),
        zone_ramp: [
            rgb(0x00, 0x4d, 0x40),
            rgb(0x00, 0x9e, 0x73),
            rgb(0x4d, 0xb8, 0x9a),
            rgb(0xa8, 0xdc, 0xc9),
            rgb(0x8c, 0x8c, 0x8c),
            rgb(0xf6, 0xc1, 0xb0),
            rgb(0xe8, 0x8b, 0x6b),
            rgb(0xd5, 0x5e, 0x00),
            rgb(0x8f, 0x3a, 0x00),
        ],
    },
    VisionPalette {
        key: "achromat",
        label: "Achromatopsia (no color)",
        pair: (rgb(0xff, 0xff, 0xff), rgb(0x8c, 0x8c, 0x8c)),
        dodge_burn: (rgb(0xff, 0xff, 0xff), rgb(0x8c, 0x8c, 0x8c)),
        zone_ramp: [
            rgb(0x1e, 0x1e, 0x1e),
            rgb(0x3c, 0x3c, 0x3c),
            rgb(0x5a, 0x5a, 0x5a),
            rgb(0x73, 0x73, 0x73),
            rgb(0x8c, 0x8c, 0x8c),
            rgb(0xa5, 0xa5, 0xa5),
            rgb(0xbe, 0xbe, 0xbe),
            rgb(0xd7, 0xd7, 0xd7),
            rgb(0xeb, 0xeb, 0xeb),
        ],
    },
];

impl VisionPalette {
    /// The pair the Clipping Overlay paints: clipped highlights, then shadows.
    #[must_use]
    pub fn clipping(self) -> (Color32, Color32) {
        self.pair
    }
}

/// The palette for a stored choice, or Standard for an unknown key.
#[must_use]
pub fn palette_for(key: &str) -> VisionPalette {
    PALETTES
        .iter()
        .copied()
        .find(|palette| palette.key == key)
        .unwrap_or(PALETTES[0])
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_has_a_unique_key_and_nine_zone_colors() {
        for (index, palette) in PALETTES.iter().enumerate() {
            assert_eq!(palette.zone_ramp.len(), 9);
            assert!(
                PALETTES[..index]
                    .iter()
                    .all(|other| other.key != palette.key)
            );
        }
    }

    #[test]
    fn an_unknown_key_falls_back_to_standard() {
        assert_eq!(palette_for("missing").key, "standard");
        assert_eq!(palette_for("tritan").key, "tritan");
    }

    #[test]
    fn every_dodge_and_burn_reads_as_text_on_the_panel() {
        let background = Theme::default().bg_panel;
        for palette in PALETTES {
            for color in [palette.dodge_burn.0, palette.dodge_burn.1] {
                assert!(Theme::contrast(color, background) >= 3.0, "{}", palette.key);
            }
        }
    }
}
