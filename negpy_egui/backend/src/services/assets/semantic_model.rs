//! CLIP model download glue.
//!
//! Mirrors `download_clip_model` in `negpy/services/assets/semantic_model.py`: each file
//! lands under a `.part` name and is renamed only when complete, so an interrupted
//! download never reads as ready. Progress counts bytes across the whole set.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::embeddings::{MODEL_FILES, MODEL_VERSION, clip_model_ready, model_dir};

/// The model repository's published base URL.
pub const BASE_URL: &str = "https://huggingface.co/Xenova/clip-vit-base-patch16/resolve/main";
/// The user-facing download size the panels warn with.
pub const MODEL_DOWNLOAD_SIZE: &str = "about 150 MB";
const CHUNK: usize = 256 * 1024;

/// A model download failure, shown to the user verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipDownloadError(pub String);

impl std::fmt::Display for ClipDownloadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ClipDownloadError {}

/// One model file source. A test supplies a local map; the app supplies HTTP.
pub trait ModelSource {
    /// The file's size, or 0 when the server does not state it.
    fn content_length(&mut self, remote_path: &str) -> u64;

    /// Stream one file into `sink`; the sink can stop the read by returning an error.
    fn read(
        &mut self,
        remote_path: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), ClipDownloadError>,
    ) -> Result<(), ClipDownloadError>;
}

/// Fetch every missing model file into the versioned cache directory.
pub fn download_clip_model(
    cache_dir: &Path,
    on_progress: Option<&mut dyn FnMut(u64, u64)>,
    is_cancelled: Option<&dyn Fn() -> bool>,
) -> Result<PathBuf, ClipDownloadError> {
    let dir = model_dir(cache_dir, MODEL_VERSION);
    let mut source = UreqModelSource::new(BASE_URL);
    download_from(&mut source, &dir, on_progress, is_cancelled)?;
    Ok(dir)
}

/// Download the file set from `source`. Existing files are kept and counted as done.
pub fn download_from(
    source: &mut dyn ModelSource,
    dir: &Path,
    mut on_progress: Option<&mut dyn FnMut(u64, u64)>,
    is_cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), ClipDownloadError> {
    std::fs::create_dir_all(dir)
        .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
    if is_cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(ClipDownloadError("Download cancelled.".to_string()));
    }
    let mut sizes = Vec::with_capacity(MODEL_FILES.len());
    for (remote, name) in MODEL_FILES {
        let dest = dir.join(name);
        sizes.push(if dest.is_file() {
            std::fs::metadata(&dest).map_or(0, |meta| meta.len())
        } else {
            source.content_length(remote)
        });
    }
    let total = sizes.iter().sum();
    let mut done = 0u64;
    for ((remote, name), size) in MODEL_FILES.iter().zip(sizes) {
        let dest = dir.join(name);
        if dest.is_file() {
            done += size;
            if let Some(progress) = on_progress.as_deref_mut() {
                progress(done, total);
            }
            continue;
        }
        let temporary = PathBuf::from(format!("{}.part", dest.display()));
        if is_cancelled.is_some_and(|cancelled| cancelled()) {
            discard(&temporary);
            return Err(ClipDownloadError("Download cancelled.".to_string()));
        }
        let mut file = std::fs::File::create(&temporary)
            .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
        let result = source.read(remote, &mut |chunk| {
            if is_cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(ClipDownloadError("Download cancelled.".to_string()));
            }
            file.write_all(chunk)
                .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
            done += chunk.len() as u64;
            if let Some(progress) = on_progress.as_deref_mut() {
                progress(done, total);
            }
            Ok(())
        });
        if let Err(error) = result {
            drop(file);
            discard(&temporary);
            return Err(error);
        }
        drop(file);
        std::fs::rename(&temporary, &dest)
            .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
    }
    debug_assert!(clip_model_ready(dir));
    Ok(())
}

fn discard(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// The real HTTP source.
pub struct UreqModelSource {
    base_url: String,
    user_agent: String,
}

impl UreqModelSource {
    /// A source rooted at `base_url`, identifying this app version.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            user_agent: format!(
                "NegPy/{} (+https://github.com/marcinz606/NegPy)",
                crate::kernel::system::version::app_version()
            ),
        }
    }

    fn url(&self, remote_path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), remote_path)
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build()
            .new_agent()
    }
}

