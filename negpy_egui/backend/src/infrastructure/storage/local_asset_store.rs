//! The thumbnail cache and content identity for source files.
//!
//! Mirrors `negpy/infrastructure/storage/local_asset_store.py`. A thumbnail is keyed by the
//! content hash alone, so a cache entry written at another size must read as a miss.

use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{ImageFormat, RgbImage};
use sha2::{Digest, Sha256};

const HEAD_TAIL: u64 = 1024 * 1024;
const INTERIOR_CHUNKS: u64 = 16;
const INTERIOR_CHUNK: usize = 256 * 1024;
const COMMENT_PREFIX: &str = "negpy-thumb:";
const THUMB_JPEG_QUALITY: u8 = 85;

/// The on-disk asset cache.
#[derive(Clone, Debug)]
pub struct LocalAssetStore {
    cache_dir: PathBuf,
    icc_dir: PathBuf,
    thumb_dir: PathBuf,
    thumbnail_size: i64,
}

impl LocalAssetStore {
    #[must_use]
    pub fn new(
        cache_dir: impl Into<PathBuf>,
        icc_dir: impl Into<PathBuf>,
        thumbnail_size: i64,
    ) -> Self {
        let cache_dir = cache_dir.into();
        Self {
            thumb_dir: cache_dir.join("thumbnails"),
            cache_dir,
            icc_dir: icc_dir.into(),
            thumbnail_size,
        }
    }

    pub fn initialize(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.cache_dir)?;
        fs::create_dir_all(&self.icc_dir)?;
        fs::create_dir_all(&self.thumb_dir)?;
        Ok(())
    }

    #[must_use]
    pub fn thumb_dir(&self) -> &Path {
        &self.thumb_dir
    }

    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// The per-session directory for copied uploads.
    pub fn session_dir(&self, session_id: &str) -> std::io::Result<PathBuf> {
        let path = self.cache_dir.join(session_id);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// The content fingerprint used as the edit-store identity.
    pub fn register_path(&self, source: &Path) -> std::io::Result<Option<(PathBuf, String)>> {
        if !source.exists() {
            return Ok(None);
        }
        Ok(Some((source.to_path_buf(), file_hash(source)?)))
    }

    /// A cached thumbnail, only when it is the configured size and decodes fully.
    pub fn get_thumbnail(&self, file_hash: &str) -> Option<RgbImage> {
        let path = self.thumb_path(file_hash);
        let data = fs::read(path).ok()?;
        if !has_end_of_image(&data) {
            return None;
        }
        let image = image::load_from_memory_with_format(&data, ImageFormat::Jpeg).ok()?;
        let (width, height) = (i64::from(image.width()), i64::from(image.height()));
        if width.max(height) != self.thumbnail_size {
            return None;
        }
        Some(image.to_rgb8())
    }

    /// Store a thumbnail as JPEG with the fingerprint in its COM segment.
    pub fn save_thumbnail(
        &self,
        file_hash: &str,
        image: &RgbImage,
        fingerprint: Option<&str>,
    ) -> std::io::Result<()> {
        fs::create_dir_all(&self.thumb_dir)?;
        let mut encoded = Vec::new();
        JpegEncoder::new_with_quality(&mut encoded, THUMB_JPEG_QUALITY)
            .encode_image(image)
            .map_err(std::io::Error::other)?;
        let bytes = match encode_comment(fingerprint) {
            Some(comment) => insert_comment(&encoded, &comment),
            None => encoded,
        };
        fs::write(self.thumb_path(file_hash), bytes)
    }

    /// The stored fingerprint, read from the JPEG header without decoding pixels.
    #[must_use]
    pub fn get_thumbnail_fingerprint(&self, file_hash: &str) -> Option<String> {
        let data = fs::read(self.thumb_path(file_hash)).ok()?;
        decode_comment(&read_comment(&data)?)
    }

    /// Remove a managed cache file; a path outside the cache is left alone.
    pub fn remove(&self, file_path: &Path) -> std::io::Result<()> {
        if !file_path.exists() {
            return Ok(());
        }
        let cache = fs::canonicalize(&self.cache_dir).unwrap_or_else(|_| self.cache_dir.clone());
        let target = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
        if target.starts_with(cache) {
            fs::remove_file(target)?;
        }
        Ok(())
    }

    pub fn clear_session_assets(&self, session_id: &str) -> std::io::Result<()> {
        let path = self.cache_dir.join(session_id);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    /// `(count, total bytes)` of cached thumbnails.
    #[must_use]
    pub fn thumbnail_stats(&self) -> (usize, u64) {
        let Ok(entries) = fs::read_dir(&self.thumb_dir) else {
            return (0, 0);
        };
        let mut count = 0;
        let mut bytes = 0;
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
            {
                count += 1;
                bytes += metadata.len();
            }
        }
        (count, bytes)
    }

    pub fn clear_thumbnails(&self) -> std::io::Result<()> {
        if self.thumb_dir.exists() {
            fs::remove_dir_all(&self.thumb_dir)?;
        }
        fs::create_dir_all(&self.thumb_dir)
    }

    pub fn clear_all(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    fs::remove_dir_all(path)?;
                } else {
                    fs::remove_file(path)?;
                }
            }
        }
        self.initialize()
    }

    fn thumb_path(&self, file_hash: &str) -> PathBuf {
        self.thumb_dir.join(format!("{file_hash}.jpg"))
    }
}

