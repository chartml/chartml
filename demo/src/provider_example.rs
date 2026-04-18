//! Phase 4 demo page: drive `ChartMLChart` with a registered
//! [`DataSourceProvider`].
//!
//! Three side-by-side cards exercise the new prop surface:
//!
//! 1. **Flat datasource** — `data: { datasource, query }` shape, single
//!    source, no transform. Renders bars from a fixed mock table.
//! 2. **Named-multi + transform** — KYO-79 shape with two named sources
//!    (`visitors`, `sessions`) joined via a SQL transform on `date`.
//! 3. **Auto-refresh** — same flat-datasource spec but with `cache.ttl: 5s`
//!    + `cache.autoRefresh: true`. Shows a "last refreshed at" indicator
//!      plus a manual "Refresh now" button so the difference between
//!      auto-cadence and forced refresh is visible side by side.
//!
//! All three cards share a single `MockProvider` whose `fetch` adds a
//! short artificial latency so the loading state is visible. The provider
//! keys responses off the `query` text, mirroring how a real Kyomi/BigQuery
//! provider would route by SQL — see `MockProvider::fetch` for the table.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_core::data::DataTable;
use chartml_core::resolver::{DataSourceProvider, FetchError, FetchRequest, FetchResult};
use chartml_core::ChartML;
use chartml_datafusion::DataFusionTransform;
use chartml_leptos::{ChartMLChart, ChartMLRef, ProviderRef};
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use serde_json::json;

/// Spec 1: flat `{ datasource, query }` rendered via the mock provider.
const FLAT_SPEC: &str = r##"type: chart
version: 1
title: "Flat datasource — single source"
data:
  datasource: warehouse
  query: SELECT month, revenue FROM monthly_revenue ORDER BY month
visualize:
  type: bar
  columns: month
  rows: revenue
"##;

/// Spec 2: named-multi + transform, the KYO-79 join shape.
const NAMED_MULTI_SPEC: &str = r##"type: chart
version: 1
title: "Named multi-source + SQL join"
data:
  visitors:
    datasource: warehouse
    query: SELECT date, n FROM visitors
  sessions:
    datasource: warehouse
    query: SELECT date, n FROM sessions
transform:
  sql: |
    SELECT visitors.date AS date,
           sessions.n / visitors.n AS sessions_per_visitor
    FROM visitors
    JOIN sessions ON visitors.date = sessions.date
    ORDER BY visitors.date
visualize:
  type: line
  columns: date
  rows: sessions_per_visitor
"##;

/// Spec 3: same flat spec but with sub-minute auto-refresh wired on.
const AUTO_REFRESH_SPEC: &str = r##"type: chart
version: 1
title: "Auto-refresh every 5s"
data:
  datasource: warehouse
  query: SELECT month, revenue FROM monthly_revenue ORDER BY month
  cache:
    ttl: 5s
    autoRefresh: true
visualize:
  type: bar
  columns: month
  rows: revenue
"##;

/// Mock provider that pretends to be a warehouse. Routes by query text and
/// returns deterministic-but-uniquely-jittered numbers per call so the
/// auto-refresh card visibly changes between ticks. Also injects a 200ms
/// artificial delay so the `.chartml-loading` indicator gets a chance to
/// flash.
struct MockProvider;

// Mirror the cfg-gating on the trait declaration: native targets require
// `Send` futures (the resolver may multiplex them across `tokio::spawn`),
// WASM does not (single-threaded; futures stay on the local thread).
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSourceProvider for MockProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError> {
        // 200ms latency simulates a real network round-trip — enough to see
        // the loading state in the UI.
        sleep_ms(200).await;

        let query = request.spec.query.unwrap_or_default();
        let now = js_sys::Date::now();
        // Rotate through a small jitter window keyed off wall-clock time so
        // each refresh produces a visibly different number.
        let jitter = (now / 1_000.0).floor() as u64 % 30;
        let jitter = jitter as f64;

        let rows = if query.contains("monthly_revenue") {
            vec![
                row_pair("month", "Jan", "revenue", 125_000.0 + jitter * 1_000.0),
                row_pair("month", "Feb", "revenue", 138_000.0 + jitter * 1_500.0),
                row_pair("month", "Mar", "revenue", 152_000.0 + jitter * 2_000.0),
                row_pair("month", "Apr", "revenue", 165_000.0 + jitter * 1_750.0),
            ]
        } else if query.contains("FROM visitors") {
            vec![
                date_n_row("2024-01-01", 100.0 + jitter * 5.0),
                date_n_row("2024-01-02", 150.0 + jitter * 5.0),
                date_n_row("2024-01-03", 200.0 + jitter * 5.0),
            ]
        } else if query.contains("FROM sessions") {
            vec![
                date_n_row("2024-01-01", 25.0 + jitter * 1.0),
                date_n_row("2024-01-02", 60.0 + jitter * 1.5),
                date_n_row("2024-01-03", 90.0 + jitter * 2.0),
            ]
        } else {
            return Err(FetchError::QueryFailed(format!(
                "MockProvider has no canned table for query: {query}",
            )));
        };

        let data = DataTable::from_rows(&rows).map_err(|e| {
            FetchError::DecodeFailed(format!("MockProvider row build failed: {e}"))
        })?;

        let mut metadata = HashMap::new();
        metadata.insert("mock_jitter".into(), json!(jitter));
        metadata.insert("mock_query".into(), json!(query));

        Ok(FetchResult { data, metadata })
    }
}

