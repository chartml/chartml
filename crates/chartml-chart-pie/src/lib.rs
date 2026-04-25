use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::shapes::{ArcGenerator, PieLayout};
use chartml_core::spec::{VisualizeSpec, FieldRef};
use chartml_core::layout::{calculate_legend_layout, LegendConfig, LegendAlignment, TextMetrics};

#[derive(Default)]
pub struct PieRenderer;

impl PieRenderer {
    pub fn new() -> Self { Self }
}

impl ChartRenderer for PieRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let chart_type = &config.visualize.chart_type;
        let is_doughnut = chart_type == "doughnut";

        // Get fields
        let col_field = get_field_name(&config.visualize.columns)?;
        let row_field = get_field_name(&config.visualize.rows)?;

        // Extract data
        let mut labels = Vec::new();
        let mut values = Vec::new();
        for i in 0..data.num_rows() {
            if let (Some(label), Some(value)) = (data.get_string(i, &col_field), data.get_f64(i, &row_field)) {
                labels.push(label);
                values.push(value);
            }
        }

        if values.is_empty() {
            return Err(ChartError::DataError("No data for pie chart".into()));
        }

        let width = config.width;
        let height = config.height;

        // Reserve space at the bottom for the legend + 5% bottom margin.
        // The 50px is a conservative upper-bound for single-row legend height
        // (30px gap + ~20px legend row); multi-row legends are accommodated by
        // the actual legend_layout.total_height used for positioning below.
        let bottom_margin = height * 0.05;
        let legend_reserved = 50.0 + bottom_margin;
        let radius = (width.min(height - legend_reserved) / 2.0) - 40.0;
        let inner_radius = if is_doughnut { radius * 0.5 } else { 0.0 };
        let cx = width / 2.0;
        // Shift pie center up slightly to make room for legend below
        let cy = (height - legend_reserved) / 2.0;

        // Compute pie layout
        let pie = PieLayout::new();
        let slices = pie.layout(&values);

        // Generate arc paths
        let arc = ArcGenerator::new(inner_radius, radius);
        let mut slice_elements = Vec::new();

        for (i, slice) in slices.iter().enumerate() {
            let path_d = arc.generate(slice.start_angle, slice.end_angle);
            let color = config.colors.get(i % config.colors.len())
                .cloned()
                .unwrap_or_else(|| "#999".to_string());

            let data = ElementData::new(&labels[slice.index], format!("{}", values[slice.index]))
                .with_series(&labels[slice.index]);

            slice_elements.push(ChartElement::Path {
                d: path_d,
                fill: Some(color),
                stroke: Some(config.theme.bg.clone()),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                opacity: None,
                class: "chartml-pie-slice".to_string(),
                data: Some(data),
                animation_origin: None,
            });
        }

        // Title is rendered as HTML outside the SVG — not added here.
        let mut children = Vec::new();

        // Pie group (centered)
        children.push(ChartElement::Group {
            class: "chartml-pie".to_string(),
            transform: Some(Transform::Translate(cx, cy)),
            children: slice_elements,
        });

        // Legend — rendered below the pie, horizontally centered
        let legend_config = LegendConfig {
            alignment: LegendAlignment::Center,
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        // Build ordered labels and colors (original data order matches color palette order)
        let legend_colors: Vec<String> = (0..labels.len())
            .map(|i| config.colors.get(i % config.colors.len()).cloned().unwrap_or_else(|| "#999".to_string()))
            .collect();
        let legend_layout = calculate_legend_layout(&labels, &legend_colors, width, &legend_config);
        // Position legend so the last text baseline sits at least 5% from the bottom edge
        let legend_y = height - legend_layout.total_height - bottom_margin;
        for item in legend_layout.items.iter().filter(|i| i.visible) {
            // Colored swatch rect
            children.push(ChartElement::Rect {
                x: item.x,
                y: legend_y + item.y,
                width: legend_config.symbol_size,
                height: legend_config.symbol_size,
                fill: item.color.clone(),
                stroke: None,
                rx: None,
                ry: None,
                class: "legend-symbol".to_string(),
                data: None,
                animation_origin: None,
            });
            // Label text
            let ts = TextStyle::for_role(&config.theme, TextRole::LegendLabel);
            children.push(ChartElement::Text {
                x: item.x + legend_config.symbol_size + legend_config.symbol_text_gap,
                y: legend_y + item.y + 10.0, // vertical center of 20px row_height
                content: item.label.clone(),
                anchor: TextAnchor::Start,
                dominant_baseline: None,
                transform: None,
                font_family: ts.font_family,
                font_size: ts.font_size,
                font_weight: ts.font_weight,
                letter_spacing: ts.letter_spacing,
                text_transform: ts.text_transform,
                fill: Some(config.theme.text_secondary.clone()),
                class: "legend-label".to_string(),
                data: None,
            });
        }

        Ok(ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, width, height),
            width: Some(width),
            height: Some(height),
            class: "chartml-chart chartml-pie-chart".to_string(),
            children,
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(400.0))
    }
}

