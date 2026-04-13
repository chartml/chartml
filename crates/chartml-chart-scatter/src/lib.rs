use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::scales::{ScaleLinear, ScaleSqrt};
use chartml_core::spec::{VisualizeSpec, FieldRef, MarkEncoding};
use chartml_core::layout::margins::{MarginConfig, calculate_margins};
use chartml_core::layout::labels::{approximate_text_width_at, format_tick_value_si};
use chartml_core::layout::legend::{LegendMark, LegendConfig, calculate_legend_layout, generate_legend_elements};
pub struct ScatterRenderer;

impl ScatterRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ScatterRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartRenderer for ScatterRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let x_field = get_field_name(&config.visualize.columns)?;
        let y_field = get_field_name(&config.visualize.rows)?;
        let color_field = get_color_field(config);
        let size_field = get_size_field(config);

        let width = config.width;
        let height = config.height;

        // Color mapping — compute early so has_legend is accurate
        let color_categories: Vec<String> = if let Some(ref cf) = color_field {
            data.unique_values(cf)
        } else {
            vec![]
        };

        let has_legend = color_categories.len() > 1;
        let legend_height = if has_legend {
            let legend_config = LegendConfig::default();
            calculate_legend_layout(&color_categories, &config.colors, width, &legend_config).total_height
        } else {
            0.0
        };
        let margin_config = MarginConfig {
            legend_height,
            chart_height: height,
            ..Default::default()
        };
        let margins = calculate_margins(&margin_config);
        let inner_width = margins.inner_width(width);
        let inner_height = margins.inner_height(height);

        // Compute domains
        let x_extent = data.extent(&x_field)
            .ok_or_else(|| ChartError::DataError(format!("No numeric data for field '{}'", x_field)))?;
        let y_extent = data.extent(&y_field)
            .ok_or_else(|| ChartError::DataError(format!("No numeric data for field '{}'", y_field)))?;

        // Use actual data extent for axis domains so tightly-clustered data
        // fills the plot area instead of being crammed near a forced zero origin.
        let x_domain = (x_extent.0, x_extent.1);
        let y_domain = (y_extent.0, y_extent.1);
        // Size scale (if marks.size present)
        let size_scale = size_field.as_ref().and_then(|f| {
            data.extent(f).map(|ext| ScaleSqrt::new(ext, (3.0, 20.0))) // radius 3-20px
        });

        // Compute maximum point radius to inset scale ranges, ensuring circles
        // don't extend past the SVG edges. Use at least 5% of inner dimension.
        let max_radius = match (&size_field, &size_scale) {
            (Some(sf), Some(ss)) => {
                let mut mr = 5.0_f64;
                for i in 0..data.num_rows() {
                    if let Some(v) = data.get_f64(i, sf) {
                        mr = mr.max(ss.map(v));
                    }
                }
                mr
            }
            _ => 5.0,
        };
        let x_inset = max_radius.max(inner_width * 0.05);
        let y_inset = max_radius.max(inner_height * 0.05);

        let x_scale = ScaleLinear::new(x_domain, (margins.left + x_inset, margins.left + inner_width - x_inset)).nice(5);
        let y_scale = ScaleLinear::new(y_domain, (margins.top + inner_height - y_inset, margins.top + y_inset)).nice(5); // inverted for SVG

        // Generate scatter points
        let mut point_elements = Vec::new();
        for i in 0..data.num_rows() {
            let x_val = data.get_f64(i, &x_field);
            let y_val = data.get_f64(i, &y_field);

            if let (Some(x), Some(y)) = (x_val, y_val) {
                let cx = x_scale.map(x);
                let cy = y_scale.map(y);

                let r = match (&size_field, &size_scale) {
                    (Some(sf), Some(ss)) => {
                        data.get_f64(i, sf).map(|v| ss.map(v)).unwrap_or(5.0)
                    }
                    _ => 5.0,
                };

                let color_idx = if let Some(ref cf) = color_field {
                    data.get_string(i, cf)
                        .and_then(|v| color_categories.iter().position(|c| c == &v))
                        .unwrap_or(0)
                } else {
                    0
                };
                let fill = config.colors.get(color_idx % config.colors.len())
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                let label = data.get_string(i, &x_field).unwrap_or_default();
                let value = format!("{}", y);
                let mut el_data = ElementData::new(label, value);
                if let Some(ref cf) = color_field {
                    if let Some(series_name) = data.get_string(i, cf) {
                        el_data = el_data.with_series(series_name);
                    }
                }

                point_elements.push(ChartElement::Circle {
                    cx,
                    cy,
                    r,
                    fill,
                    stroke: Some(config.theme.bg.clone()),
                    class: "chartml-scatter-point dot-marker".to_string(),
                    data: Some(el_data),
                });
            }
        }

        // Build SVG
        let mut children = Vec::new();

        // Grid lines + axes
        let x_ticks = x_scale.ticks(((inner_width / 50.0).floor() as usize).clamp(4, 10));
        let y_ticks = y_scale.ticks(((inner_height / 50.0).floor() as usize).clamp(4, 10));
        let mut axis_elements = Vec::new();

        // Compute tick steps for formatting
        let y_tick_step = compute_tick_step(&y_ticks);
        let x_tick_step = compute_tick_step(&x_ticks);

        // Y-axis label skip factor: skip labels that would overlap vertically
        let y_label_height = 18.0; // 14px font + 4px spacing
        let y_skip = if y_ticks.len() > 1 {
            let px_per_tick = inner_height / (y_ticks.len() - 1) as f64;
            (y_label_height / px_per_tick).ceil() as usize
        } else {
            1
        }.max(1);

        // Horizontal grid lines + y-axis ticks
        for (i, &val) in y_ticks.iter().enumerate() {
            let y = y_scale.map(val);
            // Grid line — always rendered
            axis_elements.push(ChartElement::Line {
                x1: margins.left, y1: y, x2: margins.left + inner_width, y2: y,
                stroke: config.theme.grid.clone(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "grid-line".to_string(),
            });
            // Tick mark — always rendered
            axis_elements.push(ChartElement::Line {
                x1: margins.left - 5.0, y1: y, x2: margins.left, y2: y,
                stroke: config.theme.tick.clone(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "tick".to_string(),
            });
            // Label — only if not skipped
            if i % y_skip == 0 {
                let label = format_tick_value_si(val, y_tick_step);
                let ts = TextStyle::for_role(&config.theme, TextRole::TickValue);
                axis_elements.push(ChartElement::Text {
                    x: margins.left - 8.0, y,
                    content: label, anchor: TextAnchor::End,
                    dominant_baseline: Some("middle".to_string()),
                    transform: None,
                    font_family: ts.font_family,
                    font_size: ts.font_size,
                    font_weight: ts.font_weight,
                    letter_spacing: ts.letter_spacing,
                    text_transform: ts.text_transform,
                    fill: Some(config.theme.text_secondary.clone()), class: "tick-label tick-value".to_string(), data: None,
                });
            }
        }

        // X-axis label skip factor: skip labels that would overlap horizontally
        let x_label_widths: Vec<f64> = x_ticks.iter()
            .map(|&v| approximate_text_width_at(&format_tick_value_si(v, x_tick_step), 11.0))
            .collect();
        let x_widest = x_label_widths.iter().cloned().fold(0.0_f64, f64::max);
        let x_skip = if x_ticks.len() > 1 {
            let px_per_tick = inner_width / (x_ticks.len() - 1) as f64;
            let needed = x_widest + 8.0; // label width + small gap
            (needed / px_per_tick).ceil() as usize
        } else {
            1
        }.max(1);

        // Vertical grid lines + x-axis ticks
        let x_axis_y = margins.top + inner_height;
        for (i, &val) in x_ticks.iter().enumerate() {
            let x = x_scale.map(val);
            // Grid line — always rendered
            axis_elements.push(ChartElement::Line {
                x1: x, y1: margins.top, x2: x, y2: x_axis_y,
                stroke: config.theme.grid.clone(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "grid-line".to_string(),
            });
            // Tick mark — always rendered
            axis_elements.push(ChartElement::Line {
                x1: x, y1: x_axis_y, x2: x, y2: x_axis_y + 5.0,
                stroke: config.theme.tick.clone(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "tick".to_string(),
            });
            // Label — only if not skipped
            if i % x_skip == 0 {
                let label = format_tick_value_si(val, x_tick_step);
                let ts = TextStyle::for_role(&config.theme, TextRole::TickValue);
                axis_elements.push(ChartElement::Text {
                    x, y: x_axis_y + 18.0,
                    content: label, anchor: TextAnchor::Middle,
                    dominant_baseline: None, transform: None,
                    font_family: ts.font_family,
                    font_size: ts.font_size,
                    font_weight: ts.font_weight,
                    letter_spacing: ts.letter_spacing,
                    text_transform: ts.text_transform,
                    fill: Some(config.theme.text_secondary.clone()),
                    class: "tick-label tick-value".to_string(), data: None,
                });
            }
        }

        // Axis lines
        axis_elements.push(ChartElement::Line {
            x1: margins.left, y1: margins.top, x2: margins.left, y2: x_axis_y,
            stroke: config.theme.axis_line.clone(), stroke_width: Some(1.0),
            stroke_dasharray: None, class: "axis-line".to_string(),
        });
        axis_elements.push(ChartElement::Line {
            x1: margins.left, y1: x_axis_y, x2: margins.left + inner_width, y2: x_axis_y,
            stroke: config.theme.axis_line.clone(), stroke_width: Some(1.0),
            stroke_dasharray: None, class: "axis-line".to_string(),
        });

        children.push(ChartElement::Group {
            class: "axes".to_string(),
            transform: None,
            children: axis_elements,
        });

        // Title is rendered as HTML outside the SVG — not added here.

        // Points group
        children.push(ChartElement::Group {
            class: "chartml-scatter-points".to_string(),
            transform: None,
            children: point_elements,
        });

        // Legend — reuse color_categories computed earlier
        if color_categories.len() > 1 {
            let legend_config = LegendConfig::default();
            let legend_layout = calculate_legend_layout(&color_categories, &config.colors, width, &legend_config);
            let legend_y = height - legend_layout.total_height - 8.0;
            let legend_elements = generate_legend_elements(
                &color_categories,
                &config.colors,
                width,
                legend_y,
                LegendMark::Circle,
                &config.theme,
            );
            children.push(ChartElement::Group {
                class: "legend".to_string(),
                transform: None,
                children: legend_elements,
            });
        }

        Ok(ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, width, height),
            width: Some(width),
            height: Some(height),
            class: "chartml-chart chartml-scatter-chart".to_string(),
            children,
        })
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        Some(Dimensions::new(400.0))
    }
}

