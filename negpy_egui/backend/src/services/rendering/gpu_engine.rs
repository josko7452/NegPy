//! The GPU chain: geometry, normalization and the display encode on one device.
//!
//! Bounds and meters stay on the CPU, so the chain takes the bounds a base pass
//! measured and stretches the same log densities. Each pass keeps its texture,
//! so a later stage reads the previous result without a readback.

use std::sync::Arc;

use crate::domain::models::WorkspaceConfig;
use crate::features::exposure::exposure_shader::{
    ExposureRun, ExposureUniforms, run as run_exposure,
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
use crate::features::process::path::is_mono;
use crate::features::transparency::logic::DensityStage;
use crate::infrastructure::gpu::device::GpuDevice;
use crate::infrastructure::gpu::resources::GpuTexture;
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
        })
    }

    /// The adapter the chain runs on.
    #[must_use]
    pub fn device(&self) -> &GpuDevice {
        &self.gpu
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
        let source = GpuTexture::new(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
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

        let encoded = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "encoded");
        run_encode(&self.gpu, &self.loaders, &normalized, &encoded);
        let rgba = encoded.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(out_w * out_h * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
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
        let source = GpuTexture::new(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
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

        let encoded = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "encoded");
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
        let source = GpuTexture::new(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
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

        let printed = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "printed");
        run_exposure(
            &self.gpu,
            &self.loaders,
            &normalized,
            &printed,
            &ExposureUniforms::pack(print),
        );

        let window = roi.unwrap_or((0, out_h as i64, 0, out_w as i64));
        let (cropped, crop_uniforms) =
            crop_shader::target(&self.gpu, window, out_w, out_h, "cropped");
        crop_shader::run(&self.gpu, &self.loaders, &printed, &cropped, &crop_uniforms);
        let rgba = cropped.readback(&self.gpu);
        let mut pixels = Vec::with_capacity(cropped.width as usize * cropped.height as usize * 3);
        for texel in rgba.chunks_exact(4) {
            pixels.extend_from_slice(&texel[..3]);
        }
        if !encode {
            pixels = crate::features::exposure::analysis::working_oetf_decode(&pixels);
        }
        ImageBuffer::from_rgb(cropped.width as usize, cropped.height as usize, pixels)
    }

    fn encode_cropped(
        &self,
        source: &GpuTexture,
        roi: Option<crate::features::geometry::logic::Roi>,
        out_w: usize,
        out_h: usize,
    ) -> ImageBuffer {
        let window = roi.unwrap_or((0, out_h as i64, 0, out_w as i64));
        let (cropped, crop_uniforms) =
            crop_shader::target(&self.gpu, window, out_w, out_h, "cropped");
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
        let source = GpuTexture::new(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
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
        let source = GpuTexture::new(
            &self.gpu.device,
            image.width as u32,
            image.height as u32,
            "source",
        );
        source.upload(&self.gpu.queue, &image.pixels, false);
        let geometry = GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "geometry");
        let uniforms = GeometryUniforms::from_config(&config.geometry, out_w, out_h);
        run_geometry(&self.gpu, &self.loaders, &source, &geometry, &uniforms);

        let normalized =
            GpuTexture::new(&self.gpu.device, out_w as u32, out_h as u32, "normalized");
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
