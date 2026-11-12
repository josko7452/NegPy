//! Shared value types and the application configuration.
//!
//! Mirrors `negpy/domain/types.py`. The luma row is Adobe RGB (1998), the working space,
//! rounded so that the weights sum to exactly one: a gray's luma is then its own value.

/// Luminance row of Adobe RGB (1998), the working space, rounded so that the weights sum
/// to 1.
pub const LUMA_R: f32 = 0.2974;
pub const LUMA_G: f32 = 0.6273;
pub const LUMA_B: f32 = 0.0753;
pub const LUMA_COEFFS: [f32; 3] = [LUMA_R, LUMA_G, LUMA_B];

/// The Lab luminance row from `features/lab/logic.py` (`LUM_R`, `LUM_G`, `LUM_B`).
pub const LUM_R: f32 = 0.2973769;
pub const LUM_G: f32 = 0.6273491;
pub const LUM_B: f32 = 0.0752741;

/// A region of interest as `(y1, y2, x1, x2)`.
pub type Roi = (i64, i64, i64, i64);

/// A buffer's size as `(height, width)`.
pub type Dimensions = (i64, i64);

/// The paths and limits assembled once at startup.
#[derive(Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub thumbnail_size: i64,
    pub max_workers: i64,
    pub preview_render_size: i64,
    pub max_history_steps: i64,
    pub edits_db_path: String,
    pub settings_db_path: String,
    pub presets_dir: String,
    pub cache_dir: String,
    pub user_icc_dir: String,
    pub crosstalk_dir: String,
    pub sensor_dir: String,
    pub flatfield_dir: String,
    pub gear_dir: String,
    pub default_export_dir: String,
    pub adobe_rgb_profile: String,
    pub use_gpu: bool,
    pub override_toml_path: String,
    pub max_texture_size: Option<i64>,
    pub force_hq_preview: Option<bool>,
    /// Smaller tiles and no readback pipelining in the tiled export path, at the cost of
    /// export speed. Off by default; a user opts in from Preferences or override.toml when
    /// their GPU needs it.
    pub low_vram_export_tiling: bool,
    pub preview_cache_max_entries: i64,
    pub preview_cache_max_bytes: i64,
    /// Full-resolution (HQ) entries are hundreds of megabytes each, so they get their own
    /// slot budget inside the LRU.
    pub preview_cache_max_full_res_entries: i64,
    /// Rendered frames kept for instant navigate-back, tens of megabytes of VRAM each at
    /// `preview_render_size`.
    pub render_memo_max_entries: i64,
    pub canvas_zoom_min: f64,
    pub canvas_zoom_max: f64,
}

impl AppConfig {
    /// The defaults the Python `AppConfig` declares for the fields that have one. The
    /// path fields are empty: the caller fills them from the resolved user directory.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            thumbnail_size: 256,
            max_workers: 1,
            preview_render_size: 1600,
            max_history_steps: 100,
            edits_db_path: String::new(),
            settings_db_path: String::new(),
            presets_dir: String::new(),
            cache_dir: String::new(),
            user_icc_dir: String::new(),
            crosstalk_dir: String::new(),
            sensor_dir: String::new(),
            flatfield_dir: String::new(),
            gear_dir: String::new(),
            default_export_dir: String::new(),
            adobe_rgb_profile: String::new(),
            use_gpu: true,
            override_toml_path: String::new(),
            max_texture_size: None,
            force_hq_preview: None,
            low_vram_export_tiling: false,
            preview_cache_max_entries: 8,
            preview_cache_max_bytes: 1_200_000_000,
            preview_cache_max_full_res_entries: 2,
            render_memo_max_entries: 8,
            canvas_zoom_min: 0.25,
            canvas_zoom_max: 8.0,
        }
    }
}
