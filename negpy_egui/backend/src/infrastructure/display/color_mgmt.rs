//! ICC profile lookup, display transform and profile import.
//!
//! Mirrors `negpy/infrastructure/display/color_mgmt.py`: the same bundled profile for
//! each color space, the user's override folder first-come, and a cached display LUT.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::infrastructure::loaders::helpers::profile_description;
use crate::kernel::system::paths::resource_path;

use super::icc_lut::{
    DEFAULT_LUT_SIZE, Lut3Data, apply_lut_f32, build_3d_lut, build_3d_lut_to_srgb,
};
use super::matrix_trc::{apply_profile_pair_u8, apply_profile_pair_u16};

/// The working space the pipeline numbers are assumed to be in.
pub const WORKING_COLOR_SPACE: &str = "Adobe RGB";

/// The bundled profile filename for each space, as the Python registry maps it.
pub const ICC_FILES: &[(&str, &str)] = &[
    ("sRGB", "sRGB-v4.icc"),
    ("Adobe RGB", "AdobeCompat-v4.icc"),
    ("ProPhoto RGB", "ProPhoto-v4.icc"),
    ("P3 D65", "DisplayP3-v4.icc"),
    ("Rec 2020", "Rec2020-v4.icc"),
    ("Greyscale", "GrayGamma2.2.icc"),
];

/// A resolved profile: the bytes and where they came from.
#[derive(Clone, Debug, PartialEq)]
pub struct IccProfile {
    pub bytes: Vec<u8>,
    pub path: PathBuf,
}

/// The bundled filename for a named space.
#[must_use]
pub fn bundled_file(color_space: &str) -> Option<&'static str> {
    ICC_FILES
        .iter()
        .find(|(name, _)| *name == color_space)
        .map(|(_, file)| *file)
}

/// The path the Python registry resolves for a named space, or `None` when none exists.
#[must_use]
pub fn profile_path(color_space: &str, user_icc_dir: &Path) -> Option<PathBuf> {
    if let Some(file) = bundled_file(color_space) {
        let path = resource_path(&format!("icc/{file}"));
        if path.is_file() {
            return Some(path);
        }
    }
    let custom = user_icc_dir.join(format!("{color_space}.icc"));
    custom.is_file().then_some(custom)
}

/// Read a named space's profile bytes.
#[must_use]
pub fn profile_bytes(color_space: &str, user_icc_dir: &Path) -> Option<IccProfile> {
    let path = profile_path(color_space, user_icc_dir)?;
    let bytes = std::fs::read(&path).ok()?;
    Some(IccProfile { bytes, path })
}

/// The bundled dispay profile paths and the user's, sorted.
#[must_use]
pub fn available_profiles(user_icc_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(resource_path("icc")) {
        paths.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| is_icc(path)),
        );
    }
    if let Ok(entries) = std::fs::read_dir(user_icc_dir) {
        paths.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| is_icc(path)),
        );
    }
    paths.sort();
    paths
}

fn is_icc(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("icc") || extension.eq_ignore_ascii_case("icm")
        })
}

/// The human-readable profile name, or a labelled fallback.
#[must_use]
pub fn describe_profile(bytes: Option<&[u8]>) -> String {
    let Some(bytes) = bytes.filter(|bytes| !bytes.is_empty()) else {
        return "sRGB fallback".to_string();
    };
    profile_description(bytes).unwrap_or_else(|| "Unknown profile".to_string())
}

/// The display LUT cache key: the source space and a hash of the destination bytes.
fn display_lut(
    working_color_space: &str,
    dest_icc: Option<&[u8]>,
    user_icc_dir: &Path,
) -> Option<Lut3Data> {
    let source = profile_bytes(working_color_space, user_icc_dir)?;
    match dest_icc.filter(|bytes| !bytes.is_empty()) {
        Some(dest) => build_3d_lut(&source.bytes, dest, 1, DEFAULT_LUT_SIZE),
        None => build_3d_lut_to_srgb(&source.bytes, 1, DEFAULT_LUT_SIZE),
    }
}

