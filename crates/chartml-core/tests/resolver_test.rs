//! Integration tests for chartml 5.0 phase 3 — `DataSourceProvider` trait,
//! resolver (cache + dedup + dispatch), built-in `InlineProvider` /
//! `HttpProvider`, and `ChartML::fetch` upgrades.
//!
//! The tests cover the design doc's "Phase 3: Tests to add" section line by
//! line, plus an end-to-end render against a mock provider mirroring the
//! KYO-79 multi-source-with-join shape.
//!
//! Native-only — uses the multi-threaded `tokio` runtime + `wiremock` HTTP
//! fixtures, neither of which compiles on `wasm32-unknown-unknown`. The
//! browser story is exercised by `tests/indexeddb_test.rs` via
//! `wasm-bindgen-test`.

#![cfg(not(target_arch = "wasm32"))]

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
    CacheBackend, CacheError, CachedEntry, DataSourceProvider, FetchError, FetchRequest,
    FetchResult, MemoryBackend,
};
use chartml_core::{ChartML, RenderOptions};
use chartml_datafusion::DataFusionTransform;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Test fixtures ────────────────────────────────────────────────────────

fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn visitors_table() -> DataTable {
    DataTable::from_rows(&[
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(100.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(150.0))]),
        make_row(vec![("date", json!("2024-01-03")), ("n", json!(200.0))]),
    ])
    .unwrap()
}

fn sessions_table() -> DataTable {
    DataTable::from_rows(&[
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(10.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(15.0))]),
        make_row(vec![("date", json!("2024-01-03")), ("n", json!(20.0))]),
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

struct CapturingRenderer {
    captured: Arc<Mutex<Option<DataTable>>>,
}

impl ChartRenderer for CapturingRenderer {
    fn render(&self, data: &DataTable, _config: &ChartConfig) -> Result<ChartElement, ChartError> {
        *self.captured.lock().unwrap() = Some(data.clone());
        Ok(ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "captured".to_string(),
            children: vec![],
        })
    }
}

/// Mock provider that returns a fixed table and counts how many times its
/// `fetch` was called. Used to prove cache hits / dedup / invalidation.
struct CountingProvider {
    table: DataTable,
    calls: Arc<AtomicU32>,
    /// Optional artificial delay so the dedup test can prove parallel
    /// requests collapse to one upstream call.
    delay: Option<Duration>,
    /// Optional metadata returned with every fetch; used by
    /// `test_fetch_result_metadata_passthrough`.
    metadata: HashMap<String, serde_json::Value>,
}

impl CountingProvider {
    fn new(table: DataTable) -> Self {
        Self {
            table,
            calls: Arc::new(AtomicU32::new(0)),
            delay: None,
            metadata: HashMap::new(),
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = metadata;
        self
    }

    fn calls(&self) -> Arc<AtomicU32> {
        self.calls.clone()
    }
}

#[async_trait]
impl DataSourceProvider for CountingProvider {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        Ok(FetchResult {
            data: self.table.clone(),
            metadata: self.metadata.clone(),
        })
    }
}

/// Provider that captures the request it received so tests can assert
/// dispatch behavior (which kind, which spec, which headers).
struct RecordingProvider {
    last_request: Arc<Mutex<Option<FetchRequest>>>,
    table: DataTable,
}

impl RecordingProvider {
    fn new(table: DataTable) -> (Self, Arc<Mutex<Option<FetchRequest>>>) {
        let last = Arc::new(Mutex::new(None));
        (
            Self {
                last_request: last.clone(),
                table,
            },
            last,
        )
    }
}

#[async_trait]
impl DataSourceProvider for RecordingProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError> {
        *self.last_request.lock().unwrap() = Some(request);
        Ok(FetchResult {
            data: self.table.clone(),
            metadata: HashMap::new(),
        })
    }
}

/// Provider that always errors. Used to test error isolation in NamedMap.
struct FailingProvider {
    message: String,
}

#[async_trait]
impl DataSourceProvider for FailingProvider {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        Err(FetchError::QueryFailed(self.message.clone()))
    }
}

/// Provider that increments a shutdown counter when `shutdown()` is called.
struct ShutdownCounter {
    count: Arc<AtomicU32>,
    table: DataTable,
}

