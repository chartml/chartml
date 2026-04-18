//! Resolver: caching + dedup + dispatch layer between `ChartML::fetch` and
//! the registered `DataSourceProvider` implementations (chartml 5.0 phase 3).
//!
//! The resolver owns:
//! - **Tier-1 cache** — a `MemoryBackend` always present.
//! - **Tier-2 cache** — optional `Arc<dyn CacheBackend>` (populated in
//!   phase 3b by `IndexedDbBackend`; phase 3 leaves it `None`).
//! - **In-flight tracker** — a `HashMap<u64, Shared<BoxFuture<...>>>` so two
//!   concurrent fetches for the same key share one provider invocation.
//! - **Provider registry** — `HashMap<String, Arc<dyn DataSourceProvider>>`,
//!   keyed by dispatch slug (`"inline"`, `"http"`, `"datasource"`, …).
//!
//! Phase 3 leaves `ResolverHooks` integration as a no-op stub; phase 3c will
//! add hook dispatch without further changes to this file's structure.

use std::collections::HashMap;
use std::sync::Arc;
// `web_time::SystemTime` works on `wasm32-unknown-unknown` (where the std
// version panics). On native it's a transparent alias for `std::time`.
use std::time::Duration;
use web_time::SystemTime;

use async_trait::async_trait;
use futures::future::{FutureExt, Shared};
use thiserror::Error;
use xxhash_rust::xxh3::Xxh3;

// `?Send` on WASM means the futures we shove into `Shared` cannot promise
// `Send`. Use `LocalBoxFuture` on WASM and `BoxFuture` (which IS `Send`) on
// native so the resolver can stay multi-threaded on tokio while compiling
// cleanly under `wasm32-unknown-unknown`.
#[cfg(not(target_arch = "wasm32"))]
type ResolverFuture<T> = futures::future::BoxFuture<'static, T>;
#[cfg(target_arch = "wasm32")]
type ResolverFuture<T> = futures::future::LocalBoxFuture<'static, T>;

// Cfg-gated shared-ownership + interior-mutability primitives. On native we
// use `Arc<Mutex<T>>` so the resolver stays multi-threaded for tokio. On
// WASM the resolver's inflight `Shared<LocalBoxFuture<...>>` map is
// inherently `?Send`, so wrapping it in `Arc<Mutex<...>>` would trip
// `clippy::arc_with_non_send_sync`. Single-threaded `Rc<RefCell<...>>` is
// the correct primitive for the wasm32-unknown-unknown target.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type SharedRef<T> = Arc<T>;
#[cfg(target_arch = "wasm32")]
pub(crate) type SharedRef<T> = std::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Lock<T> = std::sync::Mutex<T>;
#[cfg(target_arch = "wasm32")]
pub(crate) type Lock<T> = std::cell::RefCell<T>;

