//! The device singleton opens one adapter and reports what it got.

use negpy_backend::infrastructure::gpu::device::device;

#[test]
fn the_device_singleton_is_stable() {
    let first = device();
    let second = device();
    match (first, second) {
        (Some(a), Some(b)) => {
            assert!(
                std::sync::Arc::ptr_eq(&a, &b),
                "the singleton must not reopen"
            );
            assert!(!a.backend_name().is_empty());
            assert!(!a.info.name.is_empty());
        }
        (None, None) => {}
        _ => panic!("the second call changed the device outcome"),
    }
}
