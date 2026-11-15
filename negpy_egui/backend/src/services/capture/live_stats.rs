//! Clipping masks for the camera live view.

pub const ZEBRA_HIGH: u8 = 253;
pub const ZEBRA_LOW: u8 = 2;

/// `(highlights, blacks)` masks for an HxWx3 uint8 frame.
#[must_use]
pub fn clip_masks(rgb: &[u8], width: usize, height: usize) -> (Vec<bool>, Vec<bool>) {
    let mut highlights = vec![false; width * height];
    let mut blacks = vec![false; width * height];
    for (index, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
        let top = pixel[0].max(pixel[1]).max(pixel[2]);
        highlights[index] = top >= ZEBRA_HIGH;
        blacks[index] = top <= ZEBRA_LOW;
    }
    (highlights, blacks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_masks_read_the_ends_of_the_range() {
        let rgb = [0u8, 1, 2, 253, 254, 255, 0, 100, 0];
        let (high, low) = clip_masks(&rgb, 3, 1);
        assert_eq!(high, [false, true, false]);
        assert_eq!(low, [true, false, false]);
    }
}
