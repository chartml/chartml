use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, Dimensions};
use chartml_core::error::ChartError;
use chartml_core::plugin::{ChartConfig, ChartRenderer};
use chartml_core::spec::VisualizeSpec;

mod bar;
mod line;
mod area;
pub(crate) mod helpers;

pub use bar::render_bar;
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
                ChartElement::Rect { width, class, .. } if class == "bar" => {
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
        let rect_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { class, .. } if class == "bar"));
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
        let rect_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { class, .. } if class == "bar"));
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
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { class, .. } if class == "chartml-line-path"));
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
            matches!(e, ChartElement::Text { class, .. } if class == "tick-label")
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
}
