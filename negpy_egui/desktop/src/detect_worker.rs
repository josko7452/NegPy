//! Roll Analysis Detect: measure each strip frame on a background thread.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;

use negpy_backend::features::exposure::normalization::SceneAxis;

/// One Detect run over the strip's paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetectRequest {
    pub paths: Vec<Option<PathBuf>>,
    pub profile: PathBuf,
}

/// The axis each frame measured, or None when its decode failed.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectReport {
    pub axes: Vec<Option<SceneAxis>>,
}

/// The GUI side of the Detect worker.
pub struct DetectWorker {
    requests: Option<std::sync::mpsc::Sender<DetectRequest>>,
    results: Receiver<DetectReport>,
    thread: Option<JoinHandle<()>>,
}

impl DetectWorker {
    /// Start the worker thread.
    #[must_use]
    pub fn start() -> Self {
        let (requests, incoming) = channel();
        let (sender, results) = channel();
        let thread = std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                if sender.send(run(&request)).is_err() {
                    break;
                }
            }
        });
        Self {
            requests: Some(requests),
            results,
            thread: Some(thread),
        }
    }

    /// Queue one Detect run.
    pub fn submit(&self, request: DetectRequest) {
        if let Some(requests) = &self.requests {
            let _ = requests.send(request);
        }
    }

    /// Take the finished run, when the worker has one.
    #[must_use]
    pub fn drain(&self) -> Option<DetectReport> {
        match self.results.try_recv() {
            Ok(report) => Some(report),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for DetectWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Measure the light axis of every frame that decodes.
#[must_use]
pub fn run(request: &DetectRequest) -> DetectReport {
    let mut axes = Vec::with_capacity(request.paths.len());
    for path in &request.paths {
        let Some(path) = path.as_deref() else {
            axes.push(None);
            continue;
        };
        let Ok((source, config)) =
            crate::frame_source::load_linear_source(path, Some(&request.profile), 1600)
        else {
            axes.push(None);
            continue;
        };
        let processed = negpy_backend::features::geometry::processor::process(
            &source.image,
            &config.geometry,
            negpy_backend::features::geometry::processor::GeometryContext {
                scale_factor: source.image.height.max(source.image.width) as f64 / 4000.0,
            },
        );
        let width = processed.image.width;
        let height = processed.image.height;
        let (grid, grid_width, grid_height) =
            negpy_backend::features::exposure::metering::analysis_grid(
                &processed.image.pixels,
                width,
                height,
                None,
                config.process.analysis_buffer,
            );
        let unmix = negpy_backend::features::exposure::metering::effective_crosstalk_matrix(
            &config.process,
            Some(config.process.process_mode),
        );
        let grid = negpy_backend::features::exposure::metering::unmix_log_image(&grid, unmix);
        let bounds = negpy_backend::features::exposure::normalization::analyze_log_exposure_bounds(
            &grid,
            config.process.luma_range_clip,
            config.process.color_range_clip,
        );
        axes.push(
            negpy_backend::features::exposure::metering::measure_neutral_axis(
                &grid,
                grid_width,
                grid_height,
                bounds,
                true,
            )
            .map(|axis| SceneAxis {
                mid: axis.mid,
                shadow: axis.shadow,
                confidence: axis.confidence,
            }),
        );
    }
    DetectReport { axes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_reports_no_axis() {
        let dir = tempfile::tempdir().unwrap();
        let report = run(&DetectRequest {
            paths: vec![None, Some(dir.path().join("missing.tif"))],
            profile: dir.path().to_path_buf(),
        });
        assert_eq!(report.axes, vec![None, None]);
    }
}
