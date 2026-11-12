//! The frozen backend <-> GUI contract.
//!
//! Two enums replace the Python desktop layer's 456 `pyqtSignal`s. The backend emits
//! [`BackendEvent`]; the GUI sends [`BackendCommand`]. The GUI never calls into the
//! backend except through a command, and the backend never touches the GUI.
//! `negpy_egui/docs/EVENT_CONTRACT.md` puts every Python signal in one bucket.
//!
//! This module fixes the contract shape. A structured payload whose typed struct lands
//! with its feature port (D7-D12) travels as an encoded [`FeaturePayload`] until then.

use std::path::PathBuf;

/// An encoded result whose typed struct lands with its feature port.
pub type FeaturePayload = Vec<u8>;

/// A `current / total` step with a label, for one batch family.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub current: u32,
    pub total: u32,
    pub label: String,
}

/// A scan or calibration step: a fraction and a phase name.
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseProgress {
    pub fraction: f32,
    pub phase: String,
}

/// The four light levels a capture sets: red, green, blue and white.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureLevels {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
}

/// The render metrics the GUI reads (histogram, clipping, and their peers).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderMetrics(pub Vec<(String, f64)>);

/// A first-paint frame: the embedded-JPEG splash of a slow decode.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: FeaturePayload,
}

/// A finished render: its pixel size and the metrics read back with it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderResult {
    pub width: u32,
    pub height: u32,
    pub metrics: RenderMetrics,
}

/// The three candidate frames of a Pick Better comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct PickBetterFrames {
    pub current: FeaturePayload,
    pub left: FeaturePayload,
    pub right: FeaturePayload,
}

/// A pixel probe: content-normalized coordinates and the display-encoded color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelReadout {
    pub nx: f32,
    pub ny: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// A densitometer reading in density units per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DensitometerReading {
    pub density: [f32; 3],
}

/// The connection state of the camera and the Scanlight.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionStatus {
    pub usb_ok: bool,
    pub usb_model: Option<String>,
    pub light_ok: bool,
    pub light_detail: Option<String>,
}

/// A backend -> GUI notification: data or state the GUI displays.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BackendEvent {
    // Session state (negpy/desktop/session.py): what the desktop models mirror.
    StateChanged,
    FilesChanged,
    HistoryChanged,
    WorkPrintsChanged,
    SettingsSaved,
    FramePreviewsEnding,
    ActiveFileChanging,
    SettingsCopied,
    SettingsPasted,
    ColorVisionChanged,
    AutoThumbnailRefreshChanged(bool),
    CanvasBackgroundChanged,
    SettingsSynced(String),
    FramesEditedOffscreen(Vec<String>),
    BatchRecorded,
    FileSelected(String),
    SessionEmptied,
    ConfigUpdated,
    MonitorProfileChanged,

    // Frame loading and render (controller.py, workers/render.py).
    ImageUpdated,
    PreviewLoaded,
    LoadingStarted,
    LoadFailed,
    SplashReady(PreviewFrame),
    PrefetchFinished {
        generation: u64,
        path: String,
    },
    RenderBusy(String),
    RenderActivity(String),
    RenderPartial(RenderMetrics),
    RenderFinished(RenderResult),
    MetricsUpdated(RenderMetrics),
    RenderError(String),
    VramCapped {
        device: String,
        megabytes: u32,
    },
    RgbGrouped(RenderMetrics),
    SplitsDetected(RenderMetrics),
    ScenesDetected,
    FirstSceneCreated,
    RollFolderMissing(String),
    ExportViewReady,
    StripProgress(Progress),
    StripFinished,
    PickBetterReady(PickBetterFrames),
    PixelReadout(Option<PixelReadout>),
    DensitometerReadout(Option<DensitometerReading>),
    BatchStarted {
        title: String,
        abortable: bool,
    },
    BatchProgress(Progress),
    BatchFinished,
    ThumbnailRefreshProgress(String),
    BaselineProgress(String),
    ThumbnailActivity(String),
    ThumbnailRefreshStateChanged(bool),

    // Export (workers/export.py).
    ExportProgress(Progress),
    ExportFinished {
        elapsed_s: f32,
        failures: Vec<String>,
    },
    ExportCancelled,
    ExportError(String),
    ExportWarning(String),
    ContactSheetWritten(String),

    // Thumbnails (workers/render.py).
    ThumbnailRendered,
    ThumbnailFrameStarted {
        index: u32,
        total: u32,
        name: String,
    },
    ThumbnailProgress {
        done: u32,
        total: u32,
        name: String,
        decode_s: f32,
        render_s: f32,
    },
    ThumbnailFinished(u32),
    ThumbnailCancelled,

    // Library (workers/library.py).
    LibraryProgress(u64),
    LibraryFinished(Vec<String>),
    LibraryError(String),
    LibrarySearchFinished(u32),
    LibraryCleared,
    LibraryIndexingScanned(Vec<String>),

    // Composite and analysis workers, in their progress and result pairs.
    JobProgress {
        family: JobFamily,
        progress: Progress,
    },
    JobCancelled(JobFamily),
    JobError {
        family: JobFamily,
        message: String,
    },
    JobResult {
        family: JobFamily,
        payload: FeaturePayload,
    },

    // Scan (workers/scan_worker.py).
    ScanDevicesReady(Vec<String>),
    ScanStarted,
    ScanProgress(PhaseProgress),
    ScanFinished(String),
    ScanFrameDone {
        frame: u32,
        path: String,
    },
    ScanBatchFinished(Vec<String>),
    ScanError(String),
    ScanCancelled,
    ScanEjected(bool),
    ScanEjectError(String),
    ScanStripReturned(bool),
    ScanMediaPolled {
        device: String,
        present: Option<bool>,
    },
    ScanRollPreviewReady(FeaturePayload),
    ScanRollPreviewFinished,
    ScanPrescanReady(FeaturePayload),
    ScanPrescanError(String),
    ScanExposureMetered {
        readings: FeaturePayload,
        frame: u32,
    },
    ScanMeterError(String),

    // Capture (workers/capture_worker.py).
    CaptureLightSet(CaptureLevels),
    CaptureProgress(f32),
    CaptureCountdown(u32),
    CaptureChannel(String),
    CaptureCameraSettingApplied(String),
    CaptureFinished(Vec<String>),
    CaptureClipped {
        path: String,
        share: f32,
    },
    CaptureCancelled,
    CaptureError(String),
    CaptureStatus(String),
    CaptureStripFinished {
        frames: u32,
        ran_to_end: bool,
    },
    CaptureScanButtonPressed,
    CaptureFlatCaptured(String),
    LiveViewStarted(String),
    LiveViewUnsupported(String),
    LiveViewFailed(String),
    FocusMagnifierUnavailable(String),
    CalibrationProgress(PhaseProgress),
    CalibrationFinished(FeaturePayload),
    CalibrationExposure(String),
    ExposureProbeFinished(FeaturePayload),
    ExposureProbeFailed(FeaturePayload),
    SensorResponseProgress(PhaseProgress),
    SensorResponseMeasured(FeaturePayload),
    SensorResponseFailed(String),
    PresencePolled {
        camera: bool,
        scanlight: bool,
    },
    ConnectionPolled(ConnectionStatus),
    LightTempPolled(Option<f32>),

    // Progress text the status bar shows for a running backend task.
    StatusMessage {
        text: String,
        level: u8,
        detail: String,
    },
    StatusProgress {
        current: u32,
        total: u32,
    },
}

