//! The Lab passes over `shaders/lab*.wgsl` and `shaders/chroma_lowpass_*.wgsl`.
//!
//! Chroma Denoise and sharpening read precomputed neighbour state, so the run
//! dispatches the separable low-pass and blur before the main pass. The chain
//! hands the main pass the encoded frame the print and CLAHE passes write.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::domain::models::{LabConfig, SharpenMethod};
use crate::features::lab::logic::{
    GLOW_AMOUNT_MAX, chroma_denoise_radius, chroma_denoise_sigma, gaussian_kernel_1d,
    halation_amounts,
};
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The main pass.
pub const SHADER: &str = include_str!("shaders/lab.wgsl");
/// The horizontal sharpen blur.
pub const SHARPEN_H_SHADER: &str = include_str!("shaders/lab_sharpen_h.wgsl");
/// The vertical sharpen blur.
pub const SHARPEN_V_SHADER: &str = include_str!("shaders/lab_sharpen_v.wgsl");
/// The horizontal chroma low-pass.
pub const CHROMA_H_SHADER: &str = include_str!("shaders/chroma_lowpass_h.wgsl");
/// The vertical chroma low-pass.
pub const CHROMA_V_SHADER: &str = include_str!("shaders/chroma_lowpass_v.wgsl");

/// The block `LabUniforms` in `lab.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LabUniforms {
    sharpen: f32,
    chroma_denoise: f32,
    saturation: f32,
    glow_amount: f32,
    halation_strength: f32,
    scale_factor: f32,
    sharpen_radius_px: f32,
    sharpen_masking: f32,
    sharpen_method: f32,
    skin_protection: f32,
    chroma_radius_px: f32,
    auto_masking: f32,
    halation_r: f32,
    halation_g: f32,
    halation_b: f32,
    frame_short_px: f32,
}

/// The block `LabUniforms` in `lab_sharpen_*.wgsl`, which stops after the method.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct SharpenUniforms {
    sharpen: f32,
    chroma_denoise: f32,
    saturation: f32,
    glow_amount: f32,
    halation_strength: f32,
    scale_factor: f32,
    sharpen_radius_px: f32,
    sharpen_masking: f32,
    sharpen_method: f32,
    pad: [f32; 3],
}

/// The block `LabUniforms` in `chroma_lowpass_*.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ChromaUniforms {
    sharpen: f32,
    chroma_denoise: f32,
    saturation: f32,
    glow_amount: f32,
    halation_strength: f32,
    scale_factor: f32,
    sharpen_radius_px: f32,
    sharpen_masking: f32,
    sharpen_method: f32,
    skin_protection: f32,
    chroma_radius_px: f32,
    pad: f32,
}

/// The values one Lab run reads beyond the config.
#[derive(Clone, Copy, Debug)]
pub struct LabRun {
    pub mono: bool,
    pub scale_factor: f64,
    /// The auto-masking share measured on this frame, when the toggle is on.
    pub auto_masking: Option<f32>,
}

impl LabUniforms {
    /// The parameters of one full frame.
    #[must_use]
    pub fn new(config: &LabConfig, run: LabRun, width: usize, height: usize) -> Self {
        let amounts = halation_amounts(config.halation_strength, run.mono);
        let chroma_sigma = chroma_denoise_sigma(
            chroma_denoise_radius(config.chroma_denoise, run.mono),
            run.scale_factor,
        );
        let chroma_radius = if chroma_sigma > 0.0 {
            gaussian_kernel_1d(chroma_sigma).len() / 2
        } else {
            0
        };
        let sharpen_radius = if config.sharpen > 0.0 {
            gaussian_kernel_1d(config.sharpen_radius).len() / 2
        } else {
            0
        };
        Self {
            sharpen: config.sharpen as f32,
            chroma_denoise: config.chroma_denoise as f32,
            saturation: config.saturation as f32,
            glow_amount: (GLOW_AMOUNT_MAX * config.glow_amount) as f32,
            halation_strength: config.halation_strength as f32,
            scale_factor: run.scale_factor as f32,
            sharpen_radius_px: sharpen_radius as f32,
            sharpen_masking: config.sharpen_masking as f32,
            sharpen_method: f32::from(config.sharpen_method == SharpenMethod::Rl),
            skin_protection: config.skin_protection as f32,
            chroma_radius_px: chroma_radius as f32,
            auto_masking: f32::from(run.auto_masking.is_some()),
            halation_r: amounts[0],
            halation_g: amounts[1],
            halation_b: amounts[2],
            frame_short_px: height.min(width) as f32,
        }
    }
}