#[async_trait]
impl DataSourceProvider for ShutdownCounter {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        Ok(FetchResult {
            data: self.table.clone(),
            metadata: HashMap::new(),
        })
    }
    async fn shutdown(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_inline_provider() {
    let provider = CountingProvider::new(visitors_table());
    let calls_handle = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    // Override the built-in inline provider with our counter so we can
    // observe dispatch.
    chartml.register_provider("inline", provider);

    let yaml = r#"
type: chart
version: 1
data:
  rows:
    - { date: "2024-01-01", n: 100 }
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(fetched.sources.len(), 1);
    assert!(fetched.sources.contains_key("source"));
    assert_eq!(calls_handle.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_dispatch_http_provider() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"[{"x":"A","y":1},{"x":"B","y":2}]"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);

    let yaml = format!(
        r#"
type: chart
version: 1
data:
  url: "{}/data"
visualize:
  type: bar
  columns: x
  rows: y
"#,
        server.uri(),
    );
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(&yaml, &opts).await.unwrap();
    assert_eq!(fetched.sources["source"].num_rows(), 2);
    assert_eq!(fetched.metadata.cache_misses, vec!["source".to_string()]);
}

#[tokio::test]
async fn test_dispatch_datasource_provider() {
    let (provider, last_request) = RecordingProvider::new(visitors_table());
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT date, n FROM visitors"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(fetched.sources["source"].num_rows(), 3);

    let recorded = last_request.lock().unwrap().clone().expect("provider was called");
    assert_eq!(recorded.spec.datasource.as_deref(), Some("warehouse"));
    assert_eq!(
        recorded.spec.query.as_deref(),
        Some("SELECT date, n FROM visitors"),
    );
}

#[tokio::test]
async fn test_dispatch_named_map() {
    // Three providers, each delays 80 ms. If they ran sequentially the
    // total would be ~240 ms; in parallel via try_join_all it should be
    // ~80 ms. Allow generous slack so the test isn't flaky on slow CI.
    let p1 = CountingProvider::new(visitors_table()).with_delay(Duration::from_millis(80));
    let p2 = CountingProvider::new(visitors_table()).with_delay(Duration::from_millis(80));
    let p3 = CountingProvider::new(visitors_table()).with_delay(Duration::from_millis(80));
    let c1 = p1.calls();
    let c2 = p2.calls();
    let c3 = p3.calls();

    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("p1", p1);
    chartml.register_provider("p2", p2);
    chartml.register_provider("p3", p3);

    let yaml = r#"
type: chart
version: 1
data:
  a:
    provider: p1
    datasource: src_a
  b:
    provider: p2
    datasource: src_b
  c:
    provider: p3
    datasource: src_c
transform:
  sql: "SELECT * FROM a"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let start = Instant::now();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(fetched.sources.len(), 3);
    assert_eq!(c1.load(Ordering::SeqCst), 1);
    assert_eq!(c2.load(Ordering::SeqCst), 1);
    assert_eq!(c3.load(Ordering::SeqCst), 1);
    assert!(
        elapsed < Duration::from_millis(200),
        "Multi-source fetch must run in parallel; took {:?} (each provider delays 80ms — sequential would be ~240ms)",
        elapsed,
    );
}

#[tokio::test]
async fn test_normalization_unnamed_transform() {
    // Spec uses flat datasource shape PLUS a transform — should normalize
    // to NamedMap { "source": flat } so transform middleware can reference
    // FROM source.
    let provider = RecordingProvider::new(visitors_table()).0;
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);
    chartml.register_transform(DataFusionTransform);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT * FROM visitors"
transform:
  sql: "SELECT date, n FROM source ORDER BY date"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    // After normalization the source map has exactly one entry, keyed
    // "source" — proving the rewrite happened.
    assert_eq!(fetched.sources.len(), 1);
    assert!(fetched.sources.contains_key("source"));

    // And the SQL transform must succeed against `FROM source`.
    let prepared = chartml.transform(fetched, &opts).await.unwrap();
    assert_eq!(prepared.data.num_rows(), 3);
}

#[tokio::test]
async fn test_cache_hit_memory_tier() {
    let provider = CountingProvider::new(visitors_table());
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

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

    let first = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(first.metadata.cache_misses, vec!["source".to_string()]);
    assert!(first.metadata.cache_hits.is_empty());

    let second = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(second.metadata.cache_hits, vec!["source".to_string()]);
    assert!(second.metadata.cache_misses.is_empty());

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "Provider must be called exactly once across two fetches with same cache key"
    );
}

#[tokio::test]
async fn test_cache_expiry() {
    let provider = CountingProvider::new(visitors_table());
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "10ms"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "After TTL expiry the second fetch must call the provider again"
    );
}

