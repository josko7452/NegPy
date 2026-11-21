//! The print-curve compute pass over `shaders/exposure.wgsl`.
//!
//! The pass reads the normalized log densities and writes scene-linear
//! reflectance. Its uniforms are the solved curve and paper values the CPU
//! print stage builds, so the parity test compares it with `render_negative`.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroupEntry, BindGroupLayoutEntry, BindingType, BufferBindingType, ComputePassDescriptor,
    ComputePipelineDescriptor, PipelineLayoutDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension,
};

use crate::domain::models::{ExposureConfig, ProcessConfig, ProcessMode};
use crate::features::exposure::bw_filter::{collapse_weights, printed_exposure};
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{
    channel_density_ranges, effective_midtone_gamma, filtration_offsets, highlight_hold_offset,
    preflash_params, reference_linear_value, shadow_hold_offset,
};
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::papers::{
    compose_density_matrices, effective_constants, resolve_print_color, resolve_saturation_matrix,
};
use crate::features::exposure::print::{
    DiffusionPlane, MaskPlane, PrintRenderSettings, paper_dmin_rgb, per_channel_dye_separation,
    per_channel_toe_shoulder, per_channel_widths, print_settings,
};
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuBuffer, GpuTexture, WORK_FORMAT};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;

/// The shader source, compiled once by a caller-held [`ShaderLoader`].
pub const SHADER: &str = include_str!("shaders/exposure.wgsl");

/// The uniform block, laid out as `ExposureUniforms` in the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ExposureUniforms {
    pivots: [f32; 4],
    slopes: [f32; 4],
    curvatures: [f32; 4],
    cmy_offsets: [f32; 4],
    shadow_cmy: [f32; 4],
    highlight_cmy: [f32; 4],
    toe_width_r: f32,
    toe_width_g: f32,
    toe_width_b: f32,
    shoulder_width_r: f32,
    shadow_density: f32,
    d_max: f32,
    a_toe_base: f32,
    a_sh_base: f32,
    sep_damping: f32,
    toe_height: f32,
    sh_height: f32,
    zone_center: f32,
    shoulder_width_g: f32,
    sep_k_r: f32,
    mode: u32,
    v_star: f32,
    shoulder_width_b: f32,
    gamma_width: f32,
    use_dye: u32,
    bpc: f32,
    d_min_rgb: [f32; 4],
    dye_r: [f32; 4],
    dye_g: [f32; 4],
    dye_b: [f32; 4],
    ev_scale: [f32; 4],
    split_sh: [f32; 4],
    split_hi: [f32; 4],
    hue: [f32; 4],
    mask: [f32; 4],
    mask_span: [f32; 4],
    mask_tone: [f32; 4],
    keyed: [[f32; 4]; 4],
    key_meta: [f32; 4],
    diff_range: [f32; 4],
    lut: [f32; 4],
    bw: [f32; 4],
}

/// The config and measured values one print run reads.
#[derive(Clone, Copy, Debug)]
pub struct ExposureRun<'a> {
    pub config: &'a ExposureConfig,
    pub process: &'a ProcessConfig,
    pub bounds: Option<LogNegativeBounds>,
    pub density_range: Option<f64>,
    pub textural_range: Option<f64>,
    pub shadow_point: Option<f64>,
    pub highlight_point: Option<f64>,
    pub anchor: Option<f64>,
    pub mode: ProcessMode,
    pub mask: Option<MaskPlane<'a>>,
    pub diffusion: Option<DiffusionPlane<'a>>,
    pub local_exposure: Option<&'a [f32]>,
    pub local_grade: Option<&'a [f32]>,
    pub local_flash: Option<&'a [f32]>,
    pub key_params: Option<&'a [[f64; 4]]>,
    pub key_alpha: Option<(&'a [f32], usize)>,
    pub grade_deltas: Option<&'a [f32]>,
    pub black_hold: f64,
}