struct LutCache {
    entries: HashMap<String, Option<Lut3Data>>,
}

fn lut_cache() -> &'static Mutex<LutCache> {
    static CACHE: OnceLock<Mutex<LutCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(LutCache {
            entries: HashMap::new(),
        })
    })
}

fn cache_key(working_color_space: &str, dest_icc: Option<&[u8]>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(working_color_space.as_bytes());
    match dest_icc {
        Some(bytes) => {
            hasher.update([1]);
            hasher.update(bytes);
        }
        None => hasher.update([0]),
    }
    let digest = hasher.finalize();
    use std::fmt::Write;
    let mut key = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(key, "{byte:02x}");
    }
    key
}

/// The cached display LUT, built once per (`working_color_space`, destination bytes).
///
/// A space with no bundled profile reads as `None`, the Python caller's miss. A no-op
/// pair is not special-cased here: the caller skips the transform when the mapping is
/// not needed, as the sRGB-to-sRGB shortcut does.
#[must_use]
pub fn get_display_lut(
    working_color_space: &str,
    dest_icc: Option<&[u8]>,
    user_icc_dir: &Path,
) -> Option<Lut3Data> {
    let key = cache_key(working_color_space, dest_icc);
    if let Ok(cache) = lut_cache().lock() {
        if let Some(cached) = cache.entries.get(&key) {
            return cached.clone();
        }
    }
    let built = display_lut(working_color_space, dest_icc, user_icc_dir);
    if let Ok(mut cache) = lut_cache().lock() {
        if cache.entries.len() >= 16 {
            cache.entries.clear();
        }
        cache.entries.insert(key, built.clone());
    }
    built
}

/// Drop every cached display LUT, as a monitor change or a profile import does.
pub fn clear_display_luts() {
    if let Ok(mut cache) = lut_cache().lock() {
        cache.entries.clear();
    }
}

/// Convert a float RGB image from the working space to the display profile.
///
/// An image that is not three-channel float passes through unchanged, as the Python
/// function returns it for a greyscale buffer.
#[must_use]
pub fn apply_display_transform(
    buffer: &[f32],
    working_color_space: &str,
    dest_icc: Option<&[u8]>,
    user_icc_dir: &Path,
) -> Vec<f32> {
    if buffer.len() % 3 != 0 {
        return buffer.to_vec();
    }
    if working_color_space == "sRGB" && dest_icc.is_none() {
        return buffer.to_vec();
    }
    match get_display_lut(working_color_space, dest_icc, user_icc_dir) {
        Some(lut) => apply_lut_f32(buffer, &lut).unwrap_or_else(|| buffer.to_vec()),
        None => buffer.to_vec(),
    }
}

/// An 8-bit image through a profile pair, preferring the matrix/TRC path.
#[must_use]
pub fn transform_u8(img: &[u8], source: &[u8], dest: &[u8]) -> Option<Vec<u8>> {
    apply_profile_pair_u8(img, source, dest)
}

/// A 16-bit image through a profile pair, preferring the matrix/TRC path.
#[must_use]
pub fn transform_u16(img: &[u16], source: &[u8], dest: &[u8]) -> Option<Vec<u16>> {
    apply_profile_pair_u16(img, source, dest)
}

