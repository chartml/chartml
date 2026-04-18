//! Phase 2: three-stage rendering pipeline (`fetch` → `transform` →
//! `render_prepared_to_svg`) plus the `render_to_svg_async` convenience.
//!
//! These tests exercise the pipeline in isolation from any provider trait
//! (added in phase 3) — every source is either inline in the YAML or
//! pre-registered via `ChartML::register_source`. The legacy async entry
//! point `render_from_yaml_with_params_async` is also smoke-tested to prove
//! the back-compat shim still produces a `ChartElement` from the same
//! pipeline.

use chartml_core::data::{DataTable, Row};
use chartml_core::element::{ChartElement, Dimensions, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::VisualizeSpec;
use chartml_core::{ChartML, FetchedChart, PreparedChart, RenderOptions};
use chartml_datafusion::DataFusionTransform;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
    pairs
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
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

/// Mock renderer that always emits a known SVG envelope and counts how many
/// times it was invoked. Used by `test_three_stage_cached_resize` to prove
/// `render_prepared_to_svg` does no upstream data work between calls.
struct CountingRenderer {
    calls: Arc<Mutex<u32>>,
}

impl ChartRenderer for CountingRenderer {
    fn render(&self, _data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        *self.calls.lock().unwrap() += 1;
        Ok(ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
            width: Some(config.width),
            height: Some(config.height),
            class: "mock".to_string(),
            children: vec![],
        })
    }
}

/// Mock renderer with a non-default `default_dimensions()` (mirrors the
/// metric chart's 150 px short-card layout) AND a non-`<svg>` root element
/// (mirrors the metric chart's `Div` card). The non-SVG root forces
/// `element_to_svg` to wrap the output in an `<svg>` envelope using the
/// dimensions passed in — which is the precise pathway where the deleted
/// `svg_dimensions` fallback used to hardcode 400 px.
///
/// Used by `test_render_prepared_to_svg_honors_renderer_default_dimensions`
/// to prove the SVG envelope tracks `default_dimensions()` end-to-end.
struct ShortDefaultRenderer {
    height: f64,
}

impl ChartRenderer for ShortDefaultRenderer {
    fn render(&self, _data: &DataTable, _config: &ChartConfig) -> Result<ChartElement, ChartError> {
        // Non-SVG root → `element_to_svg` will wrap it in an `<svg>` envelope
        // sized from the dimensions argument. This is the codepath the bug
        // affected: a metric chart's `Div` card got wrapped at 400 px.
        Ok(ChartElement::Group {
            class: "short-card".to_string(),
            transform: None,
            children: vec![],
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(self.height))
    }
}

/// Captures the data table the renderer received so transform tests can
/// assert table identity (not just success).
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

const SINGLE_INLINE_YAML: &str = r#"
type: chart
version: 1
title: Inline single source
data:
  provider: inline
  rows:
    - { x: "A", y: 10 }
    - { x: "B", y: 20 }
visualize:
  type: bar
  columns: x
  rows: y
"#;

const MULTI_NAMED_MAP_YAML: &str = r#"
type: chart
version: 1
title: Visitors and Sessions
data:
  visitors:
    rows: []
  sessions:
    rows: []
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

const MULTI_NAMED_MAP_NO_TRANSFORM_YAML: &str = r#"
type: chart
version: 1
title: No transform
data:
  visitors:
    rows: []
  sessions:
    rows: []
visualize:
  type: bar
  columns: date
  rows: n
"#;

#[tokio::test]
async fn test_fetch_stage_single_source() {
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: Arc::new(Mutex::new(0)),
        },
    );

    let opts = RenderOptions::default();
    let fetched: FetchedChart = chartml
        .fetch(SINGLE_INLINE_YAML, &opts)
        .await
        .expect("fetch should succeed for inline source");

    assert_eq!(
        fetched.sources.len(),
        1,
        "Single inline source should produce a 1-entry sources map",
    );
    assert!(
        fetched.sources.contains_key("source"),
        "Inline source must be keyed under the canonical \"source\" name; \
         got keys: {:?}",
        fetched.sources.keys().collect::<Vec<_>>(),
    );
    assert_eq!(
        fetched.sources["source"].num_rows(),
        2,
        "Inline rows should be materialized into the source table",
    );
    // Phase 2: per_source stays empty until phase 3 adds providers.
    assert!(fetched.metadata.per_source.is_empty());
}

