//! Mirrors `negpy/services/rendering/base_read.py`: the clear-base read, measured once per
//! source and shared by the render and its prefetch.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::features::geometry::processor::normalize_detection_input;
use crate::features::geometry::skew::AUTOCROP_DETECT_RES;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The number of source reads kept. A source evicted here is measured again on its next render.
pub const CAPACITY: usize = 64;

/// The stored value for a source: `Some(None)` is a source known to carry no base, so a
/// miss needs its own marker. A `None` key is never stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CachedBase {
    NotRead,
    Read(Option<[f64; 3]>),
}

struct Reads {
    order: VecDeque<String>,
    values: std::collections::HashMap<String, Option<[f64; 3]>>,
}

fn reads() -> &'static Mutex<Reads> {
    static READS: OnceLock<Mutex<Reads>> = OnceLock::new();
    READS.get_or_init(|| {
        Mutex::new(Reads {
            order: VecDeque::new(),
            values: Default::default(),
        })
    })
}

/// The stored read for `key`, or [`CachedBase::NotRead`]. Never measures.
#[must_use]
pub fn cached_base_log(key: Option<&str>) -> CachedBase {
    let Some(key) = key else {
        return CachedBase::NotRead;
    };
    let mut reads = reads().lock().expect("base read lock");
    let Some(value) = reads.values.get(key).copied() else {
        return CachedBase::NotRead;
    };
    reads.order.retain(|candidate| candidate != key);
    reads.order.push_back(key.to_string());
    CachedBase::Read(value)
}

/// The stored read for `key`, else a new one off `img`. A `None` key measures and stores
/// nothing, so a caller without a source identity still renders.
pub fn base_log_for(key: Option<&str>, img: &ImageBuffer) -> Option<[f64; 3]> {
    let found = cached_base_log(key);
    if let CachedBase::Read(value) = found {
        return value;
    }
    // Outside the lock: a prefetch and a render may read one source twice and do not wait.
    let value = measure_base_log(img);
    if let Some(key) = key {
        store(key, value);
    }
    value
}

/// Store one read. A cache fill is the same work whether it follows a measure or a test.
fn store(key: &str, value: Option<[f64; 3]>) {
    let mut reads = reads().lock().expect("base read lock");
    reads.values.insert(key.to_string(), value);
    reads.order.retain(|candidate| candidate != key);
    reads.order.push_back(key.to_string());
    while reads.order.len() > CAPACITY {
        if let Some(dropped) = reads.order.pop_front() {
            reads.values.remove(&dropped);
        }
    }
}

/// Drop every stored read.
pub fn clear() {
    let mut reads = reads().lock().expect("base read lock");
    reads.order.clear();
    reads.values.clear();
}

/// `measure_base_log`: per-channel log10 transmittance of the clear base on the detection
/// image. None when the scan shows no film box, too few flat blocks, or a base darker than
/// the picture.
#[must_use]
pub fn measure_base_log(image: &ImageBuffer) -> Option<[f64; 3]> {
    let (det, _) = normalize_detection_input(image, AUTOCROP_DETECT_RES as u32);
    negpy_core::analysis::measure_base_log(&det.pixels, det.height, det.width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: f32) -> ImageBuffer {
        ImageBuffer::from_rgb(40, 30, vec![value; 40 * 30 * 3])
    }

    #[test]
    fn a_keyless_read_measures_and_stores_nothing() {
        clear();
        let image = frame(0.5);
        assert_eq!(base_log_for(None, &image), measure_base_log(&image));
        assert_eq!(cached_base_log(None), CachedBase::NotRead);
        clear();
    }

    #[test]
    fn a_stored_read_comes_back_without_the_image() {
        clear();
        let value = Some([0.1, 0.2, 0.3]);
        store("source-a", value);
        assert_eq!(cached_base_log(Some("source-a")), CachedBase::Read(value));
        clear();
    }

    #[test]
    fn a_known_absence_stays_absent() {
        clear();
        store("no-base", None);
        assert_eq!(cached_base_log(Some("no-base")), CachedBase::Read(None));
        clear();
    }

    #[test]
    fn the_oldest_source_is_evicted_first() {
        clear();
        for index in 0..CAPACITY {
            store(&format!("source-{index}"), None);
        }
        store("newest", None);
        assert_eq!(cached_base_log(Some("source-0")), CachedBase::NotRead);
        assert_eq!(cached_base_log(Some("newest")), CachedBase::Read(None));
        clear();
    }
}
