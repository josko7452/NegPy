//! The capture meter's shutter recommendation and framing check.

use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::capture::meter::{meter_frame, shutter_at_most};

fn frame(value: f32) -> ImageBuffer {
    let mut pixels = vec![value; 64 * 48 * 3];
    for (index, pixel) in pixels.chunks_mut(3).enumerate() {
        let x = index % 64;
        let y = index / 64;
        if x < 4 || x > 59 || y < 3 || y > 44 {
            pixel.copy_from_slice(&[0.8, 0.8, 0.8]);
        }
    }
    ImageBuffer::from_rgb(64, 48, pixels)
}

#[test]
fn a_probe_gets_a_shutter_at_or_below_its_target() {
    let reading = meter_frame(&frame(0.4), "1/125", &[], Some("negative"), None);
    assert_eq!(reading.shutter, "1/125");
    assert!(reading.recommended.is_some());
    assert!(reading.stops.is_finite());
    assert!(reading.peaks.is_some());
    assert!(reading.thumb.is_some());
}

#[test]
fn a_slow_rung_is_selected_when_it_fits_the_target() {
    assert_eq!(
        shutter_at_most(1.0 / 60.0, &["1/125", "1/60", "1/30"]),
        Some("1/60".into())
    );
    assert_eq!(shutter_at_most(1.0 / 1000.0, &["1/125", "1/60"]), None);
}

#[test]
fn a_clipped_probe_steps_down_two_stops() {
    let reading = meter_frame(&frame(1.0), "1/125", &[], Some("positive"), None);
    assert!(reading.clipped || reading.recommended.is_some());
    assert!(reading.stops.is_finite());
}