#[tokio::test]
async fn test_fetch_stage_named_map() {
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: Arc::new(Mutex::new(0)),
        },
    );
    chartml.register_source("visitors", visitors_table());
    chartml.register_source("sessions", sessions_table());

    let opts = RenderOptions::default();
    let fetched = chartml
        .fetch(MULTI_NAMED_MAP_YAML, &opts)
        .await
        .expect("fetch should succeed when both sources are pre-registered");

    let keys: Vec<String> = fetched.sources.keys().cloned().collect();
    assert_eq!(
        keys,
        vec!["visitors".to_string(), "sessions".to_string()],
        "NamedMap fetch must preserve YAML insertion order",
    );
    assert_eq!(fetched.sources["visitors"].num_rows(), 3);
    assert_eq!(fetched.sources["sessions"].num_rows(), 3);
}

#[tokio::test]
async fn test_transform_passthrough() {
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: Arc::new(Mutex::new(0)),
        },
    );

    let opts = RenderOptions::default();
    let fetched = chartml.fetch(SINGLE_INLINE_YAML, &opts).await.unwrap();

    // Snapshot the source table BEFORE transform so we can prove identity.
    let original = fetched.sources["source"].clone();

    let prepared: PreparedChart = chartml
        .transform(fetched, &opts)
        .await
        .expect("Single-source-no-transform must passthrough");

    assert!(
        !prepared.metadata.transform_applied,
        "Passthrough must NOT mark transform_applied",
    );
    assert_eq!(prepared.metadata.sources_used, vec!["source".to_string()]);
    assert_eq!(
        prepared.data.num_rows(),
        original.num_rows(),
        "Passthrough data must equal the source table row-for-row",
    );
    // Field schemas must match exactly — passthrough returns the source as-is.
    fn field_names(t: &DataTable) -> Vec<String> {
        t.schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }
    assert_eq!(
        field_names(&prepared.data),
        field_names(&original),
        "Passthrough must preserve the original column ordering",
    );
}

#[tokio::test]
async fn test_transform_multi_no_transform_error() {
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: Arc::new(Mutex::new(0)),
        },
    );
    chartml.register_source("visitors", visitors_table());
    chartml.register_source("sessions", sessions_table());

    let opts = RenderOptions::default();
    let fetched = chartml
        .fetch(MULTI_NAMED_MAP_NO_TRANSFORM_YAML, &opts)
        .await
        .unwrap();

    let err = chartml
        .transform(fetched, &opts)
        .await
        .expect_err("Multi-source without transform must error");

    let msg = err.to_string();
    assert!(
        msg.contains("Named data sources require a transform block when multiple sources are defined"),
        "Error must begin with the React-matching wording (extra source-count context is appended); got: {}",
        msg,
    );
}

#[tokio::test]
async fn test_render_prepared_to_svg() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CapturingRenderer {
            captured: captured.clone(),
        },
    );

    let opts = RenderOptions::default();
    let fetched = chartml.fetch(SINGLE_INLINE_YAML, &opts).await.unwrap();
    let prepared = chartml.transform(fetched, &opts).await.unwrap();

    let svg = chartml
        .render_prepared_to_svg(&prepared, &opts)
        .expect("render_prepared_to_svg must succeed for a valid PreparedChart");

    assert!(
        svg.starts_with("<svg"),
        "render_prepared_to_svg must produce an <svg> string; got: {}",
        &svg[..svg.len().min(80)],
    );
    assert!(svg.contains("</svg>"));
    assert!(
        captured.lock().unwrap().is_some(),
        "The renderer should have been called with the prepared data",
    );
}

