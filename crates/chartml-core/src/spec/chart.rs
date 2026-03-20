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

// --- DataRef: either a string reference or inline data ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DataRef {
    Named(String),
    Inline(InlineData),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub provider: String,
    pub rows: Option<Vec<serde_json::Value>>,
    pub url: Option<String>,
    pub endpoint: Option<String>,
    pub cache: Option<CacheConfig>,
}

// --- StyleRef for chart-level: string or inline style ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StyleRefOrInline {
    Named(String),
    Inline(ChartStyleSpec),
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
    Detailed(FieldSpec),
    Multiple(Vec<FieldRefItem>),
}

/// Items within a FieldRef::Multiple array — each can be a string or a detailed spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldRefItem {
    Simple(String),
    Detailed(FieldSpec),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSpec {
    pub field: String,
    pub mark: Option<String>,
    pub axis: Option<String>,
    pub label: Option<String>,
    pub color: Option<String>,
    pub format: Option<String>,
    pub data_labels: Option<DataLabelsSpec>,
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
    #[serde(alias = "columns")]
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
}

// --- ChartStyleSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartStyleSpec {
    pub height: Option<f64>,
    pub width: Option<f64>,
    pub colors: Option<Vec<String>>,
    pub grid: Option<GridSpec>,
    pub show_dots: Option<bool>,
    pub stroke_width: Option<f64>,
    pub curve_type: Option<String>,
    pub fill_opacity: Option<f64>,
    pub fonts: Option<FontsSpec>,
    pub legend: Option<LegendSpec>,
}

// --- LayoutSpec ---

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSpec {
    pub col_span: Option<u32>,
}
