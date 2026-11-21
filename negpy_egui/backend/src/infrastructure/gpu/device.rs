//! Mirrors `negpy/infrastructure/gpu/device.py`: the hardware adapter and device.
//!
//! A device is expensive to open and is shared by every render, so one lazy
//! singleton owns it. An adapter that cannot be opened leaves the singleton
//! unavailable and the CPU engine renders instead.

use std::sync::{Arc, Mutex, OnceLock};

use wgpu::{AdapterInfo, Device, DeviceType, Instance, PowerPreference, Queue};

/// The limits the shaders need from the adapter.
const REQUIRED_LIMITS: wgpu::Limits = wgpu::Limits::downlevel_defaults();

/// The hardware device and the queue that submits its work.
pub struct GpuDevice {
    pub adapter: wgpu::Adapter,
    pub device: Device,
    pub queue: Queue,
    pub info: AdapterInfo,
}

impl GpuDevice {
    /// The backend name, as the Python device reports it.
    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        match self.adapter.get_info().backend {
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "Dx12",
            wgpu::Backend::Gl => "Gl",
            wgpu::Backend::BrowserWebGpu => "WebGpu",
            wgpu::Backend::Noop => "Noop",
        }
    }

    /// True for a GPU that shares its memory with the CPU.
    #[must_use]
    pub fn is_integrated(&self) -> bool {
        self.info.device_type == DeviceType::IntegratedGpu
    }

    /// Block until the queue has finished the submitted work.
    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait);
    }
}

static DEVICE: OnceLock<Mutex<Open>> = OnceLock::new();

/// One process-wide open attempt; a failed attempt stores `None`.
#[derive(Default)]
struct Open {
    attempted: bool,
    device: Option<Arc<GpuDevice>>,
}

/// The process's device, or `None` when no adapter opened.
///
/// The first call tries every enabled backend once; a later call reads the
/// stored outcome instead of reopening the device.
pub fn device() -> Option<Arc<GpuDevice>> {
    let cache = DEVICE.get_or_init(|| Mutex::new(Open::default()));
    let mut cache = cache.lock().ok()?;
    if !cache.attempted {
        cache.attempted = true;
        cache.device = try_open();
    }
    cache.device.clone()
}

fn try_open() -> Option<Arc<GpuDevice>> {
    let instance = Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("negfe"),
        required_features: wgpu::Features::empty(),
        required_limits: REQUIRED_LIMITS,
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .ok()?;
    let info = adapter.get_info();
    Some(Arc::new(GpuDevice {
        adapter,
        device,
        queue,
        info,
    }))
}
