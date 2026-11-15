//! The sheet-side frame turn rule from `negpy/services/export/contact_sheet_layout.py`.

use crate::services::film_edge::geometry::FilmGeometry;

/// `np.rot90` k that lays an upright frame on the strip as it lies on the film.
///
/// A frame scanned upright turns clockwise, as a vertical shot held release-up lies on the film.
#[must_use]
pub fn frame_turns(
    upright_aspect: Option<f64>,
    geometry: &FilmGeometry,
    rotation: i64,
    flip_h: bool,
    flip_v: bool,
) -> i64 {
    let Some(aspect) = upright_aspect else {
        return 0;
    };
    if aspect.ln().abs() < 0.02 || geometry.window_aspect().ln().abs() < 0.02 {
        return 0;
    }
    if (aspect > 1.0) == (geometry.window_aspect() > 1.0) {
        return 0;
    }
    let k = rotation.rem_euclid(4);
    if k % 2 != 0 {
        if flip_h != flip_v {
            k
        } else {
            (-k).rem_euclid(4)
        }
    } else {
        3
    }
}
