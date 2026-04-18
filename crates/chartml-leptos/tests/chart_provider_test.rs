//! Phase 4 browser integration tests for [`chartml_leptos::ChartMLChart`].
//!
//! Each test mounts a `ChartMLChart` against a freshly-created `<div>`,
//! exercises the resolver-side wiring against a `RecordingProvider` that
//! counts inbound calls, and asserts on either the rendered DOM or the
//! provider's recorded state.
//!
//! These tests are browser-only — they use `web_sys::Document`, real
//! `setTimeout`-based sleeps, and the live browser visibility API. Native
//! `cargo test` runs compile this file as an empty module via the
//! top-level `cfg(target_arch = "wasm32")` gate.
//!
//! Drive with: `wasm-pack test --firefox --headless -p chartml-leptos`.
//! (Chrome / Safari work too if their headless drivers are available.)
//!
//! ## Why imperative DOM polling instead of `<Suspense>` assertions
//!
//! `ChartMLChart` drives its loading / error state through plain
//! `RwSignal`s rather than a `<Suspense>` boundary because Leptos 0.8's
//! `Resource` requires `Send` futures and the chartml resolver is `?Send`
//! on `wasm32-unknown-unknown` (its inflight `Shared<LocalBoxFuture<...>>`
//! map is single-threaded). We assert on the same DOM nodes either path
//! produces — `.chartml-loading`, `.chartml-error`, and `.chartml-svg-host`
//! — so the tests don't depend on which control-flow primitive renders
//! them.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_core::data::DataTable;
use chartml_core::resolver::{DataSourceProvider, FetchError, FetchRequest, FetchResult};
use chartml_core::ChartML;
use chartml_leptos::{ChartMLChart, ChartMLRef, ProviderRef};
use leptos::prelude::*;
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ── Test fixtures ────────────────────────────────────────────────────────

/// `DataSourceProvider` impl that records every inbound `fetch` call so
/// tests can assert "this fired N times" without scraping logs. Returns a
/// fixed table cloned from the constructor; the `fail_first` knob makes
/// the first N calls fail with `FetchError::QueryFailed`, used by
/// `test_chart_shows_error_state` to inject a transient failure that the
/// retry button can clear.
///
/// State lives behind `Arc<Mutex<...>>` because `DataSourceProvider` is
/// declared `Send + Sync` on every target — the trait bound holds even on
/// `wasm32-unknown-unknown`. `std::sync::Mutex` works fine on
/// single-threaded WASM (lock acquisition is a no-op when no contention is
/// possible).
#[derive(Clone)]
struct RecordingProvider {
    table: DataTable,
    calls: Arc<Mutex<u32>>,
    fail_first: Option<u32>,
}

impl RecordingProvider {
    fn new(table: DataTable) -> Self {
        Self {
            table,
            calls: Arc::new(Mutex::new(0)),
            fail_first: None,
        }
    }

    fn fail_first(mut self, n: u32) -> Self {
        self.fail_first = Some(n);
        self
    }

    fn calls_handle(&self) -> Arc<Mutex<u32>> {
        self.calls.clone()
    }
}

#[async_trait(?Send)]
impl DataSourceProvider for RecordingProvider {
    async fn fetch(&self, _request: FetchRequest) -> Result<FetchResult, FetchError> {
        let prev = {
            let mut c = self.calls.lock().unwrap();
            let prev = *c;
            *c = c.checked_add(1).unwrap_or(u32::MAX);
            prev
        };
        if let Some(threshold) = self.fail_first {
            if prev < threshold {
                return Err(FetchError::QueryFailed(format!(
                    "synthetic transient failure {} (provider 'recording')",
                    prev + 1,
                )));
            }
        }
        Ok(FetchResult {
            data: self.table.clone(),
            metadata: HashMap::new(),
        })
    }
}

