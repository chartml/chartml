//! Pure-Rust binary codec for persistent `CacheBackend` blobs (chartml 5.0
//! phase 3b).
//!
//! Used by [`indexeddb::IndexedDbBackend`](super::indexeddb::IndexedDbBackend)
//! today; future tier-2 backends (file-system, SQLite, …) will reuse the same
//! framing so that one set of versioned bytes is shared across every
//! persistent store.
//!
//! Lives in its own module — and crucially, is **not** gated behind any
//! `target_arch = "wasm32"` cfg — so the codec compiles and tests on every
//! platform. The IndexedDB plumbing in `indexeddb.rs` is the only piece that
//! requires a browser; the byte framing here is plain Rust over `Vec<u8>`,
//! varints, and `serde_json`, so native `cargo test --workspace` exercises
//! it directly.
//!
//! ## Stored blob format (versioned)
//!
//! ```text
//! [u8 version = BLOB_VERSION]
//! [u32 ipc_bytes_len, big-endian]
//! [ipc_bytes …]                             // DataTable Arrow IPC stream
//! [varint(uleb128) tail_json_len]
//! [json bytes: { "fetched_at_ms": u64, "ttl_ms": u64,
//!                 "tags": [String, …], "metadata": { … } }]
//! ```
//!
//! On `decode_blob` we read the version byte first; if it doesn't match
//! [`BLOB_VERSION`] callers receive [`DecodeError::VersionMismatch`] so they
//! can evict the stale entry and surface a cache miss rather than a cryptic
//! decode error. New chartml releases bump `BLOB_VERSION` whenever the
//! tail-json shape or the byte framing changes.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::data::DataTable;
use crate::resolver::cache::CachedEntry;

/// Current stored-blob format version. Bump whenever the byte layout or the
/// `TailJson` shape changes; older entries evict themselves on the next read.
pub(crate) const BLOB_VERSION: u8 = 0x01;

/// Tail-JSON shape for the cached entry's metadata / tags / timestamps.
/// Stored after the IPC payload so the per-row data and the per-entry
/// bookkeeping can evolve independently — bumping `BLOB_VERSION` only happens
/// when the framing changes, not when callers extend `metadata`.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TailJson {
    /// Wall-clock fetch timestamp as Unix milliseconds. We store ms (not the
    /// `SystemTime` itself) because IndexedDB doesn't preserve Rust types
    /// across reloads — the round-trip goes through `JsValue` regardless,
    /// and other tier-2 backends benefit from the same on-disk schema.
    pub(crate) fetched_at_ms: u64,
    /// Stored TTL in milliseconds.
    pub(crate) ttl_ms: u64,
    /// Bulk-invalidation tags from `CachedEntry::tags`.
    pub(crate) tags: Vec<String>,
    /// Provider metadata from `CachedEntry::metadata`.
    pub(crate) metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub(crate) enum DecodeError {
    /// Blob is shorter than the minimum framing requires, or lengths don't
    /// add up.
    Truncated,
    /// Version byte didn't match `BLOB_VERSION`. Triggers eviction in `get`.
    VersionMismatch,
    /// IPC bytes failed to decode into a `DataTable`.
    Data(crate::error::ChartError),
    /// Tail JSON was malformed.
    Tail(serde_json::Error),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "stored blob truncated"),
            Self::VersionMismatch => write!(f, "stored blob version mismatch"),
            Self::Data(e) => write!(f, "data decode failed: {e}"),
            Self::Tail(e) => write!(f, "tail json decode failed: {e}"),
        }
    }
}