/// Store a user ICC profile under `dest_dir` and return its path.
///
/// The bytes must open as a profile. A name taken by different bytes stores beside it
/// as `name (2).icc`; identical bytes reuse the file already there.
pub fn import_icc_profile(source: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    let bytes =
        std::fs::read(source).map_err(|error| format!("Cannot read the profile: {error}"))?;
    qcms::Profile::new_from_slice(&bytes, false)
        .ok_or_else(|| "Not a readable ICC profile.".to_string())?;
    std::fs::create_dir_all(dest_dir)
        .map_err(|error| format!("Cannot create the profile folder: {error}"))?;

    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The profile has no file name.".to_string())?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("icc");
    let mut candidate = dest_dir.join(format!("{stem}.{extension}"));
    let mut index = 2;
    while candidate.exists() {
        if std::fs::read(&candidate).ok().as_deref() == Some(bytes.as_slice()) {
            return Ok(candidate);
        }
        candidate = dest_dir.join(format!("{stem} ({index}).{extension}"));
        index += 1;
    }
    std::fs::copy(source, &candidate)
        .map_err(|error| format!("Cannot copy the profile: {error}"))?;
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled(name: &str) -> Vec<u8> {
        std::fs::read(resource_path(&format!("icc/{name}"))).expect("bundled profile")
    }

    #[test]
    fn every_export_space_resolves_to_a_bundled_profile() {
        let user = Path::new("/nonexistent/user/icc");
        for (space, _) in ICC_FILES {
            assert!(
                profile_path(space, user).is_some(),
                "{space} has no bundled profile"
            );
        }
        assert!(profile_path("ACES", user).is_none());
    }

    #[test]
    fn a_description_falls_back_when_the_bytes_are_missing() {
        assert_eq!(describe_profile(None), "sRGB fallback");
        assert_eq!(describe_profile(Some(b"")), "sRGB fallback");
        assert_eq!(describe_profile(Some(b"garbage")), "Unknown profile");
        let described = describe_profile(Some(&bundled("sRGB-v4.icc")));
        assert!(!described.is_empty());
    }

    #[test]
    fn the_adobe_working_space_maps_to_the_display_profile() {
        let user = Path::new("/nonexistent/user/icc");
        let buffer = [0.0f32, 0.5, 1.0, 0.2, 0.4, 0.6];
        let out = apply_display_transform(&buffer, WORKING_COLOR_SPACE, None, user);
        assert_eq!(out.len(), buffer.len());
        assert!(out.iter().all(|value| (0.0..=1.0).contains(value)));
    }

    #[test]
    fn an_srgb_buffer_on_an_srgb_display_passes_through() {
        let user = Path::new("/nonexistent/user/icc");
        let buffer = [0.1f32, 0.2, 0.3];
        assert_eq!(apply_display_transform(&buffer, "sRGB", None, user), buffer);
    }

    #[test]
    fn a_matrix_pair_skips_the_three_d_lut_path() {
        let icc = bundled("sRGB-v4.icc");
        let img = [0u8, 128, 255];
        let out = transform_u8(&img, &icc, &icc).expect("matrix pair");
        assert_eq!(out.len(), 3);
        assert!(transform_u16(&[0, 32768, 65535], &icc, &icc).is_some());
    }

    #[test]
    fn importing_a_profile_copies_or_reuses_it() {
        let temp = std::env::temp_dir().join(format!("negpy-icc-{}", std::process::id()));
        let source_dir = temp.join("src");
        let dest_dir = temp.join("dest");
        std::fs::create_dir_all(&source_dir).expect("source dir");
        std::fs::create_dir_all(&dest_dir).expect("dest dir");
        let source = source_dir.join("Paper.icc");
        std::fs::write(&source, bundled("sRGB-v4.icc")).expect("write source");

        let first = import_icc_profile(&source, &dest_dir).expect("import");
        let again = import_icc_profile(&source, &dest_dir).expect("reimport");
        assert_eq!(first, again);
        assert_eq!(std::fs::read_dir(&dest_dir).unwrap().count(), 1);

        let other_dir = temp.join("other");
        std::fs::create_dir_all(&other_dir).expect("other dir");
        let other = other_dir.join("Paper.icc");
        let mut bytes = bundled("sRGB-v4.icc");
        bytes.push(0);
        std::fs::write(&other, bytes).expect("write other");
        let beside = import_icc_profile(&other, &dest_dir).expect("import beside");
        assert_eq!(beside.file_name().unwrap(), "Paper (2).icc");

        let garbage = other_dir.join("notes.icc");
        std::fs::write(&garbage, b"not a profile").expect("write garbage");
        assert!(import_icc_profile(&garbage, &dest_dir).is_err());
        let _ = std::fs::remove_dir_all(&temp);
    }
}
