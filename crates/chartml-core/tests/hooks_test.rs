//! Integration tests for chartml 5.0 phase 3c — `ResolverHooks` trait + event
//! emission. Each test installs a recording hook impl that pushes events into
//! a shared `Arc<Mutex<Vec<Event>>>` so we can assert on the exact sequence
//! the resolver emitted.
//!
//! Hook callbacks are fire-and-forget on the tokio runtime, so each test
//! awaits a small `flush_pending_hooks` helper that yields several times
//! before reading the collected events. This is the same coordination
//! consumer code would do in production (e.g., a Leptos progress bar
//! subscribes via a channel).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chartml_core::data::{DataTable, Row};
use chartml_core::element::{ChartElement, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::resolver::{
    CacheHitEvent, CacheMissEvent, CacheTier, DataSourceProvider, ErrorEvent, FetchError,
    FetchRequest, FetchResult, MemoryBackend, MissReason, Phase, ProgressEvent, Resolver,
    ResolverHooks,
};
use chartml_core::spec::InlineData;
use chartml_core::{ChartML, RenderOptions};
use chartml_datafusion::DataFusionTransform;
use serde_json::json;

// ── Recording hook impl ─────────────────────────────────────────────────

/// One captured hook event. Tagged enum so a single shared `Vec` can hold
/// every event type in observed order.
#[derive(Debug, Clone)]
enum Event {
    Progress(ProgressEvent),
    CacheHit(CacheHitEvent),
    CacheMiss(CacheMissEvent),
    Error(ErrorEvent),
}

/// Hook impl that pushes every event into a shared collector. The
/// optional `delay` is used by `test_hooks_dont_block_resolver` to prove
/// fire-and-forget dispatch.
struct RecordingHooks {
    events: Arc<Mutex<Vec<Event>>>,
    delay: Option<Duration>,
}

impl RecordingHooks {
    fn new() -> (Self, Arc<Mutex<Vec<Event>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: events.clone(),
                delay: None,
            },
            events,
        )
    }

    fn with_delay(delay: Duration) -> (Self, Arc<Mutex<Vec<Event>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                events: events.clone(),
                delay: Some(delay),
            },
            events,
        )
    }

    async fn maybe_delay(&self) {
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
    }
}

#[async_trait]
impl ResolverHooks for RecordingHooks {
    async fn on_progress(&self, event: ProgressEvent) {
        self.maybe_delay().await;
        self.events.lock().unwrap().push(Event::Progress(event));
    }
    async fn on_cache_hit(&self, event: CacheHitEvent) {
        self.maybe_delay().await;
        self.events.lock().unwrap().push(Event::CacheHit(event));
    }
    async fn on_cache_miss(&self, event: CacheMissEvent) {
        self.maybe_delay().await;
        self.events.lock().unwrap().push(Event::CacheMiss(event));
    }
    async fn on_error(&self, event: ErrorEvent) {
        self.maybe_delay().await;
        self.events.lock().unwrap().push(Event::Error(event));
    }
}

/// Yield repeatedly so any spawned hook tasks get a chance to run before
/// the test reads `events`. Hook dispatch is fire-and-forget via
/// `tokio::spawn`, which doesn't synchronize with the awaiter — yielding
/// is the simplest way to ensure the collector has settled.
async fn flush_pending_hooks() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

// ── Test fixtures ────────────────────────────────────────────────────────

fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn visitors_table() -> DataTable {
    DataTable::from_rows(&[
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(100.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(150.0))]),
    ])
    .unwrap()
}

fn sessions_table() -> DataTable {
    DataTable::from_rows(&[
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(10.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(15.0))]),
    ])
    .unwrap()
}

struct MockRenderer;

impl ChartRenderer for MockRenderer {
    fn render(&self, _data: &DataTable, _config: &ChartConfig) -> Result<ChartElement, ChartError> {
        Ok(ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "mock".to_string(),
            children: vec![],
        })
    }
}

/// Provider that returns a fixed table and counts call sites.
struct CountingProvider {
    table: DataTable,
    calls: Arc<AtomicU32>,
}

impl CountingProvider {
    fn new(table: DataTable) -> Self {
        Self {
            table,
            calls: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait]
impl DataSourceProvider for CountingProvider {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(FetchResult {
            data: self.table.clone(),
            metadata: HashMap::new(),
        })
    }
}

/// Provider that always errors.
struct FailingProvider {
    message: String,
}

#[async_trait]
impl DataSourceProvider for FailingProvider {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        Err(FetchError::QueryFailed(self.message.clone()))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

/// Single-source fetch with the same key twice — the first must emit one
/// `CacheMiss` (NotFound) + one `Fetch` progress; the second must emit one
/// `CacheHit { tier: Memory }` and NO additional `CacheMiss`.
#[tokio::test]
async fn test_cache_hit_memory_emits() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", CountingProvider::new(visitors_table()));
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "60s"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();

