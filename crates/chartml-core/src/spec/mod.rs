pub mod chart;
pub mod config;
pub mod params;
pub mod source;
pub mod style;
pub mod transform;

use serde::{Deserialize, Serialize};

use crate::error::ChartError;

pub use chart::{
    AnnotationSpec, AxesSpec, AxisSpec, ChartMode, ChartSpec, ChartStyleSpec, DataLabelsSpec,
    DataRef, FieldRef, FieldRefItem, FieldSpec, InlineData, LayoutSpec, MarkEncoding,
    MarkEncodingSpec, MarksSpec, Orientation, StyleRefOrInline, VisualizeSpec,
};
pub use config::{ConfigSpec, StyleRef};
pub use params::{ParamDef, ParamsSpec};
pub use source::{CacheConfig, SourceSpec};
pub use style::{FontSpec, FontsSpec, GridSpec, LegendSpec, StyleSpec};
pub use transform::{
    AggregateSpec, Dimension, DimensionSpec, FilterGroup, FilterRule, Measure, SortSpec,
    TransformSpec,
};

/// A parsed ChartML specification: either a single component or multiple components.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ChartMLSpec {
    Single(Component),
    Array(Vec<Component>),
}

/// A component within a ChartML document, discriminated by the `type` field.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Component {
    #[serde(rename = "chart")]
    Chart(ChartSpec),
    #[serde(rename = "source")]
    Source(SourceSpec),
    #[serde(rename = "style")]
    Style(StyleSpec),
    #[serde(rename = "config")]
    Config(ConfigSpec),
    #[serde(rename = "params")]
    Params(ParamsSpec),
}

/// Parse a ChartML YAML string into a `ChartMLSpec`.
///
/// Handles both single-document and multi-document YAML (separated by `---`).
/// For multi-document YAML, each document is parsed as a separate `Component`
/// and returned as `ChartMLSpec::Array`.
pub fn parse(input: &str) -> Result<ChartMLSpec, ChartError> {
    // Check if the input contains multiple YAML documents by looking for
    // document separators. We split on "---" that appears at the start of a line.
    let documents: Vec<&str> = split_yaml_documents(input);

    if documents.len() <= 1 {
        // Single document: try to parse as a single component first,
        // then fall back to an array of components.
        let yaml_str = documents.first().copied().unwrap_or(input);
        let spec: ChartMLSpec = serde_yaml::from_str(yaml_str)?;
        Ok(spec)
    } else {
        // Multiple documents: parse each as a Component.
        let mut components = Vec::with_capacity(documents.len());
        for doc in documents {
            let trimmed = doc.trim();
            if trimmed.is_empty() {
                continue;
            }
            let component: Component = serde_yaml::from_str(trimmed)?;
            components.push(component);
        }
        Ok(ChartMLSpec::Array(components))
    }
}

