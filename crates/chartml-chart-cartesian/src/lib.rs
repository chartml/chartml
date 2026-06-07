use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, Dimensions};
use chartml_core::error::ChartError;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::VisualizeSpec;

mod bar;
mod line;
mod area;
pub(crate) mod helpers;

pub use bar::{bar_animation_origin, render_bar};
pub use line::render_line;
pub use area::render_area;

pub struct CartesianRenderer;

impl CartesianRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CartesianRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartRenderer for CartesianRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        match config.visualize.chart_type.as_str() {
            "bar" => bar::render_bar(data, config),
            "line" => line::render_line(data, config),
            "area" => area::render_area(data, config),
            other => Err(ChartError::UnknownChartType(other.to_string())),
        }
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(400.0))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::element::count_elements;
    use chartml_core::data::{Row, DataTable};
    use serde_json::json;

    fn make_bar_rows() -> Vec<Row> {
        vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200))].into_iter().collect(),
            [("month".to_string(), json!("Mar")), ("revenue".to_string(), json!(150))].into_iter().collect(),
        ]
    }

    fn make_bar_data() -> DataTable {
        DataTable::from_rows(&make_bar_rows()).unwrap()
    }

    fn make_bar_config() -> ChartConfig {
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            columns: month
            rows: revenue
        "#).unwrap();
        ChartConfig {
            visualize: viz,
            title: Some("Test Bar".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string(), "#4A7C59".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    fn make_line_config() -> ChartConfig {
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: line
            columns: month
            rows: revenue
        "#).unwrap();
        ChartConfig {
            visualize: viz,
            title: Some("Test Line".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string(), "#4A7C59".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    fn make_area_config() -> ChartConfig {
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: area
            columns: month
            rows: revenue
        "#).unwrap();
        ChartConfig {
            visualize: viz,
            title: Some("Test Area".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string(), "#4A7C59".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    // ----- Phase 4: theme typography wiring -----

    /// Verify that non-default Theme typography values flow through to the
    /// emitted `ChartElement::Text` nodes. Specifically: a custom label
    /// family, letter-spacing, and text-transform must appear on every
    /// `axis-label` / `tick-value` / `legend-label` text element, while
    /// defaults continue to produce the omitted-attribute legacy path.
    #[test]
    fn phase4_theme_typography_flows_to_axis_label_text() {
        use chartml_core::theme::{TextTransform, Theme};

        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.label_font_family = "serif".into();
        t.label_letter_spacing = 1.5;
        t.label_text_transform = TextTransform::Uppercase;
        t.label_font_weight = 600;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();

        // Walk the tree: for every axis-label text, assert the new fields
        // are set from the theme override.
        fn walk<'a>(el: &'a ChartElement, out: &mut Vec<&'a ChartElement>) {
            match el {
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for c in children {
                        walk(c, out);
                    }
                }
                _ => out.push(el),
            }
        }
        let mut leaves = Vec::new();
        walk(&element, &mut leaves);

        let mut axis_label_count = 0usize;
        for leaf in &leaves {
            if let ChartElement::Text {
                class,
                font_family,
                letter_spacing,
                text_transform,
                font_weight,
                ..
            } = leaf
            {
                // Only inspect axis-label role (which reads the label_* group).
                let is_axis_label = class
                    .split_whitespace()
                    .any(|c| c == "axis-label");
                if !is_axis_label {
                    continue;
                }
                axis_label_count += 1;

                assert_eq!(
                    font_family.as_deref(),
                    Some("serif"),
                    "axis-label text must carry theme.label_font_family"
                );
                assert_eq!(
                    letter_spacing.as_deref(),
                    Some("1.5"),
                    "axis-label text must carry theme.label_letter_spacing"
                );
                assert_eq!(
                    text_transform.as_deref(),
                    Some("uppercase"),
                    "axis-label text must carry theme.label_text_transform"
                );
                assert_eq!(
                    font_weight.as_deref(),
                    Some("600"),
                    "axis-label text must carry theme.label_font_weight"
                );
            }
        }
        assert!(
            axis_label_count > 0,
            "bar chart should have at least one axis-label text"
        );
    }

    /// Verify the same properties propagate to `tick-value` (numeric) text
    /// elements, which read from the `numeric_*` group for family/size but
    /// inherit `label_*` for weight, letter-spacing, and text-transform.
    #[test]
    fn phase4_theme_typography_flows_to_tick_value_text() {
        use chartml_core::theme::{TextTransform, Theme};

        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.numeric_font_family = "monospace".into();
        t.label_letter_spacing = 0.75;
        t.label_text_transform = TextTransform::Lowercase;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();

        let mut found = false;
        fn visit<F: FnMut(&ChartElement)>(el: &ChartElement, f: &mut F) {
            f(el);
            match el {
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for c in children {
                        visit(c, f);
                    }
                }
                _ => {}
            }
        }
        visit(&element, &mut |el| {
            if let ChartElement::Text {
                class,
                font_family,
                letter_spacing,
                text_transform,
                ..
            } = el
            {
                if class
                    .split_whitespace()
                    .any(|c| c == "tick-value")
                {
                    found = true;
                    assert_eq!(
                        font_family.as_deref(),
                        Some("monospace"),
                        "tick-value text must carry theme.numeric_font_family"
                    );
                    assert_eq!(
                        letter_spacing.as_deref(),
                        Some("0.75"),
                        "tick-value text must inherit theme.label_letter_spacing"
                    );
                    assert_eq!(
                        text_transform.as_deref(),
                        Some("lowercase"),
                        "tick-value text must inherit theme.label_text_transform"
                    );
                }
            }
        });
        assert!(found, "bar chart should emit at least one tick-value text");
    }

    #[test]
    fn bar_chart_renders() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_bar_config();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Bar render failed: {:?}", result.err());
        let element = result.unwrap();
        let rect_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { .. }));
        assert_eq!(rect_count, 3, "Should have 3 bars for 3 data points, got {}", rect_count);
    }

    #[test]
    fn bar_chart_has_svg_root() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_bar_config();
        let element = renderer.render(&data, &config).unwrap();
        assert!(matches!(element, ChartElement::Svg { .. }), "Root should be Svg");
    }

    #[test]
    fn bar_chart_has_no_title_in_svg() {
        // Title is rendered as HTML outside the SVG (matching JS chartml).
        // The SVG element tree must NOT contain a chart-title text element.
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_bar_config();
        let element = renderer.render(&data, &config).unwrap();
        let title_count = count_elements(&element, &|e| {
            matches!(e, ChartElement::Text { class, .. } if class == "chart-title")
        });
        assert_eq!(title_count, 0, "Title must not be in the SVG element tree");
    }

    #[test]
    fn bar_chart_has_axes() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_bar_config();
        let element = renderer.render(&data, &config).unwrap();
        let axis_line_count = count_elements(&element, &|e| {
            matches!(e, ChartElement::Line { class, .. } if class == "axis-line")
        });
        assert!(axis_line_count >= 1, "Should have axis lines, got {}", axis_line_count);
    }

    #[test]
    fn line_chart_renders() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_line_config();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Line render failed: {:?}", result.err());
        let element = result.unwrap();
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { .. }));
        assert!(path_count >= 1, "Should have at least 1 path for the line, got {}", path_count);
    }

    #[test]
    fn line_chart_path_has_stroke() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_line_config();
        let element = renderer.render(&data, &config).unwrap();
        // Find the path and check it has a stroke
        fn find_path(el: &ChartElement) -> Option<&ChartElement> {
            match el {
                ChartElement::Path { .. } => Some(el),
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    children.iter().find_map(find_path)
                }
                _ => None,
            }
        }
        let path = find_path(&element).expect("Should find a path element");
        match path {
            ChartElement::Path { stroke, .. } => {
                assert!(stroke.is_some(), "Line path should have a stroke");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn area_chart_renders() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_area_config();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Area render failed: {:?}", result.err());
        let element = result.unwrap();
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { .. }));
        assert!(path_count >= 1, "Should have at least 1 path for the area, got {}", path_count);
    }

    #[test]
    fn area_chart_path_has_fill() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let config = make_area_config();
        let element = renderer.render(&data, &config).unwrap();
        fn find_path(el: &ChartElement) -> Option<&ChartElement> {
            match el {
                ChartElement::Path { .. } => Some(el),
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    children.iter().find_map(find_path)
                }
                _ => None,
            }
        }
        let path = find_path(&element).expect("Should find a path element");
        match path {
            ChartElement::Path { fill, .. } => {
                assert!(fill.is_some(), "Area path should have a fill");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn unknown_type_errors() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config();
        config.visualize.chart_type = "unknown".to_string();
        let result = renderer.render(&data, &config);
        assert!(result.is_err(), "Unknown chart type should produce error");
        match result.unwrap_err() {
            ChartError::UnknownChartType(t) => assert_eq!(t, "unknown"),
            other => panic!("Expected UnknownChartType, got {:?}", other),
        }
    }

    #[test]
    fn bar_chart_no_title() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config();
        config.title = None;
        let element = renderer.render(&data, &config).unwrap();
        let title_count = count_elements(&element, &|e| {
            matches!(e, ChartElement::Text { class, .. } if class == "chart-title")
        });
        assert_eq!(title_count, 0, "Should have no title element when title is None");
    }

    #[test]
    fn default_dimensions_returns_some() {
        let renderer = CartesianRenderer::new();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            columns: x
            rows: y
        "#).unwrap();
        let dims = renderer.default_dimensions(&viz);
        assert!(dims.is_some());
        assert_eq!(dims.unwrap().height, 400.0);
    }

    #[test]
    fn bar_chart_adaptive_padding_2_bars() {
        // With n=2 bars and adaptive padding=0.2, each bar should be ~36.4% of inner_width.
        // inner_width = 800 - left_margin - right_margin ≈ 800 - 60 - 20 = 720
        // bandwidth = 0.8/2.2 * inner_width ≈ 0.3636 * inner_width ≈ 261px
        // Bar should NOT be close to 50% (which would indicate no padding).
        let rows: Vec<Row> = vec![
            [("region".to_string(), json!("US")), ("revenue".to_string(), json!(55000))].into_iter().collect(),
            [("region".to_string(), json!("EU")), ("revenue".to_string(), json!(40000))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            columns: region
            rows: revenue
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: Some("Regional Revenue".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        // Find all Rect elements (bars)
        let mut bar_widths = Vec::new();
        fn collect_bar_widths(el: &ChartElement, widths: &mut Vec<f64>) {
            match el {
                ChartElement::Rect { width, class, .. } if class.split_whitespace().any(|c| c == "bar") => {
                    widths.push(*width);
                }
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for child in children { collect_bar_widths(child, widths); }
                }
                _ => {}
            }
        }
        collect_bar_widths(&element, &mut bar_widths);

        assert_eq!(bar_widths.len(), 2, "Should have 2 bars");
        let bar_width = bar_widths[0];
        println!("Bar width: {:.2}px", bar_width);

        // JS applies maxBarWidth = inner_width * 0.2 clamp.
        // With y_tick_labels pre-computation: for revenue values 100/200, the
        // tick label "200" ≈ 21px + 15px buffer = 36px left margin.
        // inner_width = 800 - 36 - 30 = 734px → maxBarWidth = 146.8px.
        // bandwidth for 2 bars, padding=0.2 = ~234px → clamped to ~146.8px.
        assert!(
            bar_width <= 150.0,
            "Bar width {:.1}px exceeds maxBarWidth clamp",
            bar_width
        );
        assert!(
            bar_width > 50.0,
            "Bar width {:.1}px is unreasonably narrow",
            bar_width
        );
    }

    #[test]
    fn stacked_bar_chart_renders() {
        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)), ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(80)), ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: stacked
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: Some("Stacked Bar".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };
        let renderer = CartesianRenderer::new();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Stacked bar render failed: {:?}", result.err());
        let element = result.unwrap();
        let rect_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { class, .. } if class.split_whitespace().any(|c| c == "bar")));
        assert_eq!(rect_count, 4, "Should have 4 bars (2 categories x 2 series), got {}", rect_count);
    }

    #[test]
    fn grouped_bar_chart_renders() {
        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)), ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(80)), ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: grouped
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };
        let renderer = CartesianRenderer::new();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Grouped bar render failed: {:?}", result.err());
        let element = result.unwrap();
        let rect_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { class, .. } if class.split_whitespace().any(|c| c == "bar")));
        assert_eq!(rect_count, 4, "Should have 4 bars (2 categories x 2 series), got {}", rect_count);
    }

    #[test]
    fn multi_series_line_chart_renders() {
        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)), ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(80)), ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: line
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: Some("Multi Line".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };
        let renderer = CartesianRenderer::new();
        let result = renderer.render(&data, &config);
        assert!(result.is_ok(), "Multi-series line render failed: {:?}", result.err());
        let element = result.unwrap();
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { class, .. } if class.split_whitespace().any(|c| c == "chartml-line-path")));
        assert_eq!(path_count, 2, "Should have 2 line paths for 2 series, got {}", path_count);
    }

    #[test]
    fn empty_data_returns_error() {
        let renderer = CartesianRenderer::new();
        let data = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        let config = make_bar_config();
        let result = renderer.render(&data, &config);
        assert!(result.is_err(), "Empty data should produce an error");
    }

    #[test]
    fn x_axis_horizontal_few_labels() {
        use crate::helpers::{generate_x_axis, GridConfig};
        let labels = vec!["A".into(), "B".into(), "C".into()];
        let result = generate_x_axis(&crate::helpers::XAxisParams {
            labels: &labels, display_label_overrides: None,
            range: (0.0, 800.0), y_position: 350.0, available_width: 800.0,
            x_format: None, chart_height: None, grid: &GridConfig::default(), axis_label: None,
            theme: &chartml_core::theme::Theme::default(),
        });
        // Should be horizontal — no transforms on text elements
        let text_with_transform = result.elements.iter().filter(|e| {
            matches!(e, ChartElement::Text { transform: Some(_), .. })
        }).count();
        assert_eq!(text_with_transform, 0, "Horizontal strategy should have no transforms");
    }

    #[test]
    fn x_axis_rotated_many_labels() {
        use crate::helpers::{generate_x_axis, GridConfig};
        let labels: Vec<String> = (0..20).map(|i| format!("Category Number {}", i)).collect();
        let result = generate_x_axis(&crate::helpers::XAxisParams {
            labels: &labels, display_label_overrides: None,
            range: (0.0, 300.0), y_position: 350.0, available_width: 300.0,
            x_format: None, chart_height: None, grid: &GridConfig::default(), axis_label: None,
            theme: &chartml_core::theme::Theme::default(),
        });
        // Should be rotated — text elements have transforms
        let text_with_transform = result.elements.iter().filter(|e| {
            matches!(e, ChartElement::Text { transform: Some(_), .. })
        }).count();
        assert!(text_with_transform > 0, "Rotated strategy should have transforms");
    }

    #[test]
    fn x_axis_rotated_labels_preserve_full_text() {
        use crate::helpers::{generate_x_axis, GridConfig};
        // Long date-like labels matching the long_temporal_labels test case.
        // These are 25+ chars and must NOT be truncated when rotated.
        let labels: Vec<String> = vec![
            "Monday, January 6th, 2025".into(),
            "Monday, January 13th, 2025".into(),
            "Monday, January 20th, 2025".into(),
            "Monday, January 27th, 2025".into(),
            "Monday, February 3rd, 2025".into(),
            "Monday, February 10th, 2025".into(),
            "Monday, February 17th, 2025".into(),
            "Monday, February 24th, 2025".into(),
            "Monday, March 3rd, 2025".into(),
            "Monday, March 10th, 2025".into(),
            "Monday, March 17th, 2025".into(),
            "Monday, March 24th, 2025".into(),
        ];
        let result = generate_x_axis(&crate::helpers::XAxisParams {
            labels: &labels, display_label_overrides: None,
            range: (0.0, 600.0), y_position: 350.0, available_width: 600.0,
            x_format: None, chart_height: None, grid: &GridConfig::default(), axis_label: None,
            theme: &chartml_core::theme::Theme::default(),
        });
        // Collect all visible tick-label text content
        let tick_texts: Vec<&str> = result.elements.iter().filter_map(|e| {
            if let ChartElement::Text { content, class, .. } = e {
                if class.split_whitespace().any(|c| c == "tick-label") {
                    return Some(content.as_str());
                }
            }
            None
        }).collect();
        // Every visible label must contain its full original text — no ellipsis truncation
        for text in &tick_texts {
            assert!(!text.contains('\u{2026}'),
                "Rotated label should NOT be truncated but got: {text:?}");
        }
        // Check that at least some of the full labels appear verbatim
        assert!(tick_texts.iter().any(|t| *t == "Monday, January 6th, 2025"),
            "Expected full label text in output, got: {:?}", tick_texts);
    }

    #[test]
    fn x_axis_sampled_100_labels() {
        use crate::helpers::{generate_x_axis, GridConfig};
        let labels: Vec<String> = (0..100).map(|i| format!("Long Category Name {}", i)).collect();
        let result = generate_x_axis(&crate::helpers::XAxisParams {
            labels: &labels, display_label_overrides: None,
            range: (0.0, 400.0), y_position: 350.0, available_width: 400.0,
            x_format: None, chart_height: None, grid: &GridConfig::default(), axis_label: None,
            theme: &chartml_core::theme::Theme::default(),
        });
        // Should be sampled — fewer label texts than total categories
        let label_count = result.elements.iter().filter(|e| {
            matches!(e, ChartElement::Text { class, .. } if class.split_whitespace().any(|c| c == "tick-label"))
        }).count();
        assert!(label_count < 100, "Sampled should show fewer labels: got {}", label_count);
        assert!(label_count >= 3, "Should show at least a few labels");
    }

    #[test]
    fn line_chart_grid_dash_array() {
        let data = make_bar_data();
        // Use unquoted dashArray value (matching the examples_source.md spec)
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
type: line
columns: month
rows: revenue
style:
  grid:
    x: true
    y: true
    color: '#e0e0e0'
    opacity: 0.5
    dashArray: 4,4
  showDots: true
"#).unwrap();

        // Verify the grid spec parsed correctly
        let grid_spec = viz.style.as_ref().unwrap().grid.as_ref().unwrap();
        assert_eq!(grid_spec.dash_array, Some("4,4".to_string()), "GridSpec.dash_array should parse from YAML");
        assert_eq!(grid_spec.x, Some(true), "grid.x should be true");
        assert_eq!(grid_spec.y, Some(true), "grid.y should be true");

        let config = ChartConfig {
            visualize: viz,
            title: Some("Dashed Grid Test".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };

        // Verify GridConfig resolves correctly
        let grid_config = crate::helpers::GridConfig::from_config(&config);
        assert_eq!(grid_config.dash_array, Some("4,4".to_string()), "GridConfig.dash_array should be set");
        assert!(grid_config.show_x, "grid.show_x should be true");
        assert!(grid_config.show_y, "grid.show_y should be true");

        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        // Count grid lines and verify ALL have stroke_dasharray set
        let mut dashed_grid_count = 0;
        let mut total_grid_count = 0;
        fn check_grid(el: &ChartElement, dashed: &mut usize, total: &mut usize) {
            match el {
                ChartElement::Line { class, stroke_dasharray, .. } if class.contains("grid-line") => {
                    *total += 1;
                    if let Some(da) = stroke_dasharray {
                        if !da.is_empty() {
                            *dashed += 1;
                        }
                    }
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    for child in children {
                        check_grid(child, dashed, total);
                    }
                }
                _ => {}
            }
        }
        check_grid(&element, &mut dashed_grid_count, &mut total_grid_count);

        assert!(total_grid_count > 0, "Should have grid lines, got {}", total_grid_count);
        assert_eq!(dashed_grid_count, total_grid_count,
            "All {} grid lines should have stroke_dasharray='4,4', but only {} do",
            total_grid_count, dashed_grid_count);
    }

    // ----- Phase 5: theme shape/stroke wiring -----

    /// Collect every `Path` stroke_width on elements whose class matches
    /// `series-line` (the series-weight role wired in Phase 5).
    fn collect_series_stroke_widths(el: &ChartElement, out: &mut Vec<f64>) {
        match el {
            ChartElement::Path { stroke_width: Some(w), class, .. }
                if class.split_whitespace().any(|c| c == "series-line") =>
            {
                out.push(*w);
            }
            ChartElement::Svg { children, .. }
            | ChartElement::Group { children, .. } => {
                for c in children {
                    collect_series_stroke_widths(c, out);
                }
            }
            _ => {}
        }
    }

    /// Collect every `Line` stroke_width bucketed by role
    /// (`axis-line`, `grid-line`, `tick`).
    fn collect_line_stroke_widths_by_class(
        el: &ChartElement,
        out: &mut std::collections::HashMap<String, Vec<f64>>,
    ) {
        match el {
            ChartElement::Line { stroke_width: Some(w), class, .. } => {
                for token in class.split_whitespace() {
                    if matches!(token, "axis-line" | "grid-line" | "tick") {
                        out.entry(token.to_string()).or_default().push(*w);
                    }
                }
            }
            ChartElement::Svg { children, .. }
            | ChartElement::Group { children, .. } => {
                for c in children {
                    collect_line_stroke_widths_by_class(c, out);
                }
            }
            _ => {}
        }
    }

    /// Collect all `(rx, ry)` pairs on `Rect` elements with a `bar` class.
    fn collect_bar_corner_radii(
        el: &ChartElement,
        out: &mut Vec<(Option<f64>, Option<f64>)>,
    ) {
        match el {
            ChartElement::Rect { rx, ry, class, .. }
                if class.split_whitespace().any(|c| c == "bar") =>
            {
                out.push((*rx, *ry));
            }
            ChartElement::Svg { children, .. }
            | ChartElement::Group { children, .. } => {
                for c in children {
                    collect_bar_corner_radii(c, out);
                }
            }
            _ => {}
        }
    }

    /// Collect every `Circle.r` on elements whose class contains `dot-marker`.
    fn collect_dot_radii(el: &ChartElement, out: &mut Vec<f64>) {
        match el {
            ChartElement::Circle { r, class, .. }
                if class.split_whitespace().any(|c| c == "dot-marker") =>
            {
                out.push(*r);
            }
            ChartElement::Svg { children, .. }
            | ChartElement::Group { children, .. } => {
                for c in children {
                    collect_dot_radii(c, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn phase5_bar_corner_radius_omitted_by_default() {
        // Default theme MUST NOT emit rx/ry on bar rects (byte-identical contract).
        let renderer = CartesianRenderer::new();
        let element = renderer
            .render(&make_bar_data(), &make_bar_config())
            .expect("render");

        let mut radii = Vec::new();
        collect_bar_corner_radii(&element, &mut radii);
        assert!(!radii.is_empty(), "expected bar rects in default bar chart");
        for (rx, ry) in &radii {
            assert!(rx.is_none(), "default theme must leave Rect.rx == None");
            assert!(ry.is_none(), "default theme must leave Rect.ry == None");
        }
    }

    #[test]
    fn phase5_custom_bar_corner_radius_emits_rx_ry() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        let renderer = CartesianRenderer::new();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.bar_corner_radius = BarCornerRadius::Uniform(8.0);
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut radii = Vec::new();
        collect_bar_corner_radii(&element, &mut radii);
        assert!(!radii.is_empty());
        for (rx, ry) in &radii {
            assert_eq!(*rx, Some(8.0), "rx must match theme.bar_corner_radius");
            assert_eq!(*ry, Some(8.0), "ry must match theme.bar_corner_radius");
        }
    }

    // ---- Phase follow-up: BarCornerRadius::Top top-only rounding ----

    fn collect_bar_elements<'a>(el: &'a ChartElement, out: &mut Vec<&'a ChartElement>) {
        match el {
            ChartElement::Rect { class, .. } | ChartElement::Path { class, .. }
                if class.split_whitespace().any(|c| c == "bar-rect") =>
            {
                out.push(el);
            }
            ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                for c in children {
                    collect_bar_elements(c, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn phase_followup_bar_top_rounding_zero_is_plain_rect() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        let renderer = CartesianRenderer::new();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.bar_corner_radius = BarCornerRadius::Top(0.0);
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        assert!(!bars.is_empty());
        for b in &bars {
            match b {
                ChartElement::Rect { rx, ry, .. } => {
                    assert!(rx.is_none(), "Top(0.0) must emit Rect with rx=None");
                    assert!(ry.is_none(), "Top(0.0) must emit Rect with ry=None");
                }
                other => panic!("Top(0.0) must emit Rect, got {:?}", other),
            }
        }
    }

    #[test]
    fn phase_followup_bar_top_rounding_vertical() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        let renderer = CartesianRenderer::new();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.bar_corner_radius = BarCornerRadius::Top(8.0);
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        assert!(!bars.is_empty(), "expected bar elements");
        for b in &bars {
            match b {
                ChartElement::Path { d, .. } => {
                    assert_eq!(
                        d.matches("A 8,8").count(),
                        2,
                        "vertical Top(8) must produce 2 arcs, got d={d}"
                    );
                }
                other => panic!("vertical Top(8) must emit Path, got {:?}", other),
            }
        }
    }

    #[test]
    fn phase_followup_bar_top_rounding_horizontal() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        let renderer = CartesianRenderer::new();
        let mut config = make_bar_config();
        config.visualize.orientation = Some(chartml_core::spec::Orientation::Horizontal);
        let mut t = Theme::default();
        t.bar_corner_radius = BarCornerRadius::Top(8.0);
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        assert!(!bars.is_empty(), "expected bar elements (horizontal)");
        for b in &bars {
            match b {
                ChartElement::Path { d, .. } => {
                    assert_eq!(
                        d.matches("A 8,8").count(),
                        2,
                        "horizontal Top(8) must produce 2 arcs, got d={d}"
                    );
                }
                other => panic!("horizontal Top(8) must emit Path, got {:?}", other),
            }
        }
    }

    #[test]
    fn phase_followup_bar_top_rounding_negative_vertical() {
        // Drive build_bar_element directly so the test doesn't depend on a
        // chart spec that emits negative values.
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, BarRectSpec, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);

        let pos = build_bar_element(
            BarRectSpec {
                x: 100.0, y: 50.0, width: 40.0, height: 200.0,
                is_horizontal: false, is_negative: false,
                fill: "#000".into(),
                class: "bar bar-rect".into(),
                data: None,
                stack_baseline: None,
                stack_position: StackPosition::None,
            },
            &theme,
        );
        let neg = build_bar_element(
            BarRectSpec {
                x: 100.0, y: 50.0, width: 40.0, height: 200.0,
                is_horizontal: false, is_negative: true,
                fill: "#000".into(),
                class: "bar bar-rect".into(),
                data: None,
                stack_baseline: None,
                stack_position: StackPosition::None,
            },
            &theme,
        );

        let pos_d = match &pos {
            ChartElement::Path { d, .. } => d.clone(),
            _ => panic!("pos must be Path"),
        };
        let neg_d = match &neg {
            ChartElement::Path { d, .. } => d.clone(),
            _ => panic!("neg must be Path"),
        };

        assert_eq!(pos_d.matches("A 8,8").count(), 2);
        assert_eq!(neg_d.matches("A 8,8").count(), 2);

        // Positive vertical: top rounding → path starts at (x, y+r) = (100, 58).
        assert!(
            pos_d.starts_with("M 100,58"),
            "pos vertical Top path should start at y+r=58, got {pos_d}"
        );
        // Negative vertical: bottom rounding → path starts at the (square)
        // top-left corner (100, 50).
        assert!(
            neg_d.starts_with("M 100,50"),
            "neg vertical Top path should start at (x, y)=(100, 50), got {neg_d}"
        );
        // Negative path must reference the bottom-edge-minus-r coordinate
        // y1-r = 50+200-8 = 242 where its arcs live.
        assert!(
            neg_d.contains(",242"),
            "neg vertical Top path should contain y1-r=242, got {neg_d}"
        );
    }

    #[test]
    fn phase5_custom_series_line_weight_flows_to_line_path() {
        use chartml_core::theme::Theme;
        let renderer = CartesianRenderer::new();
        let mut config = make_line_config();
        let mut t = Theme::default();
        t.series_line_weight = 4.0;
        config.theme = t;
        let element = renderer
            .render(&make_bar_data(), &config)
            .expect("render");

        let mut widths = Vec::new();
        collect_series_stroke_widths(&element, &mut widths);
        assert!(!widths.is_empty(), "expected at least one series-line path");
        for w in &widths {
            assert_eq!(*w, 4.0, "series-line stroke_width must read from theme");
        }
    }

    #[test]
    fn phase5_custom_series_line_weight_flows_to_area_outline() {
        use chartml_core::theme::Theme;
        let renderer = CartesianRenderer::new();
        let mut config = make_area_config();
        let mut t = Theme::default();
        t.series_line_weight = 3.5;
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut widths = Vec::new();
        collect_series_stroke_widths(&element, &mut widths);
        assert!(!widths.is_empty(), "expected area outline series-line path");
        for w in &widths {
            assert_eq!(*w, 3.5);
        }
    }

    #[test]
    fn phase5_custom_dot_radius_flows_to_line_markers() {
        use chartml_core::theme::Theme;
        let renderer = CartesianRenderer::new();
        let mut config = make_line_config();
        let mut t = Theme::default();
        t.dot_radius = 10.0;
        config.theme = t;
        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut radii = Vec::new();
        collect_dot_radii(&element, &mut radii);
        assert!(!radii.is_empty(), "expected dot-marker circles on line chart");
        for r in &radii {
            assert_eq!(*r, 10.0);
        }
    }

    #[test]
    fn phase5_custom_axis_and_grid_line_weights_flow_to_line_strokes() {
        use chartml_core::theme::Theme;
        let renderer = CartesianRenderer::new();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.axis_line_weight = 2.5;
        t.grid_line_weight = 0.5;
        config.theme = t;

        let element = renderer.render(&make_bar_data(), &config).expect("render");

        let mut by_class: std::collections::HashMap<String, Vec<f64>> =
            std::collections::HashMap::new();
        collect_line_stroke_widths_by_class(&element, &mut by_class);

        let axis = by_class.get("axis-line").cloned().unwrap_or_default();
        let ticks = by_class.get("tick").cloned().unwrap_or_default();
        let grid = by_class.get("grid-line").cloned().unwrap_or_default();

        assert!(!axis.is_empty(), "expected axis-line elements");
        assert!(!ticks.is_empty(), "expected tick elements");
        assert!(!grid.is_empty(), "expected grid-line elements");

        for w in &axis {
            assert_eq!(*w, 2.5, "axis-line stroke_width must read from theme.axis_line_weight");
        }
        for w in &ticks {
            assert_eq!(*w, 2.5, "tick stroke_width must read from theme.axis_line_weight");
        }
        for w in &grid {
            assert_eq!(*w, 0.5, "grid-line stroke_width must read from theme.grid_line_weight");
        }
    }

    #[test]
    fn x_axis_date_labels_reformatted() {
        use crate::helpers::{generate_x_axis, GridConfig};
        let labels: Vec<String> = vec![
            "2024-01-01".into(), "2024-01-02".into(), "2024-01-03".into()
        ];
        let result = generate_x_axis(&crate::helpers::XAxisParams {
            labels: &labels, display_label_overrides: None,
            range: (0.0, 800.0), y_position: 350.0, available_width: 800.0,
            x_format: None, chart_height: None, grid: &GridConfig::default(), axis_label: None,
            theme: &chartml_core::theme::Theme::default(),
        });
        // Labels should be reformatted as "Jan 01", "Jan 02", etc.
        let has_reformatted = result.elements.iter().any(|e| {
            matches!(e, ChartElement::Text { content, .. } if content.starts_with("Jan"))
        });
        assert!(has_reformatted, "Date labels should be reformatted");
    }

    // ----- Phase 6: theme.grid_style gating -----

    /// Walk an element tree and count grid-line-x (vertical) and grid-line-y
    /// (horizontal) gridlines emitted by the cartesian renderer.
    fn count_grid_lines(el: &ChartElement) -> (usize, usize) {
        let (mut vx, mut hy) = (0usize, 0usize);
        fn visit(el: &ChartElement, vx: &mut usize, hy: &mut usize) {
            match el {
                ChartElement::Line { class, .. } => {
                    let has_x = class.split_whitespace().any(|c| c == "grid-line-x");
                    let has_y = class.split_whitespace().any(|c| c == "grid-line-y");
                    if has_x {
                        *vx += 1;
                    }
                    if has_y {
                        *hy += 1;
                    }
                }
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for c in children {
                        visit(c, vx, hy);
                    }
                }
                _ => {}
            }
        }
        visit(el, &mut vx, &mut hy);
        (vx, hy)
    }

    /// Build a bar-chart config with both horizontal and vertical gridlines
    /// enabled (show_y defaults to true; explicitly force show_x via spec).
    fn make_bar_config_both_grids() -> ChartConfig {
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            columns: month
            rows: revenue
            style:
              grid:
                x: true
                y: true
        "#).unwrap();
        ChartConfig {
            visualize: viz,
            title: Some("Test Bar GridStyle".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    #[test]
    fn phase6_grid_style_both_default_emits_both_orientations() {
        use chartml_core::theme::{GridStyle, Theme};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config_both_grids();
        let mut t = Theme::default();
        t.grid_style = GridStyle::Both;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        let (vx, hy) = count_grid_lines(&element);
        assert!(vx > 0, "Both: expected vertical gridlines (grid-line-x)");
        assert!(hy > 0, "Both: expected horizontal gridlines (grid-line-y)");
    }

    #[test]
    fn phase6_grid_style_horizontal_only_skips_vertical() {
        use chartml_core::theme::{GridStyle, Theme};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config_both_grids();
        let mut t = Theme::default();
        t.grid_style = GridStyle::HorizontalOnly;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        let (vx, hy) = count_grid_lines(&element);
        assert_eq!(vx, 0, "HorizontalOnly: no grid-line-x expected, got {}", vx);
        assert!(hy > 0, "HorizontalOnly: expected grid-line-y lines");
    }

    #[test]
    fn phase6_grid_style_vertical_only_skips_horizontal() {
        use chartml_core::theme::{GridStyle, Theme};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config_both_grids();
        let mut t = Theme::default();
        t.grid_style = GridStyle::VerticalOnly;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        let (vx, hy) = count_grid_lines(&element);
        assert!(vx > 0, "VerticalOnly: expected grid-line-x lines");
        assert_eq!(hy, 0, "VerticalOnly: no grid-line-y expected, got {}", hy);
    }

    #[test]
    fn phase6_grid_style_none_skips_all_gridlines() {
        use chartml_core::theme::{GridStyle, Theme};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config_both_grids();
        let mut t = Theme::default();
        t.grid_style = GridStyle::None;
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        let (vx, hy) = count_grid_lines(&element);
        assert_eq!(vx, 0, "None: no grid-line-x expected, got {}", vx);
        assert_eq!(hy, 0, "None: no grid-line-y expected, got {}", hy);
    }

    // ----- Phase 7: zero-line wiring -----

    fn make_bar_data_crossing_zero() -> DataTable {
        let rows = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(-5))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(0))].into_iter().collect(),
            [("month".to_string(), json!("Mar")), ("revenue".to_string(), json!(10))].into_iter().collect(),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn count_zero_lines(el: &ChartElement) -> usize {
        count_elements(el, &|e| {
            matches!(e, ChartElement::Line { class, .. } if class.split_whitespace().any(|c| c == "zero-line"))
        })
    }

    /// With the default theme (`zero_line: None`), no zero-line element must
    /// ever be emitted — even when the data range obviously crosses zero.
    #[test]
    fn phase7_default_theme_emits_no_zero_line() {
        let renderer = CartesianRenderer::new();
        let data = make_bar_data_crossing_zero();
        let config = make_bar_config();
        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(count_zero_lines(&element), 0, "default theme must not emit zero-line");
    }

    /// With a non-default `zero_line` spec AND data that strictly crosses zero,
    /// exactly one `zero-line` Line must be emitted with the spec'd color/width.
    #[test]
    fn phase7_bar_crossing_zero_emits_one_zero_line() {
        use chartml_core::theme::{Theme, ZeroLineSpec};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data_crossing_zero();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.zero_line = Some(ZeroLineSpec { color: "#ff0000".into(), width: 1.5 });
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(count_zero_lines(&element), 1, "expected exactly one zero-line");

        // Verify the emitted element has the configured stroke + width.
        fn find_zero_line(el: &ChartElement) -> Option<(String, Option<f64>)> {
            match el {
                ChartElement::Line { class, stroke, stroke_width, .. }
                    if class.split_whitespace().any(|c| c == "zero-line") =>
                {
                    Some((stroke.clone(), *stroke_width))
                }
                ChartElement::Group { children, .. } | ChartElement::Svg { children, .. } => {
                    children.iter().find_map(find_zero_line)
                }
                _ => None,
            }
        }
        let (stroke, width) = find_zero_line(&element).expect("zero-line present");
        assert_eq!(stroke, "#ff0000");
        assert_eq!(width, Some(1.5));
    }

    /// Horizontal bar parity: crossing-zero data + non-default zero_line must
    /// emit exactly one zero-line, and for a horizontal bar chart (numeric axis
    /// is x) that line must run vertically — x1 == x2 and y1 != y2.
    #[test]
    fn phase7_horizontal_bar_crossing_zero_emits_one_zero_line() {
        use chartml_core::theme::{Theme, ZeroLineSpec};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data_crossing_zero();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            orientation: horizontal
            columns: month
            rows: revenue
        "#).unwrap();
        let mut theme = Theme::default();
        theme.zero_line = Some(ZeroLineSpec { color: "#ff0000".into(), width: 1.5 });
        let config = ChartConfig {
            visualize: viz,
            title: Some("Test Horizontal Bar".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string()],
            theme,
        };

        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(count_zero_lines(&element), 1, "expected exactly one zero-line");

        // Find the emitted zero-line Line and assert it runs vertically with
        // the spec'd stroke + width.
        struct ZeroLineGeom {
            x1: f64,
            y1: f64,
            x2: f64,
            y2: f64,
            stroke: String,
            stroke_width: Option<f64>,
        }
        fn find_zero_line_geom(el: &ChartElement) -> Option<ZeroLineGeom> {
            match el {
                ChartElement::Line { class, x1, y1, x2, y2, stroke, stroke_width, .. }
                    if class.split_whitespace().any(|c| c == "zero-line") =>
                {
                    Some(ZeroLineGeom {
                        x1: *x1,
                        y1: *y1,
                        x2: *x2,
                        y2: *y2,
                        stroke: stroke.clone(),
                        stroke_width: *stroke_width,
                    })
                }
                ChartElement::Group { children, .. } | ChartElement::Svg { children, .. } => {
                    children.iter().find_map(find_zero_line_geom)
                }
                _ => None,
            }
        }
        let ZeroLineGeom { x1, y1, x2, y2, stroke, stroke_width: width } =
            find_zero_line_geom(&element).expect("zero-line present");
        assert!(
            (x1 - x2).abs() < f64::EPSILON,
            "horizontal-bar zero-line must be vertical: x1={x1} x2={x2}",
        );
        assert!(
            (y1 - y2).abs() > f64::EPSILON,
            "horizontal-bar zero-line must have non-zero height: y1={y1} y2={y2}",
        );
        assert_eq!(stroke, "#ff0000");
        assert_eq!(width, Some(1.5));
    }

    /// With a non-default `zero_line` spec BUT data entirely positive (so the
    /// domain floor is 0 and doesn't strictly cross zero), no zero-line is emitted.
    #[test]
    fn phase7_bar_all_positive_emits_no_zero_line() {
        use chartml_core::theme::{Theme, ZeroLineSpec};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data(); // values: 100, 200, 150 — all positive
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.zero_line = Some(ZeroLineSpec { color: "#ff0000".into(), width: 1.5 });
        config.theme = t;

        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(
            count_zero_lines(&element),
            0,
            "all-positive data must not emit a zero-line",
        );
    }

    /// Line chart parity: crossing-zero data + non-default zero_line emits one line.
    #[test]
    fn phase7_line_crossing_zero_emits_one_zero_line() {
        use chartml_core::theme::{Theme, ZeroLineSpec};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data_crossing_zero();
        let mut config = make_line_config();
        let mut t = Theme::default();
        t.zero_line = Some(ZeroLineSpec { color: "#00ff00".into(), width: 2.0 });
        config.theme = t;
        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(count_zero_lines(&element), 1);
    }

    // ----- Phase 8: dot_halo wiring -----

    fn count_halos(el: &ChartElement) -> usize {
        count_elements(el, &|e| matches!(e, ChartElement::Path { class, .. } if class == "dot-halo"))
    }

    fn count_dot_markers(el: &ChartElement) -> usize {
        count_elements(el, &|e| matches!(e, ChartElement::Circle { class, .. } if class.contains("dot-marker")))
    }

    #[test]
    fn phase8_line_default_theme_emits_no_halo() {
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&make_bar_data(), &make_line_config()).unwrap();
        assert_eq!(count_halos(&element), 0, "default theme line chart must emit zero halos");
    }

    #[test]
    fn phase8_line_halo_matches_dot_count_and_ordering() {
        use chartml_core::theme::Theme;
        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_line_config();
        let mut t = Theme::default();
        t.dot_halo_color = Some("#ffffff".to_string());
        t.dot_halo_width = 1.5;
        config.theme = t;
        let element = renderer.render(&data, &config).unwrap();

        let dot_n = count_dot_markers(&element);
        let halo_n = count_halos(&element);
        assert!(dot_n > 0, "line chart should produce at least one dot-marker");
        assert_eq!(halo_n, dot_n, "one halo per dot-marker required");

        // Halo must precede its dot: in the lines group, walk children and
        // assert every dot-halo Path is immediately followed by a Circle.
        fn walk_lines_group(el: &ChartElement) -> Option<&Vec<ChartElement>> {
            match el {
                ChartElement::Group { class, children, .. } if class == "lines" => Some(children),
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    children.iter().find_map(walk_lines_group)
                }
                _ => None,
            }
        }
        let lines = walk_lines_group(&element).expect("lines group");
        let mut pair = 0;
        let mut iter = lines.iter().peekable();
        while let Some(el) = iter.next() {
            if let ChartElement::Path { class, .. } = el {
                if class == "dot-halo" {
                    match iter.peek() {
                        Some(ChartElement::Circle { class: cc, .. }) => {
                            assert!(cc.contains("dot-marker"));
                            pair += 1;
                        }
                        other => panic!("halo not followed by dot: {:?}", other.map(|_| "other")),
                    }
                }
            }
        }
        assert_eq!(pair, dot_n);

        // Verify stroke / stroke-width on first halo.
        fn first_halo(el: &ChartElement) -> Option<(String, f64)> {
            match el {
                ChartElement::Path { class, stroke, stroke_width, .. } if class == "dot-halo" => {
                    Some((stroke.clone().unwrap_or_default(), stroke_width.unwrap_or(-1.0)))
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    children.iter().find_map(first_halo)
                }
                _ => None,
            }
        }
        let (stroke, width) = first_halo(&element).unwrap();
        assert_eq!(stroke, "#ffffff");
        assert!((width - 1.5).abs() < 1e-9);
    }

    /// Area chart parity: crossing-zero data + non-default zero_line emits one line.
    #[test]
    fn phase7_area_crossing_zero_emits_one_zero_line() {
        use chartml_core::theme::{Theme, ZeroLineSpec};
        let renderer = CartesianRenderer::new();
        let data = make_bar_data_crossing_zero();
        let mut config = make_area_config();
        let mut t = Theme::default();
        t.zero_line = Some(ZeroLineSpec { color: "#0000ff".into(), width: 1.0 });
        config.theme = t;
        let element = renderer.render(&data, &config).unwrap();
        assert_eq!(count_zero_lines(&element), 1);
    }

    // ----- CHA-5: stacked bar corner rounding -----

    /// Helper: build a BarRectSpec for testing build_bar_element directly.
    fn test_bar_spec(stack_position: crate::bar::StackPosition) -> crate::bar::BarRectSpec {
        crate::bar::BarRectSpec {
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 200.0,
            is_horizontal: false,
            is_negative: false,
            fill: "#000".into(),
            class: "bar bar-rect".into(),
            data: None,
            stack_baseline: None,
            stack_position,
        }
    }

    /// Uniform radius + StackPosition::None → Rect with rx/ry (all 4 corners).
    #[test]
    fn cha5_uniform_radius_none_emits_rect_with_rx() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::None), &theme);
        match el {
            ChartElement::Rect { rx, ry, .. } => {
                assert_eq!(rx, Some(8.0), "Uniform + None must emit rx=8");
                assert_eq!(ry, Some(8.0), "Uniform + None must emit ry=8");
            }
            other => panic!("Uniform + None must emit Rect, got {:?}", other),
        }
    }

    /// Uniform radius + StackPosition::Only → Rect with rx/ry (all 4 corners).
    #[test]
    fn cha5_uniform_radius_only_emits_rect_with_rx() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Only), &theme);
        match el {
            ChartElement::Rect { rx, ry, .. } => {
                assert_eq!(rx, Some(8.0));
                assert_eq!(ry, Some(8.0));
            }
            other => panic!("Uniform + Only must emit Rect, got {:?}", other),
        }
    }

    /// Uniform radius + StackPosition::Middle → plain Rect without rx/ry.
    #[test]
    fn cha5_uniform_radius_middle_emits_plain_rect() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Middle), &theme);
        match el {
            ChartElement::Rect { rx, ry, .. } => {
                assert!(rx.is_none(), "Uniform + Middle must emit rx=None");
                assert!(ry.is_none(), "Uniform + Middle must emit ry=None");
            }
            other => panic!("Uniform + Middle must emit plain Rect, got {:?}", other),
        }
    }

    /// Uniform radius + StackPosition::Top → Path with 2 arcs at value-end
    /// (y0 edge for vertical positive).
    #[test]
    fn cha5_uniform_radius_top_emits_value_end_path() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Top), &theme);
        match el {
            ChartElement::Path { d, .. } => {
                assert_eq!(
                    d.matches("A 8,8").count(),
                    2,
                    "Uniform + Top must produce 2 arcs, got d={d}"
                );
                // Vertical positive top: arcs at y0 edge → path starts at (x, y+r)
                assert!(
                    d.starts_with("M 100,58"),
                    "value-end rounding for vertical positive should start at y+r=58, got {d}"
                );
            }
            other => panic!("Uniform + Top must emit Path, got {:?}", other),
        }
    }

    /// Uniform radius + StackPosition::Bottom → Path with 2 arcs at baseline-end
    /// (y1 edge for vertical positive).
    #[test]
    fn cha5_uniform_radius_bottom_emits_baseline_end_path() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Bottom), &theme);
        match el {
            ChartElement::Path { d, .. } => {
                assert_eq!(
                    d.matches("A 8,8").count(),
                    2,
                    "Uniform + Bottom must produce 2 arcs, got d={d}"
                );
                // Vertical positive bottom: arcs at y1 edge → path starts at (x, y) = (100, 50)
                // and references y1-r = 50+200-8 = 242
                assert!(
                    d.starts_with("M 100,50"),
                    "baseline-end rounding for vertical positive should start at (x,y)=(100,50), got {d}"
                );
                assert!(
                    d.contains(",242"),
                    "baseline-end rounding should contain y1-r=242, got {d}"
                );
            }
            other => panic!("Uniform + Bottom must emit Path, got {:?}", other),
        }
    }

    /// Top radius + StackPosition::None → Path with 2 arcs at value-end
    /// (unchanged from pre-CHA-5 behavior).
    #[test]
    fn cha5_top_radius_none_emits_value_end_path() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::None), &theme);
        match el {
            ChartElement::Path { d, .. } => {
                assert_eq!(d.matches("A 8,8").count(), 2);
                assert!(d.starts_with("M 100,58"));
            }
            other => panic!("Top + None must emit Path, got {:?}", other),
        }
    }

    /// Top radius + StackPosition::Top → same as None (value-end arcs).
    #[test]
    fn cha5_top_radius_top_emits_value_end_path() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Top), &theme);
        match el {
            ChartElement::Path { d, .. } => {
                assert_eq!(d.matches("A 8,8").count(), 2);
                assert!(d.starts_with("M 100,58"));
            }
            other => panic!("Top + Top must emit Path, got {:?}", other),
        }
    }

    /// Top radius + StackPosition::Middle → plain Rect with no rounding.
    #[test]
    fn cha5_top_radius_middle_emits_plain_rect() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Middle), &theme);
        match el {
            ChartElement::Rect { rx, ry, .. } => {
                assert!(rx.is_none());
                assert!(ry.is_none());
            }
            other => panic!("Top + Middle must emit plain Rect, got {:?}", other),
        }
    }

    /// Top radius + StackPosition::Bottom → Path with 2 arcs at baseline-end.
    #[test]
    fn cha5_top_radius_bottom_emits_baseline_end_path() {
        use chartml_core::theme::{BarCornerRadius, Theme};
        use crate::bar::{build_bar_element, StackPosition};

        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);

        let el = build_bar_element(test_bar_spec(StackPosition::Bottom), &theme);
        match el {
            ChartElement::Path { d, .. } => {
                assert_eq!(d.matches("A 8,8").count(), 2);
                // Vertical positive bottom: arcs at y1 edge
                assert!(
                    d.starts_with("M 100,50"),
                    "Top + Bottom baseline-end should start at (100,50), got {d}"
                );
                assert!(
                    d.contains(",242"),
                    "Top + Bottom baseline-end should contain y1-r=242, got {d}"
                );
            }
            other => panic!("Top + Bottom must emit Path, got {:?}", other),
        }
    }

    /// Integration test: stacked bar chart with Top(8) corner radius.
    /// - Top segments get Path with 2 arcs (value-end)
    /// - Bottom segments get Path with 2 arcs (baseline-end)
    /// - Non-stacked behavior is unchanged (all Paths from Top rounding)
    #[test]
    fn cha5_stacked_bar_top_rounding_positions() {
        use chartml_core::theme::{BarCornerRadius, Theme};

        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)),  ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(80)),  ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: stacked
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Top(8.0);
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme,
        };
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        // Collect all bar elements (could be Rect or Path)
        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        // 2 categories x 2 series = 4 bars
        assert_eq!(bars.len(), 4, "Should have 4 stacked bar segments, got {}", bars.len());

        // With 2 non-zero series per category:
        // - Bottom segments (product A, lower y0) should have arcs at baseline edge
        // - Top segments (product B, higher y1) should have arcs at value edge
        // All should be Path elements (since Top(8) always emits a Path for
        // non-Middle positions)
        let mut path_count = 0;
        let mut rect_count = 0;
        for b in &bars {
            match b {
                ChartElement::Path { d, .. } => {
                    assert_eq!(
                        d.matches("A 8,8").count(),
                        2,
                        "stacked bar Path must have exactly 2 arcs, got d={d}"
                    );
                    path_count += 1;
                }
                ChartElement::Rect { .. } => {
                    rect_count += 1;
                }
                _ => panic!("unexpected bar element type"),
            }
        }
        // All 4 should be Paths (2 top + 2 bottom, each with 2 arcs)
        assert_eq!(path_count, 4, "all stacked segments should be Paths with arcs");
        assert_eq!(rect_count, 0, "no plain Rects expected in 2-series stack with Top(8)");
    }

    /// Integration test: stacked bar chart with Uniform(8) corner radius.
    /// - Top segments get value-end Path
    /// - Bottom segments get baseline-end Path
    /// - (If 3 series: middle segments would be plain Rect)
    #[test]
    fn cha5_stacked_bar_uniform_rounding_positions() {
        use chartml_core::theme::{BarCornerRadius, Theme};

        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)),  ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(30)),  ("product".to_string(), json!("C"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(200)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(80)),  ("product".to_string(), json!("B"))].into_iter().collect(),
            [("month".to_string(), json!("Feb")), ("revenue".to_string(), json!(40)),  ("product".to_string(), json!("C"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: stacked
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string(), "#4A7C59".to_string()],
            theme,
        };
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        // 2 categories x 3 series = 6 bars
        assert_eq!(bars.len(), 6, "Should have 6 stacked bar segments, got {}", bars.len());

        // With 3 non-zero series per category:
        // - Bottom (A): Path with baseline-end arcs
        // - Middle (B): plain Rect (no rounding)
        // - Top (C): Path with value-end arcs
        let mut path_count = 0;
        let mut plain_rect_count = 0;
        for b in &bars {
            match b {
                ChartElement::Path { d, .. } => {
                    assert_eq!(d.matches("A 8,8").count(), 2);
                    path_count += 1;
                }
                ChartElement::Rect { rx, ry, .. } => {
                    assert!(rx.is_none(), "middle segment Rect must have rx=None");
                    assert!(ry.is_none(), "middle segment Rect must have ry=None");
                    plain_rect_count += 1;
                }
                _ => panic!("unexpected bar element type"),
            }
        }
        // 2 categories: each has 1 bottom Path + 1 middle Rect + 1 top Path
        assert_eq!(path_count, 4, "expected 4 Paths (2 top + 2 bottom)");
        assert_eq!(plain_rect_count, 2, "expected 2 plain Rects (middle segments)");
    }

    /// Non-stacked (grouped) bar chart with Uniform(8) radius should not be
    /// affected by stack position logic — all bars get rx/ry on all 4 corners.
    #[test]
    fn cha5_grouped_bar_uniform_radius_unchanged() {
        use chartml_core::theme::{BarCornerRadius, Theme};

        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(50)),  ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: grouped
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme,
        };
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        let mut radii = Vec::new();
        collect_bar_corner_radii(&element, &mut radii);
        assert_eq!(radii.len(), 2, "grouped bar should have 2 bars");
        for (rx, ry) in &radii {
            assert_eq!(*rx, Some(8.0), "grouped bar must keep Uniform rx=8");
            assert_eq!(*ry, Some(8.0), "grouped bar must keep Uniform ry=8");
        }
    }

    /// Single-series bar chart with Uniform(8) radius is not affected by
    /// stack position logic — all bars get rx/ry.
    #[test]
    fn cha5_single_series_uniform_radius_unchanged() {
        use chartml_core::theme::{BarCornerRadius, Theme};

        let renderer = CartesianRenderer::new();
        let data = make_bar_data();
        let mut config = make_bar_config();
        let mut t = Theme::default();
        t.bar_corner_radius = BarCornerRadius::Uniform(8.0);
        config.theme = t;
        let element = renderer.render(&data, &config).unwrap();

        let mut radii = Vec::new();
        collect_bar_corner_radii(&element, &mut radii);
        assert!(!radii.is_empty());
        for (rx, ry) in &radii {
            assert_eq!(*rx, Some(8.0));
            assert_eq!(*ry, Some(8.0));
        }
    }

    /// Stacked bar where one category has a zero-value series.
    /// The zero segment should be a plain Rect (Middle), while the non-zero
    /// segment should be Only (both ends rounded).
    #[test]
    fn cha5_stacked_bar_zero_series_gets_only_position() {
        use chartml_core::theme::{BarCornerRadius, Theme};

        let rows: Vec<Row> = vec![
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(100)), ("product".to_string(), json!("A"))].into_iter().collect(),
            [("month".to_string(), json!("Jan")), ("revenue".to_string(), json!(0)),   ("product".to_string(), json!("B"))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            mode: stacked
            columns: month
            rows: revenue
            marks:
              color: product
        "#).unwrap();
        let mut theme = Theme::default();
        theme.bar_corner_radius = BarCornerRadius::Uniform(8.0);
        let config = ChartConfig {
            visualize: viz,
            title: None,
            width: 800.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string()],
            theme,
        };
        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        let mut bars = Vec::new();
        collect_bar_elements(&element, &mut bars);
        // 1 category x 2 series = 2 bars
        assert_eq!(bars.len(), 2, "Should have 2 bar segments");

        // The non-zero segment (A=100) should be StackPosition::Only → Rect with rx/ry
        // The zero segment (B=0) should be StackPosition::Middle → plain Rect
        let mut rx_count = 0;
        let mut plain_count = 0;
        for b in &bars {
            if let ChartElement::Rect { rx, ry, .. } = b {
                if rx.is_some() {
                    assert_eq!(*rx, Some(8.0));
                    assert_eq!(*ry, Some(8.0));
                    rx_count += 1;
                } else {
                    plain_count += 1;
                }
            }
        }
        assert_eq!(rx_count, 1, "expected 1 bar with rx/ry (Only position)");
        assert_eq!(plain_count, 1, "expected 1 plain bar (zero-height, Middle)");
    }

    /// When axes.rows.min is set above 0 (e.g. 90..100 for retention %),
    /// bars must grow from the visible domain minimum — not from 0.
    /// Without the fix, `scale.map(0.0)` produces a pixel coordinate
    /// thousands of pixels off-screen, creating absurdly tall bars.
    #[test]
    fn bar_chart_min_max_baseline_clamps_to_domain_min() {
        let rows: Vec<Row> = vec![
            [("plan".to_string(), json!("enterprise")), ("pct".to_string(), json!(94.6))].into_iter().collect(),
            [("plan".to_string(), json!("professional")), ("pct".to_string(), json!(93.8))].into_iter().collect(),
            [("plan".to_string(), json!("free")), ("pct".to_string(), json!(92.0))].into_iter().collect(),
            [("plan".to_string(), json!("starter")), ("pct".to_string(), json!(91.9))].into_iter().collect(),
        ];
        let data = DataTable::from_rows(&rows).unwrap();

        let viz: VisualizeSpec = serde_yaml::from_str(r#"
            type: bar
            columns: plan
            rows: pct
            axes:
              rows:
                min: 90
                max: 100
                format: ".1f"
        "#).unwrap();
        let config = ChartConfig {
            visualize: viz,
            title: Some("Retention".to_string()),
            width: 800.0,
            height: 300.0,
            colors: vec!["#2E7D9A".to_string()],
            theme: chartml_core::theme::Theme::default(),
        };

        let renderer = CartesianRenderer::new();
        let element = renderer.render(&data, &config).unwrap();

        // Collect all bar-rect elements
        let mut bars: Vec<&ChartElement> = Vec::new();
        collect_bar_elements(&element, &mut bars);
        assert_eq!(bars.len(), 4, "Should have 4 bars for 4 data points, got {}", bars.len());

        // The chart is 300px tall. inner_height is roughly 300 - margins (~220px).
        // All bar heights must be within inner_height — never thousands of pixels.
        let inner_height = 300.0; // conservative upper bound (actual inner_height is smaller)
        for bar in &bars {
            if let ChartElement::Rect { height, .. } = bar {
                assert!(
                    *height <= inner_height,
                    "Bar height {} exceeds chart inner height {}; baseline is off-screen",
                    height, inner_height
                );
                assert!(
                    *height > 0.0,
                    "Bar height should be positive, got {}",
                    height
                );
            }
        }

        // Verify proportionality: the tallest bar (94.6 - 90 = 4.6 above domain_min)
        // should be taller than the shortest bar (91.9 - 90 = 1.9 above domain_min).
        let heights: Vec<f64> = bars.iter().filter_map(|b| {
            if let ChartElement::Rect { height, .. } = b {
                Some(*height)
            } else {
                None
            }
        }).collect();
        let max_h = heights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_h = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            max_h > min_h,
            "Tallest bar ({}) should be taller than shortest bar ({})",
            max_h, min_h
        );
        // Ratio of heights should approximate ratio of (value - domain_min):
        // max_h / min_h ~ (94.6 - 90) / (91.9 - 90) = 4.6 / 1.9 ~ 2.42
        let expected_ratio = 4.6 / 1.9;
        let actual_ratio = max_h / min_h;
        assert!(
            (actual_ratio - expected_ratio).abs() < 0.5,
            "Height ratio {:.2} should be close to expected {:.2}",
            actual_ratio, expected_ratio
        );
    }
}
