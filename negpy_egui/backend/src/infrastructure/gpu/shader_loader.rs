//! Mirrors `negpy/infrastructure/gpu/shader_loader.py`: WGSL sources compiled on demand.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderSource};

/// A cache of compiled modules, keyed by shader name. A module belongs to the
/// device that compiled it, so a new device empties the cache.
#[derive(Default)]
pub struct ShaderLoader {
    modules: Mutex<HashMap<&'static str, Arc<ShaderModule>>>,
}

impl ShaderLoader {
    /// Compile `source` under `name`, or return the module compiled earlier.
    #[must_use]
    pub fn load(&self, device: &Device, name: &'static str, source: &str) -> Arc<ShaderModule> {
        let mut modules = self.modules.lock().expect("shader cache poisoned");
        if let Some(module) = modules.get(name) {
            return Arc::clone(module);
        }
        let module = Arc::new(device.create_shader_module(ShaderModuleDescriptor {
            label: Some(name),
            source: ShaderSource::Wgsl(source.into()),
        }));
        modules.insert(name, Arc::clone(&module));
        module
    }

    /// Drop every cached module, for a device that went away.
    pub fn clear(&self) {
        self.modules.lock().expect("shader cache poisoned").clear();
    }
}