/// Split a YAML string into individual documents by `---` separators.
fn split_yaml_documents(input: &str) -> Vec<&str> {
    let mut documents = Vec::new();
    let mut start = 0;

    // Walk through the input looking for lines that are exactly "---" (possibly with trailing whitespace)
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Find end of current line
        let line_start = i;
        while i < len && bytes[i] != b'\n' {
            i += 1;
        }
        let line_end = i;
        if i < len {
            i += 1; // skip newline
        }

        // Check if this line is a document separator
        let line = input[line_start..line_end].trim_end();
        if line == "---" {
            // Everything before this separator is a document (if non-empty)
            let doc = &input[start..line_start];
            if !doc.trim().is_empty() {
                documents.push(doc);
            }
            start = if i < len { i } else { line_end };
        }
    }

    // Remaining content after the last separator
    let remaining = &input[start..];
    if !remaining.trim().is_empty() {
        documents.push(remaining);
    }

    // If no separators were found, return the whole input as one document
    if documents.is_empty() && !input.trim().is_empty() {
        documents.push(input);
    }

    documents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_source() {
        let yaml = r#"
type: source
version: 1
name: sales
provider: inline
rows:
  - { month: "Jan", revenue: 100 }
  - { month: "Feb", revenue: 200 }
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Source(source)) => {
                assert_eq!(source.name, "sales");
                assert_eq!(source.provider, "inline");
                assert!(source.rows.is_some());
                assert_eq!(source.rows.unwrap().len(), 2);
            }
            other => panic!("Expected Single(Source), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_single_chart() {
        let yaml = r#"
type: chart
version: 1
title: Revenue by Month
data: sales
visualize:
  type: bar
  columns: month
  rows: revenue
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                assert_eq!(chart.title, Some("Revenue by Month".to_string()));
                assert_eq!(chart.visualize.chart_type, "bar");
                match &chart.data {
                    DataRef::Named(name) => assert_eq!(name, "sales"),
                    other => panic!("Expected Named data ref, got {:?}", other),
                }
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_multi_document() {
        let yaml = r#"---
type: source
version: 1
name: sales
provider: inline
rows:
  - { month: "Jan", revenue: 100 }
---
type: chart
version: 1
title: Revenue
data: sales
visualize:
  type: bar
  columns: month
  rows: revenue
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Array(components) => {
                assert_eq!(components.len(), 2);
                assert!(matches!(&components[0], Component::Source(_)));
                assert!(matches!(&components[1], Component::Chart(_)));
            }
            other => panic!("Expected Array, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_inline_data() {
        let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows:
    - { x: 1, y: 2 }
visualize:
  type: line
  columns: x
  rows: y
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                match &chart.data {
                    DataRef::Inline(data) => {
                        assert_eq!(data.provider, "inline");
                        assert!(data.rows.is_some());
                    }
                    other => panic!("Expected Inline data ref, got {:?}", other),
                }
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_field_ref_variants() {
        // Simple string field ref
        let yaml = r#"
type: chart
version: 1
data: test
visualize:
  type: bar
  columns: month
  rows:
    field: revenue
    label: "Revenue ($)"
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                match &chart.visualize.columns {
                    Some(FieldRef::Simple(s)) => assert_eq!(s, "month"),
                    other => panic!("Expected Simple field ref, got {:?}", other),
                }
                match &chart.visualize.rows {
                    Some(FieldRef::Detailed(spec)) => {
                        assert_eq!(spec.field, "revenue");
                        assert_eq!(spec.label, Some("Revenue ($)".to_string()));
                    }
                    other => panic!("Expected Detailed field ref, got {:?}", other),
                }
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_metric_chart() {
        let yaml = r#"
type: chart
version: 1
data: kpis
visualize:
  type: metric
  value: totalRevenue
  label: Total Revenue
  format: "$,.0f"
  compareWith: previousRevenue
  invertTrend: false
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                assert_eq!(chart.visualize.chart_type, "metric");
                assert_eq!(chart.visualize.value, Some("totalRevenue".to_string()));
                assert_eq!(chart.visualize.compare_with, Some("previousRevenue".to_string()));
                assert_eq!(chart.visualize.invert_trend, Some(false));
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_style_component() {
        let yaml = r##"
type: style
version: 1
name: custom_theme
colors: ["#4285f4", "#ea4335", "#34a853"]
grid:
  x: true
  y: true
  color: "#e0e0e0"
  opacity: 0.5
height: 400
showDots: true
strokeWidth: 2
fonts:
  title:
    family: "Inter"
    size: 16
    weight: "bold"
    color: "#333"
  axis:
    size: 12
legend:
  position: top
  orientation: horizontal
"##;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Style(style)) => {
                assert_eq!(style.name, "custom_theme");
                assert_eq!(style.colors.unwrap().len(), 3);
                assert_eq!(style.height, Some(400.0));
                assert_eq!(style.show_dots, Some(true));
                assert_eq!(style.stroke_width, Some(2.0));
                let fonts = style.fonts.unwrap();
                assert_eq!(fonts.title.unwrap().family, Some("Inter".to_string()));
                let legend = style.legend.unwrap();
                assert_eq!(legend.position, Some("top".to_string()));
            }
            other => panic!("Expected Single(Style), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_config_component() {
        let yaml = r#"
type: config
version: 1
style: custom_theme
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Config(config)) => {
                assert_eq!(config.version, 1);
                match &config.style {
                    StyleRef::Named(name) => assert_eq!(name, "custom_theme"),
                    other => panic!("Expected Named style ref, got {:?}", other),
                }
            }
            other => panic!("Expected Single(Config), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_params_component() {
        let yaml = r#"
type: params
version: 1
name: dashboard_filters
params:
  - id: date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"
  - id: regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC"]
    default: ["US", "EU"]
  - id: top_n
    type: number
    label: "Top N"
    default: 10
    placeholder: "Enter number"
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Params(params)) => {
                assert_eq!(params.name, Some("dashboard_filters".to_string()));
                assert_eq!(params.params.len(), 3);
                assert_eq!(params.params[0].id, "date_range");
                assert_eq!(params.params[0].param_type, "daterange");
                assert_eq!(params.params[1].options.as_ref().unwrap().len(), 3);
                assert_eq!(params.params[2].placeholder, Some("Enter number".to_string()));
            }
            other => panic!("Expected Single(Params), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_field_ref_multiple_with_strings() {
        let yaml = r##"
type: chart
version: 1
data: test
visualize:
  type: bar
  columns: month
  rows:
    - field: actual
      mark: bar
      color: "#4285f4"
      label: "Actual Revenue"
    - field: target
      mark: line
      color: "#ea4335"
      label: "Target"
"##;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                match &chart.visualize.rows {
                    Some(FieldRef::Multiple(items)) => {
                        assert_eq!(items.len(), 2);
                    }
                    other => panic!("Expected Multiple field ref, got {:?}", other),
                }
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_transform() {
        let yaml = r#"
type: chart
version: 1
data: raw_data
transform:
  aggregate:
    dimensions:
      - region
      - column: category
        name: Category
    measures:
      - column: amount
        aggregation: sum
        name: totalAmount
    sort:
      - field: totalAmount
        direction: desc
    limit: 10
visualize:
  type: bar
  columns: region
  rows: totalAmount
"#;
        let result = parse(yaml).unwrap();
        match result {
            ChartMLSpec::Single(Component::Chart(chart)) => {
                let transform = chart.transform.unwrap();
                let agg = transform.aggregate.unwrap();
                assert_eq!(agg.dimensions.len(), 2);
                assert_eq!(agg.measures.len(), 1);
                assert_eq!(agg.measures[0].name, "totalAmount");
                assert_eq!(agg.limit, Some(10));
            }
            other => panic!("Expected Single(Chart), got {:?}", other),
        }
    }
}
