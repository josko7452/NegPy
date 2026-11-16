//! Filesystem dates for an exported file.
//!
//! Mirrors `negpy/features/metadata/fsdate.py`. An export writes a new file, so the
//! source's mtime and atime are copied back; on macOS the creation date follows too.

use std::path::Path;

/// Stamp `output`'s mtime and atime from `source`.
///
/// Never raises: the pixels and metadata are already written when this runs, and a
/// filesystem that rejects the dates must not fail the export.
pub fn sync_export_filesystem_dates(output: &Path, source: &Path) {
    let Ok(metadata) = std::fs::metadata(source) else {
        return;
    };
    let Ok(modified) = metadata.modified() else {
        return;
    };
    let Ok(accessed) = metadata.accessed() else {
        return;
    };
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(output) else {
        return;
    };
    let times = std::fs::FileTimes::new()
        .set_accessed(accessed)
        .set_modified(modified);
    let _ = file.set_times(times);
    #[cfg(target_os = "macos")]
    sync_macos_creation_date(output, &metadata);
}

#[cfg(target_os = "macos")]
fn sync_macos_creation_date(output: &Path, metadata: &std::fs::Metadata) {
    use std::os::unix::fs::MetadataExt;

    #[repr(C)]
    struct AttrList {
        bitmapcount: u16,
        reserved: u16,
        commonattr: u32,
        volattr: u32,
        dirattr: u32,
        fileattr: u32,
        forkattr: u32,
    }

    #[repr(C)]
    struct TimeSpec {
        tv_sec: i64,
        tv_nsec: i64,
    }

    const ATTR_BIT_MAP_COUNT: u16 = 5;
    const ATTR_CMN_CRTIME: u32 = 0x0000_0200;

    unsafe extern "C" {
        fn setattrlist(
            path: *const std::ffi::c_char,
            attrs: *const AttrList,
            attr_buf: *const std::ffi::c_void,
            attr_buf_size: usize,
            options: u32,
        ) -> i32;
    }

    let birthtime = metadata.created().ok().and_then(|time| {
        time.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|duration| (duration.as_secs() as i64, duration.subsec_nanos() as i64))
    });
    let Some((seconds, nanos)) = birthtime else {
        return;
    };
    let attrs = AttrList {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: ATTR_CMN_CRTIME,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let time = TimeSpec {
        tv_sec: seconds,
        tv_nsec: nanos,
    };
    let Ok(path) = std::ffi::CString::new(output.as_os_str().as_encoded_bytes()) else {
        return;
    };
    // SAFETY: `path` is NUL-terminated, and both attribute structures outlive the call.
    let _ = unsafe {
        setattrlist(
            path.as_ptr(),
            &attrs,
            (&time as *const TimeSpec).cast(),
            std::mem::size_of::<TimeSpec>(),
            0,
        )
    };
}
