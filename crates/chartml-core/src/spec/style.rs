use serde::{Deserialize, Serialize};

/// A named `type: style` component. Like the schema's closed
/// `StyleComponent`, it rejects fields it does not know, so a misspelled theme
/// field fails instead of silently doing nothing. (Inline and Config styles
/// stay open, as CHA-51 decided.)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSpec {
    pub x: Option<bool>,
    pub y: Option<bool>,
    pub color: Option<String>,
    pub opacity: Option<f64>,
    pub dash_array: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontsSpec {
    pub title: Option<FontSpec>,
    pub axis: Option<FontSpec>,
    pub data_label: Option<FontSpec>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontSpec {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub weight: Option<FontWeight>,
    pub color: Option<String>,
}

/// A font weight: a whole number from 1 to 1000 (`weight: 600`) or a CSS
/// keyword (`normal`, `bold`, `bolder`, `lighter`), as spec.md "Typography"
/// and `chartml_schema.json` (`FontWeight`) document. A number written as a
/// string (`weight: "600"`), which stored specs from before numeric weights
/// parsed use, stays accepted. Anything else fails the render.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FontWeight {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendSpec {
    pub position: Option<String>,
    pub orientation: Option<String>,
}
