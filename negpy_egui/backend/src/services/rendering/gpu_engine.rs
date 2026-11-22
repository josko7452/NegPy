//! The GPU chain: geometry, normalization and the display encode on one device.
//!
//! Bounds and meters stay on the CPU, so the chain takes the bounds a base pass
//! measured and stretches the same log densities. Each pass keeps its texture,
//! so a later stage reads the previous result without a readback.

use std::sync::Arc;

use crate::domain::models::WorkspaceConfig;
use crate::features::exposure::exposure_shader::{
    ExposureRun, ExposureUniforms, run_with as run_exposure,
};
use crate::features::exposure::histogram_shader::{
    DensityHistogramUniforms, run as run_density_histogram,
};
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::shader::{
    CROSSOVER_TABLE_BYTES, NormalizationSetup, NormalizationUniforms, run as run_normalization,
    run_encode,
};
use crate::features::geometry::crop_shader;
use crate::features::geometry::shader::{GeometryUniforms, rotated_dims, run as run_geometry};
use crate::features::lab::clahe_shader;
use crate::features::process::path::is_mono;
use crate::features::transparency::logic::DensityStage;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::{GpuTexture, TexturePool};
use crate::infrastructure::gpu::shader_loader::ShaderLoader;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The values one base run reads beyond the config.
#[derive(Clone, Debug)]
pub struct BaseRun {
    pub bounds: LogNegativeBounds,
    pub stage: DensityStage,
    pub crossover: Vec<u8>,
    pub transfer: bool,
}

impl BaseRun {
    /// A run with no density stage and no crossover table.
    #[must_use]
    pub fn plain(bounds: LogNegativeBounds) -> Self {
        Self {
            bounds,
            stage: DensityStage {
                pre: None,
                unmix: None,
                post: None,
                anchor: None,
            },
            crossover: vec![0u8; CROSSOVER_TABLE_BYTES],
            transfer: false,
        }
    }
}

/// The GPU engine, holding the device and its shader cache.
pub struct GpuEngine {
    gpu: Arc<GpuDevice>,
    loaders: ShaderLoader,
    textures: TexturePool,
    prefix: Option<CachedPrefix>,
    source_key: String,
}

/// The last geometry and normalization output, keyed by every input they read.
struct CachedPrefix {
    key: String,
    width: usize,
    height: usize,
    normalized: GpuTexture,
}

/// The key of the prefix passes: geometry and the normalization stage.
fn prefix_key(config: &WorkspaceConfig, run: &BaseRun, source_key: &str) -> String {
    crate::kernel::caching::logic::calculate_value_hash(&serde_json::json!({
        "source": source_key,
        "geometry": config.geometry,
        "process": config.process,
        "stage": format!("{:?}", run.stage),
        "bounds": [run.bounds.floors, run.bounds.ceils],
        "crossover": run.crossover,
        "transfer": run.transfer,
    }))
}

impl GpuEngine {
    /// Open the process device, or `None` when the GPU is disabled or no
    /// adapter opens. `override.toml`'s CPU backend and Preferences both clear
    /// `use_gpu`.
    #[must_use]
    pub fn new() -> Option<Self> {
        if !crate::kernel::system::config::app_config().use_gpu {
            return None;
        }
        crate::infrastructure::gpu::device::device().map(|gpu| Self {
            gpu,
            loaders: ShaderLoader::default(),
            textures: TexturePool::default(),
            prefix: None,
            source_key: String::new(),
        })
    }

    /// The adapter the chain runs on.
    #[must_use]
    pub fn device(&self) -> &GpuDevice {
        &self.gpu
    }

    /// The cached prefix's key, or an empty string when there is none.
    #[must_use]
    pub fn cached_prefix_key(&self) -> String {
        self.prefix
            .as_ref()
            .map(|prefix| prefix.key.clone())
            .unwrap_or_default()
    }

    /// Identify the source whose pixels the prefix cache belongs to.
    pub fn set_source(&mut self, key: &str) {
        if self.source_key != key {
            self.source_key.clear();
            self.source_key.push_str(key);
            self.prefix = None;
        }
    }

