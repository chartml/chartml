//! Integration test for Phase 1: YAML with `data:` as a NamedMap of pre-registered
//! sources, joined via a SQL transform, must render end-to-end through both the
//! sync `render_from_yaml` API and the async `render_from_yaml_with_params_async`
//! API.
//!
//! Lives in `chartml-core` (not `chartml-datafusion`) because the contract under
//! test is a `chartml-core` API: a registered `TransformMiddleware` must be
//! dispatched from both render entry points. `chartml-datafusion` is a dev
//! dependency — Rust dev-dependencies do not introduce compile cycles, so the
//! test crate can pull in any concrete middleware without polluting the lib
//! dependency graph.

use chartml_core::data::{DataTable, Row};
use chartml_core::element::{ChartElement, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::ChartML;
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
    let rows = vec![
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(100.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(150.0))]),
        make_row(vec![("date", json!("2024-01-03")), ("n", json!(200.0))]),
    ];
    DataTable::from_rows(&rows).unwrap()
}

fn sessions_table() -> DataTable {
    let rows = vec![
        make_row(vec![("date", json!("2024-01-01")), ("n", json!(10.0))]),
        make_row(vec![("date", json!("2024-01-02")), ("n", json!(15.0))]),
        make_row(vec![("date", json!("2024-01-03")), ("n", json!(20.0))]),
    ];
    DataTable::from_rows(&rows).unwrap()
}

const JOIN_YAML: &str = r#"
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

/// Mock renderer that captures the data it received so tests can assert the
/// joined output reached the renderer correctly.
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

fn assert_join_result(captured: &Mutex<Option<DataTable>>) {
    let captured = captured.lock().unwrap();
    let table = captured
        .as_ref()
        .expect("Renderer must have been called with the joined data");

    assert_eq!(table.num_rows(), 3, "Joined output should have 3 rows");

    let rows = table.to_rows();
    assert_eq!(
        rows[0].get("visitors").and_then(|v| v.as_f64()),
        Some(100.0),
    );
    assert_eq!(
        rows[0].get("sessions").and_then(|v| v.as_f64()),
        Some(10.0),
    );
    assert_eq!(
        rows[2].get("visitors").and_then(|v| v.as_f64()),
        Some(200.0),
    );
    assert_eq!(
        rows[2].get("sessions").and_then(|v| v.as_f64()),
        Some(20.0),
    );
}

fn build_chartml(captured: Arc<Mutex<Option<DataTable>>>) -> ChartML {
    let mut chartml = ChartML::new();
    chartml.register_renderer("bar", CapturingRenderer { captured });
    chartml.register_transform(DataFusionTransform);
    chartml.register_source("visitors", visitors_table());
    chartml.register_source("sessions", sessions_table());
    chartml
}

#[tokio::test]
async fn named_map_sources_join_via_async_render() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let chartml = build_chartml(captured.clone());

    let result = chartml
        .render_from_yaml_with_params_async(JOIN_YAML, None, None, None)
        .await;

    assert!(
        result.is_ok(),
        "NamedMap multi-source render (async) should succeed; got: {:?}",
        result.err(),
    );

    assert_join_result(&captured);
}

/// Sync path coverage: the same YAML, rendered through the public
/// `render_from_yaml` entry point, must dispatch through the registered
/// `DataFusionTransform` middleware. This is the explicit Phase 1 exit
/// criterion: "Sync `render_to_svg` + pre-registered `NamedMap` sources +
/// transform SQL joining them → renders correctly."
#[test]
fn named_map_sources_join_via_sync_render() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let chartml = build_chartml(captured.clone());

    let result = chartml.render_from_yaml(JOIN_YAML);

    assert!(
        result.is_ok(),
        "NamedMap multi-source render (sync) should succeed; got: {:?}",
        result.err(),
    );

    assert_join_result(&captured);
}

/// A multi-source NamedMap with NO transform must be rejected with a clear
/// error — there's no defined merge semantics without a transform block.
#[tokio::test]
async fn named_map_multi_no_transform_errors() {
    let captured: Arc<Mutex<Option<DataTable>>> = Arc::new(Mutex::new(None));
    let chartml = build_chartml(captured);

    let yaml = r#"
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

    let result = chartml
        .render_from_yaml_with_params_async(yaml, None, None, None)
        .await;
    assert!(result.is_err(), "Multi-source without transform must error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("transform") || err.contains("multiple sources"),
        "Error should explain that a transform is required; got: {}",
        err,
    );
}
