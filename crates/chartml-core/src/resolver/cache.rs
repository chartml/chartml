//! Pluggable caching for fetched provider results.
//!
//! `CacheBackend` is the trait host apps implement to plug in tier-1 storage
//! (default `MemoryBackend` is in-process), tier-2 storage (`IndexedDbBackend`
//! lands in phase 3b), or anything else they like (Redis, file-system, etc.).
//!
//! `CachedEntry` carries the `DataTable`, the wall-clock time it was fetched,
//! the TTL, free-form bulk-invalidation tags, and provider metadata so
//! cache-hits return identical metadata to the original fetch.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use thiserror::Error;

use crate::data::DataTable;

/// One cached source entry. Cloning is cheap because `DataTable` is
/// `Arc`-backed; metadata + tag clones are a small `HashMap`/`Vec`.
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub data: DataTable,
    pub fetched_at: SystemTime,
    pub ttl: Duration,
    /// Free-form tags for bulk invalidation. Typical values:
    /// `["slug:kyomi-analytics", "namespace:workspace-foo"]`.
    pub tags: Vec<String>,
    /// Provider metadata preserved with the cached entry (from
    /// `FetchResult.metadata`). Survives cache hits so callers see the same
    /// `bytes_billed` / `rows_returned` as on the original fetch.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CachedEntry {
    /// `true` once the entry has aged past its TTL. Conservative — clock
    /// regressions or `SystemTime` errors return `true` (treat as expired)
    /// so we never serve a stale row by accident.
    pub fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(self.fetched_at)
            .map(|age| age > self.ttl)
            .unwrap_or(true)
    }

    /// Wall-clock age of the entry. Returns `Duration::ZERO` if the system
    /// clock has gone backwards since the entry was inserted.
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.fetched_at)
            .unwrap_or(Duration::ZERO)
    }
}

/// Cache backend errors. Backed by `thiserror` so backends can wrap their
/// native error type (`Mutex` poisoning, IndexedDB errors, IO errors, etc.).
#[derive(Debug, Error, Clone)]
pub enum CacheError {
    /// Backend storage failed (poisoned mutex, IO error, IndexedDB transaction
    /// failure, …). The string is the implementation-specific detail.
    #[error("cache backend error: {0}")]
    Backend(String),
}

/// Pluggable cache backend trait. `?Send` on WASM mirrors the
/// `DataSourceProvider` / `TransformMiddleware` story so single-threaded
/// browser environments don't need `Send` bounds.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait CacheBackend: Send + Sync {
    /// Look up an entry. Backends may return `None` for "not present" OR
    /// "present but failed to deserialize" — the resolver treats both as
    /// cache miss and falls through to the next tier or the provider.
    async fn get(&self, key: u64) -> Option<CachedEntry>;

    /// Insert or replace an entry. Returns `Err` only on backend storage
    /// failure (poisoned mutex, IndexedDB transaction failure, etc.) — TTL
    /// math happens at `get` time, not `put`.
    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError>;

    /// Remove a single entry. No-op if the key is absent.
    async fn invalidate(&self, key: u64) -> Result<(), CacheError>;

    /// Remove every entry whose `tags` contain the given tag. Used by the
    /// resolver's `invalidate_by_slug` / `invalidate_by_namespace` APIs.
    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError>;

    /// Drop everything.
    async fn clear(&self) -> Result<(), CacheError>;

    /// Optional graceful shutdown (flush pending writes, close transactions).
    /// Default no-op.
    async fn shutdown(&self) {}
}

/// Default in-process tier-1 cache. `Arc<Mutex<HashMap<u64, CachedEntry>>>` —
/// no async I/O, no external dependencies, identical behavior on native and
/// WASM.
#[derive(Debug, Default, Clone)]
pub struct MemoryBackend {
    inner: Arc<Mutex<HashMap<u64, CachedEntry>>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Snapshot the number of entries — useful for tests and metrics.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("memory cache lock poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl CacheBackend for MemoryBackend {
    async fn get(&self, key: u64) -> Option<CachedEntry> {
        let guard = self.inner.lock().expect("memory cache lock poisoned");
        guard.get(&key).cloned()
    }

    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError> {
        let mut guard = self.inner.lock().expect("memory cache lock poisoned");
        guard.insert(key, entry);
        Ok(())
    }

    async fn invalidate(&self, key: u64) -> Result<(), CacheError> {
        let mut guard = self.inner.lock().expect("memory cache lock poisoned");
        guard.remove(&key);
        Ok(())
    }

    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError> {
        let mut guard = self.inner.lock().expect("memory cache lock poisoned");
        guard.retain(|_, entry| !entry.tags.iter().any(|t| t == tag));
        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut guard = self.inner.lock().expect("memory cache lock poisoned");
        guard.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Row;
    use serde_json::json;

    fn make_entry(tags: Vec<&str>) -> CachedEntry {
        let row: Row = [("x".to_string(), json!(1.0))].into_iter().collect();
        CachedEntry {
            data: DataTable::from_rows(&[row]).unwrap(),
            fetched_at: SystemTime::now(),
            ttl: Duration::from_secs(60),
            tags: tags.into_iter().map(String::from).collect(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn memory_backend_get_put_roundtrip() {
        let backend = MemoryBackend::new();
        backend.put(1, make_entry(vec![])).await.unwrap();
        let got = backend.get(1).await;
        assert!(got.is_some());
        assert_eq!(backend.len(), 1);
    }

    #[tokio::test]
    async fn memory_backend_invalidate_single() {
        let backend = MemoryBackend::new();
        backend.put(1, make_entry(vec![])).await.unwrap();
        backend.put(2, make_entry(vec![])).await.unwrap();
        backend.invalidate(1).await.unwrap();
        assert!(backend.get(1).await.is_none());
        assert!(backend.get(2).await.is_some());
    }

    #[tokio::test]
    async fn memory_backend_invalidate_by_tag() {
        let backend = MemoryBackend::new();
        backend.put(1, make_entry(vec!["slug:foo"])).await.unwrap();
        backend.put(2, make_entry(vec!["slug:foo"])).await.unwrap();
        backend.put(3, make_entry(vec!["slug:bar"])).await.unwrap();
        backend.invalidate_by_tag("slug:foo").await.unwrap();
        assert!(backend.get(1).await.is_none());
        assert!(backend.get(2).await.is_none());
        assert!(backend.get(3).await.is_some());
    }

    #[tokio::test]
    async fn memory_backend_clear() {
        let backend = MemoryBackend::new();
        backend.put(1, make_entry(vec![])).await.unwrap();
        backend.put(2, make_entry(vec![])).await.unwrap();
        backend.clear().await.unwrap();
        assert_eq!(backend.len(), 0);
    }

    #[tokio::test]
    async fn cached_entry_expiry() {
        let mut entry = make_entry(vec![]);
        entry.ttl = Duration::from_millis(0);
        // Allow a tiny moment for SystemTime to advance past fetched_at.
        std::thread::sleep(Duration::from_millis(2));
        assert!(entry.is_expired());
    }
}