impl<'a> ExposureRun<'a> {
    /// A plain print with no local masks or maps.
    #[must_use]
    pub fn plain(
        config: &'a ExposureConfig,
        process: &'a ProcessConfig,
        mode: ProcessMode,
    ) -> Self {
        Self {
            config,
            process,
            bounds: None,
            density_range: None,
            textural_range: None,
            shadow_point: None,
            highlight_point: None,
            anchor: None,
            mode,
            mask: None,
            diffusion: None,
            local_exposure: None,
            local_grade: None,
            local_flash: None,
            key_params: None,
            key_alpha: None,
            grade_deltas: None,
            black_hold: 0.0,
        }
    }

    /// True when every active curve input has a GPU binding.
    ///
    /// Local maps and a print LUT still need their own textures, so the engine
    /// keeps those frames on the CPU until the pass binds them.
    #[must_use]
    pub fn gpu_ready(&self, paper_has_lut: bool) -> bool {
        !paper_has_lut
            && self.mask.is_none()
            && self.diffusion.is_none()
            && self.local_exposure.is_none()
            && self.local_grade.is_none()
            && self.local_flash.is_none()
            && self.key_params.is_none()
    }

    fn settings(&self) -> PrintRenderSettings<'a> {
        PrintRenderSettings {
            config: self.config,
            process: self.process,
            density_range: self.density_range,
            textural_range: self.textural_range,
            shadow_point: self.shadow_point,
            highlight_point: self.highlight_point,
            anchor: self.anchor,
            bounds: self.bounds,
            neutral_axis: None,
            mode: self.mode,
            base_black_hold: self.black_hold,
            mask: self.mask,
            diffusion: self.diffusion,
            local_exposure: self.local_exposure,
            local_grade: self.local_grade,
            local_flash: self.local_flash,
            key_params: self.key_params,
            key_alpha: self.key_alpha,
            grade_deltas: self.grade_deltas,
        }
    }
}

fn effective_d_min(
    exposure: &ExposureConfig,
    paper: &crate::features::exposure::papers::PaperProfile,
) -> f64 {
    if exposure.paper_dmin {
        paper.tonal.d_min
    } else {
        0.0
    }
}

