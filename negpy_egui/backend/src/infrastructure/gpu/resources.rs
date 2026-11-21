//! Mirrors `negpy/infrastructure/gpu/resources.py`: the float textures and buffers a pass reads.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::{
    Buffer, BufferUsages, CommandEncoderDescriptor, Device, Extent3d, MapMode, Origin3d, Queue,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
};

use crate::infrastructure::gpu::device::GpuDevice;

/// The one format every intermediate texture uses.
pub const WORK_FORMAT: TextureFormat = TextureFormat::Rgba32Float;
/// The bytes of one texel in [`WORK_FORMAT`].
pub const TEXEL_BYTES: usize = 16;
/// `copy_texture_to_buffer` requires a 256-byte row stride.
const ROW_ALIGN: usize = 256;

/// The texture pool: one row per `(width, height)` the chain has used.
#[derive(Default)]
pub struct TexturePool {
    idle: Mutex<Vec<GpuTexture>>,
    next_id: AtomicU64,
}

impl TexturePool {
    /// Take a texture of this size from the pool, or allocate one.
    #[must_use]
    pub fn take(&self, device: &Device, width: u32, height: u32, label: &str) -> GpuTexture {
        let mut idle = self.idle.lock().expect("texture pool poisoned");
        let index = idle
            .iter()
            .position(|texture| texture.width == width && texture.height == height);
        if let Some(index) = index {
            return idle.swap_remove(index);
        }
        let mut texture = GpuTexture::new(device, width, height, label);
        texture.pool_id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        texture
    }

    /// Return a texture after its pass has finished.
    pub fn put(&self, texture: GpuTexture) {
        let mut idle = self.idle.lock().expect("texture pool poisoned");
        if idle.len() < 16 {
            idle.push(texture);
        }
    }
}

/// A hardware texture with storage, sampling and copy usage.
pub struct GpuTexture {
    pub width: u32,
    pub height: u32,
    pub texture: Texture,
    pub view: TextureView,
    /// The allocation sequence the pool assigned; a reused texture keeps it.
    pub pool_id: u64,
}

impl GpuTexture {
    /// Allocate `width` by `height` at [`WORK_FORMAT`].
    #[must_use]
    pub fn new(device: &Device, width: u32, height: u32, label: &str) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: WORK_FORMAT,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width,
            height,
            texture,
            view,
            pool_id: 0,
        }
    }

    /// Upload RGB floats, padding alpha to one.
    ///
    /// The source holds `width * height * 3` samples; a pixel with an alpha
    /// plane already present passes `rgba = true`.
    pub fn upload(&self, queue: &Queue, pixels: &[f32], rgba: bool) {
        let data = if rgba {
            pixels.to_vec()
        } else {
            let mut padded = vec![1.0f32; (self.width * self.height) as usize * 4];
            for (dst, src) in padded.chunks_exact_mut(4).zip(pixels.chunks_exact(3)) {
                dst[..3].copy_from_slice(src);
            }
            padded
        };
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&data),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * TEXEL_BYTES as u32),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Download the texture as RGBA floats, row by row.
    #[must_use]
    pub fn readback(&self, gpu: &GpuDevice) -> Vec<f32> {
        let bytes_per_row = (self.width as usize * TEXEL_BYTES).div_ceil(ROW_ALIGN) * ROW_ALIGN;
        let size = bytes_per_row * self.height as usize;
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: size as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &staging,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row as u32),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = gpu.device.poll(wgpu::PollType::Wait);
        match rx.recv() {
            Ok(Ok(())) => {}
            _ => return vec![0.0; self.width as usize * self.height as usize * 4],
        }
        let view = slice.get_mapped_range();
        let mut out = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for row in view.chunks_exact(bytes_per_row) {
            let row: &[f32] = bytemuck::cast_slice(&row[..self.width as usize * TEXEL_BYTES]);
            out.extend_from_slice(row);
        }
        drop(view);
        staging.unmap();
        out
    }
}

/// A uniform or storage buffer that a pass reads.
pub struct GpuBuffer {
    pub buffer: Buffer,
}

impl GpuBuffer {
    /// Allocate `size` bytes with `usage`.
    #[must_use]
    pub fn new(device: &Device, size: u64, usage: BufferUsages, label: &str) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            }),
        }
    }

    /// Write `data` at the start of the buffer.
    pub fn upload(&self, queue: &Queue, data: &[u8]) {
        queue.write_buffer(&self.buffer, 0, data);
    }
}