fn sample_table() -> DataTable {
    DataTable::from_rows(&[
        [
            ("month".to_string(), json!("Jan")),
            ("revenue".to_string(), json!(100.0)),
        ]
        .into_iter()
        .collect(),
        [
            ("month".to_string(), json!("Feb")),
            ("revenue".to_string(), json!(200.0)),
        ]
        .into_iter()
        .collect(),
        [
            ("month".to_string(), json!("Mar")),
            ("revenue".to_string(), json!(300.0)),
        ]
        .into_iter()
        .collect(),
    ])
    .expect("build sample table")
}

/// Spec that routes through the registered `"datasource"` provider. The
/// resolver short-circuits via the inline provider when `rows:` is set, so
/// the spec MUST omit `rows` and use the `datasource` shape so our recorder
/// fires.
fn datasource_spec() -> &'static str {
    r#"
type: chart
version: 1
data:
  datasource: recording
  query: SELECT month, revenue FROM sales
visualize:
  type: bar
  columns: month
  rows: revenue
"#
}

/// Variant with `cache.autoRefresh: true` and a sub-second TTL. Tests that
/// poke the auto-refresh loop assert against this spec.
fn datasource_spec_auto_refresh() -> &'static str {
    r#"
type: chart
version: 1
data:
  datasource: recording
  query: SELECT month, revenue FROM sales
  cache:
    ttl: 1s
    autoRefresh: true
visualize:
  type: bar
  columns: month
  rows: revenue
"#
}

/// Build a `ChartMLRef` with the cartesian renderer registered (`bar` /
/// `line` route through it) so the SVG render stage doesn't fail with
/// `UnknownChartType`.
fn build_chartml() -> ChartMLRef {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    ChartMLRef::new(c)
}

/// Build a fresh `<div>` appended to `document.body`, sized so the
/// container's `client_width` resolves to a non-zero value (the chart
/// pipeline gates rendering on `width > 0`).
fn fresh_mount_root() -> web_sys::HtmlDivElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let div = document.create_element("div").unwrap();
    div.set_attribute("style", "width: 800px; height: 400px;")
        .unwrap();
    document.body().unwrap().append_child(&div).unwrap();
    div.dyn_into::<web_sys::HtmlDivElement>().unwrap()
}

/// Sleep that yields back to the JS event loop so `spawn_local` futures
/// scheduled on the microtask queue actually run between assertions. 50ms
/// is empirically enough for the fetch + transform + render pipeline to
/// settle on every browser the workspace exercises (firefox / chromium).
async fn yield_to_event_loop(ms: u32) {
    gloo_timers::future::TimeoutFuture::new(ms).await;
}

/// Find the `<div class="chartml-svg-host">` descendant of `root` and
/// return it once `inner_html` is non-empty. Returns `None` while the
/// chart is still in its loading state OR before the host node has been
/// rendered. Used by every test that needs to assert "the chart actually
/// rendered something".
fn rendered_svg_host(root: &web_sys::Element) -> Option<web_sys::Element> {
    let host = root.query_selector(".chartml-svg-host").ok().flatten()?;
    if host.inner_html().is_empty() {
        None
    } else {
        Some(host)
    }
}

/// Mount `ChartMLChart` into `root` with the supplied provider and YAML.
/// Returns the `UnmountHandle` so the caller can dispose the component on
/// test teardown (preventing reactive owners from leaking across tests).
fn mount_chart_with_provider(
    root: &web_sys::HtmlDivElement,
    chartml: ChartMLRef,
    provider: ProviderRef,
    yaml: &'static str,
) -> leptos::mount::UnmountHandle<
    <leptos::tachys::view::any_view::AnyView as leptos::tachys::view::Render>::State,
> {
    let parent: web_sys::HtmlElement = root.clone().into();
    let spec = RwSignal::new(yaml.to_string());
    leptos::mount::mount_to(parent, move || {
        let chartml = chartml.clone();
        let provider = provider.clone();
        view! {
            <ChartMLChart
                spec=Signal::derive(move || spec.get())
                chartml=chartml
                provider=provider
            />
        }
        .into_any()
    })
}