impl ExposureUniforms {
    /// Pack the uniforms the print pass reads.
    #[must_use]
    pub fn pack(run: &ExposureRun<'_>) -> Self {
        let exposure = printed_exposure(run.process, run.config);
        let settings = run.settings();
        let (_, solved, paper) = print_settings(&settings);
        let tonal = effective_constants(Some(paper));
        let d_min = effective_d_min(&exposure, paper);
        let (dye, _) = resolve_print_color(Some(paper));
        let is_bw = run.mode == ProcessMode::BW;
        let (dye_k, damping) = if is_bw {
            ([1.0; 3], 0.0)
        } else {
            (
                per_channel_dye_separation(
                    exposure.dye_separation,
                    [
                        exposure.dye_separation_trim_red,
                        exposure.dye_separation_trim_green,
                        exposure.dye_separation_trim_blue,
                    ],
                ),
                exposure.separation_damping,
            )
        };
        let sat = if damping > 0.0 && dye_k != [1.0; 3] {
            None
        } else {
            resolve_saturation_matrix(dye_k)
        };
        let composed = compose_density_matrices(dye, sat);
        let dye_rows = composed.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

        let cmy = filtration_offsets(
            [exposure.wb_cyan, exposure.wb_magenta, exposure.wb_yellow],
            run.bounds,
        )
        .map(|value| value as f32);
        let cmy_max = c::CMY_MAX_DENSITY;
        let shadow_cmy = [
            exposure.shadow_cyan * cmy_max,
            exposure.shadow_magenta * cmy_max,
            exposure.shadow_yellow * cmy_max,
        ]
        .map(|value| value as f32);
        let highlight_cmy = [
            exposure.highlight_cyan * cmy_max,
            exposure.highlight_magenta * cmy_max,
            exposure.highlight_yellow * cmy_max,
        ]
        .map(|value| value as f32);
        let (toe, shoulder) = per_channel_toe_shoulder(
            solved.toe,
            solved.shoulder,
            [
                exposure.toe_trim_red,
                exposure.toe_trim_green,
                exposure.toe_trim_blue,
            ],
            [
                exposure.shoulder_trim_red,
                exposure.shoulder_trim_green,
                exposure.shoulder_trim_blue,
            ],
        );
        let strength = c::TOE_SHOULDER_STRENGTH;
        let toe = toe.map(|value| value * strength);
        let shoulder = shoulder.map(|value| value * strength);
        let (toe_width, shoulder_width) = per_channel_widths(
            exposure.toe_width,
            exposure.shoulder_width,
            [
                exposure.toe_width_trim_red,
                exposure.toe_width_trim_green,
                exposure.toe_width_trim_blue,
            ],
            [
                exposure.shoulder_width_trim_red,
                exposure.shoulder_width_trim_green,
                exposure.shoulder_width_trim_blue,
            ],
        );
        let midtone = effective_midtone_gamma(Some(paper), exposure.midtone_gamma);
        let midtone_trims = [
            midtone + exposure.midtone_gamma_trim_red,
            midtone + exposure.midtone_gamma_trim_green,
            midtone + exposure.midtone_gamma_trim_blue,
        ]
        .map(|value| value as f32);
        let (flash_v, flash_gamma) =
            preflash_params(exposure.grade, d_min, Some(paper), Some(midtone));
        let shadow_auto = if exposure.auto_normalize_contrast {
            run.shadow_point.map_or(0.0, |point| {
                shadow_hold_offset(
                    solved.slopes[1],
                    solved.pivots[1],
                    point,
                    d_min,
                    Some(paper),
                    Some(midtone),
                )
            })
        } else {
            0.0
        };
        let highlight_auto = if exposure.auto_normalize_contrast {
            run.highlight_point.map_or(0.0, |point| {
                highlight_hold_offset(
                    solved.slopes[1],
                    solved.pivots[1],
                    point,
                    d_min,
                    Some(paper),
                    exposure.preflash,
                    exposure.grade,
                    Some(midtone),
                )
            })
        } else {
            0.0
        };
        let bw = collapse_weights(run.process, &exposure);
        let ranges = channel_density_ranges(run.bounds);
        let ev_scale = crate::features::exposure::print::local_ev_scale(run.bounds);
        let d_min_rgb = paper_dmin_rgb(d_min, Some(paper));
        Self {
            pivots: [
                solved.pivots[0] as f32,
                solved.pivots[1] as f32,
                solved.pivots[2] as f32,
                toe[0] as f32,
            ],
            slopes: [
                solved.slopes[0] as f32,
                solved.slopes[1] as f32,
                solved.slopes[2] as f32,
                toe[1] as f32,
            ],
            curvatures: [
                solved.curvatures[0] as f32,
                solved.curvatures[1] as f32,
                solved.curvatures[2] as f32,
                toe[2] as f32,
            ],
            cmy_offsets: [cmy[0], cmy[1], cmy[2], shoulder[0] as f32],
            shadow_cmy: [
                shadow_cmy[0],
                shadow_cmy[1],
                shadow_cmy[2],
                shoulder[1] as f32,
            ],
            highlight_cmy: [
                highlight_cmy[0],
                highlight_cmy[1],
                highlight_cmy[2],
                shoulder[2] as f32,
            ],
            toe_width_r: toe_width[0] as f32,
            toe_width_g: toe_width[1] as f32,
            toe_width_b: toe_width[2] as f32,
            shoulder_width_r: shoulder_width[0] as f32,
            shadow_density: (exposure.shadow_density + shadow_auto) as f32,
            d_max: tonal.d_max as f32,
            a_toe_base: tonal.toe_sharpness_base as f32,
            a_sh_base: tonal.shoulder_sharpness_base as f32,
            sep_damping: damping as f32,
            toe_height: tonal.toe_height as f32,
            sh_height: tonal.shoulder_height as f32,
            zone_center: c::REFERENCE_PRINT_DENSITY as f32,
            shoulder_width_g: shoulder_width[1] as f32,
            sep_k_r: dye_k[0] as f32,
            mode: u32::from(is_bw),
            v_star: reference_linear_value(d_min, Some(paper), None) as f32,
            shoulder_width_b: shoulder_width[2] as f32,
            gamma_width: tonal.paper_gamma_width as f32,
            use_dye: u32::from(composed.is_some()),
            bpc: if exposure.paper_black { 0.0 } else { 1.0 },
            d_min_rgb: [
                d_min_rgb[0] as f32,
                d_min_rgb[1] as f32,
                d_min_rgb[2] as f32,
                (exposure.highlight_density + highlight_auto) as f32,
            ],
            dye_r: [
                dye_rows[0][0] as f32,
                dye_rows[0][1] as f32,
                dye_rows[0][2] as f32,
                midtone_trims[0],
            ],
            dye_g: [
                dye_rows[1][0] as f32,
                dye_rows[1][1] as f32,
                dye_rows[1][2] as f32,
                midtone_trims[1],
            ],
            dye_b: [
                dye_rows[2][0] as f32,
                dye_rows[2][1] as f32,
                dye_rows[2][2] as f32,
                midtone_trims[2],
            ],
            ev_scale: [
                ev_scale[0],
                ev_scale[1],
                ev_scale[2],
                u32::from(run.local_exposure.is_some()) as f32,
            ],
            split_sh: [
                solved.shadow_grade[0] as f32,
                solved.shadow_grade[1] as f32,
                solved.shadow_grade[2] as f32,
                dye_k[1] as f32,
            ],
            split_hi: [
                solved.highlight_grade[0] as f32,
                solved.highlight_grade[1] as f32,
                solved.highlight_grade[2] as f32,
                dye_k[2] as f32,
            ],
            hue: [
                run.process.hue_trim.to_radians() as f32,
                exposure.preflash.max(0.0) as f32,
                flash_v as f32,
                flash_gamma as f32,
            ],
            mask: [0.0; 4],
            mask_span: [0.0; 4],
            mask_tone: [0.0; 4],
            keyed: [[0.0; 4]; 4],
            key_meta: [
                run.key_params.map_or(0.0, |params| params.len() as f32),
                exposure.grade as f32,
                c::ISO_R_MIN as f32,
                c::ISO_R_MAX as f32,
            ],
            diff_range: [
                ranges[0] as f32,
                ranges[1] as f32,
                ranges[2] as f32,
                run.black_hold as f32,
            ],
            lut: [0.0; 4],
            bw: [bw[0], bw[1], bw[2], 0.0],
        }
    }
}