fn texture_binding(binding: u32, output: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: if output {
            BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: WORK_FORMAT,
                view_dimension: TextureViewDimension::D2,
            }
        } else {
            BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            }
        },
        count: None,
    }
}

fn buffer_binding(binding: u32, uniform: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: if uniform {
                BufferBindingType::Uniform
            } else {
                BufferBindingType::Storage { read_only: true }
            },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn layout(
    gpu: &GpuDevice,
    label: &'static str,
    entries: &[BindGroupLayoutEntry],
) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries,
        })
}

fn pipeline(
    gpu: &GpuDevice,
    loader: &ShaderLoader,
    label: &'static str,
    name: &'static str,
    source: &str,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let module = loader.load(&gpu.device, name, source);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });
    gpu.device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
}

fn dispatch(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    width: u32,
    height: u32,
) {
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
}

fn kernel_buffer(gpu: &GpuDevice, label: &'static str, values: &[f32]) -> GpuBuffer {
    let buffer = GpuBuffer::new(
        &gpu.device,
        (values.len() * 4) as wgpu::BufferAddress,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        label,
    );
    buffer.upload(&gpu.queue, bytemuck::cast_slice(values));
    buffer
}

/// Run the Lab stage from an encoded frame into an encoded frame.
#[must_use]
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    config: &LabConfig,
    run: LabRun,
    textures: &crate::infrastructure::gpu::resources::TexturePool,
) -> LabUniforms {
    let uniforms = LabUniforms::new(config, run, output.width as usize, output.height as usize);
    let (width, height) = (output.width, output.height);
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lab") });

    let sharpen = if uniforms.sharpen > 0.0 && config.sharpen_method != SharpenMethod::Rl {
        let kernel = gaussian_kernel_1d(config.sharpen_radius);
        let buffer = kernel_buffer(gpu, "lab.sharpen_k", &kernel);
        let blur = SharpenUniforms {
            sharpen: uniforms.sharpen,
            chroma_denoise: uniforms.chroma_denoise,
            saturation: uniforms.saturation,
            glow_amount: uniforms.glow_amount,
            halation_strength: uniforms.halation_strength,
            scale_factor: uniforms.scale_factor,
            sharpen_radius_px: uniforms.sharpen_radius_px,
            sharpen_masking: uniforms.sharpen_masking,
            sharpen_method: uniforms.sharpen_method,
            pad: [0.0; 3],
        };
        let uniform_buffer = GpuBuffer::new(
            &gpu.device,
            std::mem::size_of::<SharpenUniforms>() as wgpu::BufferAddress,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "lab.sharpen_uniforms",
        );
        uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(&blur));
        let layout = layout(
            gpu,
            "lab_sharpen",
            &[
                texture_binding(0, false),
                texture_binding(1, true),
                buffer_binding(2, true),
                buffer_binding(3, false),
            ],
        );
        let horizontal = textures.take(&gpu.device, width, height, "lab.sharpen_h");
        let vertical = textures.take(&gpu.device, width, height, "lab.sharpen_v");
        let h_pipeline = pipeline(
            gpu,
            loaders,
            "lab_sharpen_h",
            "lab.sharpen_h",
            SHARPEN_H_SHADER,
            &layout,
        );
        let v_pipeline = pipeline(
            gpu,
            loaders,
            "lab_sharpen_v",
            "lab.sharpen_v",
            SHARPEN_V_SHADER,
            &layout,
        );
        let h_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lab_sharpen_h"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&horizontal.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffer.buffer.as_entire_binding(),
                },
            ],
        });
        let v_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lab_sharpen_v"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&horizontal.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&vertical.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffer.buffer.as_entire_binding(),
                },
            ],
        });
        dispatch(
            &mut encoder,
            "lab_sharpen_h",
            &h_pipeline,
            &h_bind,
            width,
            height,
        );
        dispatch(
            &mut encoder,
            "lab_sharpen_v",
            &v_pipeline,
            &v_bind,
            width,
            height,
        );
        textures.put(horizontal);
        Some(vertical)
    } else {
        None
    };
    let sharpen = sharpen.unwrap_or_else(|| {
        let texture = GpuTexture::new(&gpu.device, 1, 1, "lab.sharpen_dummy");
        texture.upload(&gpu.queue, &[0.0; 4], true);
        texture
    });

    let chroma = if uniforms.chroma_radius_px > 0.0 {
        let sigma = chroma_denoise_sigma(
            chroma_denoise_radius(config.chroma_denoise, run.mono),
            run.scale_factor,
        );
        let kernel = gaussian_kernel_1d(sigma);
        let buffer = kernel_buffer(gpu, "lab.chroma_k", &kernel);
        let chroma_uniforms = ChromaUniforms {
            sharpen: uniforms.sharpen,
            chroma_denoise: uniforms.chroma_denoise,
            saturation: uniforms.saturation,
            glow_amount: uniforms.glow_amount,
            halation_strength: uniforms.halation_strength,
            scale_factor: uniforms.scale_factor,
            sharpen_radius_px: uniforms.sharpen_radius_px,
            sharpen_masking: uniforms.sharpen_masking,
            sharpen_method: uniforms.sharpen_method,
            skin_protection: uniforms.skin_protection,
            chroma_radius_px: uniforms.chroma_radius_px,
            pad: 0.0,
        };
        let uniform_buffer = GpuBuffer::new(
            &gpu.device,
            std::mem::size_of::<ChromaUniforms>() as wgpu::BufferAddress,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            "lab.chroma_uniforms",
        );
        uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(&chroma_uniforms));
        let layout = layout(
            gpu,
            "chroma_lowpass",
            &[
                texture_binding(0, false),
                texture_binding(1, true),
                buffer_binding(2, true),
                buffer_binding(3, false),
            ],
        );
        let horizontal = textures.take(&gpu.device, width, height, "lab.chroma_h");
        let vertical = textures.take(&gpu.device, width, height, "lab.chroma_v");
        let h_pipeline = pipeline(
            gpu,
            loaders,
            "chroma_lowpass_h",
            "lab.chroma_h",
            CHROMA_H_SHADER,
            &layout,
        );
        let v_pipeline = pipeline(
            gpu,
            loaders,
            "chroma_lowpass_v",
            "lab.chroma_v",
            CHROMA_V_SHADER,
            &layout,
        );
        let h_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chroma_lowpass_h"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&horizontal.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffer.buffer.as_entire_binding(),
                },
            ],
        });
        let v_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chroma_lowpass_v"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&horizontal.view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&vertical.view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffer.buffer.as_entire_binding(),
                },
            ],
        });
        dispatch(
            &mut encoder,
            "chroma_lowpass_h",
            &h_pipeline,
            &h_bind,
            width,
            height,
        );
        dispatch(
            &mut encoder,
            "chroma_lowpass_v",
            &v_pipeline,
            &v_bind,
            width,
            height,
        );
        textures.put(horizontal);
        vertical
    } else {
        let texture = GpuTexture::new(&gpu.device, 1, 1, "lab.chroma_dummy");
        texture.upload(&gpu.queue, &[0.0; 4], true);
        texture
    };

    let auto_mask = GpuBuffer::new(
        &gpu.device,
        8,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        "lab.auto_mask",
    );
    auto_mask.upload(
        &gpu.queue,
        bytemuck::cast_slice(&[
            run.auto_masking.unwrap_or(config.sharpen_masking as f32),
            run.auto_masking.unwrap_or(0.0),
        ]),
    );
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<LabUniforms>() as wgpu::BufferAddress,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "lab.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(&uniforms));
    let layout = layout(
        gpu,
        "lab",
        &[
            texture_binding(0, false),
            texture_binding(1, true),
            buffer_binding(2, true),
            texture_binding(3, false),
            texture_binding(4, false),
            buffer_binding(5, false),
        ],
    );
    let lab_pipeline = pipeline(gpu, loaders, "lab", "lab.main", SHADER, &layout);
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("lab"),
        layout: &layout,
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
                resource: wgpu::BindingResource::TextureView(&sharpen.view),
            },
            BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&chroma.view),
            },
            BindGroupEntry {
                binding: 5,
                resource: auto_mask.buffer.as_entire_binding(),
            },
        ],
    });
    dispatch(&mut encoder, "lab", &lab_pipeline, &bind, width, height);
    gpu.queue.submit(Some(encoder.finish()));
    textures.put(sharpen);
    textures.put(chroma);
    uniforms
}
