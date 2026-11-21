//! The geometry compute pass over `shaders/transform.wgsl`.
//!
//! The pass reads the unrotated source and writes the rotated, warped frame at
//! the post-rotation size. Its uniforms are the same values the CPU kernel
//! builds, so a parity test can compare the two paths pixel for pixel.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::domain::models::GeometryConfig;
use crate::features::geometry::logic::{compute_distortion_scale, keystone_inverse_normalized};
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/transform.wgsl");

/// The geometry uniform block, laid out as `GeometryUniforms` in the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GeometryUniforms {
    rotation: i32,
    fine_rotation: f32,
    flip_h: i32,
    flip_v: i32,
    distortion: [f32; 4],
    keystone_a: [f32; 4],
    keystone_b: [f32; 4],
}

impl GeometryUniforms {
    /// The block the CPU geometry kernel would apply for `config` at this size.
    #[must_use]
    pub fn from_config(config: &GeometryConfig, width: usize, height: usize) -> Self {
        let k1 = config.distortion_k1;
        let scale = if k1 != 0.0 {
            compute_distortion_scale(k1, width, height)
        } else {
            1.0
        };
        let inverse = keystone_inverse_normalized(config.converge_v, config.converge_h);
        Self {
            rotation: config.rotation.rem_euclid(4) as i32,
            fine_rotation: config.fine_rotation as f32,
            flip_h: i32::from(config.flip_horizontal),
            flip_v: i32::from(config.flip_vertical),
            distortion: [k1 as f32, scale as f32, 0.0, 0.0],
            keystone_a: [
                inverse[0][0] as f32,
                inverse[0][1] as f32,
                inverse[0][2] as f32,
                inverse[1][0] as f32,
            ],
            keystone_b: [
                inverse[1][1] as f32,
                inverse[1][2] as f32,
                inverse[2][0] as f32,
                inverse[2][1] as f32,
            ],
        }
    }
}

/// The output size after the 90-degree rotation steps.
#[must_use]
pub fn rotated_dims(width: usize, height: usize, rotation: i64) -> (usize, usize) {
    match rotation.rem_euclid(4) {
        1 | 3 => (height, width),
        _ => (width, height),
    }
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("geometry"),
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

/// Run the geometry pass. `source` is the unrotated frame; `output` is sized to
/// [`rotated_dims`]. The caller creates the uniforms with [`GeometryUniforms::from_config`]
/// at the post-rotation size.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &GeometryUniforms,
) {
    let module = loaders.load(&gpu.device, "geometry.transform", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<GeometryUniforms>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "geometry.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("geometry"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("geometry"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("geometry"),
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
            label: Some("geometry"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("geometry"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