fn row_pair(
    k1: &str,
    v1: &str,
    k2: &str,
    v2: f64,
) -> chartml_core::data::Row {
    [(k1.to_string(), json!(v1)), (k2.to_string(), json!(v2))]
        .into_iter()
        .collect()
}

fn date_n_row(date: &str, n: f64) -> chartml_core::data::Row {
    [
        ("date".to_string(), json!(date)),
        ("n".to_string(), json!(n)),
    ]
    .into_iter()
    .collect()
}

/// `JsFuture`-based `setTimeout(ms)` so the provider can `await` on a real
/// timer. Keeps the demo dep set minimal — no extra `gloo-timers` dep just
/// for one sleep site.
async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Build a `ChartMLRef` with the cartesian renderer + DataFusion transform
/// registered. The named-multi card's SQL join lives in DataFusion; the
/// other two cards don't use a transform but share the same instance for
/// simplicity.
fn build_chartml() -> ChartMLRef {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_transform(DataFusionTransform);
    ChartMLRef::new(c)
}

/// Phase 4 demo page entry point. Mounts three `ChartMLChart`s side by
/// side, each fed the same `MockProvider` instance via the new `provider`
/// prop.
#[component]
pub fn ProviderExamplesPage() -> impl IntoView {
    // One `MockProvider` shared across all three charts. `Arc<dyn ...>`
    // because `ProviderRef` is the chartml-leptos alias.
    let provider: ProviderRef = Arc::new(MockProvider);

    // SendWrapper because Leptos's reactive function bound is `Send`.
    // `wasm32-unknown-unknown` is single-threaded so the wrapper guard is
    // a noop in practice.
    let chartml = SendWrapper::new(build_chartml());

    let flat_spec = RwSignal::new(FLAT_SPEC.to_string());
    let multi_spec = RwSignal::new(NAMED_MULTI_SPEC.to_string());
    let auto_spec = RwSignal::new(AUTO_REFRESH_SPEC.to_string());

    // Manual refresh wiring (DEMO-ONLY workaround).
    //
    // Mutating the spec is the cheapest way to force a re-fetch from
    // outside the component: we append a trailing YAML comment whose text
    // changes each click — the spec stays semantically identical but the
    // spec signal updates, which is what the chart's main effect
    // subscribes to.
    //
    // This is intentionally NOT how production callers should trigger an
    // imperative refresh. `ChartMLChart` does not yet expose a public
    // refresh handle (e.g. an imperative `ChartMLChart::refresh()` method
    // or a context-provided trigger signal). Phase 6 (Kyomi integration)
    // should drive that API extension based on real consumer needs rather
    // than introducing a half-baked surface here just to clean up the
    // demo. Until then, this YAML-mutation trick is fine for the demo
    // page because the chart re-parses cheaply and the auto-refresh
    // effect (after the MAJOR fix) no longer tears its interval down on
    // unrelated signal bumps.
    let manual_refresh = move || {
        let now = js_sys::Date::now() as u64;
        auto_spec.set(format!("{}# manual refresh: {}\n", AUTO_REFRESH_SPEC, now));
    };

    let provider_for_flat = provider.clone();
    let provider_for_multi = provider.clone();
    let provider_for_auto = provider.clone();
    let chartml_for_flat = chartml.clone();
    let chartml_for_multi = chartml.clone();
    let chartml_for_auto = chartml.clone();

    view! {
        <div class="provider-examples">
            <h2 style="margin: 16px 0 8px 0;">"Phase 4 \u{2014} DataSourceProvider integration"</h2>
            <p style="color: #555; max-width: 80ch;">
                "All three charts below are fed by a single mock provider \
                 registered via the new "<code>"provider"</code>" prop on "
                <code>"<ChartMLChart />"</code>". The mock injects a 200ms latency \
                 per fetch so the loading indicator briefly flashes; the \
                 auto-refresh card hits the cache + provider every 5s and \
                 surfaces the timestamp it last resolved successfully."
            </p>
            <div
                class="provider-card-row"
                style="display: grid; grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); gap: 16px; margin-top: 16px;"
            >
                <ProviderCard title="Flat datasource">
                    {
                        let chartml: ChartMLRef = (*chartml_for_flat).clone();
                        let provider = provider_for_flat.clone();
                        view! {
                            <ChartMLChart
                                spec=Signal::derive(move || flat_spec.get())
                                chartml=chartml
                                provider=provider
                            />
                        }
                    }
                </ProviderCard>
                <ProviderCard title="Named-multi + SQL join (KYO-79)">
                    {
                        let chartml: ChartMLRef = (*chartml_for_multi).clone();
                        let provider = provider_for_multi.clone();
                        view! {
                            <ChartMLChart
                                spec=Signal::derive(move || multi_spec.get())
                                chartml=chartml
                                provider=provider
                            />
                        }
                    }
                </ProviderCard>
                <ProviderCard title="Auto-refresh (5s)">
                    <button
                        class="provider-refresh-button"
                        type="button"
                        on:click=move |_| manual_refresh()
                        style="margin-bottom: 8px; padding: 6px 12px; cursor: pointer;"
                    >
                        "Refresh now"
                    </button>
                    <LastRefreshedReadout />
                    {
                        let chartml: ChartMLRef = (*chartml_for_auto).clone();
                        let provider = provider_for_auto.clone();
                        view! {
                            <ChartMLChart
                                spec=Signal::derive(move || auto_spec.get())
                                chartml=chartml
                                provider=provider
                            />
                        }
                    }
                </ProviderCard>
            </div>
        </div>
    }
}