/// Extract the field name from an optional FieldRef.
fn get_field_name(field_ref: &Option<FieldRef>) -> Result<String, ChartError> {
    match field_ref {
        Some(FieldRef::Simple(name)) => Ok(name.clone()),
        Some(FieldRef::Detailed(spec)) => Ok(spec.field.clone()),
        Some(FieldRef::Multiple(items)) => {
            // Use the first item
            match items.first() {
                Some(chartml_core::spec::FieldRefItem::Simple(name)) => Ok(name.clone()),
                Some(chartml_core::spec::FieldRefItem::Detailed(spec)) => Ok(spec.field.clone()),
                None => Err(ChartError::InvalidSpec("Empty field reference list".into())),
            }
        }
        None => Err(ChartError::InvalidSpec("Missing required field reference".into())),
    }
}

/// Extract the color field name from marks.color encoding, if present.
fn get_color_field(config: &ChartConfig) -> Option<String> {
    config.visualize.marks.as_ref().and_then(|marks| {
        marks.color.as_ref().map(|enc| match enc {
            MarkEncoding::Simple(name) => name.clone(),
            MarkEncoding::Detailed(spec) => spec.field.clone(),
        })
    })
}

/// Extract the size field name from marks.size encoding, if present.
fn get_size_field(config: &ChartConfig) -> Option<String> {
    config.visualize.marks.as_ref().and_then(|marks| {
        marks.size.as_ref().map(|enc| match enc {
            MarkEncoding::Simple(name) => name.clone(),
            MarkEncoding::Detailed(spec) => spec.field.clone(),
        })
    })
}

