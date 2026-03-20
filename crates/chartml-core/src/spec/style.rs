use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSpec {
    pub version: u32,
    pub name: String,
    pub colors: Option<Vec<String>>,
    pub grid: Option<GridSpec>,
    pub height: Option<f64>,
    pub show_dots: Option<bool>,
    pub stroke_width: Option<f64>,
    pub fonts: Option<FontsSpec>,
    pub legend: Option<LegendSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSpec {
    pub x: Option<bool>,
    pub y: Option<bool>,
    pub color: Option<String>,
    pub opacity: Option<f64>,
    pub dash_array: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontsSpec {
    pub title: Option<FontSpec>,
    pub axis: Option<FontSpec>,
    pub data_label: Option<FontSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSpec {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub weight: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendSpec {
    pub position: Option<String>,
    pub orientation: Option<String>,
}
