//! The normalization compute pass over `shaders/normalization.wgsl`.
//!
//! Bounds and meters stay on the CPU: the engine measures one frame, then this
//! pass stretches the same log densities the CPU kernel would. The uniforms
//! carry the density stage, the point offsets and the fade crossover table.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::process::fade::CROSSOVER_BINS;
use crate::features::transparency::logic::DensityStage;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/normalization.wgsl");

/// The shader's crossover table: `(lo, hi, on, 0)` then one row per bin.
pub const CROSSOVER_TABLE_BYTES: usize = 16 * (CROSSOVER_BINS + 1);

/// The uniform block, laid out as `NormUniforms` in the shader. Each matrix row
/// carries the affine offset in its fourth lane, and the anchor in `w`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct NormalizationUniforms {
    floors: [f32; 4],
    ceils: [f32; 4],
    mode: u32,
    transfer_flag: u32,
    wp_offset: f32,
    bp_offset: f32,
    unmix: [[f32; 4]; 3],
    cam: [[f32; 4]; 3],
    post: [[f32; 4]; 3],
    anchor: [f32; 4],
    crossover: [f32; 4],
    crossover_rows: [[f32; 4]; CROSSOVER_BINS],
}

/// The values the pass reads for one frame.
#[derive(Clone, Copy, Debug)]
pub struct NormalizationSetup<'a> {
    pub stage: &'a DensityStage,
    /// Bounds after the point offsets, as the CPU normalization reports them.
    pub bounds: LogNegativeBounds,
    /// A B&W negative, which clamps to one channel.
    pub mono: bool,
    /// The transparency curve replaces the print curve.
    pub transfer: bool,
}

/// A row of a 3x3 matrix as a vec4, the fourth lane free.
fn row3(row: [f64; 3]) -> [f32; 4] {
    [row[0] as f32, row[1] as f32, row[2] as f32, 0.0]
}

/// A 3x4 affine row as a vec4, all four lanes used.
fn row4(row: [f64; 4]) -> [f32; 4] {
    [row[0] as f32, row[1] as f32, row[2] as f32, row[3] as f32]
}

impl NormalizationUniforms {
    /// Pack the pass's uniforms. `crossover` is the shader table from
    /// [`crate::features::process::fade::crossover_uniform`], which holds
    /// [`CROSSOVER_TABLE_BYTES`] bytes.
    #[must_use]
    pub fn pack(setup: NormalizationSetup<'_>, crossover: &[u8]) -> Self {
        let stage = setup.stage;
        let mut uniforms = Self {
            floors: [
                setup.bounds.floors[0] as f32,
                setup.bounds.floors[1] as f32,
                setup.bounds.floors[2] as f32,
                0.0,
            ],
            ceils: [
                setup.bounds.ceils[0] as f32,
                setup.bounds.ceils[1] as f32,
                setup.bounds.ceils[2] as f32,
                0.0,
            ],
            mode: u32::from(setup.mono),
            transfer_flag: u32::from(setup.transfer),
            wp_offset: 0.0,
            bp_offset: 0.0,
            unmix: [[0.0; 4]; 3],
            cam: [[0.0; 4]; 3],
            post: [[0.0; 4]; 3],
            anchor: [0.0; 4],
            crossover: [0.0; 4],
            crossover_rows: [[0.0; 4]; CROSSOVER_BINS],
        };
        for (row, lanes) in uniforms.unmix.iter_mut().enumerate() {
            lanes[row] = 1.0;
        }
        if let Some(matrix) = stage.unmix {
            uniforms.unmix = matrix.map(row4);
        }
        if let Some(matrix) = stage.pre {
            uniforms.cam = matrix.map(row3);
        }
        if let Some(matrix) = stage.post {
            uniforms.post = matrix.map(row3);
            uniforms.post[0][3] = 1.0;
        }
        if let Some(anchor) = stage.anchor {
            uniforms.anchor = [anchor[0] as f32, anchor[1] as f32, anchor[2] as f32, 1.0];
        }
        if crossover.len() == CROSSOVER_TABLE_BYTES {
            let words: &[[f32; 4]] = bytemuck::cast_slice(crossover);
            uniforms.crossover = words[0];
            uniforms.crossover_rows.copy_from_slice(&words[1..]);
        }
        uniforms
    }
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("normalization"),
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

/// Run the normalization pass over a scene-linear texture.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &NormalizationUniforms,
) {
    let module = loaders.load(&gpu.device, "exposure.normalization", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<NormalizationUniforms>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "normalization.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("normalization"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("normalization"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("normalization"),
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
            label: Some("normalization"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("normalization"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}

/// The display-encode shader: scene-linear to the working space's encoded values.
pub const ENCODE_SHADER: &str = include_str!("shaders/output_encode.wgsl");

/// Run the display encode over a scene-linear texture.
pub fn run_encode(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
) {
    let module = loaders.load(&gpu.device, "exposure.output_encode", ENCODE_SHADER);
    let bind_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("output_encode"),
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
            ],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("output_encode"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("output_encode"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("output_encode"),
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
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("output_encode"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("output_encode"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