    /// Geometry, normalization and the display encode. The result is the
    /// display-encoded frame the CPU engine's last stage would hand the canvas.
    #[must_use]
    pub fn run_base(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
    ) -> ImageBuffer {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let source = self.textures.take(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            self.textures
                .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
        let uniforms = NormalizationUniforms::pack(
            NormalizationSetup {
                stage: &run.stage,
                bounds: run.bounds,
                mono: is_mono(&config.process),
                transfer: run.transfer,
            },
            &run.crossover,
        );
        run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &uniforms);

        let encoded = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "encoded");
        run_encode(&self.gpu, &self.loaders, &normalized, &encoded);
        let rgba = encoded.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(out_w * out_h * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        self.textures.put(encoded);
        self.textures.put(normalized);
        self.textures.put(geometry);
        self.textures.put(source);
        ImageBuffer::from_rgb(out_w, out_h, pixels)
    }

    /// The encoded frame and the density histogram of its normalized values.
    #[must_use]
    pub fn run_base_with_histogram(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
    ) -> (ImageBuffer, Vec<f64>) {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let source = self.textures.take(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            self.textures
                .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
        let uniforms = NormalizationUniforms::pack(
            NormalizationSetup {
                stage: &run.stage,
                bounds: run.bounds,
                mono: is_mono(&config.process),
                transfer: run.transfer,
            },
            &run.crossover,
        );
        run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &uniforms);
        let histogram = run_density_histogram(
            &self.gpu,
            &self.loaders,
            &normalized,
            &geometry,
            &DensityHistogramUniforms::new((0, out_h, 0, out_w), run.bounds),
        );

        let encoded = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "encoded");
        run_encode(&self.gpu, &self.loaders, &normalized, &encoded);
        let rgba = encoded.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(out_w * out_h * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        (ImageBuffer::from_rgb(out_w, out_h, pixels), histogram)
    }

    /// A print frame with no local maps, run to the display-encoded crop.
    ///
    /// The caller passes the solved print inputs the CPU exposure stage would
    /// read, so a frame that needs a mask or LUT stays on the CPU.
    #[must_use]
    pub fn run_print(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
        print: &ExposureRun<'_>,
        roi: Option<crate::features::geometry::logic::Roi>,
    ) -> ImageBuffer {
        self.run_print_with(image, config, run, print, roi, true)
    }