#[tokio::test]
async fn test_invalid_ttl_errors() {
    // A malformed TTL string (humantime requires a leading number, so
    // "five minutes" fails with NumberExpected) must surface as an
    // InvalidSpec error rather than silently falling back to DEFAULT_TTL.
    // Operators have no way to detect a typo otherwise.
    let provider = CountingProvider::new(visitors_table());
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

    let yaml = r#"
type: chart
version: 1
data:
  datasource: warehouse
  query: "SELECT 1"
  cache:
    ttl: "five minutes"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let err = chartml
        .fetch(yaml, &opts)
        .await
        .expect_err("malformed cache.ttl must surface as an error");
    let msg = err.to_string();
    assert!(
        msg.contains("cache.ttl"),
        "error must name the offending field: {msg}"
    );
    assert!(
        msg.contains("five minutes"),
        "error must include the offending value verbatim: {msg}"
    );
    assert!(
        matches!(err, ChartError::InvalidSpec(_)),
        "expected InvalidSpec, got {err:?}"
    );
}

#[tokio::test]
async fn test_inflight_dedup() {
    // Two concurrent fetches for the same key with a 100ms provider delay.
    // Provider must be called exactly once — both awaiters share the same
    // Shared<Future>.
    let provider = CountingProvider::new(visitors_table()).with_delay(Duration::from_millis(100));
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

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
    let chartml = Arc::new(chartml);

    let opts = RenderOptions::default();
    let c1 = chartml.clone();
    let yaml1 = yaml.to_string();
    let h1 = tokio::spawn(async move {
        let _ = c1.fetch(&yaml1, &opts).await.unwrap();
    });
    let c2 = chartml.clone();
    let yaml2 = yaml.to_string();
    let opts2 = RenderOptions::default();
    let h2 = tokio::spawn(async move {
        let _ = c2.fetch(&yaml2, &opts2).await.unwrap();
    });

    h1.await.unwrap();
    h2.await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "Concurrent same-key fetches must dedup to a single provider call"
    );
}

#[tokio::test]
async fn test_invalidate_single() {
    let provider = CountingProvider::new(visitors_table());
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

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
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Compute the key and invalidate it.
    let inline = chartml_core::spec::InlineData {
        provider: None,
        rows: None,
        url: None,
        endpoint: None,
        cache: Some(chartml_core::spec::source::CacheConfig {
            ttl: Some("60s".into()),
            auto_refresh: None,
        }),
        datasource: Some("warehouse".into()),
        query: Some("SELECT 1".into()),
    };
    let key = chartml_core::resolver::Resolver::key_for(&inline, None);
    chartml.resolver().invalidate(key).await;

    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "After explicit invalidate the next fetch must re-call the provider"
    );
}

