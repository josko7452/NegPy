//! The CLAHE passes over `shaders/clahe_*.wgsl`: a tile histogram, its clipped
//! CDFs and the remap.
//!
//! The passes mirror `negpy/features/lab/shaders/clahe_*.wgsl`. They decode the
//! working TRC, so the chain hands them the display-encoded frame the print
//! pass writes.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The histogram pass, compiled once by a caller-held [`ShaderLoader`].
pub const HIST_SHADER: &str = include_str!("shaders/clahe_hist.wgsl");
/// The CDF pass.
pub const CDF_SHADER: &str = include_str!("shaders/clahe_cdf.wgsl");
/// The remap pass.
pub const APPLY_SHADER: &str = include_str!("shaders/clahe_apply.wgsl");

/// The 8x8 tile grid every pass assumes.
pub const TILES: usize = 64;
/// The bins of one tile histogram.
pub const BINS: usize = 256;

/// The block `ClaheUniforms` in `clahe_cdf.wgsl` and `clahe_apply.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaheUniforms {
    strength: f32,
    clip_limit: f32,
    global_offset: [i32; 2],
    full_dims: [i32; 2],
    pad: [f32; 2],
}

impl ClaheUniforms {
    /// The parameters of a full frame at this strength.
    #[must_use]
    pub fn for_frame(strength: f32, width: usize, height: usize) -> Self {
        Self {
            strength,
            clip_limit: strength * 2.5,
            global_offset: [0, 0],
            full_dims: [width as i32, height as i32],
            pad: [0.0, 0.0],
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

fn buffer_binding(binding: u32, uniform: bool, writable: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: if uniform {
                BufferBindingType::Uniform
            } else {
                BufferBindingType::Storage {
                    read_only: !writable,
                }
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

/// Remap an encoded frame by its own CLAHE CDFs.
///
/// `source` and `output` hold OETF-encoded working RGB, the domain the print
/// pass writes and the Lab pass decodes.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &ClaheUniforms,
) {
    let histograms = GpuBuffer::new(
        &gpu.device,
        (TILES * BINS * 4) as wgpu::BufferAddress,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        "clahe.histograms",
    );
    histograms.upload(&gpu.queue, &vec![0u8; TILES * BINS * 4]);
    let cdfs = GpuBuffer::new(
        &gpu.device,
        (TILES * BINS * 4) as wgpu::BufferAddress,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        "clahe.cdfs",
    );
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<ClaheUniforms>() as wgpu::BufferAddress,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "clahe.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));

    let hist_layout = layout(
        gpu,
        "clahe_hist",
        &[texture_binding(0, false), buffer_binding(1, false, true)],
    );
    let hist_pipeline = pipeline(
        gpu,
        loaders,
        "clahe_hist",
        "lab.clahe_hist",
        HIST_SHADER,
        &hist_layout,
    );
    let hist_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("clahe_hist"),
        layout: &hist_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source.view),
            },
            BindGroupEntry {
                binding: 1,
                resource: histograms.buffer.as_entire_binding(),
            },
        ],
    });

    let cdf_layout = layout(
        gpu,
        "clahe_cdf",
        &[
            buffer_binding(0, false, false),
            buffer_binding(1, false, true),
            buffer_binding(2, true, false),
        ],
    );
    let cdf_pipeline = pipeline(
        gpu,
        loaders,
        "clahe_cdf",
        "lab.clahe_cdf",
        CDF_SHADER,
        &cdf_layout,
    );
    let cdf_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("clahe_cdf"),
        layout: &cdf_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: histograms.buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: cdfs.buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.buffer.as_entire_binding(),
            },
        ],
    });

    let apply_layout = layout(
        gpu,
        "clahe_apply",
        &[
            texture_binding(0, false),
            texture_binding(1, true),
            buffer_binding(2, false, false),
            buffer_binding(3, true, false),
        ],
    );
    let apply_pipeline = pipeline(
        gpu,
        loaders,
        "clahe_apply",
        "lab.clahe_apply",
        APPLY_SHADER,
        &apply_layout,
    );
    let apply_bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("clahe_apply"),
        layout: &apply_layout,
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
                resource: cdfs.buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: uniform_buffer.buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("clahe"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("clahe_hist"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&hist_pipeline);
        pass.set_bind_group(0, &hist_bind, &[]);
        pass.dispatch_workgroups(8, 8, 1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("clahe_cdf"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&cdf_pipeline);
        pass.set_bind_group(0, &cdf_bind, &[]);
        pass.dispatch_workgroups(8, 8, 1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("clahe_apply"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&apply_pipeline);
        pass.set_bind_group(0, &apply_bind, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
