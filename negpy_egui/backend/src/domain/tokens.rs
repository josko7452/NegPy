//! Mirrors `negpy/domain/tokens.py`: the source-identity token.
//!
//! A token keys every cache that holds a decoded source, and decides whether a config change
//! needs a re-decode or only a re-render. It is built from the whole config rather than a
//! hand-picked list of fields, because a field left out is invisible until a stale buffer is
//! served for the setting the user just changed.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// ``|kind:path:size:mtime_ns:...:digest`` for an assembled source, or `""` when one part is
/// unreachable.
///
/// `""` means "cannot be assembled", a part moved or deleted, and is returned rather than
/// raised because a token is built on the way to a cache lookup, not inside the decode that
/// reports the missing file. Callers that key a cache on the token must skip caching when it
/// is empty, since two different broken configs share it.
#[must_use]
pub fn composite_token<T: Serialize>(kind: &str, config: &T, paths: &[&str]) -> String {
    let mut stamps = String::new();
    for path in paths {
        if path.is_empty() {
            continue;
        }
        let Ok(metadata) = std::fs::metadata(path) else {
            return String::new();
        };
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        // Size and nanosecond mtime, not whole seconds: a file rewritten within the same
        // second as the last decode would otherwise be a cache hit.
        stamps.push_str(&format!(":{path}:{}:{mtime_ns}", metadata.len()));
    }
    let digest = config_digest(config);
    format!("|{kind}{stamps}:{digest}")
}

/// The first 16 hex digits of the SHA-256 of the config's canonical JSON, the same
/// algorithm and truncation Python uses. The digested bytes are not the Python `repr`, so a
/// token is not interchangeable with the Python one; it is a cache key that is never
/// persisted, so it only has to be stable within this process.
fn config_digest<T: Serialize>(config: &T) -> String {
    let text = serde_json::to_string(config).unwrap_or_default();
    let digest = Sha256::digest(text.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex.truncate(16);
    hex
}

/// A path's identity for a token, or `""` when it is unreachable. A bare string, not a full
/// token: callers fold it into `source_token`.
#[must_use]
pub fn path_stamp(path: &str) -> String {
    let Ok(metadata) = std::fs::metadata(path) else {
        return String::new();
    };
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    format!("{path}:{}:{mtime_ns}", metadata.len())
}

/// True when the path exists and is readable, the guard behind every assembly predicate.
#[must_use]
pub fn path_reachable(path: &str) -> bool {
    !path.is_empty() && Path::new(path).exists()
}