/// Variant of `mount_chart_with_provider` that also threads a
/// `refresh_trigger` signal through the new prop. Returns the
/// `UnmountHandle` AND the `RwSignal<u32>` the test can bump to drive
/// imperative refreshes.
fn mount_chart_with_refresh_trigger(
    root: &web_sys::HtmlDivElement,
    chartml: ChartMLRef,
    provider: ProviderRef,
    yaml: &'static str,
) -> (
    leptos::mount::UnmountHandle<
        <leptos::tachys::view::any_view::AnyView as leptos::tachys::view::Render>::State,
    >,
    RwSignal<u32>,
) {
    let parent: web_sys::HtmlElement = root.clone().into();
    let spec = RwSignal::new(yaml.to_string());
    let refresh = RwSignal::new(0_u32);
    let handle = leptos::mount::mount_to(parent, move || {
        let chartml = chartml.clone();
        let provider = provider.clone();
        view! {
            <ChartMLChart
                spec=Signal::derive(move || spec.get())
                chartml=chartml
                provider=provider
                refresh_trigger=Signal::derive(move || refresh.get())
            />
        }
        .into_any()
    });
    (handle, refresh)
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// Provider receives the fetch + the rendered SVG appears under
/// `.chartml-svg-host`. Smoke test for the entire wiring.
#[wasm_bindgen_test]
async fn test_chart_fetches_via_provider() {
    let root = fresh_mount_root();
    let provider = RecordingProvider::new(sample_table());
    let calls = provider.calls_handle();
    let provider_ref: ProviderRef = Arc::new(provider);

    let _handle = mount_chart_with_provider(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec(),
    );

    // Allow the resize observer + spawn_local fetch + transform + render
    // to settle. 200ms is well above the 200ms resize debounce ChartMLChart
    // applies after the initial measurement (the initial measurement is
    // un-debounced, so 200ms is comfortable headroom).
    yield_to_event_loop(250).await;

    let host = rendered_svg_host(&root).expect("svg host should be populated");
    assert!(
        host.inner_html().contains("<svg"),
        "rendered host must contain SVG markup, got: {}",
        host.inner_html().chars().take(120).collect::<String>(),
    );
    assert_eq!(
        *calls.lock().unwrap(),
        1,
        "provider must be called exactly once for the initial fetch",
    );
}

/// Loading state mounts a `.chartml-loading` indicator before the async
/// fetch resolves. We snapshot the DOM immediately after mount and assert
/// the loading marker is present, then wait for resolution and assert it's
/// gone.
#[wasm_bindgen_test]
async fn test_chart_shows_loading_state() {
    let root = fresh_mount_root();
    let provider = RecordingProvider::new(sample_table());
    let provider_ref: ProviderRef = Arc::new(provider);

    let _handle = mount_chart_with_provider(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec(),
    );

    // The async fetch hasn't completed yet — we should see the loading
    // marker. Yield only one microtask tick so the resize observer has
    // measured but the spawn_local fetch hasn't resolved.
    yield_to_event_loop(0).await;
    let loading_present = root.query_selector(".chartml-loading").unwrap().is_some();
    assert!(
        loading_present,
        "loading marker must be present immediately after mount",
    );

    // Wait for fetch + transform + render. Loading marker should clear.
    yield_to_event_loop(250).await;
    assert!(
        root.query_selector(".chartml-loading").unwrap().is_none(),
        "loading marker must clear after the async pipeline resolves",
    );
}

/// Failed fetch surfaces `.chartml-error` containing a Retry button. Click
/// the button → provider gets called again → after the second call the
/// error clears and the chart renders.
#[wasm_bindgen_test]
async fn test_chart_shows_error_state_with_retry() {
    let root = fresh_mount_root();
    // Fail the first call, succeed on the second.
    let provider = RecordingProvider::new(sample_table()).fail_first(1);
    let calls = provider.calls_handle();
    let provider_ref: ProviderRef = Arc::new(provider);

    let _handle = mount_chart_with_provider(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec(),
    );

    yield_to_event_loop(250).await;

    // First-attempt failure: error region must be present, retry button
    // must exist, no SVG host yet.
    let error_div = root
        .query_selector(".chartml-error")
        .unwrap()
        .expect("error region must be present after the first failed fetch");
    assert!(
        rendered_svg_host(&root).is_none(),
        "no SVG host should be populated while the chart is in the error state",
    );
    assert_eq!(*calls.lock().unwrap(), 1, "provider was called once before retry");

    let retry_button = error_div
        .query_selector(".chartml-retry-button")
        .unwrap()
        .expect("retry button must exist inside the error region");
    let html_button: web_sys::HtmlElement = retry_button
        .dyn_into::<web_sys::HtmlElement>()
        .expect("button is an HTMLElement");
    html_button.click();

    yield_to_event_loop(250).await;

    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "retry button click must trigger a second provider fetch",
    );
    assert!(
        root.query_selector(".chartml-error").unwrap().is_none(),
        "error region must clear after a successful retry",
    );
    assert!(
        rendered_svg_host(&root).is_some(),
        "SVG host must be populated after the retry succeeds",
    );
}

