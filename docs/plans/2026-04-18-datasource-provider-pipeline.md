# ChartML Data-Source Provider Pipeline

**Date:** 2026-04-18
**Status:** Ready to execute (phases runnable via `/agent-driven-development`)
**Author:** Jason Adams
**Linear:** [KYO-79](https://linear.app/kyomi/issue/KYO-79/dashboard-port-reacts-duckdb-named-sources-pattern-to-leptos-chartml) (consumer-side adoption)
**Target release:** chartml 5.0.0

---

## Summary

Introduce a first-class `DataSourceProvider` trait in `chartml-core` and split chart rendering into an explicit three-stage pipeline — **fetch → transform → render** — so host applications (Kyomi, any future Rust consumer, and the `@chartml/markdown-react` npm wrapper) no longer re-implement the data orchestration logic that JS chartml has had since the DuckDB-middleware days. Caching is pluggable via a `CacheBackend` trait with a default in-memory backend plus an opt-in `IndexedDbBackend` (WASM) so dashboard refreshes don't re-hit expensive upstreams like BigQuery.

This is a breaking architectural change (chartml 4 → 5). It is driven by a latent bug found while scoping KYO-79: `chartml-datafusion::DataFusionTransform` hardcodes the table name to `"source"` and only accepts a single `DataTable`, so multi-source joins declared in a ChartML spec cannot execute against the current Rust engine regardless of how carefully the host pre-fetches.

---

## Motivation

### What triggered this

KYO-79 asked for named-source dashboard rendering in Kyomi's Leptos frontend, assuming `chartml-core` 4.1.0 already supported the multi-source shape at runtime. It does not. It *parses* the multi-source shape (`DataRef::NamedMap`) but nothing downstream honors the names. `chartml-core/src/lib.rs:755-770` rejects `NamedMap` at `resolve_chart_data` with "requires each source to be pre-fetched and registered by name," and `chartml-datafusion/src/lib.rs:47` registers the single input as `"source"` — which makes every spec whose transform SQL references the user-chosen names (`FROM visitors v JOIN sessions s …`) fail in DataFusion.

### The wider outsourcing problem

The JS engine (`@chartml/core`) orchestrates fetch + transform + render inside `render()` and offers plug points (`registerDataSource(type, handler)`, transform middleware, hook system). Host apps plug in thin fetcher callbacks and `@chartml/markdown-react` becomes a ~300-line wrapper. The Rust engine does none of this: `ChartML::render_from_yaml_*` only consumes pre-fetched `DataTable`s registered via `register_source`. Every Rust consumer re-implements:

- Parsing the YAML to discover `data:` shape
- Fetching each source (flat vs. named, with or without transform)
- Content-addressed caching and in-flight deduplication
- TTL parsing, error surfacing, empty-result shortcuts
- Rewriting specs or aliasing names for the transform stage

This is why KYO-79, as originally scoped, asked Kyomi to port the JS `duckDbMiddleware.js` to Rust. That port should never be written. The right home for this logic is `chartml-core`.

---

## Goals

1. Named and unnamed data sources both render through a single code path in `chartml-core`.
2. Multi-source joins (`SELECT … FROM visitors JOIN sessions …`) execute correctly under `DataFusionTransform`.
3. Host applications implement a ~30-line `DataSourceProvider` trait and get fetch + cache + dedup + TTL for free.
4. The pipeline has three typed stages (`FetchedChart`, `PreparedChart`, SVG) so callers can cache at any layer and resize-render without re-fetching.
5. The design respects every shape of `DataRef` and every `type:` component defined in `docs/docs/spec.md` v1.0.
6. The JS wrapper adapts to the Rust API, not the reverse ([Rust-first principle](../../.claude/projects/-home-jason-repos-chartml/memory/feedback_rust_first.md)).
7. All changes land with code-review-architect signatures per `CLAUDE.md`.

## Non-goals

- In-browser SQL engine for source storage. DataFusion runs only when a `transform:` block is present.
- Changes to chart renderers, layout, annotations, marks, scales, or any other non-data concern.
- Migration of existing Kyomi YAMLs. The new API accepts every shape today's specs already use.
- Parallel publication of chartml 4.x patches. Fixes go to 5.0.
- Cross-tab in-flight deduplication (via `BroadcastChannel` or similar). Two tabs opening the same dashboard in parallel may both fetch once — second-opener races through before IndexedDB is written. Cost: one extra upstream call in that narrow window. Fixable in 5.x if needed.
- Cross-user cache isolation beyond origin-scoping. IndexedDB is origin-scoped; shared-machine scenarios are mitigated by key namespacing (workspace slug / user id), not by separate storage silos.

---

## Architecture

### The three-stage pipeline

```
           ┌─────────────────────────────────────────────────────────────────┐
           │                   ChartML::render_to_svg_async                   │
           │                                                                   │
  YAML ──► │  FETCH ──► FetchedChart  ──►  TRANSFORM ──►  PreparedChart ──►  │ ──► SVG
           │  (async)   { spec,            (async)         { spec,             │
           │            sources:                            data:              │
           │            IndexMap<String,                    DataTable }        │
           │            DataTable> }                                           │
           │                                                                   │
           └─────────────────────────────────────────────────────────────────┘
              │                             │                              │
              │                             │                              │
         providers +                   middleware                      sync, pure
         resolver                      (DataFusion,                    rendering
         (cache, dedup,                forecast, …)                    (current
          TTL)                                                          renderers)
```

Each stage is an explicit method on `ChartML`:

```rust
impl ChartML {
    // stage 1: spec → fetched sources (async — calls providers, uses cache)
    pub async fn fetch(&self, yaml: &str, params: Option<&ParamValues>)
        -> Result<FetchedChart, ChartError>;

    // stage 2: fetched → prepared (async — transform middleware runs here)
    pub async fn transform(&self, fetched: FetchedChart, params: Option<&ParamValues>)
        -> Result<PreparedChart, ChartError>;

    // stage 3: prepared → SVG (sync — pure compute)
    pub fn render_prepared_to_svg(&self, prepared: &PreparedChart, opts: &RenderOptions)
        -> Result<String, ChartError>;

    // convenience: full pipeline in one await
    pub async fn render_to_svg_async(&self, yaml: &str, opts: &RenderOptions, params: Option<&ParamValues>)
        -> Result<String, ChartError>;

    // existing sync render stays — "all sources already registered" fast path
    pub fn render_to_svg(&self, yaml: &str, opts: &RenderOptions) -> Result<String, ChartError>;
}
```

Separating stages is the key architectural choice. It:
- keeps the renderer pure and sync (WASM-friendly, testable, cacheable),
- makes the async boundary explicit at the fetch+transform layer (not color-infecting the whole API),
- allows consumers to resize-render from `PreparedChart` without re-fetching,
- gives each layer a single typed responsibility.

### Key types

```rust
// Rust-idiomatic async trait — matches existing TransformMiddleware pattern.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait DataSourceProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError>;

    /// Optional graceful shutdown hook. Called by `ChartML::shutdown()` on tab close /
    /// SSR request end. Default is no-op. Providers that hold pooled resources
    /// (HTTP keep-alive clients, WASM workers) can override to flush/close cleanly.
    async fn shutdown(&self) {}
}

pub struct FetchRequest {
    /// The user-chosen name for this source within the spec.
    /// `None` for unnamed (flat) `data:` forms.
    pub source_name: Option<String>,
    /// The full resolved flat-form source spec — datasource, query, url, rows, etc.
    /// Already has param substitutions applied (`$foo.bar` → concrete values).
    pub spec: InlineData,
    /// Parsed cache config from `spec.cache.ttl` if present.
    pub cache: Option<CacheConfig>,
    /// Request-level HTTP headers (merged with `HttpProvider::with_default_headers`
    /// when the `http` provider handles the request). Ignored by non-HTTP providers
    /// unless they explicitly read this field.
    pub headers: HashMap<String, String>,
    /// Tenant / workspace namespace. Folded into the cache key at every tier so
    /// two tenants sharing a slug name cannot collide. Typically the workspace slug.
    /// `None` for single-tenant deployments (skipped in the hash).
    pub namespace: Option<String>,
    /// Optional cancellation token. Reserved in 5.0 (always `None` from the resolver);
    /// providers should honor it if supplied. Forward-compatible: providers that opt
    /// into listening can cancel upstream work (e.g., BigQuery `CancelJob`) without
    /// a future breaking change.
    pub cancel_token: Option<CancellationToken>,
}

#[derive(thiserror::Error, Debug)]
pub enum FetchError {
    #[error("datasource '{slug}' not found")] SlugNotFound { slug: String },
    #[error("query failed: {0}")] QueryFailed(String),
    #[error("decode failed: {0}")] DecodeFailed(String),
    #[error("cancelled")] Cancelled,
    #[error("{0}")] Other(String),
}

/// What providers return. Richer than bare `DataTable` so upstreams can thread
/// cost, row counts, server-reported timestamps, and other metadata through to
/// hooks + `FetchedChart.metadata`.
pub struct FetchResult {
    pub data: DataTable,
    /// Free-form per-provider metadata. Keys are provider-defined. Common keys:
    /// - `"bytes_billed"` (BigQuery): u64 bytes processed
    /// - `"rows_returned"`: u64
    /// - `"server_refreshed_at"`: RFC3339 string (server's notion of freshness)
    /// - `"upstream_cache_hit"`: bool (did BigQuery serve from its own cache)
    /// - `"warnings"`: Vec<String>
    pub metadata: HashMap<String, serde_json::Value>,
}

pub struct FetchedChart {
    pub spec: ChartSpec,
    /// One entry per source the spec needs. Key = user-chosen name
    /// (or canonical `"source"` for unnamed flat data).
    pub sources: IndexMap<String, DataTable>,
    pub metadata: FetchMetadata,
}

pub struct PreparedChart {
    pub spec: ChartSpec,
    pub data: DataTable,
    pub metadata: PreparedMetadata,
}

pub struct FetchMetadata {
    pub refreshed_at: SystemTime,
    pub cache_hits: Vec<String>,      // source names served from cache
    pub cache_misses: Vec<String>,    // source names that were fetched
    /// Per-source provider metadata, indexed by source name.
    pub per_source: HashMap<String, HashMap<String, serde_json::Value>>,
}
```

#### `CancellationToken`

Concrete implementation: `Arc<AtomicBool>` (check `token.is_cancelled()`) plus a waker list for async wake-up. No external crate dependency. `Clone` is derived trivially (cheap `Arc::clone`). Compatible with both native and WASM (`?Send`) because `AtomicBool` is trivially `Send` + `Sync` but we don't require the token holder to be.

Implementation lives in `chartml-core/src/resolver/cancel.rs`; public API:

```rust
#[derive(Clone)]
pub struct CancellationToken(Arc<CancelInner>);

struct CancelInner {
    cancelled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl CancellationToken {
    pub fn new() -> Self { ... }
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    /// Yields pending until cancelled.
    pub async fn cancelled(&self);
}
```

### Transform trait upgrade (phase 1)

The existing signature takes a single table; the new one takes the map:

```rust
// BEFORE (chartml 4)
async fn transform(
    &self,
    data: DataTable,
    spec: &TransformSpec,
    context: &TransformContext,
) -> Result<TransformResult, ChartError>;

// AFTER (chartml 5)
async fn transform(
    &self,
    sources: &IndexMap<String, DataTable>,
    spec: &TransformSpec,
    context: &TransformContext,
) -> Result<TransformResult, ChartError>;
```

`DataFusionTransform` registers every map entry under its key and joins work natively:

```rust
for (name, table) in sources {
    ctx.register_table(name, Arc::new(MemTable::try_new(...)?))?;
}
// If only one entry AND it isn't already called "source", also alias it as "source"
// so legacy transform SQL continues to parse. Multi-source maps do NOT get this alias.
```

### Resolver: cache, dedup, TTL

The resolver sits between `ChartML::fetch` and the registered providers. It owns:

- **Tiered cache** — pluggable via the `CacheBackend` trait (see "Pluggable caching" below). Tier 1 is always an in-memory `MemoryBackend`; tier 2 is optional and configured per-`ChartML` instance (typically `IndexedDbBackend` in the browser).
- **In-flight tracker** — `HashMap<u64, Shared<BoxFuture<Result<FetchResult, FetchError>>>>`. If charts A and B ask for the same query in the same tick, both await the same future. Dedup is at the **operation** level (fetch + decode), matching the `duckDbMiddleware` design.
- **TTL parser** — accepts `"30s"`, `"5m"`, `"6h"`, `"1d"`, `"7d"` via `humantime` crate.
- **Invalidate API** (see "Bulk invalidation" below) — scalar + bulk variants, both clear memory and persistent tiers.

### Parallel fetch for `NamedMap`

When a spec has multiple sources (`NamedMap` with N > 1 entries, or after the unnamed-with-transform normalization that produces a single-entry map), the resolver fetches them **in parallel** via `futures::future::try_join_all`. This is a firm design commitment, not an implementation detail:

- N sources with average fetch time T → total fetch time ≈ T (not N·T).
- Failure behavior is fail-fast: first `FetchError` aborts the remaining in-flight fetches (via `try_join_all`'s drop-on-first-error semantics). Unfinished futures are dropped → provider cancellation via Drop (or `cancel_token` if the provider opts in).
- Hooks still emit per-source `ErrorEvent` for each source that failed before the abort, so telemetry surfaces "visitors failed, sessions was still in-flight when aborted" rather than one opaque error.

Rationale: multi-source dashboards would otherwise block 5–10× longer than necessary. The parallelism is invisible to providers — each sees a normal `fetch(request)` call on its own task.

### Bulk invalidation API

For "refresh all" button flows and multi-tenant cache hygiene, the resolver exposes bulk invalidation as well as scalar:

```rust
impl Resolver {
    /// Clear a specific entry from both tiers.
    pub async fn invalidate(&self, key: u64);
    /// Clear every entry from both tiers.
    pub async fn invalidate_all(&self);
    /// Clear every entry whose `FetchRequest.spec.datasource` matched the slug.
    /// (Requires the cache to track slug metadata per entry — see CacheBackend below.)
    pub async fn invalidate_by_slug(&self, slug: &str);
    /// Clear every entry under a given namespace (multi-tenant isolation).
    pub async fn invalidate_by_namespace(&self, namespace: &str);
}
```

`CacheBackend` gains `scan(predicate)` + `delete_where(predicate)` helpers for the bulk variants to iterate storage efficiently. `IndexedDbBackend` implements these with IndexedDB's cursor API.

### Error isolation within `NamedMap`

If fetching 5 named sources produces 2 errors:

- **Fail-fast at the chart level** — overall `fetch()` returns `FetchError` for the first source that failed (documented behavior; downstream transform needs every source).
- **Per-source hooks fire** — every source that errored before the abort emits `ResolverHooks::on_error(ErrorEvent { source_name: Some("visitors"), ... })`. Consumers see which sources fail most often even when only the first bubbles up as the user-facing error.
- **Cross-chart isolation stays intact** — a dashboard with 4 independent charts and one failing chart continues to render the other three. That isolation is at the `ChartMLChart` level (each chart is its own fetch pipeline), not the resolver level.

### Pluggable caching

```rust
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait CacheBackend {
    async fn get(&self, key: u64) -> Option<CachedEntry>;
    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError>;
    async fn invalidate(&self, key: u64) -> Result<(), CacheError>;
    /// Delete every entry whose `CachedEntry.tags` contain the given tag.
    /// Used for `invalidate_by_slug` / `invalidate_by_namespace`.
    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError>;
    /// Drop everything.
    async fn clear(&self) -> Result<(), CacheError>;
    /// Optional graceful shutdown (flush pending writes, close transactions).
    /// Default is no-op.
    async fn shutdown(&self) {}
}

pub struct CachedEntry {
    pub data: DataTable,
    pub fetched_at: SystemTime,
    pub ttl: Duration,
    /// Free-form tags for bulk invalidation. Typical values:
    /// `["slug:kyomi-analytics", "namespace:workspace-foo"]`.
    pub tags: Vec<String>,
    /// Provider metadata preserved with the cached entry (from `FetchResult.metadata`).
    /// Survives round-trips through the persistent tier.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CachedEntry {
    pub fn is_expired(&self) -> bool {
        SystemTime::now().duration_since(self.fetched_at).map(|age| age > self.ttl).unwrap_or(true)
    }
}
```

Two backends ship with 5.0:

**`MemoryBackend`** — `Arc<Mutex<HashMap<u64, CachedEntry>>>`. Always available, default on every `ChartML` instance. Cleared when the instance drops.

**`IndexedDbBackend`** — behind the `wasm-indexeddb` cargo feature in `chartml-core`. Uses the `idb` crate. Persists via `DataTable::to_ipc_bytes()` on `put` and `DataTable::from_ipc_bytes()` on `get` — both methods already exist in `chartml-core`. Key namespacing **is required at construction** (`IndexedDbBackend::new(database_name, namespace)`): no default, forces a conscious choice per consumer to avoid cross-user leakage on shared browsers.

#### Stored blob format

Every persisted entry carries a version byte so we can evolve `CachedEntry` serialization without breaking users who upgrade across a chartml release:

```
[u8 version = 0x01] [u32 ipc_bytes_len] [ipc_bytes…] [varint tags_len] [...tags] [json metadata_blob]
```

On `get`: read version byte, if ≠ current, treat as cache miss and schedule eviction of the stale entry. Consumers see "empty cache" after upgrade and pay one provider call per source; never a cryptic decode error.

The resolver walks tiers in order: memory miss → persistent miss → provider. On a persistent-tier hit, memory is hydrated synchronously; subsequent reads in the same session never touch IndexedDB again. On provider success, both tiers are written.

```rust
impl Resolver {
    async fn fetch(&self, key: u64, req: FetchRequest) -> Result<FetchResult, FetchError> {
        // Tier 1
        if let Some(entry) = self.memory.get(key).await {
            if !entry.is_expired() {
                self.hooks.on_cache_hit(CacheHitEvent { key, tier: Tier::Memory, age: entry.age(), ... }).await;
                return Ok(FetchResult { data: entry.data, metadata: entry.metadata });
            }
        }
        // Tier 2 (optional)
        if let Some(persistent) = &self.persistent {
            if let Some(entry) = persistent.get(key).await {
                if !entry.is_expired() {
                    self.memory.put(key, entry.clone()).await.ok();
                    self.hooks.on_cache_hit(CacheHitEvent { key, tier: Tier::Persistent, ... }).await;
                    return Ok(FetchResult { data: entry.data, metadata: entry.metadata });
                }
                persistent.invalidate(key).await.ok(); // evict expired
            }
        }
        self.hooks.on_cache_miss(CacheMissEvent { key, reason: MissReason::NotFound, ... }).await;

        // In-flight dedup guards concurrent first-fetch
        self.inflight.get_or_fetch(key, || async {
            let result = self.providers.dispatch(req.clone()).await?;
            let tags = build_tags(&req);  // ["slug:...", "namespace:..."]
            let entry = CachedEntry {
                data: result.data.clone(),
                metadata: result.metadata.clone(),
                fetched_at: SystemTime::now(),
                ttl: req.cache.and_then(|c| c.ttl_duration()).unwrap_or(DEFAULT_TTL),
                tags,
            };
            self.memory.put(key, entry.clone()).await.ok();
            if let Some(p) = &self.persistent { p.put(key, entry).await.ok(); }
            Ok(result)
        }).await
    }
}
```

Server-side consumers (Kyomi's SSR / PDF export / MCP chart app) only wire `MemoryBackend` — they're short-lived processes where disk persistence would add complexity without clear benefit. Browser consumers wire both tiers and get page-refresh cache hits for free.

### Observability: `ResolverHooks`

The resolver emits typed events at each decision point so consumers can wire progress bars, cache-hit telemetry, or custom error recovery without reading private state. This mirrors the JS `PLUGIN_HOOKS.md` semantics but with a Rust-idiomatic trait-based API.

```rust
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait ResolverHooks {
    async fn on_progress(&self, _event: ProgressEvent) {}
    async fn on_cache_hit(&self, _event: CacheHitEvent) {}
    async fn on_cache_miss(&self, _event: CacheMissEvent) {}
    async fn on_error(&self, _event: ErrorEvent) {}
}

pub struct ProgressEvent {
    pub phase: Phase,          // Fetch | Transform | Render
    pub source_name: Option<String>,
    pub loaded: Option<u64>,
    pub total: Option<u64>,
    pub message: String,
}

pub struct CacheHitEvent {
    pub key: u64,
    pub source_name: Option<String>,
    pub tier: CacheTier,        // Memory | Persistent
    pub age: Duration,
}

pub struct CacheMissEvent {
    pub key: u64,
    pub source_name: Option<String>,
    pub reason: MissReason,     // NotFound | Expired | Invalidated
}

pub struct ErrorEvent {
    pub phase: Phase,
    pub source_name: Option<String>,
    pub error: String,
}
```

All methods have default no-op implementations; consumers implement only the ones they care about. Hooks are wired once per `ChartML` instance via `set_hooks(impl ResolverHooks + 'static)` and fire on every subsequent `fetch` / `transform` / `render` call. Hook errors never propagate — they're logged (via `tracing`) and swallowed, matching the JS "don't block on hooks" rule.

Rust's async story is such that hooks emit fire-and-forget (`tokio::spawn` or `wasm_bindgen_futures::spawn_local`) so a slow telemetry sink can't stall the resolver.

### Dispatch rules (DataRef shape → provider)

| YAML shape | Detected as | Dispatch |
|---|---|---|
| `data: "name"` | `DataRef::Named` | Look up in `self.sources` (registered via `register_source`). No provider call. |
| `data: { rows: [...] }` | `DataRef::Inline` (inline provider) | Built-in `inline` provider materializes the rows. |
| `data: { url: "..." }` | `DataRef::Inline` (http provider) | Built-in `http` provider does a GET. |
| `data: { datasource, query }` | `DataRef::Inline` (datasource) | Dispatched to the registered `datasource` provider. No `provider:` field required. |
| `data: { provider: "x", ... }` | `DataRef::Inline` (custom) | Dispatched by explicit name. |
| `data: { name1: {...}, name2: {...} }` | `DataRef::NamedMap` | Each entry's value is dispatched independently through the four rules above. |

**Routing algorithm inside `Resolver::dispatch(request)`:**

```
if spec.provider.is_some():
    → dispatch to provider_registered_as(spec.provider.unwrap())
elif spec.rows.is_some():
    → dispatch to provider_registered_as("inline")
elif spec.url.is_some():
    → dispatch to provider_registered_as("http")
elif spec.datasource.is_some():
    → dispatch to provider_registered_as("datasource")
else:
    → FetchError::Other("no dispatch match for spec")
```

Precedence is explicit: `provider` key wins over inferred shape. This means a spec with `{ provider: "custom", datasource: "slug" }` dispatches to `"custom"` (consumer's decision), not `"datasource"`. Built-in providers are registered under `"inline"` and `"http"` on `ChartML::new()`; hosts that want to override either can re-register.

### Normalization at the gate

The resolver applies this rewrite **before** any branching, matching `duckDbMiddleware:137-139`:

```
if spec.data is Inline (flat) AND spec.transform is present:
    rewrite spec.data to NamedMap { "source": <original_inline> }
```

Downstream, only two shapes exist: `Named` (registered lookup) or `NamedMap` (provider-driven). The flat path without transform is the only case that stays flat — and it's trivial (one provider call, no transform).

### Passthrough / error rules (post-fetch, pre-transform)

| Sources map size | Transform present? | Behavior |
|---|---|---|
| 0 | n/a | Impossible — fetch fails earlier. |
| 1 | no | Prepared.data = the one source. No DataFusion invocation. |
| 1 | yes | Transform runs with map `{ name: data }`, plus `"source"` alias if name ≠ "source". |
| N > 1 | yes | Transform runs with the full map. |
| N > 1 | no | Error: `"Named data sources require a transform block when multiple sources are defined"`. Matches React's error text. |

### Parameter substitution

`$foo.bar` / `$inline_param` references resolve **before** any provider is called, inside `ChartML::fetch`. The `FetchRequest.spec` that a provider sees carries concrete values only. This preserves current param behavior (see `render_from_yaml_with_params_async` step 1) and keeps providers pure.

### Auto-refresh (`cache: { ttl, autoRefresh: true }`)

The ChartML spec permits `autoRefresh: true` on a source's cache config, meaning "periodically re-fetch even without a user action." The **component layer owns the timer**, not the resolver. Rationale:

- The component knows when the UI is visible (Page Visibility API, Leptos signals) and can pause refresh when the tab is hidden.
- The resolver would have to handle spawn_local + cleanup + visibility on its own, duplicating Leptos/React idioms.
- Every consumer (leptos, markdown-react, future frameworks) wires the timer in its own idiomatic way. The chartml core exposes the flag + the invalidate API; components decide when to fire.

Concretely, `chartml-leptos::ChartMLChart` (phase 4) reads `chart_spec.data` for any `cache.autoRefresh` flag, starts a Leptos interval on mount, calls `resolver.invalidate(&key); re-render()` on tick, and tears down on unmount. `@chartml/markdown-react` (phase 5) does the equivalent with `setInterval` + visibility listener.

If this ends up re-implemented across multiple consumers with the same bugs, we promote it to a resolver-owned background task in 5.x.

---

## Full chartml-spec coverage matrix

Against `docs/docs/spec.md` v1.0:

| Spec feature | Handled where | Notes |
|---|---|---|
| `type: source` (inline rows, http url, cache.ttl) | `register_component` path — unchanged | Builds `DataTable`, inserts into `self.sources`. Referenced by `DataRef::Named`. |
| `type: params` (dashboard + chart-level, `$foo.bar` / `$bar` refs) | `ChartML::fetch` step 1 — unchanged | Param resolution runs before provider calls. |
| `type: style` / `type: config` | No change | Not in pipeline. |
| `data: "name"` (string ref) | `Named` lookup | Serve from `self.sources`, no provider. |
| `data: { provider: inline, rows }` | `Inline`, built-in provider | Built-in. |
| `data: { provider: http, url, cache }` | `Inline`, built-in provider | Built-in. Resolver handles TTL. |
| `data: { datasource, query, cache? }` (flat) | `Inline`, registered `datasource` provider | The primary Kyomi shape. Normalized to `NamedMap { source: <this> }` if transform present. |
| `data: { name1: {...}, name2: {...} }` (multi-named) | `NamedMap`, per-entry dispatch | Each entry goes through the same four rules. |
| `transform.sql` | `DataFusionTransform` SQL stage — updated | Now sees all named tables. Multi-source joins finally work. |
| `transform.aggregate` (dimensions, measures, filters, sort, limit) | `DataFusionTransform` aggregate stage — unchanged internally | Same map input. |
| `transform.forecast` | `DataFusionTransform` forecast stage — unchanged internally | Same map input. |
| `visualize.*`, annotations, axes, marks, dual-axis, metric, layout.colSpan, style | Render stage — unchanged | Pure downstream. |

---

## Divergences from the JS `duckDbMiddleware`

The old JS middleware (`~/repos/kyomi@ee16f48^:apps/frontend/src/lib/chartml/plugins/duckDbMiddleware.js`) is 448 lines we spent hours getting right. The Rust design keeps the high-value patterns and departs from the rest with reasons.

### Patterns we KEEP

1. **Normalize unnamed+transform → `{source: <original>}` at the gate.** One downstream code path.
2. **Two-layer content-addressed cache.** `extract_{hash}` for per-source data, `transform_{hash}` for per-pipeline output.
3. **In-flight dedup at the operation level.** Fetch+decode is the atomic unit; simultaneous requests wait on the same `Shared<Future>`.
4. **Source name passed to provider** (`fetchData(sourceName)` → `FetchRequest.source_name`).
5. **Empty-result short-circuit.** If a fetched source has zero rows and no columns, skip running any transform against it.
6. **Passthrough / error rules** (single-source-no-transform passthrough; multi-source-no-transform error with React-matching text).
7. **TTL format** — `<number><unit>` with `s`/`m`/`h`/`d` units.

### Patterns we DIVERGE on — with rationale

| # | Area | JS middleware | Rust design | Why |
|---|---|---|---|---|
| 1 | **Source storage** | Every source is loaded into DuckDB as `CREATE TABLE __extract_{hash}` — DuckDB is simultaneously the cache and the SQL engine. | Sources held as `DataTable` (Arrow `RecordBatch`) in the resolver's `HashMap`. DataFusion is instantiated fresh per transform call. | Rust already has Arrow in-memory; double-storing into DuckDB adds overhead and bundle size. DataFusion per-transform is lightweight because only `SessionContext` setup is involved. Also avoids a WASM-side SQL engine dependency. |
| 2 | **Format branching** | Provider returns `{data, metadata: {format: "arrow"\|"json"}}`; middleware branches on format and calls different DuckDB loaders. | `DataSourceProvider::fetch → DataTable`. Any JSON→Arrow conversion happens inside the provider (helpers available via `DataTable::from_rows`). | One type all the way through. No downstream branching on format. Providers are free to fetch whatever wire format is convenient. |
| 3 | **`bypassCache` boolean** | Threaded through every function signature as `context.bypassCache`. | `resolver.invalidate(&key)` explicitly clears a key. Refresh = invalidate + re-render. | Cleaner separation — cache is its own object with its own API. No boolean plumbing through 10 function signatures. Same observable effect. |
| 4 | **Transform input** | DuckDB pre-loads every source as a table; middleware passes `null` data and builds SQL that references DuckDB state. | `TransformMiddleware::transform(sources: &IndexMap<String, DataTable>, …)` takes the full map explicitly. | Rust has no persistent DuckDB state to lean on. Explicit data-in is the only option, and it's arguably cleaner — the middleware has no hidden dependency on external SQL state. |
| 5 | **Intermediate table cleanup** | Pipeline creates `__transform_stage_*` tables, caller drops them in a `finally` block. | Transform stages use a session-scoped `SessionContext` that's destroyed when the transform call returns. | Rust ownership > manual cleanup. Zero chance of leaking intermediate tables. |
| 6 | **Hash function** | Async SHA-based hash (`hashAsync`) for cache keys. | Fast non-crypto hash (`xxhash-rust` or equivalent) with stable output. | Cache keys are internal and never persisted across process boundaries; crypto hash is wasted work. `DefaultHasher` is specifically unsuitable — not stable across Rust versions. |
| 7 | **Result type** | `{data: Array<Object>, metadata: {refreshedAt, cacheHit, tableId}}` — JSON-shaped. | Typed structs (`FetchedChart { spec, sources, metadata: FetchMetadata }`, `FetchMetadata { refreshed_at: SystemTime, cache_hits, cache_misses }`). | Rust. |
| 8 | **Provider dispatch** | Single `fetchData(sourceName)` callback. Host encodes all its routing logic in that one callback. | `DataSourceProvider` trait; `FetchRequest` carries the whole `InlineData` spec so providers can inspect `datasource`, `query`, `url`, `provider`, `rows`, etc. | Composability. A single host can register multiple providers (e.g., one for `datasource:` and a separate one for a custom `provider: "snowflake"` plugin). Testing a provider doesn't require mocking the whole host. |
| 9 | **Cross-session persistence** | DuckDB WASM retains data across re-renders via OPFS-backed tables. | Pluggable `CacheBackend` trait; browser consumers wire `IndexedDbBackend`. Arrow IPC bytes in/out, content-hash keys unchanged. | Same end-user outcome (page refresh = cache hit within TTL) without the JS-specific storage engine dependency. Trait-pluggable so server-side consumers can skip it or use a different backend. |
| 10 | **Stage-logic package** | Pipeline stages lived in `@kyomi/chartml-transform` (separate package from DuckDB I/O adapter). | Stages are in `chartml-datafusion`. Providers are in `chartml-core`. I/O adapter is the provider impl in the host. | Rust crate boundary does the same thing: transforms are one crate, providers are another, neither knows about the other. |


---

## API surface changes

### Breaking

- `TransformMiddleware::transform` signature: `data: DataTable` → `sources: &IndexMap<String, DataTable>`. All implementors must update.
- `ChartML::render_from_yaml_with_data_async` behavior: the `NamedMap` case currently errors ("requires each source to be pre-fetched and registered by name, then `data:` rewritten to that name"). **After phase 1**, `NamedMap` works natively when sources are pre-registered via `register_source`. **After phase 3**, it also works with registered providers (no pre-registration needed). The old error message goes away in phase 1.
- `ChartML::render_to_svg` (sync) behavior for `NamedMap`: today errors in `resolve_chart_data`. **After phase 1**, works when all NamedMap entries are pre-registered via `register_source`. Sync path is a first-class supported entry point for the pre-registered case, not a second-class citizen that only works after phase 3.
- Legacy transform SQL that hardcodes `FROM source`: continues to work in the single-source case (alias), breaks in the named-multi case (explicit names required). Documented in migration notes.

### Additive

- `DataSourceProvider` trait + `FetchRequest` / `FetchResult` / `FetchError` / `CacheConfig` / `CancellationToken`.
- `DataSourceProvider::shutdown()` optional async method (default no-op).
- `ChartML::register_provider(kind, impl)`.
- `ChartML::fetch` / `transform` / `render_prepared_to_svg` / `render_to_svg_async`.
- `ChartML::shutdown()` — async, calls `shutdown()` on every registered provider + cache backend.
- `FetchedChart` / `PreparedChart` / `FetchMetadata` / `PreparedMetadata` types (per-source metadata passthrough included).
- `CacheBackend` trait + `CachedEntry` (with `tags` + `metadata`) + `CacheError` + built-in `MemoryBackend`.
- `IndexedDbBackend` behind the `wasm-indexeddb` cargo feature (version-prefixed blob format, required namespace).
- `ChartML::with_cache(backend)` / `ChartML::set_cache(backend)` for wiring persistence.
- `ResolverHooks` trait + `ChartML::set_hooks(impl ResolverHooks)` for progress / cache-hit / cache-miss / error events.
- Event types: `ProgressEvent`, `CacheHitEvent`, `CacheMissEvent`, `ErrorEvent`.
- Built-in `InlineProvider` and `HttpProvider`. `HttpProvider::with_default_headers(headers)` + per-request `FetchRequest.headers` for auth, content-type, and other custom headers.
- `resolver.invalidate(&key)` / `invalidate_all` / `invalidate_by_slug(&slug)` / `invalidate_by_namespace(&ns)`.

### Unchanged

- `render_from_yaml` / `render_from_yaml_with_size` / `render_from_yaml_with_params` (sync) — legacy callers that have all sources pre-registered keep working.
- `register_renderer`, `register_source`, `register_transform`, `register_component`, `set_theme`, `set_default_palette`.
- Every chart renderer crate (`chartml-chart-*`) — no changes.
- The chartml YAML spec.

### Version

chartml 4.x → 5.0.0. All nine workspace crates bump together. npm packages (`@chartml/core`, `@chartml/react`, `@chartml/markdown-react`, `@chartml/datafusion`) bump to `5.0.0` when phase 5 lands.

---

## Phase packages

Each phase below is a self-contained work package designed for `/agent-driven-development`. A fresh `feature-implementation-engineer` agent should be able to read a single phase's section and execute without needing conversation history.

Every phase ends with a `code-review-architect` signature on the commit per `CLAUDE.md`. Golden SVG baselines only need chart-evaluator signatures for phases that touch rendered output (phases 4 and later can re-evaluate only specific golden outputs that drift; phases 1–3 shouldn't touch SVGs).

---

### Phase 1 — Transform trait accepts `IndexMap<String, DataTable>`

**Branch:** `jason/phase-1-transform-named-sources`
**Depends on:** nothing (can start immediately)
**Parallel with:** none — blocks everything downstream
**Estimated size:** M (1 PR, ~6 files touched)

#### Goal

`TransformMiddleware::transform` receives a map of named `DataTable`s instead of a single table. `DataFusionTransform` registers every entry under its name and aliases single-entry maps as `"source"` for back-compat. Multi-source SQL joins execute correctly **on both sync and async render paths** when sources are pre-registered via `register_source`.

#### Context to load

- `crates/chartml-core/src/plugin/transform.rs` — current trait signature (to change).
- `crates/chartml-core/src/lib.rs` — `render_from_yaml_with_params_async` (~lines 615–750) and `render_from_yaml_with_data_async` (~lines 693–760). Two call sites pass data to transform.
- `crates/chartml-core/src/lib.rs` — `resolve_chart_data` (~line 755) currently rejects `NamedMap`. Will change to materialize the map from pre-registered sources.
- `crates/chartml-datafusion/src/lib.rs` — current `DataFusionTransform::transform` impl (lines 30-103). Hardcodes `ctx.register_table("source", ...)`.
- `crates/chartml-datafusion/src/stages/*.rs` — SQL/aggregate/forecast stages; check if any hardcode the table name `"source"` (they reference `current_table: "source"` at line 52 of lib.rs — verify stages).

#### Deliverables

New/changed types and methods:

1. `plugin/transform.rs` — new trait signature:
   ```rust
   async fn transform(
       &self,
       sources: &IndexMap<String, DataTable>,
       spec: &TransformSpec,
       context: &TransformContext,
   ) -> Result<TransformResult, ChartError>;
   ```

2. `chartml-datafusion::DataFusionTransform::transform` — registers every entry by name. If `sources.len() == 1` and the sole key is not already `"source"`, additionally register under `"source"` as alias. Multi-entry maps are NOT aliased.

3. `chartml-core::lib.rs` async render path — build `IndexMap` from `self.sources` (preserve insertion order from the YAML `data:` map); for `NamedMap`, look up each entry's `InlineData` in `self.sources` by name; for `Named(n)`, pass `{n: data}`; for `Inline(flat)`, pass `{"source": data}`.

4. `chartml-core::lib.rs` `resolve_chart_data` — for `NamedMap`, returns the whole map (all sources must be pre-registered; error with a clear message if any are missing). For `Named` and `Inline`, wraps the single table into a 1-entry map. **Both sync (`render_to_svg` path) and async (`render_from_yaml_with_params_async`) call sites must use this updated `resolve_chart_data`** — the old "NamedMap is not supported" error goes away in phase 1, not phase 3. Consumers who pre-register named sources via `register_source` and call sync render with a `NamedMap` YAML get correct behavior after phase 1 lands.

#### Implementation steps

1. Add `indexmap` to `chartml-core/Cargo.toml` if not already present (spec/chart.rs uses it — likely already there).
2. Edit `plugin/transform.rs` to change `transform` signature. Update `TransformContext` / `TransformResult` if needed (shouldn't need to).
3. Edit `chartml-datafusion/src/lib.rs` `DataFusionTransform::transform` to iterate `sources`, register each as a MemTable under its name, apply `"source"` alias only if `sources.len() == 1 && !sources.contains_key("source")`. Track `current_table` starting from the sole name (for single-source case) or `"source"` alias.
4. Audit stages in `chartml-datafusion/src/stages/*.rs` — they currently take `current_table: &str` and receive `"source"`. No changes needed there; the new signature means `current_table` starts as whatever the sole source is named (or `"source"` via the alias for single-entry back-compat).
5. Edit both `render_from_yaml_with_params_async` and `render_from_yaml_with_data_async` call sites in `lib.rs`. Where they currently pass `chart_data: DataTable`, they now pass `&sources_map: &IndexMap<String, DataTable>`.
6. Edit the sync `extract_data` path (`lib.rs:391`) to handle `NamedMap` with pre-registered sources lookup (for the sync render path that's still used by callers who pre-register).

#### Tests to add

- `crates/chartml-datafusion/src/lib.rs::tests::test_multi_source_join` — build two `DataTable`s (visitors, sessions), construct `TransformSpec { sql: "SELECT v.date, v.n AS visitors, s.n AS sessions FROM visitors v JOIN sessions s USING (date)" }`, assert joined result. **Acceptance test for the core bug.**
- `crates/chartml-datafusion/src/lib.rs::tests::test_single_source_alias` — single-entry map with custom name (`"revenue"`), transform SQL references `FROM source` — passes because of the alias.
- `crates/chartml-datafusion/src/lib.rs::tests::test_single_source_own_name` — single-entry map with custom name (`"revenue"`), transform SQL references `FROM revenue` — passes.
- `crates/chartml-datafusion/src/lib.rs::tests::test_multi_source_no_alias` — two-entry map, transform SQL references `FROM source` — fails with a clear DataFusion "table not found" error (no alias applied for multi-entry maps).
- `crates/chartml-core/tests/named_map_sources_test.rs` — integration test: YAML with `NamedMap` + pre-registered sources via `register_source` + transform with SQL join → renders correctly via `render_to_svg_async`.

#### Exit criteria

- `cargo test --workspace` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes (repo policy: no lint suppressions).
- Existing single-source callers still work (migration of internal call sites covered).
- **Sync `render_to_svg` + pre-registered `NamedMap` sources + transform SQL joining them → renders correctly** (not just async path).
- **Multi-source YAML rendered through the public render API with only `register_source` (no provider trait) — asserts the sync path works end-to-end after phase 1, independent of phases 2 & 3.**
- `code-review-architect` approves the PR.
- No golden SVG changes (phase touches data path, not rendering).

#### Non-obvious concerns

- `IndexMap` preserves insertion order — which matters because `DataFusionTransform` stages use "first table" as their implicit input. Use `sources.iter().next()` to get the first (for the single-source alias logic), not `sources["source"]`.
- `register_table` in DataFusion requires `'static` table names (or `Arc<str>`). Convert `&String` to `String::from` or use owned strings throughout the registration loop.
- If a source name conflicts with a DataFusion reserved keyword (e.g., `"table"`, `"index"`), wrap in double quotes when building SQL dynamically. Should be a non-issue because transform SQL is user-authored — if they name a source `table`, their SQL has to deal with it, not our registration code.
- `TransformContext` today carries `params`. Verify no downstream usage assumes a single-table world.
- **`current_table` initial value**: for single-source maps, initialize to either the sole source name OR `"source"` (alias is registered so both resolve). For multi-source maps, there is no sensible initial `current_table` — the SQL stage must produce its own output name and register it. Audit `chartml-datafusion/src/stages/sql_stage.rs` to confirm it only reads `current_table` as "prior stage output," not as "base input table" (the base input is the registered source(s), not `current_table`).
- **Single-source alias**: the `"source"` alias is registered whenever `sources.len() == 1 && !sources.contains_key("source")`, **regardless of whether the user's transform SQL references it**. This is harmless — DataFusion doesn't care about unused registered tables — and keeps the logic simple.

---

### Phase 2 — Three-stage pipeline types + explicit `fetch`/`transform`/`render` methods

**Branch:** `jason/phase-2-pipeline-types`
**Depends on:** Phase 1 landed on main
**Parallel with:** none
**Estimated size:** M (~8 files, no new external deps)

#### Goal

Introduce `FetchedChart` / `PreparedChart` / `FetchMetadata` / `PreparedMetadata` types and expose explicit `ChartML::fetch` / `transform` / `render_prepared_to_svg` / `render_to_svg_async` methods. No provider trait yet — `fetch` reads from pre-registered `self.sources` only. This phase formalizes the shape without introducing new I/O behavior.

#### Context to load

- `crates/chartml-core/src/lib.rs` — `render_from_yaml_with_params_async` (the method being split).
- `docs/plans/2026-04-18-datasource-provider-pipeline.md` — "Key types" section.

#### Deliverables

New types in `chartml-core/src/pipeline/mod.rs` (new module). **All must derive `Clone`** — `DataTable` is `Arc`-backed so cloning is cheap, and `Clone` is required for the "resize-render from `PreparedChart`" use case.

- `#[derive(Clone)] FetchedChart { spec: ChartSpec, sources: IndexMap<String, DataTable>, metadata: FetchMetadata }`
- `#[derive(Clone)] PreparedChart { spec: ChartSpec, data: DataTable, metadata: PreparedMetadata }`
- `#[derive(Clone)] FetchMetadata { refreshed_at: SystemTime, cache_hits: Vec<String>, cache_misses: Vec<String>, per_source: HashMap<String, HashMap<String, serde_json::Value>> }` (per_source stays empty in phase 2 — populated in phase 3)
- `#[derive(Clone)] PreparedMetadata { refreshed_at: SystemTime, transform_applied: bool, sources_used: Vec<String> }`
- `#[derive(Clone, Default)] RenderOptions { width: Option<f64>, height: Option<f64>, params: Option<ParamValues> }` — consolidates the current scatter of function args.

New `ChartML` methods:

- `async fn fetch(&self, yaml: &str, opts: &RenderOptions) -> Result<FetchedChart, ChartError>` — parses YAML, resolves params, builds `IndexMap` from pre-registered sources (phase 3 will add provider dispatch here).
- `async fn transform(&self, fetched: FetchedChart, opts: &RenderOptions) -> Result<PreparedChart, ChartError>` — applies transform middleware (phase 1's new signature) to produce single-table result. Handles passthrough (1 source, no transform) + multi-source-no-transform error.
- `fn render_prepared_to_svg(&self, prepared: &PreparedChart, opts: &RenderOptions) -> Result<String, ChartError>` — sync; runs rendering against already-prepared data.
- `async fn render_to_svg_async(&self, yaml: &str, opts: &RenderOptions) -> Result<String, ChartError>` — convenience: `fetch + transform + render_prepared_to_svg`.

#### Implementation steps

1. Create `crates/chartml-core/src/pipeline/mod.rs` with the four types. Re-export from `lib.rs`.
2. Create `crates/chartml-core/src/pipeline/render_options.rs` with `RenderOptions`.
3. Split `render_from_yaml_with_params_async` into the three new methods. The existing async method becomes a thin alias over `render_to_svg_async` (for now — deprecate in phase 7).
4. Ensure `render_from_yaml` (sync) is untouched — the "fast path" for callers with everything pre-registered keeps its exact signature.
5. Add validation logic to `transform()`:
   - `sources.len() == 0` → internal invariant violation (shouldn't happen; `fetch` always produces ≥1 entry).
   - `sources.len() == 1 && transform_spec.is_none()` → passthrough (`data = only source's DataTable`).
   - `sources.len() > 1 && transform_spec.is_none()` → error: "Named data sources require a transform block when multiple sources are defined" (exact text from React).
   - Otherwise → run transform middleware.

#### Tests to add

- `crates/chartml-core/tests/pipeline_test.rs`:
  - `test_fetch_stage_single_source` — parses YAML with `data: { provider: inline, rows: [...] }`, verifies `FetchedChart.sources` has one entry keyed `"source"`.
  - `test_fetch_stage_named_map` — parses multi-source YAML, verifies `sources` has N entries with correct names (after phase 3 this test will go through providers; for phase 2 all sources are pre-registered via `register_source`).
  - `test_transform_passthrough` — single source, no transform → `PreparedChart.data` equals source table exactly.
  - `test_transform_multi_no_transform_error` — multi-source, no transform → error with exact React-matching text.
  - `test_render_prepared_to_svg` — given a constructed `PreparedChart`, renders SVG.
  - `test_three_stage_cached_resize` — fetch once, transform once, call `render_prepared_to_svg` with three different widths → three SVGs, zero additional data work.

#### Exit criteria

- `cargo test --workspace` green.
- Existing tests that use `render_from_yaml_with_params_async` still pass.
- `code-review-architect` signature.
- New `pipeline` module exported from `chartml-core::lib.rs`.

#### Non-obvious concerns

- `RenderOptions` replaces a scattered parameter list. Ensure no callers miss a field — default values should preserve current behavior bit-for-bit.
- `FetchedChart` and `PreparedChart` own their data (`IndexMap<String, DataTable>` and `DataTable`), not references. Moving them between methods is cheap because `DataTable` is `Arc`-backed internally.
- `ChartSpec` is held by value inside `FetchedChart`/`PreparedChart`. If this becomes a cost issue (spec has `Vec`-heavy fields), revisit with `Arc<ChartSpec>`. Fine for phase 2.

---

### Phase 3 — `DataSourceProvider` trait + resolver + `CacheBackend` + `MemoryBackend` + built-in providers

**Branch:** `jason/phase-3-provider-trait-resolver`
**Depends on:** Phase 2 landed on main
**Parallel with:** none (phase 3b + 3c build on this)
**Estimated size:** L (1 large PR; ~15 files)

#### Goal

Host apps can implement `DataSourceProvider` and the resolver handles cache (tiered + TTL), in-flight dedup, bulk invalidation, parallel fetch for `NamedMap`, normalization at the gate, and dispatch for all six `DataRef` shapes. Built-in `InlineProvider` + `HttpProvider` ship. `MemoryBackend` is the default `CacheBackend`.

#### Context to load

- `crates/chartml-core/src/pipeline/mod.rs` (from phase 2).
- `crates/chartml-core/src/spec/chart.rs` — `DataRef` + `InlineData`.
- This design doc, sections "Key types", "Resolver: cache, dedup, TTL", "Parallel fetch", "Bulk invalidation", "Error isolation", "Pluggable caching", "Dispatch rules", "Normalization at the gate".
- `~/repos/kyomi@ee16f48^:apps/frontend/src/lib/chartml/plugins/duckDbMiddleware.js` for behavioral reference (patterns to keep are enumerated in this doc's "Divergences from the JS duckDbMiddleware" section).

#### Deliverables

New module `crates/chartml-core/src/resolver/mod.rs`:

- `DataSourceProvider` trait (with shutdown hook, `?Send` on WASM).
- `FetchRequest { source_name, spec, cache, headers, cancel_token }`.
- `FetchResult { data, metadata }`.
- `FetchError` enum (`thiserror`).
- `CancellationToken` — thin wrapper, concrete impl TBD in-phase (leaning `Arc<AtomicBool>` + waker list).
- `CacheConfig` parsed from spec + TTL parsing via `humantime`.
- `CacheBackend` trait + `CachedEntry` (with `tags` + `metadata`) + `CacheError` + `MemoryBackend` (default).
- `Resolver` struct holding: memory backend, optional persistent backend, inflight tracker (`HashMap<u64, Shared<BoxFuture<...>>>`), registered providers (`HashMap<String, Arc<dyn DataSourceProvider>>`), hooks reference.
- `Resolver::fetch(key, request) -> Result<FetchResult, FetchError>` with tiered-cache + dedup + hooks logic from the design doc.
- `Resolver::key_for(spec: &InlineData, namespace: Option<&str>) -> u64` — **public** helper that returns the cache key a given spec would use. Phase 4 (Leptos refresh button) and phase 6 (Kyomi invalidate-on-change) depend on computing the exact key the resolver uses internally; exposing this as a public method avoids every caller re-implementing the hash.
- Key computation: `xxhash_rust::xxh3::xxh3_64` of `(namespace, spec.datasource, spec.query, spec.url, spec.provider, spec.rows_hash)`. `namespace` first so different tenants always produce different keys even when everything else matches. `None` fields contribute a sentinel byte, not the literal string `"None"` (avoids collision with a literal `"None"` datasource name).
- Bulk invalidation API (`invalidate`, `invalidate_all`, `invalidate_by_slug`, `invalidate_by_namespace`).

Built-in providers in `resolver/builtin.rs`:

- `InlineProvider` — handles `InlineData { rows: Some(...) }`. Materializes rows into `DataTable` via `DataTable::from_rows`.
- `HttpProvider` — handles `InlineData { url: Some(...) }`. **Uses `reqwest` across both native and WASM targets** (`reqwest` v0.12+ supports `wasm32-unknown-unknown` natively). Single crate, single abstraction, no feature-flag branching. Supports `with_default_headers(HashMap<String, String>)` + merges `FetchRequest.headers` (per-request overrides default). Response body → Arrow IPC bytes via `DataTable::from_ipc_bytes` if `Content-Type: application/vnd.apache.arrow.*`; else JSON parsed and converted via `DataTable::from_rows`.

New `ChartML` methods:

- `register_provider(kind: &str, provider: impl DataSourceProvider + 'static)` — dispatch key. Built-in `"inline"` + `"http"` slots pre-registered by default; `"datasource"` is a host-supplied convention slot (intentionally not pre-registered — host apps register their own implementation).
- `set_cache(backend: impl CacheBackend + 'static)` — replaces `MemoryBackend` with the supplied one as tier 1. (Persistent/tier 2 added via `set_persistent_cache` in phase 3b.)
- `with_cache(backend)` — builder variant.
- `async fn shutdown(&self)` — iterates providers + cache backends, calls shutdown on each.
- `resolver()` accessor for the `invalidate*` API.

Upgrade `ChartML::fetch` (from phase 2):

- Parses YAML → `ChartSpec`.
- Resolves param substitutions.
- Computes `DataRef` shape.
- Applies "normalize unnamed+transform → `{source: <original>}`" rule before branching.
- Dispatches each source through the resolver (which handles cache / dedup / provider call).
- Parallel fetch via `try_join_all` for multi-source maps.
- Returns `FetchedChart` with populated `metadata.per_source`.

#### Implementation steps

1. Add deps to `chartml-core/Cargo.toml`: `xxhash-rust` (with `xxh3` feature), `humantime`, `thiserror` (if not present), `futures` (already present), `reqwest` (with `rustls-tls` on native, wasm target uses reqwest's built-in wasm support). No pin-project needed; `CancellationToken` uses only `Arc<AtomicBool>` + `Mutex<Vec<Waker>>`.
2. Build `resolver/mod.rs` skeleton: trait, request/result/error types, module layout.
3. Implement `MemoryBackend` with `Arc<Mutex<HashMap<u64, CachedEntry>>>`. Tag-based invalidation via linear scan (acceptable for in-memory; IndexedDB will use proper indexes in 3b).
4. Implement `Resolver::fetch` with tiered cache + inflight dedup. Inflight map stores `Shared<BoxFuture<Result<FetchResult, FetchError>>>`. Clean up entries on completion.
5. Implement `InlineProvider` + `HttpProvider`. Both register under their respective `kind` keys.
6. Implement bulk invalidation (`invalidate_all`, `invalidate_by_slug`, `invalidate_by_namespace`). Slug/namespace tags attached on cache write.
7. Wire `ChartML::fetch` to use resolver for non-`Named` sources. `Named(name)` still goes through `self.sources` registry (pre-registered fast path).
8. Implement parallel fetch via `futures::future::try_join_all(sources.iter().map(|(name, spec)| self.resolver.fetch(key_of(spec), request_of(name, spec))))`.
9. Implement the unnamed-with-transform normalization at the top of `fetch()` — if `DataRef::Inline(flat)` and spec has `transform:`, build a fake `NamedMap { "source": flat }` internally before dispatch. Don't mutate the spec, just take the `NamedMap` code path.
10. Wire `ChartML::shutdown()` to iterate and await all provider + backend shutdowns.
11. Document `register_provider` kinds: `"inline"`, `"http"`, `"datasource"` (convention — see Dispatch rules).

#### Tests to add

All in `crates/chartml-core/tests/resolver_test.rs` unless noted:

- `test_dispatch_inline_provider` — YAML `data: { rows: [...] }` → `InlineProvider::fetch` called once.
- `test_dispatch_http_provider` — mock HTTP server (e.g., `wiremock`), YAML `data: { url: "http://..." }` → body fetched → `DataTable` populated.
- `test_dispatch_datasource_provider` — register a mock `DataSourceProvider`, YAML `data: { datasource: "foo", query: "..." }` → provider called with correct `FetchRequest`.
- `test_dispatch_named_map` — YAML with `NamedMap` of 3 entries, all three providers called in parallel. Use `tokio::time::Instant` or mock delays to verify parallelism (total time < sum of per-source times within reasonable tolerance).
- `test_normalization_unnamed_transform` — YAML `data: { datasource, query } + transform: { sql: "SELECT * FROM source" }` → works (source is aliased to `"source"` inside the `NamedMap { "source": ... }` branch).
- `test_cache_hit_memory_tier` — two consecutive `fetch` calls with same key → second served from memory, provider called exactly once.
- `test_cache_expiry` — `MemoryBackend` with TTL = 10ms, fetch → sleep 15ms → fetch → provider called twice.
- `test_inflight_dedup` — spawn two concurrent `fetch` calls with same key, mock provider with 100ms delay → provider called exactly once, both await same result.
- `test_invalidate_single` — fetch, invalidate by key, fetch again → provider called twice.
- `test_invalidate_by_slug` — fetch 3 sources (2 from slug `"foo"`, 1 from slug `"bar"`), invalidate_by_slug(`"foo"`) → only foo entries evicted.
- `test_invalidate_all` — fetch 3 sources, invalidate_all → all evicted.
- `test_multi_source_no_transform_error` — YAML `NamedMap` with 2 entries + no transform → error with React-matching text.
- `test_single_source_passthrough` — 1 entry `NamedMap` + no transform → `PreparedChart.data` equals fetched source.
- `test_http_provider_default_headers` — `HttpProvider::with_default_headers({"Authorization": "Bearer X"})` → mock server asserts header present.
- `test_http_provider_request_headers_override` — default + `FetchRequest.headers` → per-request overrides default.
- `test_fetch_error_isolation` — `NamedMap` of 3 sources, 1 fails → overall error, per-source hook records the failing source (asserts via a test hook impl).
- `test_shutdown_invokes_providers` — register a provider with a shutdown counter, call `chartml.shutdown()` → counter increments.
- `test_fetch_result_metadata_passthrough` — provider returns `FetchResult { data, metadata: {"bytes_billed": 12345} }` → `FetchedChart.metadata.per_source["source_name"]["bytes_billed"] == 12345`.

#### Exit criteria

- `cargo test --workspace` green (including all new tests).
- `cargo clippy` green.
- `cargo doc --workspace --no-deps` builds without warnings.
- End-to-end test: full KYO-79-shape YAML (named-multi + transform + joins) renders to SVG via `render_to_svg_async` against a mock provider.
- `code-review-architect` signature.

#### Non-obvious concerns

- `futures::future::Shared` requires `Clone` on the output type (`Result<FetchResult, FetchError>`). Both `FetchResult` and `FetchError` must derive / implement `Clone`. `DataTable` is `Arc`-backed so `Clone` is cheap.
- Inflight map cleanup: when a fetch completes, the `Shared` future's internal state goes from "pending" to "ready." Remove from inflight map on the first waker that sees completion (avoid leaking futures).
- `xxh3_64` is deterministic but not stable across crate major versions. That's fine for in-memory; `IndexedDbBackend` in 3b needs to either lock to a specific xxhash version or store the hash algorithm identifier.
- `?Send` on WASM means `CancellationToken` uses `Arc<AtomicBool>` + `std::sync::Mutex<Vec<Waker>>`. No tokio or parking_lot deps.
- `register_provider("datasource", ...)` implicit convention: any `InlineData` with `datasource: Some(_)` and no explicit `provider` field dispatches to the `"datasource"` key (see Dispatch rules table in Architecture).
- `HttpProvider` uses `reqwest` for both native and WASM. No feature flags needed — reqwest v0.12+ handles wasm32 target via `web-sys` internally. Enable `rustls-tls` feature on native (avoids openssl). For WASM, reqwest auto-uses `fetch`.
- **Namespace in cache key**: the `Resolver::fetch` call site (inside `ChartML::fetch`) must populate `FetchRequest.namespace` from a `ChartML`-level field (`ChartML::with_namespace(slug)` / `ChartML::set_namespace(slug)` — add these in phase 3 as small additive API). For single-tenant consumers (demos, server-side), `namespace = None` is fine.

---

### Phase 3b — `IndexedDbBackend` for persistent browser cache

**Branch:** `jason/phase-3b-indexeddb-backend`
**Depends on:** Phase 3 landed on main
**Parallel with:** Phase 3c (no shared files)
**Estimated size:** M (WASM-specific, ~5 files)

#### Goal

Ship a `CacheBackend` impl that persists cached sources in IndexedDB so page refreshes don't re-hit expensive upstreams within TTL. Behind a `wasm-indexeddb` cargo feature to keep non-browser builds lean.

#### Context to load

- `crates/chartml-core/src/resolver/mod.rs` from phase 3.
- `crates/chartml-core/src/data/mod.rs` — `DataTable::to_ipc_bytes` / `from_ipc_bytes`.
- `idb` crate docs (https://crates.io/crates/idb).

#### Deliverables

- `crates/chartml-core/src/resolver/backends/indexeddb.rs` behind `#[cfg(all(target_arch = "wasm32", feature = "wasm-indexeddb"))]`.
- Cargo feature `wasm-indexeddb` in `chartml-core/Cargo.toml` that pulls in `idb` + `js-sys` + `wasm-bindgen`.
- `IndexedDbBackend::new(database_name: &str, namespace: &str) -> Result<Self, CacheError>` — namespace is **required** (no `Default` impl).
- Full `CacheBackend` impl (`get`, `put`, `invalidate`, `invalidate_by_tag`, `clear`, `shutdown`).
- Version byte in stored blob (`0x01` for 5.0), with eviction on mismatch.

#### Implementation steps

1. Add `[features]` block to `chartml-core/Cargo.toml`:
   ```toml
   wasm-indexeddb = ["dep:idb", "dep:js-sys", "dep:wasm-bindgen"]
   ```
   with `idb`, `js-sys`, `wasm-bindgen` listed as optional deps.
2. Build `indexeddb.rs` with module-level `#[cfg]` so it only compiles on WASM with the feature on.
3. Use `idb::Database::open(database_name, 1, |event| { /* create object store scoped by namespace */ })`.
4. Serialization: `[0x01, ipc_len:u32, ipc_bytes, tags_json, metadata_json]` packed into a `Vec<u8>` → `JsValue::from_serde` or `js_sys::Uint8Array::from(&bytes)`.
5. `get(key)` — read `JsValue` → deserialize → version check → if mismatch, fire-and-forget `invalidate(key)` + return `None`.
6. `put(key, entry)` — serialize, write within an IndexedDB transaction.
7. `invalidate_by_tag(tag)` — iterate via cursor (`store.open_cursor()`), check tags, collect matching keys, delete.
8. `clear()` — drop and recreate the object store.
9. `shutdown()` — close database handle.

#### Tests to add

All using `wasm-bindgen-test` harness (`wasm-pack test --firefox --headless` or similar):

- `test_put_get_roundtrip_survives_reconstruction` — put, drop backend, construct new backend with same namespace, get → same entry.
- `test_namespace_isolation` — two backends with different namespaces, put in one, get in other → `None`.
- `test_version_mismatch_evicts` — manually write a blob with version `0xFF`, get → `None` + blob evicted on next read.
- `test_tag_invalidate` — put 3 entries with different tags, invalidate by one tag → only those entries evicted.
- `test_concurrent_writes` — concurrent `put` calls on same key → last-write-wins (or no panic).
- `test_shutdown_closes_cleanly` — construct, shutdown, construct again → works (no stale handle).

#### Exit criteria

- `cargo test --workspace` green with default features.
- `wasm-pack test --firefox --headless -p chartml-core --features wasm-indexeddb` green.
- `cargo build --target wasm32-unknown-unknown --features wasm-indexeddb -p chartml-core` builds.
- `code-review-architect` signature.

#### Non-obvious concerns

- `idb` crate uses `JsValue` heavily. All `JsValue` ops are `!Send` — `IndexedDbBackend` is `?Send` only. This matches the `CacheBackend` trait's `cfg_attr` bounds.
- IndexedDB transactions auto-commit when control returns to the event loop. Don't hold a transaction across `.await` of unrelated work.
- Quota errors: if storage is full, `put` can fail with `QuotaExceededError`. Should bubble up as `CacheError::StorageFull`. Resolver should log the error via hooks and fall through — a failed `put` doesn't block the overall fetch.
- Origin-scoped storage means shared-machine cross-user caching unless namespaces differ. Document the namespace requirement in `IndexedDbBackend::new` docstring.
- `idb` v0.6 API is stable but expect minor breaking changes across minor releases. Pin to a specific minor (`idb = "0.6"`).

---

### Phase 3c — `ResolverHooks` trait + event emission

**Branch:** `jason/phase-3c-resolver-hooks`
**Depends on:** Phase 3 landed on main
**Parallel with:** Phase 3b (no shared files)
**Estimated size:** S (~4 files)

#### Goal

Ship observability into the resolver. Consumers implement `ResolverHooks` and get `on_progress` / `on_cache_hit` / `on_cache_miss` / `on_error` callbacks emitted at every pipeline decision point.

#### Context to load

- `crates/chartml-core/src/resolver/mod.rs` from phase 3.
- This design doc's "Observability: ResolverHooks" section.
- `packages/core/PLUGIN_HOOKS.md` (JS reference for field names / event phases).

#### Deliverables

- `crates/chartml-core/src/resolver/hooks.rs` with trait + event types.
- Resolver instrumentation at 6+ sites:
  - Before tier-1 check → nothing (too chatty).
  - Tier-1 hit → `on_cache_hit` (tier: Memory).
  - Tier-2 hit → `on_cache_hit` (tier: Persistent).
  - Cache miss (both tiers) → `on_cache_miss` (reason: NotFound / Expired).
  - Provider call start → `on_progress` (phase: Fetch, message: "Fetching {source_name}").
  - Provider call success → `on_progress` (phase: Fetch, message: "Fetched {bytes} from {source_name}") + cache writes.
  - Provider call error → `on_error` (phase: Fetch).
  - Transform start → `on_progress` (phase: Transform).
  - Transform success / error → corresponding event.
- `ChartML::set_hooks(impl ResolverHooks + 'static)` setter.
- Default `NullHooks` impl (no-op) used when none set.

#### Implementation steps

1. Define the trait with four methods, all defaulted to empty.
2. Define event structs (`ProgressEvent`, `CacheHitEvent`, `CacheMissEvent`, `ErrorEvent`).
3. Wrap hook calls in `spawn_local` / `tokio::spawn` so slow hooks can't stall the resolver. Errors from hooks are caught + logged via `tracing::warn!`, never propagated.
4. Add hook emission at each instrumentation site in `Resolver::fetch` and `ChartML::transform`.
5. Add `NullHooks` as a zero-cost default.

#### Tests to add

All in `crates/chartml-core/tests/hooks_test.rs`:

- `test_cache_hit_memory_emits` — hook impl collects events; fetch twice with same key → exactly one `on_cache_miss` (first call) + one `on_cache_hit` (second).
- `test_cache_hit_persistent_emits_tier` — fetch, drop memory only (keep persistent), fetch again → `on_cache_hit { tier: Persistent }`.
- `test_provider_error_emits_per_source` — `NamedMap` with 2 sources, one fails → at least one `on_error` event with `source_name: Some("failing_source")`.
- `test_hooks_dont_block_resolver` — hook impl with 500ms artificial delay → fetch completes in < 100ms (hook runs on background task).
- `test_hook_error_isolated` — hook impl that returns an error from one of its methods → resolver still completes the fetch successfully; error logged via `tracing::warn!` and not propagated. (Note: hooks must be **panic-free** — `catch_unwind` doesn't work across `.await` nor on WASM. Document this constraint in the trait docstring.)
- `test_multi_chart_scenario_event_order` — full dashboard-shape scenario, assert event sequence matches expectation:
  1. `CacheMiss` × N (per source, first render)
  2. `Progress(Fetch)` × N
  3. `Progress(Transform)` × 1
  4. `Progress(Render)` × 1 (optional; render emits when hooked)

#### Exit criteria

- `cargo test --workspace` green.
- `code-review-architect` signature.

#### Non-obvious concerns

- Hook emission on WASM uses `wasm_bindgen_futures::spawn_local`; on native, uses `tokio::spawn` when available, else just inline call. Helper function `spawn_hook<F: Future>(fut)` abstracts.
- **Hooks must be panic-free.** `std::panic::catch_unwind` does not work across `.await` points and is not available on WASM at all. We do not try to catch hook panics — if a hook panics, the behavior is implementation-defined (native: task crashes; WASM: usually aborts the whole module). Document this in the `ResolverHooks` trait docstring. Hooks should always catch their own errors internally and return `Ok(())` / a logged error value.
- Don't deadlock: hooks must never acquire any lock the resolver holds. If a hook wants to re-enter the resolver (weird but possible), that's explicitly unsupported — document.
- Ordering guarantees: events fire in the order the resolver reaches them, but due to spawn, consumers may observe re-ordering across concurrent fetches. Document as "per-source ordering preserved, cross-source ordering not guaranteed."

---

### Phase 4 — `chartml-leptos::ChartMLChart` consumes new API + auto-refresh timer

**Branch:** `jason/phase-4-leptos-provider-integration`
**Depends on:** Phases 1, 2, 3 landed on main. 3b/3c optional but recommended.
**Parallel with:** Phase 5
**Estimated size:** M (~10 files in chartml-leptos + demo)

#### Goal

`chartml-leptos::ChartMLChart` accepts a `DataSourceProvider` (prop or Leptos context), drives fetch/transform/render via Leptos resources with proper loading/error states, and implements component-driven auto-refresh per the spec's `cache.autoRefresh` flag.

#### Context to load

- `crates/chartml-leptos/src/chart.rs` — current `ChartMLChart` component.
- `crates/chartml-core/src/resolver/mod.rs` — new API.
- Leptos 0.7+ resources + signals docs.

#### Deliverables

- Updated `ChartMLChart` prop list:
  - `provider: Option<Arc<dyn DataSourceProvider>>` (or via Leptos context if not supplied).
  - `cache_backend: Option<Arc<dyn CacheBackend>>` (optional persistent tier).
  - `hooks: Option<Arc<dyn ResolverHooks>>`.
- Leptos resource wrapping `ChartML::fetch + transform` — re-runs when YAML or params signal changes.
- Loading state: shown while resource `loading()`.
- Error state: shown when resource errors; exposes refresh button.
- Auto-refresh: if any source in the spec has `cache.autoRefresh: true`, spawn a Leptos `set_interval_with_handle` that calls `resolver.invalidate(&key)` + triggers resource re-fetch, pauses when `document.visibilityState != "visible"` (via `web_sys::document` listener).
- Resize: render uses `PreparedChart` (already fetched) with new width/height — no re-fetch on resize.
- New example in `demo/` or `chartml-leptos/examples/` demonstrating provider registration.

#### Implementation steps

1. Plumb `provider` / `cache_backend` / `hooks` through props, falling back to Leptos context `use_context::<Arc<dyn DataSourceProvider>>()` when not directly supplied.
2. Replace direct `chartml.render_to_svg(spec)` with `chartml.render_to_svg_async(spec, opts)`.
3. Wrap in a Leptos `Resource`, drive re-fetching off the YAML + params signals.
4. Loading/error UI: borrow `<Suspense>` + `<ErrorBoundary>` patterns; reuse Kyomi's existing loading icon path if it lives here (check).
5. Auto-refresh logic: scan the `ChartSpec`'s `data:` for any source with `cache.autoRefresh == true`. Parse its TTL, spawn a `set_interval_with_handle` that invalidates + refetches.
6. Visibility pause: `web_sys::document().add_event_listener_with_callback("visibilitychange", ...)`. When hidden, `handle.clear()`. When visible again, re-arm.
7. Update `demo/` with a new page that demonstrates:
   - Flat `{datasource, query}` YAML rendering via a mock provider.
   - Named-multi + transform YAML rendering.
   - Manual refresh button + auto-refresh demo.

#### Tests to add

- `crates/chartml-leptos/tests/chart_provider_test.rs` — wasm-bindgen-test integration:
  - `test_chart_fetches_via_provider` — mount `ChartMLChart` with a mock provider, assert provider was called with correct spec.
  - `test_chart_shows_loading_state` — slow mock provider, assert loading DOM node present.
  - `test_chart_shows_error_state` — failing mock provider, assert error DOM node + retry button present.
  - `test_chart_auto_refresh_fires` — provider with `autoRefresh: true` + short TTL (1s), assert provider called multiple times across 3s.
  - `test_chart_auto_refresh_paused_hidden` — mock `document.visibilityState = "hidden"`, assert no refresh fires.

#### Exit criteria

- `cargo test --workspace` green.
- `wasm-pack test --firefox --headless -p chartml-leptos` green.
- Demo example renders all three KYO-79 YAML shapes against a local mock provider.
- `code-review-architect` signature.
- Chart-evaluator signature on any new golden SVGs added in demo.

#### Non-obvious concerns

- Leptos 0.7 resource semantics: resource only re-runs when its input signal changes, not on every render. Auto-refresh must trigger a signal change (e.g., increment a `refresh_count: RwSignal<u32>`).
- `set_interval_with_handle` returns a handle that must be dropped to stop the interval. Leak = duplicate refreshes forever.
- `Arc<dyn DataSourceProvider>` requires `?Send` bound propagation — check Leptos context API supports non-`Send` types (via `StoredValue` if not).
- Don't forget to invalidate the correct `key`. Key must match what the resolver uses (same xxhash of same fields) — expose a helper `resolver.key_for(&spec) -> u64`.

---

### Phase 5 — WASM binding + `@chartml/markdown-react` refactor

**Branch:** `jason/phase-5-wasm-markdown-react`
**Depends on:** Phase 3 landed.
**Parallel with:** Phase 4.
**Estimated size:** L (cross-language; ~15 files)

#### Goal

Expose the new async resolver/provider API through the WASM binding so JS consumers can register callback-style providers. `@chartml/markdown-react` internals swap its bespoke orchestration for core-driven fetch/transform/render.

#### Context to load

- `crates/chartml-wasm/src/lib.rs` — current WASM bindings.
- `packages/core/src/index.ts` / `index.js` — current JS surface.
- `packages/markdown-react/src/index.js` — current orchestration logic (340 lines; target for deletion).
- `packages/core/src/pluginRegistry.js` — current plugin registration pattern (for API symmetry).

#### Deliverables

- WASM additions in `chartml-wasm`:
  - `ChartML::registerProvider(kind: string, callback: (request: JsValue) => Promise<JsValue>)` — async JS callback bridged to a `DataSourceProvider` impl.
  - `ChartML::fetch(yaml: string) -> Promise<FetchedChart>` (mirror).
  - `ChartML::transform(fetched: FetchedChart) -> Promise<PreparedChart>`.
  - `ChartML::renderPreparedToSvg(prepared: PreparedChart, opts) -> string`.
  - `ChartML::renderToSvgAsync(yaml, opts) -> Promise<string>`.
  - `ChartML::setCache(backend: string | CacheBackend)` — accept a named built-in (`"memory"`, `"indexeddb"`) or a JS object implementing the backend interface.
  - Hooks exposure via `registerHooks({ onProgress, onCacheHit, ... })`.
- TypeScript types in `packages/core/src/index.ts`.
- Refactor `packages/markdown-react/src/index.js`:
  - Delete bespoke two-pass rendering orchestration.
  - Use `chartmlInstance.renderToSvgAsync(yamlText)` for each chart block.
  - Preserve public API (`ChartMLCodeBlock({ chartmlInstance, ... })`).

#### Implementation steps

1. Extend `chartml-wasm::ChartML` with async methods. Use `wasm-bindgen-futures` for promise bridging.
2. Implement a `JsCallbackProvider` struct in `chartml-wasm` that wraps a `js_sys::Function` + implements `DataSourceProvider`.
3. Build TypeScript definitions in `packages/core/src/types.ts` for the new surface.
4. Audit `packages/markdown-react/src/index.js` — identify the orchestration code (2-pass rendering, source registration loop, inline param handling). Replace with single calls to the new async API.
5. Preserve chart-wrapper / params-wrapper / code-renderer customization points (they're UI concerns, unaffected by data changes).
6. Update vitest tests in `packages/markdown-react` to reflect the simplified code path.
7. Rebuild WASM via `packages/core/build.sh`.

#### Tests to add

- `packages/markdown-react/test/chartml-integration.test.js` — vitest:
  - `test_full_pipeline_flat` — YAML with `data: { datasource, query }` + mock provider → rendered SVG in DOM.
  - `test_full_pipeline_named_multi` — full KYO-79 YAML shape → works.
  - `test_cache_survives_rerender` — render twice, provider called once (memory cache).
  - `test_indexeddb_survives_remount` — render, unmount component, remount → cached (requires IndexedDB mock or real).
- `packages/core/test/wasm-provider-test.js` — vitest:
  - `test_js_callback_provider_registered_and_called` — register callback, invoke fetch → callback called with correct request shape.

#### Exit criteria

- `npm test` green in both `packages/core` and `packages/markdown-react`.
- `packages/markdown-react/src/index.js` line count reduced by ~150 lines (removal of bespoke orchestration).
- WASM rebuild succeeds; `demo/` still renders.
- `code-review-architect` signature.

#### Non-obvious concerns

- JS callback → Rust trait bridging: `js_sys::Function::call1(&request_js_val)` returns `JsValue`. Must be a `Promise`; convert via `wasm_bindgen_futures::JsFuture::from(promise)`. Error types become `JsValue` on the Rust side; convert to `FetchError::Other`.
- Thread safety: callbacks are single-threaded (WASM). All `DataSourceProvider` bounds are `?Send` on WASM anyway — align.
- Backwards compat: users of `@chartml/markdown-react` with the current chartml 4.x pattern must still work. Provide migration notes in the phase 7 CHANGELOG.
- `IndexedDbBackend` wrapper: expose a JS-friendly constructor like `new ChartMLIndexedDbCache({ databaseName, namespace })` that just instantiates the Rust struct.

---

### Phase 6 — KYO-79 in Kyomi repo: `KyomiDatasourceProvider`

**Branch:** `jason/kyo-79-dashboard-port-reacts-duckdb-named-sources-pattern-to-leptos` (branch name from Linear ticket)
**Depends on:** Phases 1–4 published as chartml 5.0 pre-release (or path dep during dev).
**Parallel with:** none
**Estimated size:** M (Kyomi-side; ~8 files)

#### Goal

Kyomi's Leptos dashboard renders all three prod YAML shapes (flat / named-single / named-multi + transform) by implementing a thin `DataSourceProvider` that wraps the existing `query_datasource_arrow` server function. The bespoke `_remote` path in `markdown_renderer.rs` is deleted.

#### Context to load

- `~/repos/kyomi/crates/kyomi-ui/src/components/dashboard/markdown_renderer.rs` — current `ChartBlock` component (~lines 500–730) and `extract_datasource` / `extract_query` (lines 298–311).
- `~/repos/kyomi/crates/kyomi-ui/src/server_fns/datasources.rs` — `query_datasource_arrow` server function.
- `~/repos/kyomi/Cargo.toml` — chartml version pins (currently `"4"`, bumping to `"5"`).
- KYO-79 ticket (Linear) for acceptance YAMLs.
- This design doc phases 1–4 for the API surface that now exists.

#### Deliverables

- New `~/repos/kyomi/crates/kyomi-ui/src/chartml_provider.rs`:
  - `KyomiDatasourceProvider` struct.
  - `DataSourceProvider` impl calling `query_datasource_arrow` + decoding Arrow IPC → `DataTable`.
- Update `chartml-*` deps in `~/repos/kyomi/Cargo.toml` to `"5"` (or `"=5.0.0-pre.1"` for pre-release during chartml phase 7).
- Refactor `markdown_renderer.rs`:
  - Delete `extract_datasource` / `extract_query` (no longer needed — resolver handles dispatch).
  - Delete the bespoke fetch/register loop in `ChartBlock` (~lines 600–720).
  - Replace with a single `ChartMLChart` component instantiation with the `KyomiDatasourceProvider` wired via Leptos context.
- Wire `IndexedDbBackend` with `namespace = workspace_slug` so dashboards across workspaces don't bleed cache.
- Optional: wire `ResolverHooks` to Kyomi's telemetry to surface query cost.

#### Implementation steps

1. Bump chartml deps in Cargo.toml. Run `cargo update -p chartml-core` etc. Verify Cargo.lock shows 5.x.
2. Build `KyomiDatasourceProvider::fetch` — ~30 lines:
   - Extract `slug` and `query` from `request.spec`.
   - Call `query_datasource_arrow(slug, query, None).await`.
   - Base64 decode the IPC bytes.
   - `DataTable::from_ipc_bytes(&bytes)`.
   - Return `FetchResult { data, metadata: { bytes_billed, rows_returned, ... } }`.
3. Set up Leptos context at the dashboard root: `provide_context(Arc::new(KyomiDatasourceProvider::new(workspace_slug)))`.
4. Set up IndexedDB cache: `chartml.set_persistent_cache(IndexedDbBackend::new("kyomi-chartml-cache", &workspace_slug)?)`.
5. Rewrite `ChartBlock` to delegate everything to `ChartMLChart` from phase 4. The remote/inline distinction disappears.
6. Thread `parameters: Signal<HashMap<String, String>>` through the new component.
7. Delete removed code (extract helpers, remote fetch effect, spec rewriting to `"_remote"`).

#### Tests to add

- `~/repos/kyomi/crates/kyomi-ui/tests/chartml_provider_test.rs` — unit tests for provider:
  - `test_provider_calls_query_datasource_arrow` — mock server fn, assert called with correct args.
  - `test_provider_decodes_ipc_bytes` — given IPC bytes, produces correct `DataTable`.
- Browser E2E via `/kyomi-test` skill — new fixtures covering KYO-79's three YAML shapes.

#### Exit criteria

- Three KYO-79 YAML shapes render without error in local dev.
- `cargo test --workspace` green in Kyomi repo.
- E2E tests pass via `/kyomi-test dashboard-viewer`.
- Linear KYO-79 moves to "In Review".
- `code-review-architect` signature (in Kyomi repo, same rules).
- Page refresh within TTL does NOT trigger BigQuery — verify in Network tab.

#### Non-obvious concerns

- Kyomi uses a server function (`query_datasource_arrow`) that only runs in the Leptos server context. The provider needs to call it from the browser; that routing already works in Kyomi but verify with a smoke test.
- Base64 decode on the client adds latency. Consider wire-format change to binary later; for now, match existing flow.
- `workspace_slug` might not be known at WASM init time (loaded from async auth). Pattern: defer the `KyomiDatasourceProvider::new()` call until slug is available, or pass a `Fn() -> String` closure for lazy lookup.
- Arrow decode errors should surface as `FetchError::DecodeFailed`, not `Other`.

---

### Phase 7 — Release: chartml 5.0.0

**Branch:** `jason/phase-7-release-5.0`
**Depends on:** Phases 1–6 merged.
**Parallel with:** none
**Estimated size:** S (mostly versioning + publishing)

#### Goal

Publish chartml 5.0.0 to crates.io and npm, with CHANGELOG and migration notes.

#### Context to load

- Existing `CHANGELOG.md`.
- `PUBLISHING.md` in the repo root.
- Current version strings in `crates/*/Cargo.toml` and `packages/*/package.json`.

#### Deliverables

- Version bump to `5.0.0` in:
  - All `crates/*/Cargo.toml` (`chartml-core`, `-render`, `-chart-*`, `-datafusion`, `-forecast`, `-leptos`, `-wasm`, `-wasm-datafusion`).
  - Internal dep versions bumped to match.
  - All `packages/*/package.json` (`@chartml/core`, `@chartml/react`, `@chartml/markdown-react`, `@chartml/datafusion`, `@chartml/markdown-it`, `@chartml/markdown-common`).
  - Inter-package dep versions bumped.
- CHANGELOG entry for 5.0.0 — breaking changes called out:
  - `TransformMiddleware::transform` signature change (phase 1).
  - New `DataSourceProvider` trait + `register_provider` API (phase 3).
  - New three-stage pipeline methods (phase 2).
  - `CacheBackend` + optional `IndexedDbBackend` (phase 3/3b).
  - `ResolverHooks` (phase 3c).
  - HTTP provider with headers (phase 3).
- New `docs/migration/4-to-5.md` covering:
  - How to update `TransformMiddleware` impls.
  - How to register a `DataSourceProvider`.
  - How to wire IndexedDB cache.
  - How to adopt hooks.
- Publish order (verified against current dep graph): `cargo publish` crates in this order:
  1. `chartml-core` (no chartml deps)
  2. `chartml-forecast` (depends on chartml-core)
  3. `chartml-chart-cartesian`, `chartml-chart-pie`, `chartml-chart-scatter`, `chartml-chart-metric`, `chartml-chart-table` (each depends on chartml-core; can publish in parallel)
  4. `chartml-datafusion` (depends on chartml-core)
  5. `chartml-render` (depends on chartml-core)
  6. `chartml-leptos` (depends on chartml-core + chart crates)
  7. `chartml-wasm` (depends on chartml-core + chart crates + chartml-render)
  8. `chartml-wasm-datafusion` (depends on chartml-core + chartml-datafusion)

  `chartml-render` must publish before `chartml-wasm` (the latter depends on the former).
- `npm publish` packages.
- Git tag `v5.0.0`.

#### Exit criteria

- `crates.io` shows v5.0.0 for every crate.
- `npmjs.com` shows v5.0.0 for every package.
- GitHub Actions CI green on tag.
- Migration doc published.

#### Non-obvious concerns

- Kyomi needs to consume the published 5.0 (not path dep) for the final phase-6 merge. Coordinate Kyomi Cargo.lock update as the last step.
- Crate publishing is idempotent per a recent CI fix — if a crate is already at 5.0, the publish skips. Safe to re-run.

---

## Open questions

1. **Progress reporting for long transforms.** DataFusion can expose stage progress (per-stage completion callbacks). Could plumb this into `ProgressEvent` during phase 3c. Park or do in 5.0? Leaning park — depends on DataFusion-side API work.
2. **Eviction policy for `IndexedDbBackend`.** 5.0 relies on TTL-driven eviction (stale keys removed on read). Worth adding LRU or size-capped eviction in 5.0, or park until observed storage pressure? Browsers auto-evict under quota pressure anyway; park.
3. **Cache key namespacing default.** `IndexedDbBackend::new()` will require an explicit namespace rather than defaulting to a shared one — forces the conscious choice and avoids accidental cross-user bleed. Confirmed.

---

## Deferred to 5.x

Explicitly out of 5.0 scope but documented so future design work can pick up with context.

| Feature | Why deferred | When to revisit |
|---|---|---|
| **Retry / backoff for transient provider errors** | Belongs in a decorator (`RetryingProvider<P>`) that consumers compose. Keeps core simple. | When retry logic shows up in multiple consumers. |
| **Streaming / paginated fetch** (`Stream<DataTable>`) | Major API shift; most analytics dashboards pull bounded result sets. | If a consumer needs progressive rendering for very large sources. |
| **Prefetch API** (`resolver.prefetch(spec)` without rendering) | Useful for hover-to-prefetch UX but speculative. | When a consumer ships predictive loading and asks for the hook. |
| **Spec dry-run validation** (`ChartML::validate(yaml)`) | Downstream errors already surface clearly. Nice-to-have. | When design tooling wants a no-fetch validation step. |
| **Cross-tab in-flight dedup** via `BroadcastChannel` | Already in non-goals. One-extra-fetch race is tolerable. | If telemetry shows a material % of duplicate concurrent fetches from multi-tab users. |
| **Cache introspection API** (`resolver.stats()`, `resolver.entries()`) | Debug-only. | When a DevTools panel or debugging UI is scoped. |
| **Cost tracking beyond pass-through metadata** | `FetchResult.metadata` already threads through; consumers aggregate. | If multiple consumers reimplement the same cost aggregation. |
| **Persistent cache on server side** (`FileSystemBackend`) | Kyomi server pods are short-lived; memory cache is enough. Disk adds complexity without clear benefit. | If long-running server deployments need warm caches. |
| **DataFusion-side progress emission** (stage-level hooks) | DataFusion API work required; chartml-side `ProgressEvent` type is ready to receive them. | When DataFusion exposes stage callbacks. |
| **LRU / size-capped eviction** for `IndexedDbBackend` | TTL-driven eviction + browser auto-evict under quota pressure is sufficient for 5.0. | If dashboards accumulate enough cold entries to hit quota. |
| **Resolver-owned auto-refresh timers** | Component-driven (phase 4) in 5.0 avoids WASM timer lifecycle complexity. | If auto-refresh logic ends up duplicated with the same bugs across consumers. |
| **Per-provider cache-key influence** (`provider.cache_key(request) -> Option<u64>`) | Content-hash is reliable; let providers handle semantic-equivalence caching internally. | Specific request from a provider maintainer. |
| **Schema evolution detection** beyond TTL | Out of scope for 5.0. TTL catches stale schemas eventually. | When a consumer needs fast schema-change detection. |

## Appendix A — Rejected alternatives

**Alt 1: Only fix `DataFusionTransform`, leave orchestration to consumers.** Rejected — recreates the outsourcing problem and leaves `@chartml/markdown-react`'s own orchestration duplicating what should be core.

**Alt 2: Make `render_to_svg` async end-to-end.** Rejected — color-infects WASM boundary, breaks the "pure sync render" property that makes caching by size/theme trivial.

**Alt 3: Port `duckDbMiddleware.js` to WASM via DuckDB-WASM.** Rejected — adds large WASM dependency, duplicates storage (Arrow in core + DuckDB tables), and locks Rust design to a JS-ism.

**Alt 4: One unified "source handler" trait combining fetch + transform.** Rejected — conflates I/O and compute; forces every host to reason about transform semantics when they only want to plug in a fetcher.

---

## Appendix B — References

- **KYO-79 ticket** (Linear) — tracking issue for the Kyomi-side adoption (phase 6).
- **`~/repos/kyomi@ee16f48^:apps/frontend/src/lib/chartml/plugins/duckDbMiddleware.js`** — the 448-line JS middleware this design's patterns come from. Reference via `git -C ~/repos/kyomi show ee16f48^:apps/frontend/src/lib/chartml/plugins/duckDbMiddleware.js`.
- **`packages/core/PLUGIN_HOOKS.md`** — JS hook system, source of the hook event shapes (phase 3c).
- **`packages/core/src/index.js:430-530`** — JS `_resolveDataSource` + `_applyTransform` implementation.
- **`packages/markdown-react/src/index.js`** — JS wrapper this design's phase 5 refactor replaces.
- **`crates/chartml-core/src/plugin/transform.rs`** — current `TransformMiddleware` trait (changing in phase 1).
- **`crates/chartml-datafusion/src/lib.rs:30-103`** — current `DataFusionTransform::transform` (changing in phase 1).
- **`docs/docs/spec.md`** — ChartML v1.0 spec this design covers.
- **`idb` crate** (https://crates.io/crates/idb) — IndexedDB bindings for phase 3b.
- **`xxhash-rust` crate** (https://crates.io/crates/xxhash-rust) — cache key hashing.
- **`humantime` crate** (https://crates.io/crates/humantime) — TTL parsing.

## Appendix C — Execution via `/agent-driven-development`

Each phase above is structured for independent execution by a fresh feature-implementation-engineer agent. An ADD runner can iterate through phases in order (1 → 2 → 3 → [3b ∥ 3c] → 4 → [5 ∥ 6 after 5 publishes] → 7) with the following conventions:

- **Branch per phase.** Each phase lists its branch name at the top.
- **Context loading.** Each phase lists the exact files the agent should read first. No conversation history needed.
- **Deliverables.** Each phase lists what concrete code artifacts should exist when complete.
- **Implementation steps.** Ordered checklist — follow in order or justify deviation.
- **Tests to add.** Name and semantics of every new test case. Agent writes these; code-review-architect checks they fire for the right reasons.
- **Exit criteria.** Objective checks (tests pass, clippy clean, `code-review-architect` signature obtained, etc.).
- **Non-obvious concerns.** Foot-guns the agent must be aware of. If the agent hits one of these and can't resolve, it escalates.

Mandatory per `CLAUDE.md`:

- Every commit requires a `code-review-architect` signature.
- Every golden SVG change requires a `chart-evaluator` signature.
- No lint suppressions (`#[allow(...)]`); fix the warning instead.

Phases 3b and 3c are mutually independent once phase 3 has landed — ADD can run them in parallel git worktrees.
