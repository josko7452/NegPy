//! The normalized-density histogram pass over `shaders/density_hist.wgsl`.
//!
//! The pass counts the normalized log density in a crop window. Its bins match
//! `negpy_core::histogram::density_histogram`: 120 bins over -0.1..1.1 for R, G,
//! B and luma, in that order.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::features::exposure::analysis::DENSITY_HIST_BINS;
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::GpuTexture;
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/density_hist.wgsl");

/// The crop window and bounds the pass reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DensityHistogramUniforms {
    roi_offset: [u32; 2],
    crop_dims: [u32; 2],
    floors: [f32; 4],
    ceils: [f32; 4],
}

impl DensityHistogramUniforms {
    /// A window `(x1, y1, x2, y2)` in the texture and the bounds its values came from.
    #[must_use]
    pub fn new(roi: (usize, usize, usize, usize), bounds: LogNegativeBounds) -> Self {
        let (y1, y2, x1, x2) = roi;
        Self {
            roi_offset: [x1 as u32, y1 as u32],
            crop_dims: [(x2 - x1) as u32, (y2 - y1) as u32],
            floors: [
                bounds.floors[0] as f32,
                bounds.floors[1] as f32,
                bounds.floors[2] as f32,
                0.0,
            ],
            ceils: [
                bounds.ceils[0] as f32,
                bounds.ceils[1] as f32,
                bounds.ceils[2] as f32,
                0.0,
            ],
        }
    }
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("density_hist"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
}

const PARADE_WORDS: usize = 3 * 128 * 256;
const FULL_SCALE_WORDS: usize = 3 * 256;

/// Run the histogram pass and return its `(4, 120)` bins.
#[must_use]
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    normalized: &GpuTexture,
    geometry: &GpuTexture,
    uniforms: &DensityHistogramUniforms,
) -> Vec<f64> {
    let bins = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_hist.bins"),
        size: (DENSITY_HIST_BINS * 4 * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let parade = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_hist.parade"),
        size: (PARADE_WORDS * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let full_scale = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_hist.full_scale"),
        size: (FULL_SCALE_WORDS * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue
        .write_buffer(&bins, 0, &vec![0u8; DENSITY_HIST_BINS * 16]);
    gpu.queue
        .write_buffer(&parade, 0, &vec![0u8; PARADE_WORDS * 4]);
    gpu.queue
        .write_buffer(&full_scale, 0, &vec![0u8; FULL_SCALE_WORDS * 4]);
    let uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_hist.uniforms"),
        size: std::mem::size_of::<DensityHistogramUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue
        .write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("density_hist.readback"),
        size: (DENSITY_HIST_BINS * 4 * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let module = loaders.load(&gpu.device, "exposure.density_hist", SHADER);
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("density_hist"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("density_hist"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("density_hist"),
        layout: &bind_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&normalized.view),
            },
            BindGroupEntry {
                binding: 1,
                resource: bins.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: parade.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&geometry.view),
            },
            BindGroupEntry {
                binding: 5,
                resource: full_scale.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("density_hist"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("density_hist"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            uniforms.crop_dims[0].div_ceil(16),
            uniforms.crop_dims[1].div_ceil(16),
            1,
        );
    }
    encoder.copy_buffer_to_buffer(&bins, 0, &staging, 0, (DENSITY_HIST_BINS * 4 * 4) as u64);
    gpu.queue.submit(Some(encoder.finish()));
    read_bins(gpu, &staging)
}

fn read_bins(gpu: &GpuDevice, staging: &wgpu::Buffer) -> Vec<f64> {
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = gpu.device.poll(wgpu::PollType::Wait);
    if !matches!(rx.recv(), Ok(Ok(()))) {
        return vec![0.0; DENSITY_HIST_BINS * 4];
    }
    let view = slice.get_mapped_range();
    let words: &[u32] = bytemuck::cast_slice(&view);
    let out = words.iter().map(|value| f64::from(*value)).collect();
    drop(view);
    staging.unmap();
    out
}