/// Pack a `CachedEntry` into the on-disk byte layout. See module docs for
/// the format spec.
pub(crate) fn encode_blob(entry: &CachedEntry) -> Result<Vec<u8>, String> {
    let ipc = entry
        .data
        .to_ipc_bytes()
        .map_err(|e| format!("ipc encode: {e}"))?;
    if ipc.len() > u32::MAX as usize {
        return Err(format!(
            "ipc payload too large for u32 length prefix: {} bytes",
            ipc.len()
        ));
    }

    let fetched_at_ms = system_time_to_millis(entry.fetched_at);
    let ttl_ms = duration_to_millis(entry.ttl);
    let tail = TailJson {
        fetched_at_ms,
        ttl_ms,
        tags: entry.tags.clone(),
        metadata: entry.metadata.clone(),
    };
    let tail_json = serde_json::to_vec(&tail).map_err(|e| format!("tail json: {e}"))?;

    let mut out = Vec::with_capacity(1 + 4 + ipc.len() + 5 + tail_json.len());
    out.push(BLOB_VERSION);
    out.extend_from_slice(&(ipc.len() as u32).to_be_bytes());
    out.extend_from_slice(&ipc);
    write_uleb128(&mut out, tail_json.len() as u64);
    out.extend_from_slice(&tail_json);
    Ok(out)
}

/// Reverse `encode_blob`. Returns `DecodeError::VersionMismatch` when the
/// version byte doesn't match the current `BLOB_VERSION` so callers can
/// distinguish "needs eviction" from generic corruption.
pub(crate) fn decode_blob(raw: &[u8]) -> Result<CachedEntry, DecodeError> {
    if raw.is_empty() {
        return Err(DecodeError::Truncated);
    }
    let version = raw[0];
    if version != BLOB_VERSION {
        return Err(DecodeError::VersionMismatch);
    }
    if raw.len() < 1 + 4 {
        return Err(DecodeError::Truncated);
    }
    let ipc_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;
    let ipc_start: usize = 5;
    let ipc_end = ipc_start
        .checked_add(ipc_len)
        .ok_or(DecodeError::Truncated)?;
    if ipc_end > raw.len() {
        return Err(DecodeError::Truncated);
    }
    let ipc_bytes = &raw[ipc_start..ipc_end];
    let data = DataTable::from_ipc_bytes(ipc_bytes).map_err(DecodeError::Data)?;

    let (tail_len, after_varint) =
        read_uleb128(&raw[ipc_end..]).ok_or(DecodeError::Truncated)?;
    let tail_start = ipc_end + after_varint;
    let tail_end = tail_start
        .checked_add(tail_len as usize)
        .ok_or(DecodeError::Truncated)?;
    if tail_end > raw.len() {
        return Err(DecodeError::Truncated);
    }
    let tail: TailJson =
        serde_json::from_slice(&raw[tail_start..tail_end]).map_err(DecodeError::Tail)?;

    Ok(CachedEntry {
        data,
        fetched_at: millis_to_system_time(tail.fetched_at_ms),
        ttl: Duration::from_millis(tail.ttl_ms),
        tags: tail.tags,
        metadata: tail.metadata,
    })
}

/// Cheap predicate used by `invalidate_by_tag` — just decode the tail JSON
/// and check the tags vec. We intentionally skip IPC decoding here since we
/// don't need the data to know whether a tag matches.
pub(crate) fn entry_has_tag(raw: &[u8], tag: &str) -> bool {
    if raw.is_empty() || raw[0] != BLOB_VERSION || raw.len() < 5 {
        return false;
    }
    let ipc_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;
    let Some(after_ipc) = 5usize.checked_add(ipc_len) else {
        return false;
    };
    if after_ipc > raw.len() {
        return false;
    }
    let Some((tail_len, after_varint)) = read_uleb128(&raw[after_ipc..]) else {
        return false;
    };
    let tail_start = after_ipc + after_varint;
    let Some(tail_end) = tail_start.checked_add(tail_len as usize) else {
        return false;
    };
    if tail_end > raw.len() {
        return false;
    }
    let Ok(tail) = serde_json::from_slice::<TailJson>(&raw[tail_start..tail_end]) else {
        return false;
    };
    tail.tags.iter().any(|t| t == tag)
}

/// uleb128 encoder — 1 byte per 7 bits, MSB set on continuation. Matches the
/// "varint" wire format used by protobuf so any future need to consume the
/// blob from another language has a well-known reference.
pub(crate) fn write_uleb128(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// uleb128 decoder. Returns `(value, bytes_read)` on success, `None` on
/// truncated or oversized input. Bails after 10 bytes (max for u64) so a
/// corrupt continuation chain can't loop forever.
pub(crate) fn read_uleb128(input: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, &byte) in input.iter().enumerate().take(10) {
        let value = (byte & 0x7F) as u64;
        result |= value.checked_shl(shift)?;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift = shift.checked_add(7)?;
    }
    None
}