/// Compute the tick step from a slice of ticks.
fn compute_tick_step(ticks: &[f64]) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1] - ticks[0]).abs()
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use chartml_core::data::Row;
    use chartml_core::spec::{VisualizeSpec, MarksSpec, MarkEncoding};

    fn make_row(pairs: &[(&str, serde_json::Value)]) -> Row {
        let mut map = HashMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v.clone());
        }
        map
    }

    fn make_scatter_data() -> DataTable {
        let rows = vec![
            make_row(&[("price", serde_json::json!(10.0)), ("units", serde_json::json!(100.0)), ("category", serde_json::json!("A"))]),
            make_row(&[("price", serde_json::json!(20.0)), ("units", serde_json::json!(200.0)), ("category", serde_json::json!("B"))]),
            make_row(&[("price", serde_json::json!(30.0)), ("units", serde_json::json!(150.0)), ("category", serde_json::json!("A"))]),
            make_row(&[("price", serde_json::json!(40.0)), ("units", serde_json::json!(300.0)), ("category", serde_json::json!("B"))]),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn make_scatter_config() -> ChartConfig {
        ChartConfig {
            visualize: VisualizeSpec {
                chart_type: "scatter".to_string(),
                mode: None,
                orientation: None,
                columns: Some(FieldRef::Simple("price".to_string())),
                rows: Some(FieldRef::Simple("units".to_string())),
                marks: Some(MarksSpec {
                    color: Some(MarkEncoding::Simple("category".to_string())),
                    size: None,
                    shape: None,
                    text: None,
                }),
                axes: None,
                annotations: None,
                style: None,
                value: None,
                label: None,
                format: None,
                compare_with: None,
                invert_trend: None,
                data_labels: None,
            },
            title: Some("Scatter Test".to_string()),
            width: 800.0,
            height: 400.0,
            colors: vec![
                "#2E7D9A".to_string(),
                "#E8533E".to_string(),
                "#4CAF50".to_string(),
            ],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    fn make_bubble_data() -> DataTable {
        let rows = vec![
            make_row(&[("x", serde_json::json!(5.0)), ("y", serde_json::json!(10.0)), ("size", serde_json::json!(100.0))]),
            make_row(&[("x", serde_json::json!(15.0)), ("y", serde_json::json!(20.0)), ("size", serde_json::json!(400.0))]),
            make_row(&[("x", serde_json::json!(25.0)), ("y", serde_json::json!(15.0)), ("size", serde_json::json!(200.0))]),
        ];
        DataTable::from_rows(&rows).unwrap()
    }

    fn make_bubble_config() -> ChartConfig {
        ChartConfig {
            visualize: VisualizeSpec {
                chart_type: "scatter".to_string(),
                mode: None,
                orientation: None,
                columns: Some(FieldRef::Simple("x".to_string())),
                rows: Some(FieldRef::Simple("y".to_string())),
                marks: Some(MarksSpec {
                    color: None,
                    size: Some(MarkEncoding::Simple("size".to_string())),
                    shape: None,
                    text: None,
                }),
                axes: None,
                annotations: None,
                style: None,
                value: None,
                label: None,
                format: None,
                compare_with: None,
                invert_trend: None,
                data_labels: None,
            },
            title: None,
            width: 600.0,
            height: 400.0,
            colors: vec!["#2E7D9A".to_string()],
            theme: chartml_core::theme::Theme::default(),
        }
    }

    #[test]
    fn scatter_chart_renders() {
        let renderer = ScatterRenderer::new();
        let result = renderer.render(&make_scatter_data(), &make_scatter_config());
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let element = result.unwrap();
        let circle_count = count_elements(&element, &|e| matches!(e, ChartElement::Circle { .. }));
        assert_eq!(circle_count, 6); // 4 data points + 2 legend circles (categories A, B)
    }

    #[test]
    fn scatter_with_size_encoding() {
        let renderer = ScatterRenderer::new();
        let result = renderer.render(&make_bubble_data(), &make_bubble_config());
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let element = result.unwrap();
        let circle_count = count_elements(&element, &|e| matches!(e, ChartElement::Circle { .. }));
        assert!(circle_count > 0);
    }

    #[test]
    fn scatter_data_series_populated_with_color_encoding() {
        let renderer = ScatterRenderer::new();
        let element = renderer.render(&make_scatter_data(), &make_scatter_config()).unwrap();
        // Collect series values from data circles (not legend circles)
        let mut series_values = Vec::new();
        fn collect_series(el: &ChartElement, out: &mut Vec<Option<String>>) {
            match el {
                ChartElement::Circle { data: Some(d), class, .. } if !class.contains("legend") => {
                    out.push(d.series.clone());
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    for child in children { collect_series(child, out); }
                }
                _ => {}
            }
        }
        collect_series(&element, &mut series_values);
        assert_eq!(series_values.len(), 4, "Expected 4 data circles");
        for (i, series) in series_values.iter().enumerate() {
            assert!(series.is_some(), "Circle {} has null data.series", i);
        }
        // Verify actual category values are present
        let series_strs: Vec<&str> = series_values.iter().map(|s| s.as_deref().unwrap()).collect();
        assert!(series_strs.contains(&"A"));
        assert!(series_strs.contains(&"B"));
    }

    #[test]
    fn scatter_empty_data_errors() {
        let renderer = ScatterRenderer::new();
        let data = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        let result = renderer.render(&data, &make_scatter_config());
        assert!(result.is_err());
    }
}