/// Auto-refresh interval fires on its TTL cadence. With a 1s TTL, the
/// provider should be called 1 (initial) + ≥1 (auto-refresh tick) inside
/// a 2.5s observation window.
#[wasm_bindgen_test]
async fn test_chart_auto_refresh_fires() {
    let root = fresh_mount_root();
    let provider = RecordingProvider::new(sample_table());
    let calls = provider.calls_handle();
    let provider_ref: ProviderRef = Arc::new(provider);

    let _handle = mount_chart_with_provider(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec_auto_refresh(),
    );

    // Initial fetch settles within ~250ms.
    yield_to_event_loop(300).await;
    let initial_calls = *calls.lock().unwrap();
    assert_eq!(
        initial_calls, 1,
        "provider should be called once for the initial fetch (got {initial_calls})",
    );

    // Wait long enough for at least 2 auto-refresh ticks (TTL is 1s so 2.4s
    // should produce at least 2 ticks even with timer jitter).
    yield_to_event_loop(2400).await;

    let final_calls = *calls.lock().unwrap();
    assert!(
        final_calls >= 3,
        "auto-refresh must fire ≥2 times in a 2.4s window with a 1s TTL (initial=1, refreshes≥2; got total={final_calls})",
    );
}

/// Visibility pause: when `document.visibilityState` flips to "hidden" and
/// a `visibilitychange` event fires, the auto-refresh interval must stop
/// firing. After the document becomes "visible" again, the interval re-arms.
///
/// Implementation note: we shadow the `visibilityState` getter with
/// `Object.defineProperty` so production code's `document.visibility_state()`
/// reads our injected value. JS `dispatchEvent(new Event("visibilitychange"))`
/// fires the listener `ChartMLChart` registered on mount.
#[wasm_bindgen_test]
async fn test_chart_auto_refresh_paused_when_hidden() {
    let root = fresh_mount_root();
    let provider = RecordingProvider::new(sample_table());
    let calls = provider.calls_handle();
    let provider_ref: ProviderRef = Arc::new(provider);

    let _handle = mount_chart_with_provider(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec_auto_refresh(),
    );

    // Initial fetch settles.
    yield_to_event_loop(300).await;
    let baseline = *calls.lock().unwrap();
    assert_eq!(baseline, 1, "initial fetch should fire once");

    // Flip visibility to hidden + dispatch the event.
    set_document_visibility("hidden");
    dispatch_visibility_change();

    // Wait through one would-be tick window. Provider call count must NOT
    // change while the document is hidden.
    yield_to_event_loop(1500).await;
    let hidden_calls = *calls.lock().unwrap();
    assert_eq!(
        hidden_calls, baseline,
        "no auto-refresh fetches should fire while the document is hidden \
         (baseline={baseline}, after_hidden={hidden_calls})",
    );

    // Restore visibility + dispatch again. Interval re-arms; another tick
    // should fire within the 1s TTL window.
    set_document_visibility("visible");
    dispatch_visibility_change();

    yield_to_event_loop(1500).await;
    let resumed_calls = *calls.lock().unwrap();
    assert!(
        resumed_calls > hidden_calls,
        "auto-refresh must resume after visibilitychange → 'visible' \
         (after_hidden={hidden_calls}, resumed={resumed_calls})",
    );

    // Tidy up the property override so subsequent tests in the same
    // browser session see the real visibility state.
    restore_document_visibility();
}