    chartml.fetch(yaml, &opts).await.unwrap();
    chartml.fetch(yaml, &opts).await.unwrap();
    flush_pending_hooks().await;

    let events = events.lock().unwrap();
    let misses: Vec<&Event> = events.iter().filter(|e| matches!(e, Event::CacheMiss(_))).collect();
    let hits: Vec<&Event> = events.iter().filter(|e| matches!(e, Event::CacheHit(_))).collect();

    assert_eq!(misses.len(), 1, "expected exactly 1 cache miss across two fetches; got events: {events:?}");
    assert_eq!(hits.len(), 1, "expected exactly 1 cache hit across two fetches; got events: {events:?}");

    if let Event::CacheMiss(m) = misses[0] {
        assert_eq!(m.reason, MissReason::NotFound);
    } else {
        unreachable!()
    }
    if let Event::CacheHit(h) = hits[0] {
        assert_eq!(h.tier, CacheTier::Memory);
    } else {
        unreachable!()
    }
}

/// Persistent-tier hit emits `tier: Persistent`. We register a manual
/// persistent backend (just a second `MemoryBackend`), prime both tiers
/// with a fetch, then SWAP the primary cache for an empty `MemoryBackend`
/// so the next fetch misses tier-1 and hits tier-2.
#[tokio::test]
async fn test_cache_hit_persistent_emits_tier() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", CountingProvider::new(visitors_table()));
    chartml.set_hooks(hooks);

    // Wire a persistent backend (a second MemoryBackend stands in for
    // IndexedDbBackend, which lands in phase 3b).
    let persistent: Arc<MemoryBackend> = Arc::new(MemoryBackend::new());
    chartml.resolver().set_persistent_cache(persistent.clone());

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "60s"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    chartml.fetch(yaml, &opts).await.unwrap();

    // Swap the primary (tier-1) cache for an empty MemoryBackend so the
    // next fetch misses tier-1 and falls through to the persistent tier.
    chartml.resolver().set_primary_cache(Arc::new(MemoryBackend::new()));

    // Clear collected events so we only assert on the second fetch.
    events.lock().unwrap().clear();

    chartml.fetch(yaml, &opts).await.unwrap();
    flush_pending_hooks().await;

    let events = events.lock().unwrap();
    let hits: Vec<&CacheHitEvent> = events
        .iter()
        .filter_map(|e| {
            if let Event::CacheHit(h) = e {
                Some(h)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected exactly 1 cache hit on second fetch; got events: {events:?}"
    );
    assert_eq!(
        hits[0].tier,
        CacheTier::Persistent,
        "tier-2 hit must report `Persistent`; got {:?}",
        hits[0].tier
    );
}

/// `NamedMap` with one failing source — at least one `on_error` event
/// must be observed with `source_name: Some("failing_source")`.
#[tokio::test]
async fn test_provider_error_emits_per_source() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("good", CountingProvider::new(visitors_table()));
    chartml.register_provider(
        "failing",
        FailingProvider {
            message: "synthetic upstream error".to_string(),
        },
    );
    chartml.register_transform(DataFusionTransform);
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  ok_source:
    provider: good
    datasource: ok
  failing_source:
    provider: failing
    datasource: bad
transform:
  sql: "SELECT * FROM ok_source"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let _ = chartml.fetch(yaml, &opts).await.expect_err("multi-source fetch must fail");
    flush_pending_hooks().await;

    let events = events.lock().unwrap();
    let failing_errors: Vec<&ErrorEvent> = events
        .iter()
        .filter_map(|e| match e {
            Event::Error(e) if e.source_name.as_deref() == Some("failing_source") => Some(e),
            _ => None,
        })
        .collect();
    assert!(
        !failing_errors.is_empty(),
        "expected at least one on_error event tagged source_name=Some(\"failing_source\"); got: {events:?}"
    );
    assert_eq!(failing_errors[0].phase, Phase::Fetch);
}

/// A hook impl with a 500ms artificial delay must NOT stall the resolver.
/// The fetch should complete in well under 100ms because hook dispatch is
/// fire-and-forget via `tokio::spawn`.
#[tokio::test]
async fn test_hooks_dont_block_resolver() {
    let (hooks, _events) = RecordingHooks::with_delay(Duration::from_millis(500));
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", CountingProvider::new(visitors_table()));
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();

    let start = Instant::now();
    chartml.fetch(yaml, &opts).await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(100),
        "fetch should not be blocked by 500ms hook delay; took {:?}",
        elapsed,
    );
}