fn has_end_of_image(jpeg: &[u8]) -> bool {
    jpeg.len() >= 2 && jpeg[jpeg.len() - 2..] == [0xFF, 0xD9]
}

/// Size plus a bounded head, tail and interior sample of the file's bytes.
pub fn file_hash(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = Sha256::new();
    hasher.update(size.to_string().as_bytes());

    let mut head = vec![0u8; HEAD_TAIL.min(size) as usize];
    file.read_exact(&mut head)?;
    hasher.update(&head);
    if size > 2 * HEAD_TAIL {
        file.seek(SeekFrom::End(-(HEAD_TAIL as i64)))?;
        let mut tail = vec![0u8; HEAD_TAIL as usize];
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
        let step = (size - 2 * HEAD_TAIL) / INTERIOR_CHUNKS;
        for index in 0..INTERIOR_CHUNKS {
            file.seek(SeekFrom::Start(HEAD_TAIL + index * step))?;
            let mut chunk = vec![0u8; INTERIOR_CHUNK];
            let read = file.read(&mut chunk)?;
            hasher.update(&chunk[..read]);
        }
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn encode_comment(fingerprint: Option<&str>) -> Option<Vec<u8>> {
    fingerprint.map(|value| format!("{COMMENT_PREFIX}{value}").into_bytes())
}

fn decode_comment(comment: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(comment).ok()?;
    text.strip_prefix(COMMENT_PREFIX)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Insert one JPEG COM segment after the SOI marker.
fn insert_comment(jpeg: &[u8], comment: &[u8]) -> Vec<u8> {
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return jpeg.to_vec();
    }
    let length = u16::try_from(comment.len() + 2).unwrap_or(u16::MAX);
    let payload = &comment[..usize::from(length.saturating_sub(2))];
    let mut out = Vec::with_capacity(jpeg.len() + payload.len() + 4);
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(&[0xFF, 0xFE]);
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Read the first JPEG COM segment, stopping at the scan data.
fn read_comment(jpeg: &[u8]) -> Option<Vec<u8>> {
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return None;
    }
    let mut cursor = Cursor::new(jpeg);
    cursor.seek(SeekFrom::Start(2)).ok()?;
    loop {
        let mut marker = [0u8; 2];
        cursor.read_exact(&mut marker).ok()?;
        while marker[0] != 0xFF {
            marker[0] = marker[1];
            cursor.read_exact(&mut marker[1..]).ok()?;
        }
        let kind = marker[1];
        if kind == 0xDA || kind == 0xD9 {
            return None;
        }
        if (0xD0..=0xD7).contains(&kind) || kind == 0x01 {
            continue;
        }
        let mut length_bytes = [0u8; 2];
        cursor.read_exact(&mut length_bytes).ok()?;
        let length = usize::from(u16::from_be_bytes(length_bytes));
        if length < 2 {
            return None;
        }
        let mut payload = vec![0u8; length - 2];
        cursor.read_exact(&mut payload).ok()?;
        if kind == 0xFE {
            return Some(payload);
        }
    }
}