#[component]
fn ProviderCard(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div
            class="provider-card"
            style="background: #fff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 16px;"
        >
            <h3 style="margin: 0 0 12px 0;">{title}</h3>
            {children()}
        </div>
    }
}

/// Small reactive readout that tails the `data-last-refreshed-ms` attribute
/// `ChartMLChart` writes to its container after every successful resolve.
/// Mounted as a sibling of the chart so users can compare the displayed
/// timestamp against the chart's data.
///
/// The 1s polling interval is owned by a leptos `IntervalHandle`, dropped
/// in `on_cleanup` so unmounting the component cancels the timer and
/// releases the JS closure rather than leaking it for the page lifetime.
#[component]
fn LastRefreshedReadout() -> impl IntoView {
    let display = RwSignal::new(String::from("(not yet refreshed)"));

    // Poll every 1s so the readout reflects auto-refresh ticks. 1s
    // matches the auto-refresh cadence of the demo's 5s TTL chart
    // closely enough that the indicator never lags by more than ~1s.
    let tick = move || {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        // Find any `.chartml-container` whose data attribute is set —
        // the auto-refresh chart is the only one writing it within
        // this card's subtree.
        let Some(host) = document
            .query_selector(".provider-card .chartml-container[data-last-refreshed-ms]")
            .ok()
            .flatten()
        else { return };
        let Some(ms_str) = host.get_attribute("data-last-refreshed-ms") else { return };
        let Ok(ms) = ms_str.parse::<f64>() else { return };
        let secs_ago = (js_sys::Date::now() - ms) / 1_000.0;
        display.set(format!("Last refreshed: {:.1}s ago", secs_ago.max(0.0)));
    };

    // `IntervalHandle` is RAII-friendly: `.clear()` cancels the timer and
    // drops the boxed closure. Stashed in a `StoredValue` (local-storage
    // variant — `IntervalHandle` is not `Send + Sync`, but wasm32 is
    // single-threaded so the local-storage arena is correct here) so the
    // cleanup closure can reach it across the function boundary.
    let handle = leptos::prelude::set_interval_with_handle(
        tick,
        std::time::Duration::from_millis(1_000),
    )
    .ok();
    let stored_handle = StoredValue::new_local(handle);

    on_cleanup(move || {
        if let Some(h) = stored_handle.try_update_value(|slot| slot.take()).flatten() {
            h.clear();
        }
    });

    view! {
        <div
            class="last-refreshed"
            style="margin-bottom: 8px; color: #666; font-family: monospace; font-size: 12px;"
        >
            {move || display.get()}
        </div>
    }
}