fn get_field_name(field_ref: &Option<FieldRef>) -> Result<String, ChartError> {
    fn field_or_err(spec: &chartml_core::spec::FieldSpec) -> Result<String, ChartError> {
        spec.field
            .clone()
            .ok_or_else(|| ChartError::MissingField("field (range-mark specs are not supported for pie charts)".into()))
    }
    match field_ref {
        Some(FieldRef::Simple(name)) => Ok(name.clone()),
        Some(FieldRef::Detailed(spec)) => field_or_err(spec),
        Some(FieldRef::Multiple(items)) => {
            match items.first() {
                Some(chartml_core::spec::FieldRefItem::Simple(s)) => Ok(s.clone()),
                Some(chartml_core::spec::FieldRefItem::Detailed(spec)) => field_or_err(spec),
                None => Err(ChartError::MissingField("field".into())),
            }
        }
        None => Err(ChartError::MissingField("columns/rows field".into())),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use chartml_core::data::Row;
    use chartml_core::element::count_elements;
    use serde_json::json;

    fn make_pie_data() -> DataTable {
        let rows: Vec<Row> = vec![
            [("region".to_string(), json!("North")), ("revenue".to_string(), json!(100))].into_iter().collect(),
            [("region".to_string(), json!("South")), ("revenue".to_string(), json!(200))].into_iter().collect(),
            [("region".to_string(), json!("East")), ("revenue".to_string(), json!(150))].into_iter().collect(),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn make_pie_config(chart_type: &str) -> ChartConfig {
        let viz: chartml_core::spec::VisualizeSpec = serde_yaml::from_str(&format!(r#"
            type: {}
            columns: region
            rows: revenue
        "#, chart_type)).unwrap();
        ChartConfig {
            visualize: viz,
            title: Some("Test Pie".to_string()),
            width: 400.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string(), "#D4A445".to_string(), "#4A7C59".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    #[test]
    fn pie_chart_renders() {
        let renderer = PieRenderer::new();
        let result = renderer.render(&make_pie_data(), &make_pie_config("pie"));
        assert!(result.is_ok(), "Pie render failed: {:?}", result.err());
        let element = result.unwrap();
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { .. }));
        assert_eq!(path_count, 3, "Should have 3 slices");
    }

    #[test]
    fn doughnut_chart_renders() {
        let renderer = PieRenderer::new();
        let result = renderer.render(&make_pie_data(), &make_pie_config("doughnut"));
        assert!(result.is_ok());
        let element = result.unwrap();
        let path_count = count_elements(&element, &|e| matches!(e, ChartElement::Path { .. }));
        assert_eq!(path_count, 3);
    }

    #[test]
    fn pie_has_no_title_in_svg() {
        // Title is rendered as HTML outside the SVG.
        let renderer = PieRenderer::new();
        let element = renderer.render(&make_pie_data(), &make_pie_config("pie")).unwrap();
        let text_count = count_elements(&element, &|e| matches!(e, ChartElement::Text { class, .. } if class == "chart-title"));
        assert_eq!(text_count, 0);
    }

    #[test]
    fn pie_has_legend() {
        let renderer = PieRenderer::new();
        let element = renderer.render(&make_pie_data(), &make_pie_config("pie")).unwrap();
        // 3 slices = 3 legend swatches (Rect) + 3 legend labels (Text with class "legend-label")
        let swatch_count = count_elements(&element, &|e| matches!(e, ChartElement::Rect { class, .. } if class == "legend-symbol"));
        assert_eq!(swatch_count, 3, "Should have 3 legend swatches (one per slice)");
        let label_count = count_elements(&element, &|e| matches!(e, ChartElement::Text { class, .. } if class == "legend-label"));
        assert_eq!(label_count, 3, "Should have 3 legend labels (one per slice)");
    }

    #[test]
    fn pie_legend_respects_5pct_bottom_margin() {
        let renderer = PieRenderer::new();
        let config = make_pie_config("pie");
        let height = config.height;
        let element = renderer.render(&make_pie_data(), &config).unwrap();
        // Find the maximum y coordinate of any legend-label text element
        let mut max_text_y: f64 = 0.0;
        fn collect_max_y(el: &ChartElement, max_y: &mut f64) {
            match el {
                ChartElement::Text { y, class, .. } if class == "legend-label" => {
                    if *y > *max_y { *max_y = *y; }
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    for child in children { collect_max_y(child, max_y); }
                }
                _ => {}
            }
        }
        collect_max_y(&element, &mut max_text_y);
        let bottom_gap = height - max_text_y;
        assert!(
            bottom_gap >= height * 0.05,
            "Bottom gap {:.1}px ({:.1}%) is below 5% threshold on {:.0}px chart",
            bottom_gap, bottom_gap / height * 100.0, height
        );
    }

    #[test]
    fn pie_legend_colors_match_slices() {
        let renderer = PieRenderer::new();
        let config = make_pie_config("pie");
        let element = renderer.render(&make_pie_data(), &config).unwrap();
        // Collect legend swatch fills in document order
        let mut fills = Vec::new();
        fn collect_fills(el: &ChartElement, fills: &mut Vec<String>) {
            match el {
                ChartElement::Rect { fill, class, .. } if class == "legend-symbol" => {
                    fills.push(fill.clone());
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    for child in children { collect_fills(child, fills); }
                }
                _ => {}
            }
        }
        collect_fills(&element, &mut fills);
        assert_eq!(fills.len(), 3, "Expected 3 legend swatches");
        // Colors must match the configured palette in order
        assert_eq!(fills[0], config.colors[0]);
        assert_eq!(fills[1], config.colors[1]);
        assert_eq!(fills[2], config.colors[2]);
    }
}
