//! Mirrors `negpy/domain/tokens.py`: the source-identity token.
//!
//! A token keys every cache that holds a decoded source, and decides whether a config change
//! needs a re-decode or only a re-render. It is built from the whole config rather than a
//! hand-picked list of fields, because a field left out is invisible until a stale buffer is
//! served for the setting the user just changed.
//!
//! The digest is a 64-bit FNV-1a over the config's canonical JSON, not the Python sha256:
//! the token is a cache key that is never persisted, so the Python bytes are not a contract,
//! and no hash crate is available in this offline build. Two configs share a token only if
//! they serialize to the same JSON.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

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

/// A stable 64-bit FNV-1a over the config's canonical JSON, as 16 hex digits.
fn config_digest<T: Serialize>(config: &T) -> String {
    let text = serde_json::to_string(config).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
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
