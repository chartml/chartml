use std::collections::HashMap;
use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::spec::VisualizeSpec;

/// Height in px reserved for the trend row, and the trend span's line box.
///
/// The two must be the same number: the row is reserved on every card whether
/// or not a trend is rendered, so if the rendered trend were taller the row
/// would grow only on the cards that have one and the value band would drift
/// out of alignment across a dashboard row.
///
/// 20px is the label's line box at its 14px font size, so the trend reads as a
/// matching band at the foot of the card. It is deliberately a fixed px value
/// and not an `em`: an `em` on the card resolves against the card's font size,
/// not the trend's smaller one.
const TREND_ROW_HEIGHT_PX: u32 = 20;

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
                ("color".to_string(), format!("var(--chartml-text-secondary, {})", config.theme.text_secondary)),
                ("grid-row".to_string(), "label".to_string()),
                ("grid-column".to_string(), "1".to_string()),
                // `justify-items: center` makes every child fit-content sized, and
                // fit-content has a min-content floor — a label with no break
                // opportunity (the auto-derived field name below is snake_case)
                // would otherwise grow its own box past the column and bleed
                // outside the card, since `overflow: hidden` clips the span's
                // contents, not the span's overflow of its grid area.
                ("max-width".to_string(), "100%".to_string()),
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
                ("color".to_string(), format!("var(--chartml-text, {})", config.theme.text)),
                ("grid-row".to_string(), "value".to_string()),
                ("grid-column".to_string(), "1".to_string()),
                // Same fit-content exposure as the label — and the value has no
                // `overflow: hidden` to fall back on.
                ("max-width".to_string(), "100%".to_string()),
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
                    format!("var(--chartml-text-secondary, {})", config.theme.text_secondary)
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
                        ("grid-row".to_string(), "trend".to_string()),
                        ("grid-column".to_string(), "1".to_string()),
                        ("max-width".to_string(), "100%".to_string()),
                        // Pin the line box to a fixed px height so the trend row
                        // reserved on the card (`TREND_ROW_HEIGHT_PX` below) is
                        // exactly the height this span occupies. Without it the
                        // line box is font-dependent and the reservation would
                        // only approximately match the rendered trend.
                        ("line-height".to_string(), format!("{TREND_ROW_HEIGHT_PX}px")),
                        ("align-self".to_string(), "end".to_string()),
                        ("text-align".to_string(), "center".to_string()),
                    ]),
                    content: trend_text,
                });
            }
        }

        // The card is a single-column grid. The explicit column plus
        // `justify-items: center` is what actually centres the children in the
        // card — without it the children auto-place into implicit columns that
        // are packed at the start of the card, which reads as left-aligned no
        // matter how much `text-align: center` the spans carry.
        //
        // The trend gets its own row rather than sharing the value cell, so the
        // two can never vertically overlap on a short card. That row is
        // *reserved* — `minmax(TREND_ROW_HEIGHT_PX, max-content)`, mirroring the
        // label row's `minmax(2.5em, max-content)` idiom — rather than `auto`.
        // An `auto` row collapses to zero when no trend is rendered, and the
        // `1fr` value track absorbs the difference, which would move the big
        // number's centre depending on whether a sibling card happens to have a
        // comparison. Reserving it keeps every card's value band in the same
        // place, which is the KYO-131 promise.
        Ok(ChartElement::Div {
            class: "chartml-metric-card".to_string(),
            style: HashMap::from([
                ("display".to_string(), "grid".to_string()),
                ("grid-template-rows".to_string(), format!("[label] minmax(2.5em, max-content) [value] 1fr [trend] minmax({TREND_ROW_HEIGHT_PX}px, max-content)")),
                ("grid-template-columns".to_string(), "minmax(0, 1fr)".to_string()),
                ("justify-items".to_string(), "center".to_string()),
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

    /// Split a `grid-template-*` value into its tokens, keeping bracketed line
    /// names (`[label]`) and parenthesised functions (`minmax(0, 1fr)`) whole.
    fn tokenize_track_list(template: &str) -> Vec<String> {
        let mut tokens: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut paren_depth = 0usize;
        let mut in_brackets = false;

        let flush = |current: &mut String, tokens: &mut Vec<String>| {
            if !current.is_empty() {
                tokens.push(std::mem::take(current));
            }
        };

        for ch in template.chars() {
            match ch {
                '[' => {
                    flush(&mut current, &mut tokens);
                    in_brackets = true;
                    current.push(ch);
                }
                ']' => {
                    current.push(ch);
                    in_brackets = false;
                    flush(&mut current, &mut tokens);
                }
                '(' => {
                    paren_depth += 1;
                    current.push(ch);
                }
                ')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    current.push(ch);
                }
                c if c.is_whitespace() && paren_depth == 0 && !in_brackets => {
                    flush(&mut current, &mut tokens);
                }
                c => current.push(c),
            }
        }
        flush(&mut current, &mut tokens);
        tokens
    }

    /// Count the explicit tracks in a `grid-template-*` value.
    ///
    /// Parenthesised argument lists (`minmax(0, 1fr)`) collapse to a single
    /// token and bracketed line names (`[label]`) are dropped, so the result is
    /// the number of actual grid tracks the declaration defines.
    fn count_explicit_tracks(template: &str) -> usize {
        tokenize_track_list(template)
            .iter()
            .filter(|t| !t.starts_with('['))
            .count()
    }

    /// The track sizing function that follows the named grid line `name`.
    fn track_after_line_name(template: &str, name: &str) -> Option<String> {
        let tokens = tokenize_track_list(template);
        let marker = format!("[{name}]");
        let idx = tokens.iter().position(|t| *t == marker)?;
        tokens.get(idx + 1).cloned()
    }

    /// The guaranteed minimum height a track sizing function reserves, in px.
    ///
    /// `minmax(20px, max-content)` reserves 20px; `20px` reserves 20px. Anything
    /// whose floor is content-derived (`auto`, `max-content`, `1fr`, `2.5em` —
    /// an `em` resolves against the *container's* font size, not the item's, so
    /// it is not a dependable reservation for a differently-sized child)
    /// reserves nothing this test can rely on, and yields `None`.
    fn reserved_floor_px(track: &str) -> Option<f64> {
        let floor = match track.strip_prefix("minmax(") {
            Some(args) => args.split(',').next()?.trim().to_string(),
            None => track.trim().to_string(),
        };
        floor.strip_suffix("px")?.parse::<f64>().ok()
    }

    /// Render one metric card and return `(card style, child spans)`.
    fn render_metric_card(
        label: &str,
        compare: bool,
        width: f64,
        height: f64,
    ) -> (HashMap<String, String>, Vec<ChartElement>) {
        let compare_line = if compare { "compareWith: previous" } else { "" };
        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(&format!(
            r#"
            type: metric
            value: current
            label: "{label}"
            format: "$,.0f"
            {compare_line}
            "#
        ))
        .expect("metric VisualizeSpec YAML should parse");
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width,
            height,
            colors: vec![],
            theme: chartml_core::theme::Theme::default(),
        };

        let rows: Vec<Row> = vec![[
            ("current".to_string(), json!(1234567)),
            ("previous".to_string(), json!(1100000)),
        ]
        .into_iter()
        .collect()];
        let data = DataTable::from_rows(&rows).expect("DataTable::from_rows should succeed");

        let element = MetricRenderer::new()
            .render(&data, &config)
            .unwrap_or_else(|e| panic!("render failed for {label:?} (compare={compare}): {e:?}"));

        match element {
            ChartElement::Div { class, style, children } => {
                assert_eq!(class, "chartml-metric-card", "card outer class drifted");
                (style, children)
            }
            other => panic!("card {label:?} must render as a Div, got {other:?}"),
        }
    }

    #[test]
    fn metric_cards_produce_consistent_grid_styles_for_alignment() {
        // KYO-131: side-by-side metric cards on a dashboard row must align their
        // labels and values vertically. Alignment holds because every card emits
        // the same CSS grid template plus identical grid-row / grid-column /
        // clamp / align-self styles on the label and value spans. This test locks
        // those style outputs in so the alignment contract can't drift silently.
        let labels = [
            "Users",
            "Total Active Customers",
            "Average Monthly Revenue Per Paying Account",
            "Conversion Rate",
        ];

        let rendered: Vec<(HashMap<String, String>, Vec<ChartElement>)> = labels
            .iter()
            .map(|label| render_metric_card(label, false, 300.0, 150.0))
            .collect();

        // Extract the canonical grid template once from the first card.
        let expected_grid_template = rendered[0]
            .0
            .get("grid-template-rows")
            .expect("first card must have grid-template-rows")
            .clone();
        assert_eq!(
            expected_grid_template,
            "[label] minmax(2.5em, max-content) [value] 1fr [trend] minmax(20px, max-content)",
            "canonical grid-template-rows drifted from KYO-131 contract",
        );

        for (i, (style, children)) in rendered.iter().enumerate() {
            let label = labels[i];

            // Outer card: grid container styles that enable row-track alignment.
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
                style.get("grid-template-columns").map(String::as_str),
                Some("minmax(0, 1fr)"),
                "card {i} ({label:?}) outer grid-template-columns drifted",
            );
            assert_eq!(
                style.get("justify-items").map(String::as_str),
                Some("center"),
                "card {i} ({label:?}) outer justify-items drifted",
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
                        style.get("grid-column").map(String::as_str),
                        Some("1"),
                        "card {i} ({label:?}) label grid-column drifted",
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
                        style.get("grid-column").map(String::as_str),
                        Some("1"),
                        "card {i} ({label:?}) value grid-column drifted",
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

    #[test]
    fn metric_card_children_are_centred_in_a_single_explicit_column() {
        // CHA-11: metric card content must be horizontally centred in the card.
        //
        // Centring is a *positional* property of the grid, so this test derives
        // it from the placement of each child rather than string-matching the
        // emitted CSS. The pre-existing guards (SVG snapshots and the
        // cross-card consistency test) compare markup for equality, which is
        // exactly why the regression shipped: styles that are identical across
        // cards and byte-stable across runs can still be uniformly wrong.
        //
        // Both card sizes are exercised: 300x150 is the renderer's own default
        // and the size at which layout defects show most clearly, 800x400 the
        // size the snapshot fixtures render at.
        for (width, height) in [(300.0, 150.0), (800.0, 400.0)] {
            for compare in [true, false] {
                let case = format!("{width}x{height} compare={compare}");
                let (card_style, children) =
                    render_metric_card("Average Monthly Revenue Per Paying Account", compare, width, height);
                let expected_children = if compare { 3 } else { 2 };
                assert_eq!(
                    children.len(),
                    expected_children,
                    "{case}: expected {expected_children} child spans",
                );

                // 1. The card defines exactly one explicit column, and items are
                //    centred within it. Without an explicit column track there is
                //    nothing spanning the card's content box for the children to
                //    be centred in.
                let columns = card_style
                    .get("grid-template-columns")
                    .unwrap_or_else(|| panic!("{case}: card must declare grid-template-columns"));
                assert!(
                    !columns.contains("repeat("),
                    "{case}: grid-template-columns must not use repeat() — the single-column \
                     invariant cannot be verified through it (got {columns:?})",
                );
                assert_eq!(
                    count_explicit_tracks(columns),
                    1,
                    "{case}: card must define exactly one explicit column track, got {columns:?}",
                );
                assert_eq!(
                    card_style.get("justify-items").map(String::as_str),
                    Some("center"),
                    "{case}: card must centre its items within the column track",
                );

                // 2. Every child carries an explicit grid-column. This is the
                //    direct guard against the root cause: a child left to
                //    auto-placement is put into an *implicit* column of its own,
                //    which sits outside the explicit (centred) column and drags
                //    the layout off-centre.
                // 3. Every child caps its width at the column. `justify-items:
                //    center` sizes items fit-content, and fit-content has a
                //    min-content floor — an unbreakable string would otherwise
                //    grow the item's box past the column and bleed outside the
                //    card, over neighbouring cards and out of the viewBox.
                // 4. No two children may share a cell (they would visually
                //    overlap), and no child may sit outside column 1.
                let mut cells: Vec<(String, String)> = Vec::new();
                for (i, child) in children.iter().enumerate() {
                    let (class, style) = match child {
                        ChartElement::Span { class, style, .. } => (class, style),
                        other => panic!("{case}: child {i} must be a Span, got {other:?}"),
                    };
                    let row = style
                        .get("grid-row")
                        .unwrap_or_else(|| panic!("{case}: {class} must declare grid-row"))
                        .clone();
                    let column = style
                        .get("grid-column")
                        .unwrap_or_else(|| {
                            panic!(
                                "{case}: {class} has no explicit grid-column — it would \
                                 auto-place into an implicit column and break centring"
                            )
                        })
                        .clone();
                    assert_eq!(
                        column, "1",
                        "{case}: {class} must sit in the single explicit column",
                    );
                    assert_eq!(
                        style.get("max-width").map(String::as_str),
                        Some("100%"),
                        "{case}: {class} must cap its width at the column or unbreakable \
                         content will bleed outside the card",
                    );
                    cells.push((row, column));
                }

                let mut distinct = cells.clone();
                distinct.sort();
                distinct.dedup();
                assert_eq!(
                    distinct.len(),
                    cells.len(),
                    "{case}: children must each occupy their own grid cell, got {cells:?}",
                );

                // Every row a child is placed in must be a named line the card's
                // row template actually defines, or the child silently lands in
                // an implicit row outside the shared alignment template.
                let rows_template = card_style
                    .get("grid-template-rows")
                    .unwrap_or_else(|| panic!("{case}: card must declare grid-template-rows"));
                for (row, _) in &cells {
                    assert!(
                        rows_template.contains(&format!("[{row}]")),
                        "{case}: grid-row {row:?} is not a line defined by {rows_template:?}",
                    );
                }

                // 5. The trend row must reserve a fixed, non-zero height on
                //    *every* card, present trend or not. A content-sized row
                //    (`auto`) resolves to 0 without a trend and to the trend's
                //    height with one; the `1fr` value track absorbs the
                //    difference, so the big number's centre would move depending
                //    on whether a sibling card happens to have a comparison —
                //    the block-axis form of the same defect, and invisible to a
                //    string comparison because both cards emit the identical
                //    template either way.
                let trend_track = track_after_line_name(rows_template, "trend").unwrap_or_else(|| {
                    panic!("{case}: row template {rows_template:?} defines no [trend] track")
                });
                let floor = reserved_floor_px(&trend_track).unwrap_or_else(|| {
                    panic!(
                        "{case}: trend track {trend_track:?} reserves no fixed px height — it \
                         collapses on cards without a trend and shifts the value band"
                    )
                });
                assert!(
                    floor > 0.0,
                    "{case}: trend track {trend_track:?} must reserve a non-zero height",
                );

                // The reservation is only honest if the trend span's own line box
                // is that exact height; otherwise the row grows past the floor on
                // the cards that have a trend.
                if let Some(ChartElement::Span { style, .. }) = children.get(2) {
                    assert_eq!(
                        style.get("line-height").map(String::as_str),
                        Some(format!("{floor}px").trim_end_matches(".0")),
                        "{case}: trend line-height must equal the reserved row floor of {floor}px",
                    );
                }
            }
        }
    }
}
