//! The vignette and carrier pass over `shaders/finish.wgsl`.
//!
//! The pass reads scene-linear pixels and writes the finished frame. Its carrier
//! uses the same profile and tone tables as `negpy_core::finish`.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::domain::models::FinishConfig;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/finish.wgsl");

/// The uniform block, laid out as `FinishUniforms` in the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FinishUniforms {
    vignette_stops: f32,
    vignette_size: f32,
    vignette_roundness: f32,
    full_crop_w: f32,
    full_crop_h: f32,
    tile_off_x: f32,
    tile_off_y: f32,
    carrier_width_px: f32,
    carrier_rough: f32,
    carrier_flare: f32,
    carrier_corner: f32,
    paper_r: f32,
    paper_g: f32,
    paper_b: f32,
}

impl FinishUniforms {
    /// The vignette inputs of a full frame with the carrier off.
    #[must_use]
    pub fn from_config(
        config: &FinishConfig,
        width: usize,
        height: usize,
        paper: [f32; 3],
    ) -> Self {
        Self {
            vignette_stops: config.vignette_stops as f32,
            vignette_size: config.vignette_size as f32,
            vignette_roundness: config.vignette_roundness as f32,
            full_crop_w: width as f32,
            full_crop_h: height as f32,
            tile_off_x: 0.0,
            tile_off_y: 0.0,
            carrier_width_px: 0.0,
            carrier_rough: config.carrier_rough as f32,
            carrier_flare: config.carrier_flare as f32,
            carrier_corner: config.carrier_corner as f32,
            paper_r: paper[0],
            paper_g: paper[1],
            paper_b: paper[2],
        }
    }

    /// Turn the carrier on at a width in pixels.
    #[must_use]
    pub fn with_carrier(mut self, width_px: f32) -> Self {
        self.carrier_width_px = width_px;
        self
    }
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    let texture = |binding: u32| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: false },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let storage = |binding: u32| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("finish"),
            entries: &[
                texture(0),
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
                storage(3),
                storage(4),
            ],
        })
}

/// Run the finish pass with the vignette only.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &FinishUniforms,
) {
    run_with_tables(gpu, loaders, source, output, uniforms, &[], &[]);
}

/// Run the finish pass with a carrier profile and tone table.
///
/// An empty table binds one zero so the shader layout stays complete.
pub fn run_with_tables(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &FinishUniforms,
    profile: &[f32],
    tone: &[[f32; 3]],
) {
    let module = loaders.load(&gpu.device, "finish.vignette", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<FinishUniforms>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "finish.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let profile_buffer = GpuBuffer::new(
        &gpu.device,
        4,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        "finish.profile",
    );
    let profile_values: Vec<f32> = if profile.is_empty() {
        vec![0.0]
    } else {
        profile.to_vec()
    };
    profile_buffer.upload(&gpu.queue, bytemuck::cast_slice(profile_values.as_slice()));
    let tone_buffer = GpuBuffer::new(
        &gpu.device,
        12,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        "finish.tone",
    );
    let tone_values: Vec<f32> = if tone.is_empty() {
        vec![0.0; 3]
    } else {
        tone.iter().flatten().copied().collect()
    };
    tone_buffer.upload(&gpu.queue, bytemuck::cast_slice(tone_values.as_slice()));
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("finish"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("finish"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("finish"),
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
            BindGroupEntry {
                binding: 3,
                resource: profile_buffer.buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: tone_buffer.buffer.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("finish"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("finish"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
