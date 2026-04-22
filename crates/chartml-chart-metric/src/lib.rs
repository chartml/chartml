use std::collections::HashMap;
use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::spec::VisualizeSpec;

#[derive(Default)]
pub struct MetricRenderer;

impl MetricRenderer {
    pub fn new() -> Self { Self }
}

impl ChartRenderer for MetricRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let viz = &config.visualize;

        // Get the value field name
        let value_field = viz.value.as_ref()
            .ok_or_else(|| ChartError::MissingField("visualize.value".into()))?;

        // Check for empty data
        if data.is_empty() {
            return Err(ChartError::DataError("No data for metric chart".into()));
        }

        // Extract the value from the first row
        let raw_value = data.get_f64(0, value_field)
            .ok_or_else(|| ChartError::DataError(format!("No numeric value for field '{}'", value_field)))?;

        // Format the value
        let formatted_value = if let Some(fmt_str) = &viz.format {
            NumberFormatter::new(fmt_str).format(raw_value)
        } else {
            format!("{}", raw_value)
        };

        // Get label (from viz.label, or config.title, or the field name)
        let label = viz.label.clone()
            .or_else(|| config.title.clone())
            .unwrap_or_else(|| value_field.clone());

        // Build the metric card using Div/Span elements
        let mut card_children: Vec<ChartElement> = Vec::new();

        // Label
        card_children.push(ChartElement::Span {
            class: "chartml-metric-label".to_string(),
            style: HashMap::from([
                ("font-size".to_string(), "14px".to_string()),
                ("color".to_string(), config.theme.text_secondary.clone()),
                ("grid-row".to_string(), "label".to_string()),
                ("display".to_string(), "-webkit-box".to_string()),
                ("-webkit-box-orient".to_string(), "vertical".to_string()),
                ("-webkit-line-clamp".to_string(), "2".to_string()),
                ("overflow".to_string(), "hidden".to_string()),
                ("text-align".to_string(), "center".to_string()),
            ]),
            content: label,
        });

        // Value
        card_children.push(ChartElement::Span {
            class: "chartml-metric-value".to_string(),
            style: HashMap::from([
                ("font-size".to_string(), "36px".to_string()),
                ("font-weight".to_string(), "bold".to_string()),
                ("color".to_string(), config.theme.text.clone()),
                ("grid-row".to_string(), "value".to_string()),
                ("align-self".to_string(), "center".to_string()),
                ("text-align".to_string(), "center".to_string()),
            ]),
            content: formatted_value,
        });

        // Comparison/trend (if compareWith is specified)
        if let Some(compare_field) = &viz.compare_with {
            if let Some(compare_value) = data.get_f64(0, compare_field) {
                let change = raw_value - compare_value;
                let pct_change = if compare_value != 0.0 {
                    (change / compare_value) * 100.0
                } else {
                    0.0
                };

                let invert = viz.invert_trend.unwrap_or(false);
                let is_positive = if invert { change < 0.0 } else { change > 0.0 };
                let trend_color = if change == 0.0 {
                    config.theme.text_secondary.clone()
                } else if is_positive {
                    "#34a853".to_string() // green
                } else {
                    "#dc3545".to_string() // red
                };

                let arrow = if change > 0.0 { "↑" } else if change < 0.0 { "↓" } else { "→" };
                let trend_text = format!("{} {:.1}%", arrow, pct_change.abs());

                card_children.push(ChartElement::Span {
                    class: "chartml-metric-trend".to_string(),
                    style: HashMap::from([
                        ("font-size".to_string(), "14px".to_string()),
                        ("color".to_string(), trend_color),
                        ("grid-row".to_string(), "value".to_string()),
                        ("align-self".to_string(), "end".to_string()),
                        ("text-align".to_string(), "center".to_string()),
                    ]),
                    content: trend_text,
                });
            }
        }

        Ok(ChartElement::Div {
            class: "chartml-metric-card".to_string(),
            style: HashMap::from([
                ("display".to_string(), "grid".to_string()),
                ("grid-template-rows".to_string(), "[label] minmax(2.5em, max-content) [value] 1fr".to_string()),
                ("padding".to_string(), "20px".to_string()),
                ("height".to_string(), "100%".to_string()),
                ("box-sizing".to_string(), "border-box".to_string()),
            ]),
            children: card_children,
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(150.0)) // Metric cards are shorter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chartml_core::data::Row;
    use chartml_core::element::count_elements;
    use serde_json::json;

    fn make_metric_data() -> DataTable {
        let rows: Vec<Row> = vec![
            [("current".to_string(), json!(1234567)), ("previous".to_string(), json!(1100000))].into_iter().collect(),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn make_metric_config() -> ChartConfig {
        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(r#"
            type: metric
            value: current
            label: "Total Revenue"
            format: "$,.0f"
            compareWith: previous
        "#).unwrap();
        ChartConfig {
            visualize: viz,
            title: None,
            width: 300.0,
            height: 150.0,
            colors: vec![],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    #[test]
    fn metric_renders() {
        let renderer = MetricRenderer::new();
        let result = renderer.render(&make_metric_data(), &make_metric_config());
        assert!(result.is_ok(), "Metric render failed: {:?}", result.err());
    }

    #[test]
    fn metric_has_formatted_value() {
        let renderer = MetricRenderer::new();
        let element = renderer.render(&make_metric_data(), &make_metric_config()).unwrap();
        // Should have a Span with the formatted value "$1,234,567"
        let span_count = count_elements(&element, &|e| matches!(e, ChartElement::Span { .. }));
        assert!(span_count >= 2, "Should have label + value spans, got {}", span_count);
    }

    #[test]
    fn metric_has_trend_indicator() {
        let renderer = MetricRenderer::new();
        let element = renderer.render(&make_metric_data(), &make_metric_config()).unwrap();
        // Should have 3 spans: label, value, trend
        let span_count = count_elements(&element, &|e| matches!(e, ChartElement::Span { .. }));
        assert_eq!(span_count, 3, "Should have label + value + trend spans");
    }

    #[test]
    fn metric_inverted_trend() {
        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(r#"
            type: metric
            value: current
            format: ".2%"
            compareWith: previous
            invertTrend: true
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 300.0,
            height: 150.0,
            colors: vec![],
            theme: chartml_core::theme::Theme::default(),
        };
        // Error rate going down (0.023 < 0.031) should be green with invertTrend
        let rows: Vec<Row> = vec![
            [("current".to_string(), json!(0.023)), ("previous".to_string(), json!(0.031))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let renderer = MetricRenderer::new();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn metric_default_dimensions() {
        let renderer = MetricRenderer::new();
        let dims = renderer.default_dimensions(&serde_yaml::from_str("type: metric").unwrap());
        assert_eq!(dims.unwrap().height, 150.0);
    }

    #[test]
    fn metric_empty_data_errors() {
        let renderer = MetricRenderer::new();
        let data = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        assert!(renderer.render(&data, &make_metric_config()).is_err());
    }
}