/// The trait docstring documents that hooks must be panic-free. We can't
/// catch panics across `.await` (and not at all on WASM), so the contract
/// is enforced via documentation. This test reads the source file
/// directly and asserts the wording is present.
#[test]
fn test_hook_panic_is_documented() {
    let source = include_str!("../src/resolver/hooks.rs");
    assert!(
        source.contains("Hooks must be panic-free")
            || source.contains("must not panic"),
        "hooks.rs must document the panic-free requirement"
    );
    assert!(
        source.contains("catch_unwind"),
        "hooks.rs must explain why panics aren't catchable (catch_unwind across await)"
    );
    assert!(
        source.contains("fire-and-forget on the runtime")
            || source.contains("no ordering guarantee"),
        "hooks.rs must document the fire-and-forget / no-ordering semantics"
    );
}

/// Full dashboard-shape scenario: 2 named sources, transform, render.
/// Asserts unordered multiset membership of the events the resolver
/// emits — NOT ordering. Hook dispatch is fire-and-forget via
/// `tokio::spawn`, so neither cross-source nor per-source ordering is
/// guaranteed: each emit submits an independent task with no
/// happens-before relationship to subsequent emits.
///
/// Membership we DO require:
///   1. Exactly two `CacheMiss` events — one per source name
///      (`visitors`, `sessions`).
///   2. At least one Fetch-phase `Progress` event per source.
///   3. At least one Transform-phase `Progress` event.
#[tokio::test]
async fn test_multi_chart_scenario_event_membership() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("v_provider", CountingProvider::new(visitors_table()));
    chartml.register_provider("s_provider", CountingProvider::new(sessions_table()));
    chartml.register_transform(DataFusionTransform);
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  visitors:
    provider: v_provider
    datasource: visitors
    query: "SELECT date, n FROM visitors"
  sessions:
    provider: s_provider
    datasource: sessions
    query: "SELECT date, n FROM sessions"
transform:
  sql: |
    SELECT v.date, v.n AS visitors, s.n AS sessions
    FROM visitors v JOIN sessions s USING (date)