/// The composite and analysis families that share one progress/result event pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobFamily {
    Stitch,
    Hdr,
    FrameMerge,
    Triplet,
    Embedding,
    Chart,
    AutoCrop,
}

/// A GUI -> backend request.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum BackendCommand {
    // Render and load.
    Render(RenderParams),
    PreviewLoad(PreviewLoadRequest),
    PrefetchLoad(PreviewLoadRequest),
    Normalize(NormalizationRequest),
    BackgroundNormalize(NormalizationRequest),
    BatchAutoCrop { generation: u64 },
    ThumbnailRender(ThumbnailRenderRequest),
    ThumbnailUpdate(ThumbnailUpdateRequest),
    ThumbnailCancel,
    ThumbnailRequest(Vec<String>),
    Embedding(Vec<String>),
    AnalysisBufferPreview(f32),

    // Assets and library.
    Discover(DiscoveryRequest),
    AutoDetectAllSplits { generation: u64 },
    LibrarySearch(String),
    LibraryIndexScan(Vec<PathBuf>),

    // Composite jobs and the export-size view.
    Stitch(Vec<String>),
    ContactSheet(FeaturePayload),
    Hdr(Vec<String>),
    FrameMerge(Vec<String>),
    TrichromeMerge(Vec<String>),
    ExportView { key: String },

    // In-canvas tools that the backend computes on the rendered buffer.
    TestStrip(FeaturePayload),
    PickBetter(FeaturePayload),
    PrintNotes,

    // Scan.
    ScanDevices,
    SetScanBackend(String),
    Scan(FeaturePayload),
    ScanBatch(FeaturePayload),
    ScanEject(String),
    ScanMediaPoll(String),
    ScanRollPreview(FeaturePayload),
    ScanPrescan(FeaturePayload),
    ScanMeter(FeaturePayload),

    // Capture.
    SetCaptureLight { levels: CaptureLevels, port: String },
    Capture(FeaturePayload),
    CaptureEject(String),
    LiveView(FeaturePayload),
    StopLiveView,
    CloseCameraSession,
    SetFocusMagnifier(bool),
    SetFocusMagnifierPos { x: i32, y: i32 },
    SetCameraSetting(CameraSetting),
    Calibrate(FeaturePayload),
    ExposureProbe(FeaturePayload),
    SensorResponse(FeaturePayload),
    PollPresence(String),
    PollConnection(String),
    PollLightTemp(String),
}

/// One camera setting write: its id, the value and the value's kind.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraSetting {
    pub id: String,
    pub value: i32,
    pub kind: String,
}

/// The parameters of one render. The newest request wins while a slider moves.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderParams {
    pub source_hash: String,
    pub generation: u64,
    pub preview_size: f32,
    pub interactive: bool,
    pub full_resolution: bool,
    pub gpu_enabled: bool,
    pub readback_metrics: bool,
    pub compare: bool,
    pub wants_thumbnail: bool,
    pub ephemeral: bool,
}

/// A frame decode request: the path, the generation and the decode mode.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewLoadRequest {
    pub file_path: String,
    pub generation: u64,
    pub full_resolution: bool,
    pub color_space: String,
    pub use_splash: bool,
}

/// A bounds-normalization run over one or more frames.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationRequest {
    pub frames: Vec<String>,
    pub detect: bool,
    pub background: bool,
}

/// A thumbnail render batch.
#[derive(Clone, Debug, PartialEq)]
pub struct ThumbnailRenderRequest {
    pub frames: Vec<String>,
    pub generation: u64,
}

/// One in-memory thumbnail update for a known frame hash.
#[derive(Clone, Debug, PartialEq)]
pub struct ThumbnailUpdateRequest {
    pub file_hash: String,
    pub persist: bool,
}

/// A folder-import discovery run.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryRequest {
    pub paths: Vec<PathBuf>,
    pub auto_open: bool,
}