/// Cfg-gated extension trait so `self.field.write_lock("…").something()` and
/// `self.field.read_lock("…").something()` work uniformly across the native
/// `Mutex` and the WASM `RefCell`. Eliminates per-call-site `cfg` blocks
/// without losing the panic message that distinguishes which lock failed.
trait LockExt<T> {
    type Read<'a>: std::ops::Deref<Target = T>
    where
        Self: 'a;
    type Write<'a>: std::ops::DerefMut<Target = T>
    where
        Self: 'a;
    fn read_lock(&self, label: &'static str) -> Self::Read<'_>;
    fn write_lock(&self, label: &'static str) -> Self::Write<'_>;
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> LockExt<T> for std::sync::Mutex<T> {
    type Read<'a>
        = std::sync::MutexGuard<'a, T>
    where
        Self: 'a;
    type Write<'a>
        = std::sync::MutexGuard<'a, T>
    where
        Self: 'a;
    fn read_lock(&self, label: &'static str) -> Self::Read<'_> {
        self.lock()
            .unwrap_or_else(|_| panic!("resolver {label} lock poisoned"))
    }
    fn write_lock(&self, label: &'static str) -> Self::Write<'_> {
        self.lock()
            .unwrap_or_else(|_| panic!("resolver {label} lock poisoned"))
    }
}

#[cfg(target_arch = "wasm32")]
impl<T> LockExt<T> for std::cell::RefCell<T> {
    type Read<'a>
        = std::cell::Ref<'a, T>
    where
        Self: 'a;
    type Write<'a>
        = std::cell::RefMut<'a, T>
    where
        Self: 'a;
    fn read_lock(&self, label: &'static str) -> Self::Read<'_> {
        self.try_borrow()
            .unwrap_or_else(|_| panic!("resolver {label} cell already borrowed mutably"))
    }
    fn write_lock(&self, label: &'static str) -> Self::Write<'_> {
        self.try_borrow_mut()
            .unwrap_or_else(|_| panic!("resolver {label} cell already borrowed"))
    }
}

use crate::data::DataTable;
use crate::error::ChartError;
use crate::spec::source::CacheConfig as SpecCacheConfig;
use crate::spec::InlineData;

pub mod builtin;
pub mod cache;
pub mod cancel;
pub mod hooks;

// Phase 3b: persistent cache backends. The container module is unconditional
// because it hosts the pure-Rust `codec` submodule (shared blob framing) that
// needs to compile + test on every target. Backend implementations themselves
// (`indexeddb`, future `sqlite`/`fs`/…) carry their own `#[cfg(...)]` gates
// inside `backends/mod.rs`, so non-browser builds and browser builds without
// the `wasm-indexeddb` feature don't compile any of the IndexedDB-specific
// code (which depends on the `idb` crate, brought in only by that feature).
pub mod backends;

pub use builtin::{HttpProvider, InlineProvider};
pub use cache::{CacheBackend, CacheError, CachedEntry, MemoryBackend};
pub use cancel::CancellationToken;
pub use hooks::{
    CacheHitEvent, CacheMissEvent, CacheTier, ErrorEvent, HooksRef, MissReason, NullHooks, Phase,
    ProgressEvent, ResolverHooks,
};

/// Default TTL applied when a spec doesn't declare one. Five minutes matches
/// the JS middleware's `CACHE_TTL_DEFAULT_MS`.
pub const DEFAULT_TTL: Duration = Duration::from_secs(5 * 60);

/// Sentinel byte mixed into the hash for `None` fields. Distinct from any
/// UTF-8 byte (0xFE is invalid as a UTF-8 start byte) so `None` can never
/// collide with a real string value (e.g., a literal `"None"` datasource).
const HASH_NONE_SENTINEL: u8 = 0xFE;
const HASH_FIELD_SEP: u8 = 0xFF;

/// Provider dispatch + fetch trait. Host apps implement this for their
/// custom data sources (BigQuery, Snowflake, internal REST APIs, …) and
/// register them via `ChartML::register_provider(kind, provider)`.
///
/// `?Send` on WASM matches `TransformMiddleware` so single-threaded
/// browser environments can hold non-`Send` state inside provider impls.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait DataSourceProvider: Send + Sync {
    /// Fetch one source. The resolver handles caching + dedup + parallelism;
    /// providers only see this single call per actual upstream invocation.
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError>;

    /// Optional graceful shutdown hook. Called by `ChartML::shutdown()` on
    /// SSR request end / tab close. Default no-op so providers that hold no
    /// pooled resources don't have to implement it.
    async fn shutdown(&self) {}
}

/// Per-source request context. Carries the resolved spec (params already
/// substituted) plus cache + header + namespace + cancellation hints.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// User-chosen name for this source within the chart spec. `None` for
    /// unnamed (flat) `data:` forms.
    pub source_name: Option<String>,
    /// Fully resolved flat-form source spec — `datasource`, `query`, `url`,
    /// `rows`, etc. with `$param.name` references already substituted.
    pub spec: InlineData,
    /// Parsed cache config from `spec.cache.ttl` (and `auto_refresh`).
    pub cache: Option<CacheConfig>,
    /// Request-level HTTP headers (merged with `HttpProvider` defaults).
    /// Ignored by non-HTTP providers unless they explicitly read this field.
    pub headers: HashMap<String, String>,
    /// Tenant / workspace namespace folded into the cache key. `None` for
    /// single-tenant deployments.
    pub namespace: Option<String>,
    /// Optional cancellation token. Phase 3 always passes `None` from the
    /// resolver; providers that opt into honoring it stay forward-compatible
    /// with future tab-close / cancel-on-route-change wiring.
    pub cancel_token: Option<CancellationToken>,
}

/// Provider response. `Clone` is required because `futures::future::Shared`
/// (used for in-flight dedup) takes a `Future<Output: Clone>`. `DataTable`
/// is `Arc`-backed so cloning is cheap.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub data: DataTable,
    /// Free-form per-provider metadata — `bytes_billed`, `rows_returned`,
    /// `server_refreshed_at`, `upstream_cache_hit`, `warnings`, etc.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Provider failures. `Clone` matches `FetchResult` so `Shared` can clone
/// the error when multiple in-flight callers receive the same outcome.
#[derive(Debug, Error, Clone)]
pub enum FetchError {
    #[error("datasource '{slug}' not found")]
    SlugNotFound { slug: String },

    #[error("query failed: {0}")]
    QueryFailed(String),

    #[error("decode failed: {0}")]
    DecodeFailed(String),

    #[error("cancelled")]
    Cancelled,

    #[error("no provider registered for kind '{kind}'")]
    ProviderNotFound { kind: String },

    #[error("cache backend error: {0}")]
    Cache(String),

    #[error("{0}")]
    Other(String),
}

impl From<FetchError> for ChartError {
    fn from(err: FetchError) -> Self {
        match err {
            FetchError::SlugNotFound { slug } => {
                ChartError::DataError(format!("datasource '{slug}' not found"))
            }
            FetchError::QueryFailed(msg) => ChartError::DataError(format!("query failed: {msg}")),
            FetchError::DecodeFailed(msg) => ChartError::DataError(format!("decode failed: {msg}")),
            FetchError::Cancelled => ChartError::DataError("fetch cancelled".to_string()),
            FetchError::ProviderNotFound { kind } => ChartError::PluginError(format!(
                "no provider registered for kind '{kind}'"
            )),
            FetchError::Cache(msg) => ChartError::DataError(format!("cache error: {msg}")),
            FetchError::Other(msg) => ChartError::DataError(msg),
        }
    }
}

impl From<CacheError> for FetchError {
    fn from(err: CacheError) -> Self {
        FetchError::Cache(err.to_string())
    }
}

/// Parsed cache config. Built from `spec::source::CacheConfig` (which carries
/// the raw YAML strings) so the resolver doesn't have to re-parse `humantime`
/// formats on every fetch.
#[derive(Debug, Clone, Default)]
pub struct CacheConfig {
    /// TTL parsed via `humantime` (`"30s"`, `"5m"`, `"6h"`, `"1d"`, `"7d"`).
    /// `None` → default TTL applies.
    pub ttl: Option<Duration>,
    /// Component-layer hint — the resolver doesn't auto-refresh, but the
    /// flag is preserved end-to-end so consumers (Leptos, React) can read it.
    pub auto_refresh: bool,
}

impl CacheConfig {
    /// Parse a `spec::source::CacheConfig` (the raw YAML form) into the
    /// resolver-friendly shape. Returns `Ok(None)` when the input is `None`
    /// so the upstream `Option` plumbing stays clean. Returns `Err` when the
    /// declared TTL string is present but unparseable — silent fallback to
    /// `DEFAULT_TTL` would let an operator's typo (`"5 minutes"` instead of
    /// `"5m"`) ship to production unnoticed, so the parser's complaint is
    /// surfaced to the caller verbatim.
    pub fn from_spec(spec: Option<&SpecCacheConfig>) -> Result<Option<Self>, ChartError> {
        let Some(spec) = spec else {
            return Ok(None);
        };
        let ttl = match spec.ttl.as_deref() {
            Some(s) => Some(humantime::parse_duration(s).map_err(|e| {
                ChartError::InvalidSpec(format!(
                    "invalid cache.ttl value {s:?}: {e} (expected humantime format like \"30s\", \"5m\", \"6h\", \"1d\")"
                ))
            })?),
            None => None,
        };
        Ok(Some(Self {
            ttl,
            auto_refresh: spec.auto_refresh.unwrap_or(false),
        }))
    }

    /// Convenience accessor matching the design doc's `ttl_duration()` name.
    pub fn ttl_duration(&self) -> Option<Duration> {
        self.ttl
    }
}

/// Outcome of a `Resolver::fetch` call: the provider's result PLUS whether
/// the value came from a cache tier or a fresh provider invocation.
///
/// `ChartML::fetch` uses this to populate `FetchMetadata.cache_hits` /
/// `cache_misses` per source name without needing the resolver to know about
/// source names itself (which it doesn't — it works in keys, not names).
#[derive(Debug, Clone)]
pub struct ResolveOutcome {
    pub result: FetchResult,
    pub cache_hit: bool,
}

/// Tag prefix for source-slug-based bulk invalidation. Public so consumers
/// computing tags out-of-band (e.g., a custom CacheBackend that wants to
/// pre-populate entries) match the resolver's wire format exactly.
pub const TAG_SLUG_PREFIX: &str = "slug:";
/// Tag prefix for namespace-based bulk invalidation.
pub const TAG_NAMESPACE_PREFIX: &str = "namespace:";

/// Public alias for the shared-ownership wrapper around `Resolver`. `Arc` on
/// native (so consumers using `tokio::spawn` can move resolver handles across
/// task boundaries) and `Rc` on WASM (where the resolver's inflight map is
/// inherently `?Send`, so an `Arc` would be both incorrect and rejected by
/// `clippy::arc_with_non_send_sync`). Returned by `ChartML::resolver()` and
/// accepted by every host that wants a long-lived handle for the bulk
/// `invalidate*` API.
#[cfg(not(target_arch = "wasm32"))]
pub type ResolverRef = std::sync::Arc<Resolver>;
#[cfg(target_arch = "wasm32")]
pub type ResolverRef = std::rc::Rc<Resolver>;

/// Provider dispatch + cache + dedup orchestration.
///
/// One `Resolver` per `ChartML` instance. The resolver is held inside
/// `ChartML` as `SharedRef<Resolver>` (`Arc` on native, `Rc` on WASM) so
/// consumers can grab a handle for the `invalidate*` API while the chart
/// instance keeps using it.
pub struct Resolver {
    /// Default in-process cache (always present, never replaced — kept for
    /// the rare case a consumer wants to introspect the in-memory tier
    /// directly even after swapping `primary`).
    memory: Arc<MemoryBackend>,
    /// Tier-1 cache. Defaults to the always-present in-memory backend; can
    /// be replaced via `ChartML::set_cache(...)` (e.g., a host's custom
    /// process-wide LRU). Behind a `Lock<Arc<...>>` so swaps are atomic
    /// and don't require `&mut self` (the resolver is held inside a
    /// `SharedRef`).
    primary: Lock<Arc<dyn CacheBackend>>,
    /// Tier-2 (persistent) cache. `None` in phase 3 — phase 3b populates it
    /// with `IndexedDbBackend` for browser consumers.
    persistent: Lock<Option<Arc<dyn CacheBackend>>>,
    inflight: SharedRef<Lock<HashMap<u64, SharedFetch>>>,
    providers: Lock<HashMap<String, Arc<dyn DataSourceProvider>>>,
    /// Optional hook impl. Wrapped in `Lock<Option<...>>` so `set_hooks`
    /// works without `&mut self` (the resolver lives behind a `SharedRef`).
    /// Snapshotted into a local clone before each instrumentation site so
    /// the hook lock is never held across an `await`.
    hooks: Lock<Option<HooksRef>>,
}

/// Shared in-flight future type. Boxed for dyn-trait erasure; `Shared` lets
/// multiple awaiters poll the same future without cloning the work. The
/// inner `ResolverFuture` is `BoxFuture` (Send) on native and
/// `LocalBoxFuture` (?Send) on WASM so we don't conflict with the
/// `?Send` async traits the providers and cache backends use.
type SharedFetch = Shared<ResolverFuture<Result<FetchResult, FetchError>>>;

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_persistent = self.persistent.read_lock("persistent cache").is_some();
        let provider_keys: Vec<String> = self
            .providers
            .read_lock("providers")
            .keys()
            .cloned()
            .collect();
        f.debug_struct("Resolver")
            .field("memory", &self.memory)
            .field("has_persistent", &has_persistent)
            .field("providers", &provider_keys)
            .finish_non_exhaustive()
    }
}

impl Resolver {
    /// New resolver with the default `MemoryBackend` as tier-1, no tier-2,
    /// and no providers registered. `ChartML::new()` registers the built-in
    /// `inline` + `http` providers immediately after construction.
    pub fn new() -> Self {
        let memory = Arc::new(MemoryBackend::new());
        let primary: Arc<dyn CacheBackend> = memory.clone();
        Self {
            memory,
            primary: Lock::new(primary),
            persistent: Lock::new(None),
            inflight: SharedRef::new(Lock::new(HashMap::new())),
            providers: Lock::new(HashMap::new()),
            hooks: Lock::new(None),
        }
    }

    /// Replace the tier-1 cache backend. Used by `ChartML::set_cache`.
    /// The fresh backend starts empty — entries in the old backend are not
    /// migrated (caller's responsibility if they want to).
    pub fn set_primary_cache(&self, backend: Arc<dyn CacheBackend>) {
        let mut guard = self.primary.write_lock("primary cache");
        *guard = backend;
    }

    /// Set the optional tier-2 (persistent) cache. Phase 3 leaves this
    /// public so phase 3b's `IndexedDbBackend` can wire in without further
    /// surface changes.
    pub fn set_persistent_cache(&self, backend: Arc<dyn CacheBackend>) {
        let mut guard = self.persistent.write_lock("persistent cache");
        *guard = Some(backend);
    }

    /// Register a [`ResolverHooks`] impl. Replaces any previously registered
    /// hooks; passes the new impl in as a `HooksRef` (`Arc` on native, `Rc`
    /// on WASM). After this call every `Resolver::fetch` invocation emits
    /// progress / cache / error events through the new impl.
    pub fn set_hooks(&self, hooks: HooksRef) {
        let mut guard = self.hooks.write_lock("hooks");
        *guard = Some(hooks);
    }

    /// Clear any previously registered hooks. Subsequent `fetch` calls
    /// behave as if `set_hooks` had never been called (no-op emission).
    pub fn clear_hooks(&self) {
        let mut guard = self.hooks.write_lock("hooks");
        *guard = None;
    }

    /// Snapshot the current hooks `HooksRef` (or `None`) so the resolver
    /// can release the lock before entering any cache walk or `.await`.
    /// Must be called once at the top of `fetch`; downstream sites use the
    /// snapshot rather than re-acquiring the lock to keep the hook lock
    /// off the hot path. Also called by `ChartML::transform` (one crate up)
    /// at the top of the transform stage for the same reason.
    pub(crate) fn hooks_snapshot(&self) -> Option<HooksRef> {
        self.hooks.read_lock("hooks").clone()
    }

    /// Snapshot the tier-1 cache `Arc` so we can drop the lock before the
    /// async cache call. Always returns a backend (the field starts as
    /// `MemoryBackend` and `set_primary_cache` only replaces, never clears).
    fn primary_snapshot(&self) -> Arc<dyn CacheBackend> {
        self.primary.read_lock("primary cache").clone()
    }

    /// Snapshot the optional tier-2 cache `Arc` (or `None`) for the same
    /// reason `primary_snapshot` exists — release the sync lock before
    /// the async cache call.
    fn persistent_snapshot(&self) -> Option<Arc<dyn CacheBackend>> {
        self.persistent.read_lock("persistent cache").clone()
    }

    /// Register a provider under a dispatch key (`"inline"`, `"http"`,
    /// `"datasource"`, or any custom slug). Re-registration replaces the
    /// previously registered provider for that key.
    pub fn register_provider(&self, kind: &str, provider: Arc<dyn DataSourceProvider>) {
        let mut providers = self.providers.write_lock("providers");
        providers.insert(kind.to_string(), provider);
    }

    /// Snapshot the registered provider kinds. Useful for tests and
    /// host-app diagnostics.
    pub fn provider_kinds(&self) -> Vec<String> {
        self.providers
            .read_lock("providers")
            .keys()
            .cloned()
            .collect()
    }

    /// Compute the cache key the resolver would use for a given spec.
    ///
    /// Public so phase 4 (Leptos refresh button) and phase 6 (Kyomi
    /// invalidate-on-change) can compute the exact key the resolver caches
    /// under without re-implementing the hash.
    ///
    /// Hashes `(namespace, datasource, query, url, provider, rows_hash)` in
    /// that order. `None` fields contribute a sentinel byte (`0xFE`, an
    /// invalid UTF-8 start byte) so they cannot collide with a real string
    /// value of `"None"`. Field separator is `0xFF` (also invalid UTF-8) so
    /// adjacent fields can't bleed into each other ("a|b" vs "ab|").
    pub fn key_for(spec: &InlineData, namespace: Option<&str>) -> u64 {
        let mut hasher = Xxh3::new();

        let fields: [Option<&str>; 5] = [
            namespace,
            spec.datasource.as_deref(),
            spec.query.as_deref(),
            spec.url.as_deref(),
            spec.provider.as_deref(),
        ];
        for field in fields {
            match field {
                Some(s) => hasher.update(s.as_bytes()),
                None => hasher.update(&[HASH_NONE_SENTINEL]),
            }
            hasher.update(&[HASH_FIELD_SEP]);
        }

        // Inline `rows` is hashed via its canonical JSON serialization so
        // two specs with the same row data hash identically regardless of
        // hashmap iteration order. `None` rows contribute the sentinel.
        match spec.rows.as_ref() {
            Some(rows) => match serde_json::to_vec(rows) {
                Ok(bytes) => hasher.update(&bytes),
                Err(_) => hasher.update(&[HASH_NONE_SENTINEL]),
            },
            None => hasher.update(&[HASH_NONE_SENTINEL]),
        }

        hasher.digest()
    }

    /// The core orchestration: tier-1 → tier-2 → in-flight dedup → provider.
    ///
    /// Returns `ResolveOutcome` (result + cache-hit flag) so the caller can
    /// classify the source under `cache_hits` vs `cache_misses` in
    /// `FetchMetadata`. Tier-1 hits hydrate from memory only; tier-2 hits
    /// also re-populate tier-1 so subsequent reads in the same session
    /// short-circuit.
    pub async fn fetch(
        &self,
        key: u64,
        request: FetchRequest,
    ) -> Result<ResolveOutcome, FetchError> {
        let primary = self.primary_snapshot();
        let persistent = self.persistent_snapshot();
        // Snapshot the hooks `HooksRef` ONCE up front so the hooks lock is
        // never held across an `.await` and downstream sites can share the
        // same clone without re-acquiring.
        let hooks = self.hooks_snapshot();
        let source_name = request.source_name.clone();

        // ── Tier 1: in-process cache ──
        let mut tier1_expired = false;
        if let Some(entry) = primary.get(key).await {
            if !entry.is_expired() {
                let age = entry.age();
                emit_cache_hit(&hooks, key, &source_name, hooks::CacheTier::Memory, age);
                return Ok(ResolveOutcome {
                    result: FetchResult {
                        data: entry.data,
                        metadata: entry.metadata,
                    },
                    cache_hit: true,
                });
            }
            // Expired — let it fall through; we'll overwrite on success.
            tier1_expired = true;
        }

        // ── Tier 2: persistent cache (phase 3b populates this) ──
        let mut tier2_expired = false;
        if let Some(p) = &persistent {
            if let Some(entry) = p.get(key).await {
                if !entry.is_expired() {
                    let age = entry.age();
                    // Hydrate tier-1 so subsequent reads stay in-process.
                    // Memory hydration is silent (no event) — only the
                    // logical tier-2 hit fires.
                    let _ = primary.put(key, entry.clone()).await;
                    emit_cache_hit(
                        &hooks,
                        key,
                        &source_name,
                        hooks::CacheTier::Persistent,
                        age,
                    );
                    return Ok(ResolveOutcome {
                        result: FetchResult {
                            data: entry.data,
                            metadata: entry.metadata,
                        },
                        cache_hit: true,
                    });
                }
                // Expired — evict from tier-2 too.
                let _ = p.invalidate(key).await;
                tier2_expired = true;
            }
        }

        // Both tiers missed — emit one cache-miss with the most specific
        // reason we can prove. `Expired` if either tier had an entry that
        // had aged out; `NotFound` otherwise. (`Invalidated` is reserved
        // for explicit invalidation flows wired later.)
        let miss_reason = if tier1_expired || tier2_expired {
            hooks::MissReason::Expired
        } else {
            hooks::MissReason::NotFound
        };
        emit_cache_miss(&hooks, key, &source_name, miss_reason);

        // Provider call start — fire BEFORE the dedup wait so consumers
        // see a "fetching" event for every distinct cache miss, not just
        // the first concurrent one.
        emit_progress(
            &hooks,
            hooks::Phase::Fetch,
            &source_name,
            None,
            None,
            format!(
                "Fetching {}",
                source_name.as_deref().unwrap_or("source"),
            ),
        );

        // ── In-flight dedup ──
        // If another fetch for the same key is already in flight, await its
        // shared future. Otherwise install a new shared future and run it.
        let shared = self.intern_inflight(key, request, primary, persistent);
        let result = shared.await;

        // Cleanup: drop the inflight slot whether the fetch succeeded or
        // failed so subsequent calls re-dispatch instead of replaying a
        // stale terminal state. Errors clear too — failed fetches should
        // retry, not stick.
        self.inflight.write_lock("inflight").remove(&key);

        match result {
            Ok(fetch_result) => {
                let row_count = fetch_result.data.num_rows();
                emit_progress(
                    &hooks,
                    hooks::Phase::Fetch,
                    &source_name,
                    Some(row_count as u64),
                    None,
                    format!(
                        "Fetched {} ({} rows)",
                        source_name.as_deref().unwrap_or("source"),
                        row_count,
                    ),
                );
                Ok(ResolveOutcome {
                    result: fetch_result,
                    cache_hit: false,
                })
            }
            Err(err) => {
                emit_error(&hooks, hooks::Phase::Fetch, &source_name, err.to_string());
                Err(err)
            }
        }
    }

    /// Get-or-insert the in-flight `Shared` future for a key. Returns a
    /// clone of the `Shared` so the caller can `await` it; the original
    /// stays in the map until removed by the caller.
    fn intern_inflight(
        &self,
        key: u64,
        request: FetchRequest,
        primary: Arc<dyn CacheBackend>,
        persistent: Option<Arc<dyn CacheBackend>>,
    ) -> SharedFetch {
        let mut inflight = self.inflight.write_lock("inflight");
        if let Some(existing) = inflight.get(&key) {
            return existing.clone();
        }

        // Build the work future: dispatch to the right provider, write to
        // both cache tiers on success. The future is `'static` because it
        // owns everything it touches via `Arc` clones.
        let providers = self.snapshot_providers();
        let cache_cfg = request.cache.clone();
        let namespace = request.namespace.clone();
        let slug = request.spec.datasource.clone();

        let work = async move {
            let provider = dispatch_provider(&providers, &request.spec)?;
            let result = provider.fetch(request).await?;

            // Write-through: tier-1 always; tier-2 if configured.
            let entry = CachedEntry {
                data: result.data.clone(),
                fetched_at: SystemTime::now(),
                ttl: cache_cfg
                    .as_ref()
                    .and_then(|c| c.ttl_duration())
                    .unwrap_or(DEFAULT_TTL),
                tags: build_tags(slug.as_deref(), namespace.as_deref()),
                metadata: result.metadata.clone(),
            };
            // Cache write failures must not poison the result — log via
            // tracing in 3c, but for now just swallow (matches the design
            // doc's `.ok()` guidance).
            let _ = primary.put(key, entry.clone()).await;
            if let Some(p) = &persistent {
                let _ = p.put(key, entry).await;
            }
            Ok(result)
        };

        // Pick the right Box variant for the target. `boxed()` is Send-only
        // (native), `boxed_local()` is ?Send (WASM).
        #[cfg(not(target_arch = "wasm32"))]
        let boxed: ResolverFuture<Result<FetchResult, FetchError>> = work.boxed();
        #[cfg(target_arch = "wasm32")]
        let boxed: ResolverFuture<Result<FetchResult, FetchError>> = work.boxed_local();

        let future = boxed.shared();
        inflight.insert(key, future.clone());
        future
    }

    /// Snapshot the provider map for use inside the inflight future. We
    /// snapshot rather than holding the lock so the future stays `'static`.
    fn snapshot_providers(&self) -> HashMap<String, Arc<dyn DataSourceProvider>> {
        self.providers.read_lock("providers").clone()
    }

    // ── Bulk invalidation API ──

    /// Drop a single entry from every cache tier.
    pub async fn invalidate(&self, key: u64) {
        let primary = self.primary_snapshot();
        let persistent = self.persistent_snapshot();
        let _ = primary.invalidate(key).await;
        if let Some(p) = &persistent {
            let _ = p.invalidate(key).await;
        }
    }

    /// Drop every cached entry across all tiers.
    pub async fn invalidate_all(&self) {
        let primary = self.primary_snapshot();
        let persistent = self.persistent_snapshot();
        let _ = primary.clear().await;
        if let Some(p) = &persistent {
            let _ = p.clear().await;
        }
    }

    /// Drop every entry whose source spec carried the given `datasource`
    /// slug. Useful for "datasource X was edited; invalidate all queries
    /// against it" workflows.
    pub async fn invalidate_by_slug(&self, slug: &str) {
        let tag = format!("{TAG_SLUG_PREFIX}{slug}");
        let primary = self.primary_snapshot();
        let persistent = self.persistent_snapshot();
        let _ = primary.invalidate_by_tag(&tag).await;
        if let Some(p) = &persistent {
            let _ = p.invalidate_by_tag(&tag).await;
        }
    }

    /// Drop every entry tagged with the given namespace. Used for tenant
    /// isolation flows ("user logged out; clear their cached data").
    pub async fn invalidate_by_namespace(&self, namespace: &str) {
        let tag = format!("{TAG_NAMESPACE_PREFIX}{namespace}");
        let primary = self.primary_snapshot();
        let persistent = self.persistent_snapshot();
        let _ = primary.invalidate_by_tag(&tag).await;
        if let Some(p) = &persistent {
            let _ = p.invalidate_by_tag(&tag).await;
        }
    }

    /// Iterate every registered provider AND cache backend, awaiting their
    /// `shutdown` hook in turn. Wired up by `ChartML::shutdown()`.
    pub async fn shutdown(&self) {
        let providers = self.snapshot_providers();
        for (_, provider) in providers {
            provider.shutdown().await;
        }
        let primary = self.primary_snapshot();
        primary.shutdown().await;
        if let Some(p) = self.persistent_snapshot() {
            p.shutdown().await;
        }
    }
}

/// Emit a [`hooks::CacheHitEvent`] through the registered hook impl, if
/// any. Fire-and-forget via `spawn_hook`; never blocks the resolver.
fn emit_cache_hit(
    hooks: &Option<HooksRef>,
    key: u64,
    source_name: &Option<String>,
    tier: hooks::CacheTier,
    age: Duration,
) {
    let Some(h) = hooks.as_ref() else { return };
    let h = h.clone();
    let event = hooks::CacheHitEvent {
        key,
        source_name: source_name.clone(),
        tier,
        age,
    };
    hooks::spawn_hook(async move {
        h.on_cache_hit(event).await;
    });
}

/// Emit a [`hooks::CacheMissEvent`] through the registered hook impl.
fn emit_cache_miss(
    hooks: &Option<HooksRef>,
    key: u64,
    source_name: &Option<String>,
    reason: hooks::MissReason,
) {
    let Some(h) = hooks.as_ref() else { return };
    let h = h.clone();
    let event = hooks::CacheMissEvent {
        key,
        source_name: source_name.clone(),
        reason,
    };
    hooks::spawn_hook(async move {
        h.on_cache_miss(event).await;
    });
}

/// Emit a [`hooks::ProgressEvent`] through the registered hook impl.
/// `pub(crate)` so `ChartML::transform` / `render_prepared_to_svg` can
/// emit transform/render-phase progress without re-implementing the
/// snapshot dance.
pub(crate) fn emit_progress(
    hooks: &Option<HooksRef>,
    phase: hooks::Phase,
    source_name: &Option<String>,
    loaded: Option<u64>,
    total: Option<u64>,
    message: String,
) {
    let Some(h) = hooks.as_ref() else { return };
    let h = h.clone();
    let event = hooks::ProgressEvent {
        phase,
        source_name: source_name.clone(),
        loaded,
        total,
        message,
    };
    hooks::spawn_hook(async move {
        h.on_progress(event).await;
    });
}

/// Emit a [`hooks::ErrorEvent`] through the registered hook impl.
/// `pub(crate)` so `ChartML::transform` can emit transform-phase errors.
pub(crate) fn emit_error(
    hooks: &Option<HooksRef>,
    phase: hooks::Phase,
    source_name: &Option<String>,
    error: String,
) {
    let Some(h) = hooks.as_ref() else { return };
    let h = h.clone();
    let event = hooks::ErrorEvent {
        phase,
        source_name: source_name.clone(),
        error,
    };
    hooks::spawn_hook(async move {
        h.on_error(event).await;
    });
}

/// Build the tag list applied to `CachedEntry` on write. `slug` and
/// `namespace` are optional — entries without one of them simply skip the
/// corresponding tag (no empty-string tag pollution).
fn build_tags(slug: Option<&str>, namespace: Option<&str>) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(slug) = slug {
        tags.push(format!("{TAG_SLUG_PREFIX}{slug}"));
    }
    if let Some(ns) = namespace {
        tags.push(format!("{TAG_NAMESPACE_PREFIX}{ns}"));
    }
    tags
}

/// Apply the design-doc dispatch routing. Precedence: explicit `provider`
/// key wins over inferred shape (rows → inline, url → http, datasource →
/// datasource). Returns the `Arc` so the caller can `await` the trait
/// method without holding any lock.
fn dispatch_provider(
    providers: &HashMap<String, Arc<dyn DataSourceProvider>>,
    spec: &InlineData,
) -> Result<Arc<dyn DataSourceProvider>, FetchError> {
    let kind = if let Some(kind) = spec.provider.as_deref() {
        kind
    } else if spec.rows.is_some() {
        "inline"
    } else if spec.url.is_some() {
        "http"
    } else if spec.datasource.is_some() {
        "datasource"
    } else {
        return Err(FetchError::Other(
            "no dispatch match for spec — needs one of `provider`, `rows`, `url`, or `datasource`"
                .to_string(),
        ));
    };

    providers
        .get(kind)
        .cloned()
        .ok_or_else(|| FetchError::ProviderNotFound {
            kind: kind.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_inline() -> InlineData {
        InlineData {
            provider: None,
            rows: None,
            url: None,
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        }
    }

    #[test]
    fn key_for_is_deterministic() {
        let spec = InlineData {
            datasource: Some("warehouse".into()),
            query: Some("SELECT 1".into()),
            ..empty_inline()
        };
        let k1 = Resolver::key_for(&spec, Some("ns"));
        let k2 = Resolver::key_for(&spec, Some("ns"));
        assert_eq!(k1, k2);
    }

    #[test]
    fn key_for_namespace_changes_key() {
        let spec = InlineData {
            datasource: Some("warehouse".into()),
            ..empty_inline()
        };
        let k1 = Resolver::key_for(&spec, Some("tenant-a"));
        let k2 = Resolver::key_for(&spec, Some("tenant-b"));
        assert_ne!(k1, k2);
    }

    #[test]
    fn key_for_none_distinguishes_from_literal_none_string() {
        let spec_none = InlineData {
            datasource: None,
            url: Some("https://x".into()),
            ..empty_inline()
        };
        let spec_literal = InlineData {
            datasource: Some("None".into()),
            url: Some("https://x".into()),
            ..empty_inline()
        };
        // Sentinel byte must NOT collide with the literal "None" string.
        assert_ne!(
            Resolver::key_for(&spec_none, None),
            Resolver::key_for(&spec_literal, None),
        );
    }

    #[test]
    fn key_for_field_separator_prevents_bleed() {
        // `(datasource="ab", query=None)` must hash differently from
        // `(datasource="a", query="b")` — without a separator they'd both
        // serialize to "ab".
        let merged = InlineData {
            datasource: Some("ab".into()),
            ..empty_inline()
        };
        let split = InlineData {
            datasource: Some("a".into()),
            query: Some("b".into()),
            ..empty_inline()
        };
        assert_ne!(
            Resolver::key_for(&merged, None),
            Resolver::key_for(&split, None),
        );
    }

    #[test]
    fn dispatch_provider_precedence_explicit_wins() {
        let providers: HashMap<String, Arc<dyn DataSourceProvider>> = [(
            "custom".to_string(),
            Arc::new(InlineProvider::new()) as Arc<dyn DataSourceProvider>,
        )]
        .into_iter()
        .collect();
        let spec = InlineData {
            provider: Some("custom".into()),
            // rows would normally route to "inline", but explicit wins.
            rows: Some(vec![]),
            ..empty_inline()
        };
        // We can't compare trait objects, but `is_ok()` is sufficient: the
        // ONLY registered provider key is "custom", so reaching `Ok` means
        // dispatch picked it. Routing rows alone (no `provider`) would
        // search for "inline", which is unregistered → would return
        // `ProviderNotFound`.
        assert!(dispatch_provider(&providers, &spec).is_ok());
    }

    #[test]
    fn dispatch_provider_inferred_inline() {
        let providers: HashMap<String, Arc<dyn DataSourceProvider>> = [(
            "inline".to_string(),
            Arc::new(InlineProvider::new()) as Arc<dyn DataSourceProvider>,
        )]
        .into_iter()
        .collect();
        let spec = InlineData {
            rows: Some(vec![]),
            ..empty_inline()
        };
        assert!(dispatch_provider(&providers, &spec).is_ok());
    }

    #[test]
    fn dispatch_provider_missing_kind_errors() {
        let providers: HashMap<String, Arc<dyn DataSourceProvider>> = HashMap::new();
        let spec = InlineData {
            datasource: Some("warehouse".into()),
            ..empty_inline()
        };
        let err = dispatch_provider(&providers, &spec).err().expect("dispatch must error");
        assert!(matches!(err, FetchError::ProviderNotFound { ref kind } if kind == "datasource"));
    }

    #[test]
    fn dispatch_provider_unmatched_spec_errors() {
        let providers: HashMap<String, Arc<dyn DataSourceProvider>> = HashMap::new();
        let spec = empty_inline();
        let err = dispatch_provider(&providers, &spec).err().expect("dispatch must error");
        assert!(matches!(err, FetchError::Other(_)));
    }
}
