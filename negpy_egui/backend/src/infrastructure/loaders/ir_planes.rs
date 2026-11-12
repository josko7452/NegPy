//! Reading an IR plane from a TIFF-structured container.

/// Normalize one IR plane to float32 in 0..1.
#[must_use]
pub fn normalize_ir(samples: &[u16], max: u16) -> Vec<f32> {
    let divisor = f32::from(max.max(1));
    samples
        .iter()
        .map(|sample| (f32::from(*sample) / divisor).clamp(0.0, 1.0))
        .collect()
}

/// A candidate IR page's dimensions and markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrPage {
    pub width: usize,
    pub height: usize,
    pub new_subfile_type: Option<u32>,
    pub photometric: Option<u16>,
}

const TRANSPARENCY_MASK: u32 = 4;
const MIN_IS_BLACK: u16 = 1;

/// Whether a page at the main image's dimensions reads as an IR plane.
#[must_use]
pub fn is_ir_page(page: IrPage, main_width: usize, main_height: usize) -> bool {
    if page.width != main_width || page.height != main_height {
        return false;
    }
    page.new_subfile_type == Some(TRANSPARENCY_MASK) || page.photometric == Some(MIN_IS_BLACK)
}

/// The first exact-size page that reads as an IR plane.
#[must_use]
pub fn find_ir_plane(page: &[IrPage], main_width: usize, main_height: usize) -> Option<usize> {
    page.iter()
        .position(|page| is_ir_page(*page, main_width, main_height))
}