/// Imperative `refresh_trigger` prop fires a fresh provider call each
/// time the parent bumps the trigger signal. The first mount also fires
/// once (the initial fetch), so the call count progression is
/// `1 → 2 → 3` across `(initial, bump #1, bump #2)`.
#[wasm_bindgen_test]
async fn test_chart_refresh_trigger_invalidates_and_refetches() {
    let root = fresh_mount_root();
    let provider = RecordingProvider::new(sample_table());
    let calls = provider.calls_handle();
    let provider_ref: ProviderRef = Arc::new(provider);

    let (_handle, refresh) = mount_chart_with_refresh_trigger(
        &root,
        build_chartml(),
        provider_ref,
        datasource_spec(),
    );

    // Initial fetch settles within the same window the smoke test uses.
    yield_to_event_loop(300).await;
    assert_eq!(
        *calls.lock().unwrap(),
        1,
        "initial mount must fetch once before any trigger bump",
    );

    // Bump #1 — the chart should invalidate the resolver key, re-run the
    // fetch effect, and surface a second provider call.
    refresh.update(|c| *c = c.wrapping_add(1));
    yield_to_event_loop(300).await;
    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "first refresh_trigger bump must fire one extra provider call",
    );

    // Bump #2 — same story.
    refresh.update(|c| *c = c.wrapping_add(1));
    yield_to_event_loop(300).await;
    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "second refresh_trigger bump must fire one more provider call",
    );
}

// ── Visibility-state shimming helpers ────────────────────────────────────

#[wasm_bindgen]
extern "C" {
    /// Stub `Object.defineProperty(document, "visibilityState", { get, configurable: true })`.
    /// `value_getter` must be a JS function returning the string we want
    /// `document.visibilityState` to evaluate to.
    #[wasm_bindgen(js_namespace = Object, js_name = defineProperty)]
    fn define_property(target: &JsValue, prop: &str, descriptor: &JsValue) -> JsValue;
}

/// Replace `document.visibilityState` with a getter returning `value` (a
/// string like `"hidden"` or `"visible"`). Re-callable — each call
/// overrides the previous descriptor.
fn set_document_visibility(value: &str) {
    let document = web_sys::window().unwrap().document().unwrap();
    let descriptor = js_sys::Object::new();
    let value_owned = value.to_string();
    let getter = Closure::<dyn Fn() -> JsValue>::new(move || {
        JsValue::from_str(&value_owned)
    });
    js_sys::Reflect::set(
        &descriptor,
        &JsValue::from_str("get"),
        getter.as_ref().unchecked_ref(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &descriptor,
        &JsValue::from_str("configurable"),
        &JsValue::from_bool(true),
    )
    .unwrap();
    let document_value: JsValue = document.into();
    define_property(&document_value, "visibilityState", &descriptor.into());
    // Leak the closure — the test process exits when wasm-bindgen-test
    // finishes, so freeing JS callbacks across many tests would only
    // matter for very long-running suites. Simpler than threading a
    // stash through every assertion.
    getter.forget();
}

/// Restore the platform default by removing our shadowing descriptor.
/// Browsers' built-in `visibilityState` lives on the `Document.prototype`
/// chain, so deleting our own-property descriptor exposes it again.
fn restore_document_visibility() {
    let document = web_sys::window().unwrap().document().unwrap();
    let document_value: JsValue = document.into();
    let _ = js_sys::Reflect::delete_property(
        &document_value.dyn_into::<js_sys::Object>().unwrap(),
        &JsValue::from_str("visibilityState"),
    );
}

/// Dispatch a synthetic `visibilitychange` event so the listener
/// `ChartMLChart` registered on mount sees the (already-shimmed)
/// `visibilityState` change.
fn dispatch_visibility_change() {
    let document = web_sys::window().unwrap().document().unwrap();
    let event = web_sys::Event::new("visibilitychange").unwrap();
    document.dispatch_event(&event).unwrap();
}
