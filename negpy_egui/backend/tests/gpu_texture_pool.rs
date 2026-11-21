//! The texture pool reuses a texture of matching size.

use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::TexturePool;

#[test]
fn a_returned_texture_is_taken_again() {
    let Some(gpu) = device() else {
        return;
    };
    let pool = TexturePool::default();
    let first = pool.take(&gpu.device, 16, 12, "first");
    let id = first.pool_id;
    pool.put(first);
    let second = pool.take(&gpu.device, 16, 12, "second");
    assert_eq!(second.pool_id, id);
    assert_eq!((second.width, second.height), (16, 12));
}

#[test]
fn a_different_size_allocates_a_new_texture() {
    let Some(gpu) = device() else {
        return;
    };
    let pool = TexturePool::default();
    let first = pool.take(&gpu.device, 16, 12, "first");
    let id = first.pool_id;
    pool.put(first);
    let second = pool.take(&gpu.device, 8, 8, "second");
    assert_ne!(second.pool_id, id);
}