fn layout(gpu: &GpuDevice) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("exposure"),
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
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
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
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        })
}

fn dummy(gpu: &GpuDevice, label: &str) -> GpuTexture {
    let texture = GpuTexture::new(&gpu.device, 1, 1, label);
    texture.upload(&gpu.queue, &[0.0, 0.0, 0.0], false);
    texture
}

/// Run the print pass over the normalized texture.
pub fn run(
    gpu: &GpuDevice,
    loaders: &ShaderLoader,
    source: &GpuTexture,
    output: &GpuTexture,
    uniforms: &ExposureUniforms,
) {
    let module = loaders.load(&gpu.device, "exposure.print", SHADER);
    let uniform_buffer = GpuBuffer::new(
        &gpu.device,
        std::mem::size_of::<ExposureUniforms>() as u64,
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        "exposure.uniforms",
    );
    uniform_buffer.upload(&gpu.queue, bytemuck::bytes_of(uniforms));
    let ev = dummy(gpu, "exposure.ev");
    let mask = dummy(gpu, "exposure.mask");
    let key = dummy(gpu, "exposure.key");
    let lut = dummy(gpu, "exposure.lut");
    let bind_layout = layout(gpu);
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("exposure"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("exposure"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("exposure"),
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
                resource: wgpu::BindingResource::TextureView(&ev.view),
            },
            BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&mask.view),
            },
            BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&key.view),
            },
            BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&lut.view),
            },
        ],
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("exposure"),
        });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("exposure"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(output.width.div_ceil(8), output.height.div_ceil(8), 1);
    }
    gpu.queue.submit(Some(encoder.finish()));
}