    /// The same chain, encoded only when the CPU tail is not going to run.
    #[must_use]
    pub fn run_print_with(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
        print: &ExposureRun<'_>,
        roi: Option<crate::features::geometry::logic::Roi>,
        encode: bool,
    ) -> ImageBuffer {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let key = prefix_key(config, run, &self.source_key);
        let reuse = self.prefix.as_ref().is_some_and(|prefix| {
            prefix.key == key && prefix.width == out_w && prefix.height == out_h
        });
        if !reuse {
            self.prefix = None;
            let source = self.textures.take(
                &self.gpu.device,
                image.width as u32,
                image.height as u32,
                "source",
            );
            source.upload(&self.gpu.queue, &image.pixels, false);
            let geometry =
                self.textures
                    .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
            let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
            run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);
            let normalized =
                self.textures
                    .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
            let value = NormalizationUniforms::pack(
                NormalizationSetup {
                    stage: &run.stage,
                    bounds: run.bounds,
                    mono: is_mono(&config.process),
                    transfer: run.transfer,
                },
                &run.crossover,
            );
            run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &value);
            self.textures.put(source);
            self.textures.put(geometry);
            self.prefix = Some(CachedPrefix {
                key,
                width: out_w,
                height: out_h,
                normalized,
            });
        }
        let prefix = self.prefix.as_ref().expect("the prefix was just filled");
        let normalized = &prefix.normalized;

        let printed = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "printed");
        let planes = print.borrowed_planes();
        run_exposure(
            &self.gpu,
            &self.loaders,
            normalized,
            &printed,
            &ExposureUniforms::pack_with(print, &planes),
            &planes,
        );

        let window = roi.unwrap_or((0, out_h as i64, 0, out_w as i64));
        let (cropped, crop_uniforms) = {
            let (out_w, out_h) = {
                let (y1, y2, x1, x2) = window;
                ((x2 - x1) as u32, (y2 - y1) as u32)
            };
            let texture = self
                .textures
                .take(&self.gpu.device, out_w, out_h, "cropped");
            let (_, uniforms) =
                crop_shader::target(&self.gpu, window, out_w as usize, out_h as usize, "cropped");
            (texture, uniforms)
        };
        crop_shader::run(&self.gpu, &self.loaders, &printed, &cropped, &crop_uniforms);
        let rgba = cropped.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(cropped.width as usize * cropped.height as usize * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        if !encode {
            pixels = crate::features::exposure::analysis::working_oetf_decode(&pixels);
        }
        let (width, height) = (cropped.width as usize, cropped.height as usize);
        self.textures.put(cropped);
        self.textures.put(printed);
        ImageBuffer::from_rgb(width, height, pixels)
    }

    /// The print chain through toning and its crop, as encoded pixels.
    ///
    /// The full frame is kept through CLAHE and Lab, whose grids span the
    /// rotated frame; the toning pass reads the crop window. The finish stage
    /// reads the cropped, display-encoded frame.
    #[must_use]
    pub fn run_tail_input(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
        print: &ExposureRun<'_>,
        lab: crate::features::lab::lab_shader::LabRun,
        roi: Option<crate::features::geometry::logic::Roi>,
    ) -> (
        ImageBuffer,
        crate::features::lab::lab_shader::LabOutput,
        Vec<f64>,
    ) {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let key = prefix_key(config, run, &self.source_key);
        let reuse = self.prefix.as_ref().is_some_and(|prefix| {
            prefix.key == key && prefix.width == out_w && prefix.height == out_h
        });
        if !reuse {
            self.prefix = None;
            let source = self.textures.take(
                &self.gpu.device,
                image.width as u32,
                image.height as u32,
                "source",
            );
            source.upload(&self.gpu.queue, &image.pixels, false);
            let geometry =
                self.textures
                    .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
            let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
            run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);
            let normalized =
                self.textures
                    .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
            let value = NormalizationUniforms::pack(
                NormalizationSetup {
                    stage: &run.stage,
                    bounds: run.bounds,
                    mono: is_mono(&config.process),
                    transfer: run.transfer,
                },
                &run.crossover,
            );
            run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &value);
            self.textures.put(source);
            self.textures.put(geometry);
            self.prefix = Some(CachedPrefix {
                key,
                width: out_w,
                height: out_h,
                normalized,
            });
        }
        let prefix = self.prefix.as_ref().expect("the prefix was just filled");
        let normalized = &prefix.normalized;

        let printed = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "printed");
        let planes = print.borrowed_planes();
        run_exposure(
            &self.gpu,
            &self.loaders,
            normalized,
            &printed,
            &ExposureUniforms::pack_with(print, &planes),
            &planes,
        );
        let density_histogram = run_density_histogram(
            &self.gpu,
            &self.loaders,
            normalized,
            &printed,
            &DensityHistogramUniforms::new((0, out_h, 0, out_w), run.bounds),
        );

        let strength = config.lab.clahe_strength as f32;
        let remapped = if strength > 0.0 {
            let texture = self
                .textures
                .take(&self.gpu.device, out_w as u32, out_h as u32, "clahe");
            clahe_shader::run(
                &self.gpu,
                &self.loaders,
                &printed,
                &texture,
                &clahe_shader::ClaheUniforms::for_frame(strength, out_w, out_h),
            );
            self.textures.put(printed);
            texture
        } else {
            printed
        };

        let labbed = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "lab");
        let lab_out = crate::features::lab::lab_shader::run(
            &self.gpu,
            &self.loaders,
            &remapped,
            &labbed,
            &config.lab,
            lab,
            &self.textures,
        );
        self.textures.put(remapped);

        // The alt-process passes are not ported yet, so their frames stay on
        // the CPU engine (`gpu_eligible` rejects them).
        let window = roi.unwrap_or((0, out_h as i64, 0, out_w as i64));
        let (crop_w, crop_h) = ((window.3 - window.2) as u32, (window.1 - window.0) as u32);
        let toned = self
            .textures
            .take(&self.gpu.device, crop_w, crop_h, "toned");
        crate::features::toning::shader::run(
            &self.gpu,
            &self.loaders,
            &labbed,
            &toned,
            &crate::features::toning::shader::ToningUniforms::new(
                &config.toning,
                config.lab.saturation,
                is_mono(&config.process),
                config.altproc.alt_process,
                [window.2 as i32, window.0 as i32],
            ),
        );
        self.textures.put(labbed);

        let rgba = toned.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(crop_w as usize * crop_h as usize * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        let (width, height) = (toned.width as usize, toned.height as usize);
        self.textures.put(toned);
        (
            ImageBuffer::from_rgb(width, height, pixels),
            lab_out,
            density_histogram,
        )
    }

    fn encode_cropped(
        &self,
        source: &GpuTexture,
        roi: Option<crate::features::geometry::logic::Roi>,
        out_w: usize,
        out_h: usize,
    ) -> ImageBuffer {
        let window = roi.unwrap_or((0, out_h as i64, 0, out_w as i64));
        let (cropped, crop_uniforms) = {
            let (out_w, out_h) = {
                let (y1, y2, x1, x2) = window;
                ((x2 - x1) as u32, (y2 - y1) as u32)
            };
            let texture = self
                .textures
                .take(&self.gpu.device, out_w, out_h, "cropped");
            let (_, uniforms) =
                crop_shader::target(&self.gpu, window, out_w as usize, out_h as usize, "cropped");
            (texture, uniforms)
        };
        crop_shader::run(&self.gpu, &self.loaders, source, &cropped, &crop_uniforms);
        let encoded = GpuTexture::new(&self.gpu.device, cropped.width, cropped.height, "encoded");
        run_encode(&self.gpu, &self.loaders, &cropped, &encoded);
        let rgba = encoded.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(encoded.width as usize * encoded.height as usize * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        ImageBuffer::from_rgb(encoded.width as usize, encoded.height as usize, pixels)
    }

    /// Geometry, normalization, crop and the display encode, held on the device.
    #[must_use]
    pub fn run_base_cropped(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
        roi: Option<crate::features::geometry::logic::Roi>,
    ) -> ImageBuffer {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let source = self.textures.take(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            self.textures
                .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
        let uniforms = NormalizationUniforms::pack(
            NormalizationSetup {
                stage: &run.stage,
                bounds: run.bounds,
                mono: is_mono(&config.process),
                transfer: run.transfer,
            },
            &run.crossover,
        );
        run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &uniforms);

        self.encode_cropped(&normalized, roi, out_w, out_h)
    }

    /// Run the geometry and normalization passes and return the normalized frame.
    #[must_use]
    pub fn run_normalized(
        &mut self,
        image: &ImageBuffer,
        config: &WorkspaceConfig,
        run: &BaseRun,
    ) -> ImageBuffer {
        let (out_w, out_h) = rotated_dims(image.width, image.height, config.geometry.rotation);
        let source = self.textures.take(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = self
            .textures
            .take(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            self.textures
                .take(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
        let uniforms = NormalizationUniforms::pack(
            NormalizationSetup {
                stage: &run.stage,
                bounds: run.bounds,
                mono: is_mono(&config.process),
                transfer: run.transfer,
            },
            &run.crossover,
        );
        run_normalization(&self.gpu, &self.loaders, &geometry, &normalized, &uniforms);
        let rgba = normalized.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(out_w * out_h * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        ImageBuffer::from_rgb(out_w, out_h, pixels)
    }
}