visualize:
  type: bar
  columns: date
  rows: visitors
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    let _prepared = chartml.transform(fetched, &opts).await.unwrap();
    flush_pending_hooks().await;

    let events = events.lock().unwrap();

    // ── 1. Exactly two cache misses, one per source ──
    let misses: Vec<&CacheMissEvent> = events
        .iter()
        .filter_map(|e| {
            if let Event::CacheMiss(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        misses.len(),
        2,
        "expected exactly two cache misses (one per source); got: {events:?}"
    );
    let miss_names: std::collections::HashSet<&str> =
        misses.iter().filter_map(|m| m.source_name.as_deref()).collect();
    assert!(miss_names.contains("visitors"), "miss for `visitors` missing");
    assert!(miss_names.contains("sessions"), "miss for `sessions` missing");

    // ── 2. At least one Fetch-phase Progress event per source ──
    let fetch_progress: Vec<&ProgressEvent> = events
        .iter()
        .filter_map(|e| match e {
            Event::Progress(p) if p.phase == Phase::Fetch => Some(p),
            _ => None,
        })
        .collect();
    let fetch_names: std::collections::HashSet<&str> =
        fetch_progress.iter().filter_map(|p| p.source_name.as_deref()).collect();
    assert!(fetch_names.contains("visitors"), "fetch progress for `visitors` missing");
    assert!(fetch_names.contains("sessions"), "fetch progress for `sessions` missing");

    // ── 3. At least one Transform-phase Progress event ──
    let transform_progress: Vec<&ProgressEvent> = events
        .iter()
        .filter_map(|e| match e {
            Event::Progress(p) if p.phase == Phase::Transform => Some(p),
            _ => None,
        })
        .collect();
    assert!(
        !transform_progress.is_empty(),
        "expected at least one Transform-phase progress event; got: {events:?}"
    );
}

/// Helper: build the same `InlineData` shape the resolver hashes for the
/// per-key invalidation tests below. Mirrors what `ChartML::fetch` would
/// produce for the YAML literal we hand it.
fn datasource_inline(slug: &str, query: &str) -> InlineData {
    InlineData {
        provider: None,
        rows: None,
        url: None,
        endpoint: None,
        cache: None,
        datasource: Some(slug.to_string()),
        query: Some(query.to_string()),
    }
}

/// Per-key `Resolver::invalidate(key)` followed by a re-fetch must surface
/// `MissReason::Invalidated` (not `NotFound`). Exercises the
/// `recently_invalidated.keys` path.
#[tokio::test]
async fn test_invalidate_emits_invalidated_miss_reason() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", CountingProvider::new(visitors_table()));
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "60s"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();

    // First fetch primes the cache.
    chartml.fetch(yaml, &opts).await.unwrap();
    // Confirm a hit on second fetch (sanity — sets up the invalidate scenario).
    chartml.fetch(yaml, &opts).await.unwrap();

    // Invalidate the exact resolver key the spec would hash to. We
    // recompute it via `Resolver::key_for` against the same `InlineData`
    // shape `ChartML::fetch` builds internally.
    let inline = datasource_inline("warehouse", "SELECT 1");
    let key = Resolver::key_for(&inline, None);
    chartml.resolver().invalidate(key).await;

    // Drain previously-collected events so the assertion only sees the
    // post-invalidate fetch. Flush FIRST so any in-flight fire-and-forget
    // hook events from the priming fetches above settle before we clear,
    // otherwise their late-arriving emissions would land in the "post"
    // collector and skew the assertion.
    flush_pending_hooks().await;
    events.lock().unwrap().clear();

    chartml.fetch(yaml, &opts).await.unwrap();
    flush_pending_hooks().await;

    {
        let post_invalidate = events.lock().unwrap();
        let misses: Vec<&CacheMissEvent> = post_invalidate
            .iter()
            .filter_map(|e| {
                if let Event::CacheMiss(m) = e {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            misses.len(),
            1,
            "expected exactly one cache miss after the invalidated fetch; got: {post_invalidate:?}"
        );
        assert_eq!(
            misses[0].reason,
            MissReason::Invalidated,
            "post-invalidate miss must report `Invalidated`, not `{:?}`",
            misses[0].reason,
        );
    }

    // Re-fetch a third time: now the cache is repopulated, so we should
    // see a hit (NOT another `Invalidated` miss — the per-key entry was
    // drained on first observation).
    flush_pending_hooks().await;
    events.lock().unwrap().clear();
    chartml.fetch(yaml, &opts).await.unwrap();
    flush_pending_hooks().await;

    let post_refetch = events.lock().unwrap();
    let third_misses: Vec<&CacheMissEvent> = post_refetch
        .iter()
        .filter_map(|e| {
            if let Event::CacheMiss(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    assert!(
        third_misses.is_empty(),
        "third fetch must hit the cache (no further misses); got: {post_refetch:?}"
    );
}

/// Bulk `Resolver::invalidate_by_slug(slug)` followed by a re-fetch must
/// surface `MissReason::Invalidated` for the FIRST post-bulk miss.
/// Subsequent misses fall back to `NotFound` (documented one-shot bulk
/// reporting — see `Resolver::recently_invalidated` field docs).
#[tokio::test]
async fn test_invalidate_by_slug_emits_invalidated() {
    let (hooks, events) = RecordingHooks::new();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", CountingProvider::new(visitors_table()));
    chartml.set_hooks(hooks);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "60s"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    chartml.fetch(yaml, &opts).await.unwrap();

    // Bulk-invalidate the slug — this clears the cache and arms the
    // bulk-pending flag so the very next miss surfaces as `Invalidated`.
    chartml.resolver().invalidate_by_slug("warehouse").await;

    // Flush late-arriving priming-fetch events BEFORE clearing the
    // collector (see test_invalidate_emits_invalidated_miss_reason for
    // the same coordination).
    flush_pending_hooks().await;
    events.lock().unwrap().clear();

    chartml.fetch(yaml, &opts).await.unwrap();
    flush_pending_hooks().await;

    let events_snapshot = events.lock().unwrap();
    let misses: Vec<&CacheMissEvent> = events_snapshot
        .iter()
        .filter_map(|e| {
            if let Event::CacheMiss(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        misses.len(),
        1,
        "expected exactly one cache miss after invalidate_by_slug; got: {events_snapshot:?}"
    );
    assert_eq!(
        misses[0].reason,
        MissReason::Invalidated,
        "first post-bulk-invalidate miss must report `Invalidated`, not `{:?}`",
        misses[0].reason,
    );
}
