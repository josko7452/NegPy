//! The named icon map over the bundled Phosphor set.
//!
//! A caller asks for a role, so a glyph swap happens here and not in every view. The
//! names follow the action, not the drawing.

use eframe::egui::FontDefinitions;
use egui_phosphor::regular as phosphor;

/// The icon roles the capture views use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Search,
    Sliders,
    Gear,
    Image,
    Film,
    Metadata,
    Export,
    Scan,
    Camera,
    CameraRotate,
    Exchange,
    Plug,
    Crop,
    Lightbulb,
    FolderOpen,
    Info,
    Close,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Print,
    WhitePaper,
    Barcode,
    Tag,
    PickFrames,
    TurnPage,
    Help,
    Check,
    Circle,
    Folder,
    Stack,
    Grid,
    Menu,
    Reset,
    ArrowLeft,
    ArrowRight,
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
    Undo,
    Redo,
    Wrench,
    GridFour,
    LockOpen,
    LockSimple,
    ArrowsHorizontal,
    ArrowsVertical,
    ListDashes,
    Crosshair,
    More,
}

impl Icon {
    /// The Phosphor glyph for this role.
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Search => phosphor::MAGNIFYING_GLASS,
            Self::Sliders => phosphor::SLIDERS,
            Self::Gear => phosphor::GEAR,
            Self::Image => phosphor::IMAGE,
            Self::Film => phosphor::FILM_STRIP,
            Self::Metadata => phosphor::NOTE,
            Self::Export => phosphor::EXPORT,
            Self::Scan => phosphor::SCAN,
            Self::Camera => phosphor::CAMERA,
            Self::CameraRotate => phosphor::CAMERA_ROTATE,
            Self::Exchange => phosphor::ARROWS_LEFT_RIGHT,
            Self::Plug => phosphor::PLUG,
            Self::Crop => phosphor::CROP,
            Self::Lightbulb => phosphor::LIGHTBULB,
            Self::FolderOpen => phosphor::FOLDER_OPEN,
            Self::Info => phosphor::INFO,
            Self::Close => phosphor::X,
            Self::ChevronLeft => phosphor::CARET_LEFT,
            Self::ChevronRight => phosphor::CARET_RIGHT,
            Self::ChevronDown => phosphor::CARET_DOWN,
            Self::Print => phosphor::PRINTER,
            Self::WhitePaper => phosphor::FILE,
            Self::Barcode => phosphor::BARCODE,
            Self::Tag => phosphor::TAG,
            Self::PickFrames => phosphor::X_CIRCLE,
            Self::TurnPage => phosphor::ARROW_CLOCKWISE,
            Self::Help => phosphor::QUESTION,
            Self::Check => phosphor::CHECK,
            Self::Circle => phosphor::CIRCLE,
            Self::Folder => phosphor::FOLDER,
            Self::Stack => phosphor::STACK,
            Self::Grid => phosphor::SQUARES_FOUR,
            Self::Menu => phosphor::DOTS_THREE,
            Self::Reset => phosphor::ARROW_COUNTER_CLOCKWISE,
            Self::ArrowLeft => phosphor::ARROW_LEFT,
            Self::ArrowRight => phosphor::ARROW_RIGHT,
            Self::RotateLeft => phosphor::ARROW_COUNTER_CLOCKWISE,
            Self::RotateRight => phosphor::ARROW_CLOCKWISE,
            Self::FlipHorizontal => phosphor::FLIP_HORIZONTAL,
            Self::FlipVertical => phosphor::FLIP_VERTICAL,
            Self::Undo => phosphor::ARROW_U_UP_LEFT,
            Self::Redo => phosphor::ARROW_U_UP_RIGHT,
            Self::Wrench => phosphor::WRENCH,
            Self::GridFour => phosphor::SQUARES_FOUR,
            Self::LockOpen => phosphor::LOCK_OPEN,
            Self::LockSimple => phosphor::LOCK_SIMPLE,
            Self::ArrowsHorizontal => phosphor::ARROWS_HORIZONTAL,
            Self::ArrowsVertical => phosphor::ARROWS_VERTICAL,
            Self::ListDashes => phosphor::LIST_DASHES,
            Self::Crosshair => phosphor::CROSSHAIR,
            Self::More => phosphor::DOTS_THREE,
        }
    }
}

/// A definitions set with no family override, for the icon test.
#[cfg(test)]
fn empty_fonts() -> FontDefinitions {
    eframe::egui::FontDefinitions::default()
}

/// Add the Phosphor font to a definitions set, after the UI font.
pub fn install(fonts: &mut FontDefinitions) {
    egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_a_non_empty_glyph() {
        for icon in [
            Icon::Search,
            Icon::Sliders,
            Icon::Gear,
            Icon::Image,
            Icon::Film,
            Icon::Metadata,
            Icon::Export,
            Icon::Scan,
            Icon::Camera,
            Icon::CameraRotate,
            Icon::Exchange,
            Icon::Plug,
            Icon::Crop,
            Icon::Lightbulb,
            Icon::FolderOpen,
            Icon::Info,
            Icon::Close,
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::Print,
            Icon::WhitePaper,
            Icon::Barcode,
            Icon::Tag,
            Icon::PickFrames,
            Icon::TurnPage,
            Icon::Help,
            Icon::Check,
            Icon::Circle,
            Icon::Folder,
            Icon::Reset,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::RotateLeft,
            Icon::RotateRight,
            Icon::FlipHorizontal,
            Icon::FlipVertical,
            Icon::Undo,
            Icon::Redo,
            Icon::Wrench,
            Icon::GridFour,
        ] {
            assert!(!icon.glyph().is_empty());
        }
    }

    #[test]
    fn the_phosphor_font_joins_the_proportional_family() {
        let mut fonts = crate::styles::icons::empty_fonts();
        install(&mut fonts);
        assert!(fonts.font_data.contains_key("phosphor"));
    }
}