impl ModelSource for UreqModelSource {
    fn content_length(&mut self, remote_path: &str) -> u64 {
        let url = self.url(remote_path);
        let Ok(response) = Self::agent()
            .head(&url)
            .header("User-Agent", &self.user_agent)
            .call()
        else {
            return 0;
        };
        response
            .headers()
            .get("Content-Length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    fn read(
        &mut self,
        remote_path: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), ClipDownloadError>,
    ) -> Result<(), ClipDownloadError> {
        let url = self.url(remote_path);
        let mut response = Self::agent()
            .get(&url)
            .header("User-Agent", &self.user_agent)
            .call()
            .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
        let mut reader = response.body_mut().as_reader();
        let mut chunk = vec![0u8; CHUNK];
        loop {
            let read = reader
                .read(&mut chunk)
                .map_err(|error| ClipDownloadError(format!("Download failed: {error}")))?;
            if read == 0 {
                return Ok(());
            }
            sink(&chunk[..read])?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    struct FakeSource {
        payloads: BTreeMap<String, Vec<u8>>,
        requested: Vec<String>,
    }

    impl FakeSource {
        fn new(size: usize) -> Self {
            let payloads = MODEL_FILES
                .iter()
                .map(|(remote, _)| (remote.to_string(), vec![b'x'; size]))
                .collect();
            Self {
                payloads,
                requested: Vec::new(),
            }
        }
    }

    impl ModelSource for FakeSource {
        fn content_length(&mut self, remote_path: &str) -> u64 {
            self.payloads
                .get(remote_path)
                .map_or(0, |data| data.len() as u64)
        }

        fn read(
            &mut self,
            remote_path: &str,
            sink: &mut dyn FnMut(&[u8]) -> Result<(), ClipDownloadError>,
        ) -> Result<(), ClipDownloadError> {
            self.requested.push(remote_path.to_string());
            let data = self.payloads.get(remote_path).cloned().unwrap_or_default();
            sink(&data)
        }
    }

    fn directory() -> tempfile::TempDir {
        tempfile::tempdir().expect("temp dir")
    }

    #[test]
    fn a_download_writes_every_expected_file() {
        let dir = directory();
        let mut source = FakeSource::new(7);
        download_from(&mut source, dir.path(), None, None).expect("download");
        assert!(clip_model_ready(dir.path()));
        for (_, name) in MODEL_FILES {
            assert_eq!(std::fs::read(dir.path().join(name)).unwrap().len(), 7);
        }
    }

    #[test]
    fn progress_covers_the_whole_set() {
        let dir = directory();
        let mut source = FakeSource::new(100);
        let mut seen = Vec::new();
        download_from(
            &mut source,
            dir.path(),
            Some(&mut |done, total| seen.push((done, total))),
            None,
        )
        .expect("download");
        assert_eq!(seen.last(), Some(&(300, 300)));
    }

    #[test]
    fn a_cancel_leaves_no_ready_or_partial_file() {
        let dir = directory();
        let mut source = FakeSource::new(100);
        let error =
            download_from(&mut source, dir.path(), None, Some(&|| true)).expect_err("cancelled");
        assert_eq!(error.0, "Download cancelled.");
        assert!(!clip_model_ready(dir.path()));
        let leftovers: HashSet<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn an_existing_file_is_not_fetched_again() {
        let dir = directory();
        std::fs::create_dir_all(dir.path()).unwrap();
        let (first_remote, first_name) = MODEL_FILES[0];
        std::fs::write(dir.path().join(first_name), b"already-here").unwrap();
        let mut source = FakeSource::new(4);
        download_from(&mut source, dir.path(), None, None).expect("download");
        assert!(!source.requested.iter().any(|remote| remote == first_remote));
        assert_eq!(
            std::fs::read(dir.path().join(first_name)).unwrap(),
            b"already-here"
        );
    }

    #[test]
    fn a_download_error_removes_the_partial_file() {
        struct Failing;
        impl ModelSource for Failing {
            fn content_length(&mut self, _remote_path: &str) -> u64 {
                4
            }
            fn read(
                &mut self,
                _remote_path: &str,
                sink: &mut dyn FnMut(&[u8]) -> Result<(), ClipDownloadError>,
            ) -> Result<(), ClipDownloadError> {
                sink(b"ab")?;
                Err(ClipDownloadError("Download failed: reset".to_string()))
            }
        }
        let dir = directory();
        let error = download_from(&mut Failing, dir.path(), None, None).expect_err("failed");
        assert!(error.0.contains("reset"));
        assert!(!clip_model_ready(dir.path()));
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }
}
