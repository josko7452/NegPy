//! The crop compute pass over `shaders/crop.wgsl`.
//!
//! The pass copies a resolved window of the rotated frame, so the crop follows
//! the GPU chain instead of a CPU readback and upload.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::features::geometry::logic::Roi;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/crop.wgsl");

/// The window origin in the rotated frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CropUniforms {
    offset: [u32; 2],
}

impl CropUniforms {
    /// The top-left pixel of `roi` in a frame `width` by `height`.
    #[must_use]
    pub fn new(roi: Roi, width: usize, height: usize) -> (Self, u32, u32) {
        let (y1, y2, x1, x2) =
            clamp_roi(roi, width, height).unwrap_or((0, height as i64, 0, width as i64));
        (
            Self {
                offset: [x1 as u32, y1 as u32],
            },
            (x2 - x1) as u32,
            (y2 - y1) as u32,
        )
    }
}

fn clamp_roi(roi: Roi, width: usize, height: usize) -> Option<(i64, i64, i64, i64)> {
    let (y1, y2, x1, x2) = roi;
    let (y1, y2) = (y1.max(0), y2.min(height as i64).max(0));
    let (x1, x2) = (x1.max(0), x2.min(width as i64).max(0));
    (y2 > y1 && x2 > x1).then_some((y1, y2, x1, x2))
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("crop"),
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
                    ty: BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: WORK_FORMAT,
                        view_dimension: TextureViewDimension::D2,
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
            ],
        })
}

/// Allocate the crop texture for `roi` in a frame of `width` by `height`.
#[must_use]
pub fn target(
    gpu: &GpuDevice,
    roi: Roi,
    width: usize,
    height: usize,
    label: &str,
) -> (GpuTexture, CropUniforms) {
    let (uniforms, out_w, out_h) = CropUniforms::new(roi, width, height);
    (GpuTexture::new(&gpu.device, out_w, out_h, label), uniforms)
}

/// Run the crop pass from `source` into `output`.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &CropUniforms,
) {
    let module = loaders.load(&gpu.device, "geometry.crop", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<CropUniforms>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "crop.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("crop"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("crop"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("crop"),
        layout: &bind_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source.view),
            },
            BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&output.view),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.buffer.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("crop"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("crop"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