// ── Time helpers ────────────────────────────────────────────────────────────

pub(crate) fn system_time_to_millis(ts: SystemTime) -> u64 {
    ts.duration_since(UNIX_EPOCH)
        .map(|d| {
            let ms = d.as_millis();
            if ms > u64::MAX as u128 {
                u64::MAX
            } else {
                ms as u64
            }
        })
        .unwrap_or(0)
}

pub(crate) fn millis_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH
        .checked_add(Duration::from_millis(ms))
        .unwrap_or(UNIX_EPOCH)
}

pub(crate) fn duration_to_millis(d: Duration) -> u64 {
    let ms = d.as_millis();
    if ms > u64::MAX as u128 {
        u64::MAX
    } else {
        ms as u64
    }
}

#[cfg(test)]
mod tests {
    //! Pure-Rust unit tests for the codec — run on native via
    //! `cargo test --workspace`. The browser-side `wasm-bindgen-test` suite
    //! lives in `crates/chartml-core/tests/indexeddb_test.rs` and exercises
    //! the IDB plumbing.

    use super::*;
    use crate::data::Row;
    use serde_json::json;

    fn make_entry() -> CachedEntry {
        let row: Row = [("x".to_string(), json!(1.0))].into_iter().collect();
        CachedEntry {
            data: DataTable::from_rows(&[row]).unwrap(),
            fetched_at: SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_000),
            ttl: Duration::from_secs(300),
            tags: vec!["slug:foo".to_string(), "namespace:ws-1".to_string()],
            metadata: [("rows_returned".to_string(), json!(1))]
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn blob_roundtrip_preserves_entry() {
        let entry = make_entry();
        let bytes = encode_blob(&entry).expect("encode");
        let decoded = decode_blob(&bytes).expect("decode");
        assert_eq!(decoded.tags, entry.tags);
        assert_eq!(decoded.ttl, entry.ttl);
        assert_eq!(decoded.metadata, entry.metadata);
        assert_eq!(decoded.data.num_rows(), entry.data.num_rows());
    }

    #[test]
    fn version_mismatch_errors_distinctly() {
        let entry = make_entry();
        let mut bytes = encode_blob(&entry).expect("encode");
        bytes[0] = 0xFF; // bogus version
        match decode_blob(&bytes) {
            Err(DecodeError::VersionMismatch) => (),
            other => panic!("expected VersionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn truncated_blob_errors() {
        let entry = make_entry();
        let bytes = encode_blob(&entry).expect("encode");
        // Lop off the last 10 bytes — guaranteed to hit either the IPC body
        // or the tail JSON.
        let trimmed = &bytes[..bytes.len() - 10];
        assert!(matches!(
            decode_blob(trimmed),
            Err(DecodeError::Truncated) | Err(DecodeError::Tail(_)) | Err(DecodeError::Data(_))
        ));
    }

    #[test]
    fn entry_has_tag_matches_real_tags() {
        let entry = make_entry();
        let bytes = encode_blob(&entry).expect("encode");
        assert!(entry_has_tag(&bytes, "slug:foo"));
        assert!(entry_has_tag(&bytes, "namespace:ws-1"));
        assert!(!entry_has_tag(&bytes, "slug:bar"));
    }

    #[test]
    fn uleb128_roundtrip_small() {
        for &v in &[0u64, 1, 127, 128, 255, 16383, 16384, u32::MAX as u64, u64::MAX] {
            let mut buf = Vec::new();
            write_uleb128(&mut buf, v);
            let (decoded, _) = read_uleb128(&buf).expect("decode");
            assert_eq!(decoded, v, "roundtrip failed for {v}");
        }
    }

    #[test]
    fn uleb128_rejects_oversized_continuation() {
        // 11 continuation bytes — past the u64 cap, should refuse.
        let bad = [0x80u8; 11];
        assert!(read_uleb128(&bad).is_none());
    }
}
