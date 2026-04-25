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

        // Extract the value from the first row. A null/missing value is not an
        // error — the card renders "—" (U+2014) instead of a number, and trend
        // calculation is skipped because there is no numeric base to compare.
        let raw_value = data.get_f64(0, value_field);

        // Format the value
        let formatted_value = match raw_value {
            None => "\u{2014}".to_string(),
            Some(n) => {
                if let Some(fmt_str) = &viz.format {
                    NumberFormatter::new(fmt_str).format(n)
                } else {
                    format!("{}", n)
                }
            }
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

        // Comparison/trend (if compareWith is specified). Trend is only meaningful
        // when the primary value is numeric — skip entirely when null.
        if let (Some(raw_value), Some(compare_field)) = (raw_value, &viz.compare_with) {
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
    #![allow(clippy::unwrap_used)]
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

    #[test]
    fn metric_null_value_renders_em_dash() {
        // A null value (JSON null) must render "—" (U+2014) instead of an error.
        let rows: Vec<Row> = vec![
            [("current".to_string(), serde_json::Value::Null)].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();

        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(r#"
            type: metric
            value: current
            label: "Revenue"
            format: "$,.0f"
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 300.0,
            height: 150.0,
            colors: vec![],
            theme: chartml_core::theme::Theme::default(),
        };

        let renderer = MetricRenderer::new();
        let element = renderer.render(&data, &config)
            .expect("null value must not error — should render em dash");

        // The value span (children[1]) must contain the em dash.
        let children = match &element {
            ChartElement::Div { children, .. } => children,
            other => panic!("root must be Div, got {other:?}"),
        };
        let value_span = children.get(1).expect("missing value span");
        match value_span {
            ChartElement::Span { content, .. } => {
                assert_eq!(content, "\u{2014}", "null value span must show em dash, got {content:?}");
            }
            other => panic!("value child must be Span, got {other:?}"),
        }
    }

    #[test]
    fn metric_null_value_skips_trend() {
        // When the primary value is null, the trend span must not be rendered
        // even when compareWith is specified and the comparison field has a value.
        let rows: Vec<Row> = vec![
            [
                ("current".to_string(), serde_json::Value::Null),
                ("previous".to_string(), serde_json::json!(1000)),
            ]
            .into_iter()
            .collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();

        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(r#"
            type: metric
            value: current
            label: "Revenue"
            compareWith: previous
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 300.0,
            height: 150.0,
            colors: vec![],
            theme: chartml_core::theme::Theme::default(),
        };

        let renderer = MetricRenderer::new();
        let element = renderer.render(&data, &config)
            .expect("null value must not error");

        // Only label + value spans — no trend span.
        let span_count = count_elements(&element, &|e| matches!(e, ChartElement::Span { .. }));
        assert_eq!(span_count, 2, "null primary value must produce only label + value spans, got {span_count}");
    }

    #[test]
    fn metric_cards_produce_consistent_grid_styles_for_alignment() {
        // KYO-131: side-by-side metric cards on a dashboard row must align their
        // labels and values vertically. Alignment holds because every card emits
        // the same CSS grid template plus identical grid-row / clamp / align-self
        // styles on the label and value spans. This test locks those style
        // outputs in so the alignment contract can't drift silently.
        let labels = [
            "Users",
            "Total Active Customers",
            "Average Monthly Revenue Per Paying Account",
            "Conversion Rate",
        ];

        let renderer = MetricRenderer::new();
        let mut rendered: Vec<(String, HashMap<String, String>, Vec<ChartElement>)> = Vec::new();

        for label in labels.iter() {
            let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(&format!(
                r#"
                type: metric
                value: current
                label: "{label}"
                format: "$,.0f"
                "#
            ))
            .expect("metric VisualizeSpec YAML should parse");
            let config = ChartConfig {
                visualize: viz,
                title: None,
                width: 300.0,
                height: 150.0,
                colors: vec![],
                theme: chartml_core::theme::Theme::default(),
            };

            let rows: Vec<Row> = vec![
                [("current".to_string(), json!(1234567))].into_iter().collect(),
            ];
            let data = DataTable::from_rows(&rows).expect("DataTable::from_rows should succeed");

            let element = renderer
                .render(&data, &config)
                .unwrap_or_else(|e| panic!("render failed for card {label:?}: {e:?}"));

            match element {
                ChartElement::Div { class, style, children } => {
                    rendered.push((class, style, children));
                }
                other => panic!("card {label:?} must render as a Div, got {other:?}"),
            }
        }

        // Extract the canonical grid template once from the first card.
        let expected_grid_template = rendered[0]
            .1
            .get("grid-template-rows")
            .expect("first card must have grid-template-rows")
            .clone();
        assert_eq!(
            expected_grid_template,
            "[label] minmax(2.5em, max-content) [value] 1fr",
            "canonical grid-template-rows drifted from KYO-131 contract",
        );

        for (i, (class, style, children)) in rendered.iter().enumerate() {
            let label = labels[i];

            // Outer card: class + grid container styles that enable row-track alignment.
            assert_eq!(
                class, "chartml-metric-card",
                "card {i} ({label:?}) outer class drifted",
            );
            assert_eq!(
                style.get("display").map(String::as_str),
                Some("grid"),
                "card {i} ({label:?}) outer display drifted",
            );
            assert_eq!(
                style.get("grid-template-rows"),
                Some(&expected_grid_template),
                "card {i} ({label:?}) outer grid-template-rows drifted",
            );
            assert_eq!(
                style.get("height").map(String::as_str),
                Some("100%"),
                "card {i} ({label:?}) outer height drifted",
            );
            assert_eq!(
                style.get("box-sizing").map(String::as_str),
                Some("border-box"),
                "card {i} ({label:?}) outer box-sizing drifted",
            );

            // Label span is always children[0]; value span always children[1].
            let label_span = children
                .first()
                .unwrap_or_else(|| panic!("card {i} ({label:?}) missing label span"));
            let value_span = children
                .get(1)
                .unwrap_or_else(|| panic!("card {i} ({label:?}) missing value span"));

            match label_span {
                ChartElement::Span { class, style, .. } => {
                    assert_eq!(
                        class, "chartml-metric-label",
                        "card {i} ({label:?}) label class drifted",
                    );
                    assert_eq!(
                        style.get("grid-row").map(String::as_str),
                        Some("label"),
                        "card {i} ({label:?}) label grid-row drifted",
                    );
                    assert_eq!(
                        style.get("display").map(String::as_str),
                        Some("-webkit-box"),
                        "card {i} ({label:?}) label display drifted",
                    );
                    assert_eq!(
                        style.get("-webkit-box-orient").map(String::as_str),
                        Some("vertical"),
                        "card {i} ({label:?}) label -webkit-box-orient drifted",
                    );
                    assert_eq!(
                        style.get("-webkit-line-clamp").map(String::as_str),
                        Some("2"),
                        "card {i} ({label:?}) label -webkit-line-clamp drifted",
                    );
                    assert_eq!(
                        style.get("overflow").map(String::as_str),
                        Some("hidden"),
                        "card {i} ({label:?}) label overflow drifted",
                    );
                }
                other => panic!("card {i} ({label:?}) label child must be Span, got {other:?}"),
            }

            match value_span {
                ChartElement::Span { class, style, .. } => {
                    assert_eq!(
                        class, "chartml-metric-value",
                        "card {i} ({label:?}) value class drifted",
                    );
                    assert_eq!(
                        style.get("grid-row").map(String::as_str),
                        Some("value"),
                        "card {i} ({label:?}) value grid-row drifted",
                    );
                    assert_eq!(
                        style.get("align-self").map(String::as_str),
                        Some("center"),
                        "card {i} ({label:?}) value align-self drifted",
                    );
                }
                other => panic!("card {i} ({label:?}) value child must be Span, got {other:?}"),
            }
        }
    }
}
