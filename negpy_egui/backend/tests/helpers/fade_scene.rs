//! The crossover scene and film model from `tests/fade_crossover_scene.py`, as a test helper.

use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

/// Log10 scene exposure of each patch channel, relative to a mid gray at 0.
pub const SKY: [f64; 3] = [-0.45, -0.3, -0.1];
pub const FOLIAGE: [f64; 3] = [-0.85, -0.6, -1.1];
pub const SKIN: [f64; 3] = [-0.22, -0.38, -0.48];
pub const PATCH_STOPS: [f64; 3] = [-0.6, 0.0, 0.45];
pub const RAMP_STEPS: usize = 32;
pub const RAMP_RANGE: (f64, f64) = (-2.4, 0.9);

#[derive(Clone, Copy)]
pub struct Layer {
    pub d_min: f64,
    pub d_max: f64,
    pub gamma: f64,
    pub e0: f64,
}

#[derive(Clone, Copy)]
pub struct Film {
    pub red: Layer,
    pub green: Layer,
    pub blue: Layer,
}

fn layer_density(layer: Layer, log_e: f64) -> f64 {
    let span = layer.d_max - layer.d_min;
    layer.d_min + span / (1.0 + (4.0 * layer.gamma / span * (log_e - layer.e0)).exp())
}

fn scan_pixel(film: &Film, log_e: [f64; 3]) -> [f32; 3] {
    let channels = [
        (film.red, log_e[0]),
        (film.green, log_e[1]),
        (film.blue, log_e[2]),
    ];
    channels.map(|(layer, value)| 10f64.powf(-layer_density(layer, value)) as f32)
}

/// A crossover fade: every layer keeps its own toe and shoulder.
pub const CROSSOVER: Film = Film {
    red: Layer {
        d_min: 0.12,
        d_max: 3.2,
        gamma: 1.65,
        e0: -0.6,
    },
    green: Layer {
        d_min: 0.12,
        d_max: 2.8,
        gamma: 1.4,
        e0: -0.5,
    },
    blue: Layer {
        d_min: 0.12,
        d_max: 2.5,
        gamma: 1.85,
        e0: -0.72,
    },
};

pub struct Scene {
    pub width: usize,
    pub height: usize,
    pub log_e: Vec<[f64; 3]>,
}

/// The gray ramp on the top band and the colored patches below it.
pub fn build_scene(width: usize, height: usize) -> Scene {
    let mut log_e = vec![[0.0; 3]; width * height];
    let ramp_h = height / 3;
    let (low, high) = RAMP_RANGE;
    let step = (high - low) / (RAMP_STEPS - 1) as f64;
    for index in 0..RAMP_STEPS {
        let e = low + step * index as f64;
        let x0 = index * width / RAMP_STEPS;
        let x1 = (index + 1) * width / RAMP_STEPS;
        for y in 0..ramp_h {
            for x in x0..x1 {
                log_e[y * width + x] = [e; 3];
            }
        }
    }
    let rest_low = low + 0.3;
    let rest_high = high - 0.3;
    let step_x = (rest_high - rest_low) / (width - 1) as f64;
    for y in ramp_h..height {
        for x in 0..width {
            log_e[y * width + x] = [rest_low + step_x * x as f64; 3];
        }
    }
    let patches = [SKY, FOLIAGE, SKIN];
    let (pw, ph) = (width / 10, height / 9);
    let y0 = ramp_h + ph / 2;
    for (row, colour) in patches.iter().enumerate() {
        for (col, stop) in PATCH_STOPS.iter().enumerate() {
            let y_start = y0 + row * (ph + ph / 3);
            let x_start = width / 20 + col * (pw + pw / 3);
            for y in y_start..(y_start + ph).min(height) {
                for x in x_start..(x_start + pw).min(width) {
                    log_e[y * width + x] = std::array::from_fn(|ch| colour[ch] + *stop);
                }
            }
        }
    }
    Scene {
        width,
        height,
        log_e,
    }
}

/// The film's linear scan of `scene`.
pub fn scan_image(film: &Film, scene: &Scene) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(scene.width * scene.height * 3);
    for pixel in &scene.log_e {
        pixels.extend_from_slice(&scan_pixel(film, *pixel));
    }
    ImageBuffer::from_rgb(scene.width, scene.height, pixels)
}
