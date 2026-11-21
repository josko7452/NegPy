//! The toning pass over `shaders/toning.wgsl`, which also performs the crop.
//!
//! The pass reads scene-linear pixels at `crop_offset` and writes the cropped
//! frame, so the chain follows the upstream order: Lab, toning with the crop,
//! then finish.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::domain::models::{AltProcess, ToningConfig};
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/toning.wgsl");

/// The block `ToningUniforms` in the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ToningUniforms {
    saturation: f32,
    selenium_strength: f32,
    sepia_strength: f32,
    gamma: f32,
    crop_offset: [i32; 2],
    is_bw: u32,
    gold_strength: f32,
    shadow_tint_hue: f32,
    shadow_tint_strength: f32,
    highlight_tint_hue: f32,
    highlight_tint_strength: f32,
    blue_strength: f32,
    copper_strength: f32,
    vanadium_strength: f32,
    alt_mode: u32,
}

impl ToningUniforms {
    /// The pass parameters for a crop window whose top-left is `offset`.
    #[must_use]
    pub fn new(
        config: &ToningConfig,
        saturation: f64,
        mono: bool,
        alt_process: AltProcess,
        offset: [i32; 2],
    ) -> Self {
        let alt_mode = match alt_process {
            AltProcess::Lith => 1,
            AltProcess::Cyanotype => 2,
            AltProcess::None | AltProcess::Sabattier => 0,
        };
        Self {
            saturation: saturation as f32,
            selenium_strength: config.selenium_strength as f32,
            sepia_strength: config.sepia_strength as f32,
            gamma: 2.2,
            crop_offset: offset,
            is_bw: u32::from(mono),
            gold_strength: config.gold_strength as f32,
            shadow_tint_hue: config.shadow_tint_hue as f32,
            shadow_tint_strength: config.shadow_tint_strength as f32,
            highlight_tint_hue: config.highlight_tint_hue as f32,
            highlight_tint_strength: config.highlight_tint_strength as f32,
            blue_strength: config.blue_strength as f32,
            copper_strength: config.copper_strength as f32,
            vanadium_strength: config.vanadium_strength as f32,
            alt_mode: if mono { alt_mode } else { 0 },
        }
    }
}

/// Run the toning pass from `source` into the cropped `output`.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &ToningUniforms,
) {
    let module = loaders.load(&gpu.device, "toning", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<ToningUniforms>() as wgpu::BufferAddress,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "toning.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let bind_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("toning"),
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
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("toning"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("toning"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("toning"),
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
            label: Some("toning"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("toning"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