/// Regression: `render_prepared_to_svg` must use the renderer's
/// `default_dimensions()` for the SVG envelope when the spec sets no
/// `style.height` and the caller passes no override. Previously the SVG
/// envelope dimensions were resolved by a duplicate calculation that
/// hardcoded 400 px — leaving a metric chart (150 px renderer-default) with
/// 250 px of empty viewBox below the content. After refactoring,
/// `build_and_render` returns its resolved width/height and
/// `render_prepared_to_svg` uses those exact numbers.
#[tokio::test]
async fn test_render_prepared_to_svg_honors_renderer_default_dimensions() {
    let mut chartml = ChartML::new();
    chartml.register_renderer("metric", ShortDefaultRenderer { height: 150.0 });

    // Spec deliberately omits `style.height` so the renderer's
    // `default_dimensions()` is the only source of the 150 px height.
    const METRIC_YAML: &str = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - { current: 1234, previous: 1100 }
visualize:
  type: metric
  value: current
  compareWith: previous
"#;

    let opts = RenderOptions::default();
    let fetched = chartml.fetch(METRIC_YAML, &opts).await.unwrap();
    let prepared = chartml.transform(fetched, &opts).await.unwrap();
    let svg = chartml
        .render_prepared_to_svg(&prepared, &opts)
        .expect("render_prepared_to_svg must succeed for the metric spec");

    // Both the viewBox AND the height attribute must be 150 — proving the
    // SVG envelope picked up the renderer's `default_dimensions()` (not the
    // 400 px hardcoded fallback that the deleted `svg_dimensions` used).
    assert!(
        svg.contains(r#"viewBox="0 0 800 150""#),
        "SVG viewBox must be 800x150 (renderer default), not 800x400; got: {}",
        &svg[..svg.len().min(200)],
    );
    assert!(
        svg.contains(r#"height="150""#),
        "SVG height attribute must be 150 (renderer default); got: {}",
        &svg[..svg.len().min(200)],
    );
    assert!(
        !svg.contains(r#"height="400""#),
        "SVG must NOT carry the 400 px fallback; got: {}",
        &svg[..svg.len().min(200)],
    );
}

/// Cached resize: fetch + transform once, then render at three different
/// widths. The renderer is called three times (once per render), but
/// neither `fetch` nor `transform` runs again — proven by reusing the same
/// `PreparedChart` value across all three calls without touching the
/// pre-registered sources between calls.
#[tokio::test]
async fn test_three_stage_cached_resize() {
    let calls = Arc::new(Mutex::new(0));
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: calls.clone(),
        },
    );

    let base_opts = RenderOptions::default();
    let fetched = chartml.fetch(SINGLE_INLINE_YAML, &base_opts).await.unwrap();
    let prepared = chartml.transform(fetched, &base_opts).await.unwrap();

    // Three resizes from the SAME prepared chart — no re-fetch, no re-transform.
    let mut svgs = Vec::new();
    for width in [400.0_f64, 800.0, 1600.0] {
        let opts = RenderOptions::with_size(Some(width), Some(300.0));
        let svg = chartml.render_prepared_to_svg(&prepared, &opts).unwrap();
        assert!(
            svg.contains(&format!("width=\"{}\"", width)),
            "Resize at width={} must be reflected in the SVG header; got: {}",
            width,
            &svg[..svg.len().min(160)],
        );
        svgs.push(svg);
    }

    assert_eq!(svgs.len(), 3);
    assert_eq!(
        *calls.lock().unwrap(),
        3,
        "Renderer must be called once per resize (no skipped renders)",
    );
    // The three SVGs must differ — a passing test that emitted identical
    // strings would mean width wasn't actually flowing through.
    assert_ne!(svgs[0], svgs[1]);
    assert_ne!(svgs[1], svgs[2]);
}

/// End-to-end coverage of the convenience `render_to_svg_async` plus the
/// real `DataFusionTransform` middleware joining two pre-registered sources.
/// Proves the new pipeline can drive everything the legacy async path could.
#[tokio::test]
async fn test_render_to_svg_async_with_join() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CapturingRenderer {
            captured: captured.clone(),
        },
    );
    chartml.register_transform(DataFusionTransform);
    chartml.register_source("visitors", visitors_table());
    chartml.register_source("sessions", sessions_table());

    let opts = RenderOptions::default();
    let svg = chartml
        .render_to_svg_async(MULTI_NAMED_MAP_YAML, &opts)
        .await
        .expect("render_to_svg_async must drive fetch + transform + render");

    assert!(svg.starts_with("<svg"));
    let table = captured.lock().unwrap();
    let table = table.as_ref().expect("renderer must have received joined data");
    assert_eq!(
        table.num_rows(),
        3,
        "DataFusion join over visitors+sessions must yield 3 rows",
    );
}

/// The legacy async entry point keeps producing `ChartElement` (back-compat
/// shim). Internal callers — chartml-leptos, chartml-render's
/// `render_to_png_async`, npm wrappers — depend on the existing signature.
#[tokio::test]
async fn test_legacy_async_shim_still_returns_chart_element() {
    let mut chartml = ChartML::new();
    chartml.register_renderer(
        "bar",
        CountingRenderer {
            calls: Arc::new(Mutex::new(0)),
        },
    );

    let element = chartml
        .render_from_yaml_with_params_async(SINGLE_INLINE_YAML, None, None, None)
        .await
        .expect("legacy async shim must still work after Phase 2 refactor");

    assert!(
        matches!(element, ChartElement::Svg { .. }),
        "legacy async shim must return a ChartElement::Svg root (back-compat \
         contract for chartml-leptos / chartml-render / npm wrappers); got {:?}",
        element,
    );
}
