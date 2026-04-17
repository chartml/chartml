use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::source::CacheConfig;
use super::style::{FontsSpec, GridSpec, LegendSpec};
use super::transform::TransformSpec;

// --- Top-level ChartSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpec {
    pub version: u32,
    pub title: Option<String>,
    pub data: DataRef,
    pub transform: Option<TransformSpec>,
    pub visualize: VisualizeSpec,
    pub layout: Option<LayoutSpec>,
    pub style: Option<StyleRefOrInline>,
    pub params: Option<Vec<super::params::ParamDef>>,
}

// --- DataRef: string reference, inline/single data source, or a named-source map ---
//
// The JS chartml accepts three shapes for `data`:
//   data: sales                     # registered-source reference (Named)
//   data: { provider: ..., rows: ... }   # single inline/plugin source (Inline)
//   data:                           # multi-source map, keyed by user-chosen names (NamedMap)
//     visitors:  { datasource: ..., query: ... }
//     revenue:   { datasource: ..., query: ... }
//
// The NamedMap variant is recognised because its values are themselves maps that
// (unlike Inline's flat keys) don't contain the reserved data-source keywords
// directly on `data`. Serde's untagged enum does structural matching, so order
// here matters: Named first (plain string), Inline next (flat map with the
// reserved keys), NamedMap last (map-of-maps).

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DataRef {
    Named(String),
    Inline(InlineData),
    NamedMap(IndexMap<String, InlineData>),
}

/// Inline / single-source data descriptor. Every field is optional so the struct
/// matches all of JS chartml's reserved-key shapes: `rows`-based inline data,
/// `url`-based HTTP, `provider`-based plugin, and `datasource`+`query`-based
/// slug references.
///
/// `deny_unknown_fields` is critical: the untagged `DataRef` enum tries
/// `Inline` before `NamedMap`, and without the deny-unknown guard a
/// named-source map like `{ visitors: {...}, revenue: {...} }` would silently
/// deserialize into `InlineData` with every field set to `None`, masking the
/// real shape.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InlineData {
    pub provider: Option<String>,
    pub rows: Option<Vec<serde_json::Value>>,
    pub url: Option<String>,
    pub endpoint: Option<String>,
    pub cache: Option<CacheConfig>,
    /// Slug-based datasource reference (resolved at runtime by the host app).
    pub datasource: Option<String>,
    /// SQL query string for slug-based datasources.
    pub query: Option<String>,
}

// --- StyleRef for chart-level: string or inline style ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StyleRefOrInline {
    Named(String),
    Inline(Box<ChartStyleSpec>),
}

// --- VisualizeSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizeSpec {
    #[serde(rename = "type")]
    pub chart_type: String,
    pub mode: Option<ChartMode>,
    pub orientation: Option<Orientation>,
    pub columns: Option<FieldRef>,
    pub rows: Option<FieldRef>,
    pub marks: Option<MarksSpec>,
    pub axes: Option<AxesSpec>,
    pub annotations: Option<Vec<AnnotationSpec>>,
    pub style: Option<ChartStyleSpec>,
    // Metric-specific fields
    pub value: Option<String>,
    pub label: Option<String>,
    pub format: Option<String>,
    pub compare_with: Option<String>,
    pub invert_trend: Option<bool>,
    pub data_labels: Option<DataLabelsSpec>,
}

// --- ChartMode ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartMode {
    Stacked,
    Grouped,
    Normalized,
}

// --- Orientation ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// --- FieldRef ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldRef {
    Simple(String),
    Detailed(Box<FieldSpec>),
    Multiple(Vec<FieldRefItem>),
}

/// Items within a FieldRef::Multiple array — each can be a string or a detailed spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldRefItem {
    Simple(String),
    Detailed(Box<FieldSpec>),
}

/// A row/column field specification.
///
/// `field` is `Option<String>` rather than `String` because range marks —
/// used for shading forecast confidence bands on line charts — have no single
/// field name, only `upper`/`lower` bound field names. A range-mark item
/// looks like `{ mark: range, upper: upper_bound, lower: lower_bound }`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSpec {
    pub field: Option<String>,
    pub mark: Option<String>,
    pub axis: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub format: Option<String>,
    pub data_labels: Option<DataLabelsSpec>,
    /// Line style: "solid" (default), "dashed", "dotted"
    pub line_style: Option<String>,
    /// For range marks: upper bound field name
    pub upper: Option<String>,
    /// For range marks: lower bound field name
    pub lower: Option<String>,
    /// Fill opacity for range marks (default 0.15)
    pub opacity: Option<f64>,
}


// --- DataLabelsSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataLabelsSpec {
    pub show: Option<bool>,
    pub position: Option<String>,
    pub format: Option<String>,
    pub color: Option<String>,
    pub font_size: Option<f64>,
}

// --- MarksSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarksSpec {
    pub color: Option<MarkEncoding>,
    pub size: Option<MarkEncoding>,
    pub shape: Option<MarkEncoding>,
    pub text: Option<MarkEncoding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MarkEncoding {
    Simple(String),
    Detailed(MarkEncodingSpec),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkEncodingSpec {
    pub field: String,
    pub label: Option<String>,
    pub format: Option<String>,
}

// --- AxesSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxesSpec {
    #[serde(alias = "columns", alias = "bottom")]
    pub x: Option<AxisSpec>,
    #[serde(alias = "rows")]
    pub left: Option<AxisSpec>,
    pub right: Option<AxisSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisSpec {
    pub label: Option<String>,
    pub format: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub nice: Option<bool>,
}

// --- AnnotationSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationSpec {
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub axis: Option<String>,
    pub value: Option<serde_json::Value>,
    pub from: Option<serde_json::Value>,
    pub to: Option<serde_json::Value>,
    pub orientation: Option<String>,
    pub label: Option<String>,
    pub label_position: Option<String>,
    pub color: Option<String>,
    pub stroke_width: Option<f64>,
    pub dash_array: Option<String>,
    pub opacity: Option<f64>,
    pub stroke_color: Option<String>,
    /// Line style shorthand: "solid" (default), "dashed", "dotted"
    pub style: Option<String>,
}

// --- ChartStyleSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartStyleSpec {
    pub height: Option<f64>,
    pub width: Option<f64>,
    /// Color palette for chart series.
    ///
    /// Colors are assigned **per series**, not per category/bar:
    /// - Single-series chart (no `marks.color`): ALL bars/points use `colors[0]`
    /// - Multi-series chart (`marks.color` or multiple `rows`): each series gets
    ///   `colors[series_index]`, cycling if more series than colors
    ///
    /// This matches the JS chartml behavior (d3ChartMapper.js lines 139-146).
    pub colors: Option<Vec<String>>,
    pub grid: Option<GridSpec>,
    pub show_dots: Option<bool>,
    pub stroke_width: Option<f64>,
    pub curve_type: Option<String>,
    pub fill_opacity: Option<f64>,
    pub fonts: Option<FontsSpec>,
    pub legend: Option<LegendSpec>,
    /// Table-only: rows per page (default 50). Ignored by non-table chart types.
    pub page_size: Option<usize>,
}

// --- LayoutSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSpec {
    pub col_span: Option<u32>,
}