#[tokio::test]
async fn test_invalidate_by_slug() {
    let p_foo = CountingProvider::new(visitors_table());
    let p_bar = CountingProvider::new(sessions_table());
    let foo_calls = p_foo.calls();
    let bar_calls = p_bar.calls();

    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("foo", p_foo);
    chartml.register_provider("bar", p_bar);
    chartml.register_transform(DataFusionTransform);

    // Two foo entries (different queries → different keys) and one bar entry.
    let yaml = r#"
type: chart
version: 1
data:
  a:
    provider: foo
    datasource: foo
    query: "q1"
  b:
    provider: foo
    datasource: foo
    query: "q2"
  c:
    provider: bar
    datasource: bar
    query: "q3"
transform:
  sql: "SELECT * FROM a"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(foo_calls.load(Ordering::SeqCst), 2);
    assert_eq!(bar_calls.load(Ordering::SeqCst), 1);

    chartml.resolver().invalidate_by_slug("foo").await;
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    // foo entries re-fetched (2 more calls); bar entry stayed cached.
    assert_eq!(foo_calls.load(Ordering::SeqCst), 4);
    assert_eq!(bar_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_invalidate_all() {
    let p = CountingProvider::new(visitors_table());
    let calls = p.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", p);

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
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    chartml.resolver().invalidate_all().await;
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_multi_source_no_transform_error() {
    let p1 = CountingProvider::new(visitors_table());
    let p2 = CountingProvider::new(sessions_table());
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("p1", p1);
    chartml.register_provider("p2", p2);

    let yaml = r#"
type: chart
version: 1
data:
  visitors:
    provider: p1
    datasource: visitors
  sessions:
    provider: p2
    datasource: sessions
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    let err = chartml.transform(fetched, &opts).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Named data sources require a transform block when multiple sources are defined"),
        "Multi-source no-transform error must match the React wording; got: {msg}"
    );
}

#[tokio::test]
async fn test_single_source_passthrough() {
    let p = CountingProvider::new(visitors_table());
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", p);

    let yaml = r#"
type: chart
version: 1
data:
  visitors:
    datasource: warehouse
    query: "SELECT 1"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    let prepared = chartml.transform(fetched, &opts).await.unwrap();
    assert!(!prepared.metadata.transform_applied);
    assert_eq!(prepared.data.num_rows(), 3);
    assert_eq!(prepared.metadata.sources_used, vec!["visitors".to_string()]);
}

#[tokio::test]
async fn test_http_provider_default_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/auth"))
        .and(header("Authorization", "Bearer X"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"[{"x":1}]"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    // Override built-in `http` with one that injects the Authorization header.
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer X".to_string());
    let provider = chartml_core::HttpProvider::new().with_default_headers(headers);

    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("http", provider);

    let yaml = format!(
        r#"
type: chart
version: 1
data:
  url: "{}/auth"
visualize:
  type: bar
  columns: x
  rows: x
"#,
        server.uri()
    );
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(&yaml, &opts).await.unwrap();
    assert_eq!(fetched.sources["source"].num_rows(), 1);
}

#[tokio::test]
async fn test_http_provider_request_headers_override() {
    let server = MockServer::start().await;
    // Wiremock asserts the Authorization is the per-request override value.
    Mock::given(method("GET"))
        .and(path("/override"))
        .and(header("Authorization", "Bearer OVERRIDE"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"[{"x":1}]"#)
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    // Default header would be "Bearer DEFAULT" — the per-request override
    // ("Bearer OVERRIDE") must win.
    let mut defaults = HashMap::new();
    defaults.insert("Authorization".to_string(), "Bearer DEFAULT".to_string());
    let provider = chartml_core::HttpProvider::new().with_default_headers(defaults);

    // Drive the override directly through the provider's `fetch` —
    // `ChartML::fetch` doesn't expose a per-source header override in
    // phase 3; per-request headers are populated by upstream wrappers.
    let mut request = FetchRequest {
        source_name: None,
        spec: chartml_core::spec::InlineData {
            provider: None,
            rows: None,
            url: Some(format!("{}/override", server.uri())),
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        },
        cache: None,
        headers: HashMap::new(),
        namespace: None,
        cancel_token: None,
    };
    request
        .headers
        .insert("Authorization".to_string(), "Bearer OVERRIDE".to_string());

    let result = provider.fetch(request).await.unwrap();
    assert_eq!(result.data.num_rows(), 1);
}

#[tokio::test]
async fn test_fetch_error_isolation() {
    let good = CountingProvider::new(visitors_table());
    let failing = FailingProvider {
        message: "synthetic upstream failure".to_string(),
    };
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("good", good);
    chartml.register_provider("failing", failing);

    let yaml = r#"
type: chart
version: 1
data:
  ok:
    provider: good
    datasource: ok
  bad:
    provider: failing
    datasource: bad
transform:
  sql: "SELECT * FROM ok"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let err = chartml.fetch(yaml, &opts).await.unwrap_err();
    let msg = err.to_string();
    // The error message must identify which source failed (phase 3c will
    // also fire per-source ErrorEvents through ResolverHooks).
    assert!(
        msg.contains("bad"),
        "Error must identify the failing source; got: {msg}"
    );
}

#[tokio::test]
async fn test_shutdown_invokes_providers() {
    let count = Arc::new(AtomicU32::new(0));
    let provider = ShutdownCounter {
        count: count.clone(),
        table: visitors_table(),
    };
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

    chartml.shutdown().await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_fetch_result_metadata_passthrough() {
    let mut metadata = HashMap::new();
    metadata.insert("bytes_billed".to_string(), json!(12345));
    let provider = CountingProvider::new(visitors_table()).with_metadata(metadata);

    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

    let yaml = r#"
type: chart
version: 1
data:
  warehouse:
    datasource: warehouse
    query: "SELECT 1"
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    let warehouse_meta = fetched
        .metadata
        .per_source
        .get("warehouse")
        .expect("per_source must contain the named source");
    assert_eq!(
        warehouse_meta.get("bytes_billed").and_then(|v| v.as_i64()),
        Some(12345),
    );
}

/// End-to-end: full KYO-79-shape spec — multi-named NamedMap with a SQL
/// join — renders to SVG via `render_to_svg_async` against mock providers.
#[tokio::test]
async fn test_end_to_end_named_map_sql_join() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let visitors = CountingProvider::new(visitors_table());
    let sessions = CountingProvider::new(sessions_table());

    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CapturingRenderer {
            captured: captured.clone(),
        },
    );
    chartml.register_provider("visitors_provider", visitors);
    chartml.register_provider("sessions_provider", sessions);
    chartml.register_transform(DataFusionTransform);

    let yaml = r#"
type: chart
version: 1
title: Visitors and Sessions
data:
  visitors:
    provider: visitors_provider
    datasource: visitors
    query: "SELECT date, n FROM visitors"
  sessions:
    provider: sessions_provider
    datasource: sessions
    query: "SELECT date, n FROM sessions"
transform:
  sql: |
    SELECT v.date, v.n AS visitors, s.n AS sessions
    FROM visitors v JOIN sessions s USING (date)
    ORDER BY v.date
visualize:
  type: bar
  columns: date
  rows: visitors
"#;
    let opts = RenderOptions::default();
    let svg = chartml.render_to_svg_async(yaml, &opts).await.unwrap();
    assert!(svg.starts_with("<svg"));
    let table = captured.lock().unwrap();
    let table = table.as_ref().expect("renderer must have received joined data");
    assert_eq!(table.num_rows(), 3);
}

// ── Pre-registered fast path coverage ────────────────────────────────────

/// `register_source` paths must continue to work — pre-registered named
/// sources skip the resolver entirely.
#[tokio::test]
async fn test_pre_registered_named_fast_path() {
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    // Provider that would error if dispatched — proves we never hit it.
    chartml.register_provider(
        "datasource",
        FailingProvider {
            message: "should not be called".into(),
        },
    );
    chartml.register_source("visitors", visitors_table());

    let yaml = r#"
type: chart
version: 1
data: visitors
visualize:
  type: bar
  columns: date
  rows: n
"#;
    let opts = RenderOptions::default();
    let fetched = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(fetched.sources["visitors"].num_rows(), 3);
    // Pre-registered sources are NOT counted as cache hits or misses —
    // they never touched the resolver.
    assert!(fetched.metadata.cache_hits.is_empty());
    assert!(fetched.metadata.cache_misses.is_empty());
}

/// NamedMap entries that match a pre-registered name use the registered
/// table; other entries route through the resolver.
#[tokio::test]
async fn test_named_map_mixed_registered_and_provider() {
    let provider = CountingProvider::new(sessions_table());
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);
    chartml.register_transform(DataFusionTransform);
    // Pre-register `visitors` — `sessions` falls through to the provider.
    chartml.register_source("visitors", visitors_table());

    let yaml = r#"
type: chart
version: 1
data:
  visitors:
    rows: []
  sessions:
    datasource: warehouse
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
    assert_eq!(fetched.sources["visitors"].num_rows(), 3);
    assert_eq!(fetched.sources["sessions"].num_rows(), 3);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "Provider must be called for `sessions` only (visitors is pre-registered)"
    );
}

// ── Cache backend swap ───────────────────────────────────────────────────

/// Custom cache backend that records every `put` so we can prove
/// `set_cache` actually swaps the tier-1 backend.
struct RecordingCache {
    inner: MemoryBackend,
    puts: Arc<AtomicU32>,
}

#[async_trait]
impl CacheBackend for RecordingCache {
    async fn get(&self, key: u64) -> Option<CachedEntry> {
        self.inner.get(key).await
    }
    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put(key, entry).await
    }
    async fn invalidate(&self, key: u64) -> Result<(), CacheError> {
        self.inner.invalidate(key).await
    }
    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError> {
        self.inner.invalidate_by_tag(tag).await
    }
    async fn clear(&self) -> Result<(), CacheError> {
        self.inner.clear().await
    }
}

#[tokio::test]
async fn test_set_cache_swaps_backend() {
    let puts = Arc::new(AtomicU32::new(0));
    let backend = RecordingCache {
        inner: MemoryBackend::new(),
        puts: puts.clone(),
    };
    let provider = CountingProvider::new(visitors_table());
    let mut chartml = ChartML::new()
        .with_cache(backend);
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

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
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(
        puts.load(Ordering::SeqCst),
        1,
        "Custom cache backend must receive the post-fetch put"
    );
}

// ── Namespace ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_namespace_isolates_cache_keys() {
    let provider = CountingProvider::new(visitors_table());
    let calls = provider.calls();
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", MockRenderer);
    chartml.register_provider("datasource", provider);

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
    chartml.set_namespace("tenant-a");
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    chartml.set_namespace("tenant-b");
    let _ = chartml.fetch(yaml, &opts).await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "Two namespaces with the same query must produce two distinct cache keys"
    );
}
