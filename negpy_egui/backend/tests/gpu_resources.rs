//! A texture round-trips through the device, and the shader cache reuses a module.

use negpy_backend::infrastructure::gpu::device::device;
use negpy_backend::infrastructure::gpu::resources::{GpuTexture, WORK_FORMAT};
use negpy_backend::infrastructure::gpu::shader_loader::ShaderLoader;
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

const COPY: &str = r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    textureStore(output_tex, vec2<i32>(i32(gid.x), i32(gid.y)), textureLoad(input_tex, vec2<i32>(i32(gid.x), i32(gid.y)), 0));
}
"#;

fn copy_pipeline(
    gpu: &negpy_backend::infrastructure::gpu::device::GpuDevice,
) -> wgpu::ComputePipeline {
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
    let module = ShaderLoader::default().load(&gpu.device, "copy", COPY);
    gpu.device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
}

#[test]
fn an_rgb_texture_round_trips_with_alpha_padded() {
    let Some(gpu) = device() else {
        return;
    };
    let source = GpuTexture::new(&gpu.device, 4, 3, "source");
    let pixels: Vec<f32> = (0..36).map(|index| index as f32 / 36.0).collect();
    source.upload(&gpu.queue, &pixels, false);

    let target = GpuTexture::new(&gpu.device, 4, 3, "target");
    let pipeline = copy_pipeline(&gpu);
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source.view),
            },
            BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&target.view),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(4, 3, 1);
    }
    gpu.queue.submit(Some(encoder.finish()));

    let read: Vec<f32> = target
        .readback(&gpu)
        .chunks_exact(4)
        .flat_map(|texel| texel[..3].to_vec())
        .collect();
    assert_eq!(read.len(), pixels.len());
    for (got, want) in read.iter().zip(&pixels) {
        assert!((got - want).abs() < 1e-6, "{got} vs {want}");
    }
}

#[test]
fn the_shader_cache_reuses_a_module() {
    let Some(gpu) = device() else {
        return;
    };
    let loader = ShaderLoader::default();
    let first = loader.load(&gpu.device, "copy", COPY);
    let second = loader.load(&gpu.device, "copy", COPY);
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    loader.clear();
    let third = loader.load(&gpu.device, "copy", COPY);
    assert!(!std::sync::Arc::ptr_eq(&first, &third));
}
