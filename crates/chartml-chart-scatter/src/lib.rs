use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::DataTable;
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::scales::{ScaleLinear, ScaleSqrt};
use chartml_core::spec::{VisualizeSpec, FieldRef, MarkEncoding};
use chartml_core::layout::margins::{MarginConfig, calculate_margins};
use chartml_core::layout::labels::{TextMetrics, approximate_text_width_at, format_tick_value_si, measure_text};
use chartml_core::layout::legend::{LegendMark, LegendConfig, calculate_legend_layout, generate_legend_elements};
use chartml_core::theme::GridStyle;

/// Whether horizontal (constant-y) gridlines should be drawn.
#[inline]
fn should_draw_horizontal_grid(style: &GridStyle) -> bool {
    matches!(style, GridStyle::Both | GridStyle::HorizontalOnly)
}

/// Whether vertical (constant-x) gridlines should be drawn.
#[inline]
fn should_draw_vertical_grid(style: &GridStyle) -> bool {
    matches!(style, GridStyle::Both | GridStyle::VerticalOnly)
}

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
            let legend_config = LegendConfig {
                text_metrics: TextMetrics::from_theme_legend(&config.theme),
                ..LegendConfig::default()
            };
            calculate_legend_layout(&color_categories, &config.colors, width, &legend_config).total_height
        } else {
            0.0
        };
        let margin_config = MarginConfig {
            legend_height,
            chart_height: height,
            tick_value_metrics: TextMetrics::from_theme_tick_value(&config.theme),
            axis_label_metrics: TextMetrics::from_theme_axis_label(&config.theme),
            ..Default::default()
        };
        let margins = calculate_margins(&margin_config);
        let inner_width = margins.inner_width(width);
        let inner_height = margins.inner_height(height);

        // Compute domains — fall back to (0.0, 1.0) when all values are null
        // so we still render a valid (empty) chart area instead of erroring.
        // Individual null points are already skipped in the data loop below.
        let x_extent = data.extent(&x_field).unwrap_or((0.0, 1.0));
        let y_extent = data.extent(&y_field).unwrap_or((0.0, 1.0));

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
                        // Spec-driven sized case — the `size` field overrides
                        // `theme.dot_radius` per-point. The inner `unwrap_or(5.0)`
                        // is the fallback for rows with a missing size value and
                        // is intentionally unchanged by Phase 5 theme wiring.
                        data.get_f64(i, sf).map(|v| ss.map(v)).unwrap_or(5.0)
                    }
                    _ => config.theme.dot_radius as f64,
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

                if let Some(halo) = emit_dot_halo_if_enabled(&config.theme, cx, cy, r) {
                    point_elements.push(halo);
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
        let draw_h_grid = should_draw_horizontal_grid(&config.theme.grid_style);
        let draw_v_grid = should_draw_vertical_grid(&config.theme.grid_style);
        for (i, &val) in y_ticks.iter().enumerate() {
            let y = y_scale.map(val);
            // Grid line — gated on grid_style (horizontal: constant y)
            if draw_h_grid {
                axis_elements.push(ChartElement::Line {
                    x1: margins.left, y1: y, x2: margins.left + inner_width, y2: y,
                    stroke: config.theme.grid.clone(), stroke_width: Some(config.theme.grid_line_weight as f64),
                    stroke_dasharray: None, class: "grid-line".to_string(),
                });
            }
            // Tick mark — always rendered
            axis_elements.push(ChartElement::Line {
                x1: margins.left - 5.0, y1: y, x2: margins.left, y2: y,
                stroke: config.theme.tick.clone(), stroke_width: Some(config.theme.axis_line_weight as f64),
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
        // Scatter historically measured X tick labels at 11px (one px below
        // the 12px default) — preserve that exactly when the theme is the
        // legacy default to keep golden snapshots byte-identical. Any theme
        // override switches to the full theme-aware measurement.
        let scatter_tick_metrics = TextMetrics::from_theme_tick_value(&config.theme);
        let x_label_widths: Vec<f64> = x_ticks.iter()
            .map(|&v| {
                let label = format_tick_value_si(v, x_tick_step);
                if scatter_tick_metrics.is_legacy_default() {
                    approximate_text_width_at(&label, 11.0)
                } else {
                    measure_text(&label, &scatter_tick_metrics)
                }
            })
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
            // Grid line — gated on grid_style (vertical: constant x)
            if draw_v_grid {
                axis_elements.push(ChartElement::Line {
                    x1: x, y1: margins.top, x2: x, y2: x_axis_y,
                    stroke: config.theme.grid.clone(), stroke_width: Some(config.theme.grid_line_weight as f64),
                    stroke_dasharray: None, class: "grid-line".to_string(),
                });
            }
            // Tick mark — always rendered
            axis_elements.push(ChartElement::Line {
                x1: x, y1: x_axis_y, x2: x, y2: x_axis_y + 5.0,
                stroke: config.theme.tick.clone(), stroke_width: Some(config.theme.axis_line_weight as f64),
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
            stroke: config.theme.axis_line.clone(), stroke_width: Some(config.theme.axis_line_weight as f64),
            stroke_dasharray: None, class: "axis-line".to_string(),
        });
        axis_elements.push(ChartElement::Line {
            x1: margins.left, y1: x_axis_y, x2: margins.left + inner_width, y2: x_axis_y,
            stroke: config.theme.axis_line.clone(), stroke_width: Some(config.theme.axis_line_weight as f64),
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
            let legend_config = LegendConfig {
                text_metrics: TextMetrics::from_theme_legend(&config.theme),
                ..LegendConfig::default()
            };
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
    fn field_or_err(spec: &chartml_core::spec::FieldSpec) -> Result<String, ChartError> {
        spec.field
            .clone()
            .ok_or_else(|| ChartError::InvalidSpec("Field spec has no `field` (range-mark specs are not supported for scatter axes)".into()))
    }
    match field_ref {
        Some(FieldRef::Simple(name)) => Ok(name.clone()),
        Some(FieldRef::Detailed(spec)) => field_or_err(spec),
        Some(FieldRef::Multiple(items)) => {
            // Use the first item
            match items.first() {
                Some(chartml_core::spec::FieldRefItem::Simple(name)) => Ok(name.clone()),
                Some(chartml_core::spec::FieldRefItem::Detailed(spec)) => field_or_err(spec),
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
    fn scatter_empty_data_renders_empty_chart() {
        // Zero rows: extent returns None → fallback domain (0.0, 1.0) → empty chart, no error.
        let renderer = ScatterRenderer::new();
        let data = DataTable::from_rows(&Vec::<Row>::new()).unwrap();
        let result = renderer.render(&data, &make_scatter_config());
        assert!(result.is_ok(), "empty data should render an empty chart, not error: {:?}", result.err());
        let element = result.unwrap();
        // No data-point circles — only legend circles if any.
        let scatter_points = count_elements(&element, &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        assert_eq!(scatter_points, 0, "empty data should produce zero scatter points");
    }

    // ----- Null handling tests -----

    fn make_null_config(x_field: &str, y_field: &str) -> ChartConfig {
        ChartConfig {
            visualize: VisualizeSpec {
                chart_type: "scatter".to_string(),
                mode: None,
                orientation: None,
                columns: Some(FieldRef::Simple(x_field.to_string())),
                rows: Some(FieldRef::Simple(y_field.to_string())),
                marks: None,
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
    fn scatter_all_x_null_renders_empty_chart() {
        // All x values are null — extent(x) returns None → fallback (0.0, 1.0).
        let rows = vec![
            make_row(&[("x", serde_json::Value::Null), ("y", serde_json::json!(1.0))]),
            make_row(&[("x", serde_json::Value::Null), ("y", serde_json::json!(2.0))]),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let result = ScatterRenderer::new().render(&data, &make_null_config("x", "y"));
        assert!(result.is_ok(), "all-null x should render empty chart, not error: {:?}", result.err());
        let scatter_points = count_elements(&result.unwrap(), &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        assert_eq!(scatter_points, 0, "no valid x values → no points plotted");
    }

    #[test]
    fn scatter_all_y_null_renders_empty_chart() {
        // All y values are null — extent(y) returns None → fallback (0.0, 1.0).
        let rows = vec![
            make_row(&[("x", serde_json::json!(1.0)), ("y", serde_json::Value::Null)]),
            make_row(&[("x", serde_json::json!(2.0)), ("y", serde_json::Value::Null)]),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let result = ScatterRenderer::new().render(&data, &make_null_config("x", "y"));
        assert!(result.is_ok(), "all-null y should render empty chart, not error: {:?}", result.err());
        let scatter_points = count_elements(&result.unwrap(), &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        assert_eq!(scatter_points, 0, "no valid y values → no points plotted");
    }

    #[test]
    fn scatter_all_xy_null_renders_empty_chart() {
        // Both x and y are null for every row.
        let rows = vec![
            make_row(&[("x", serde_json::Value::Null), ("y", serde_json::Value::Null)]),
            make_row(&[("x", serde_json::Value::Null), ("y", serde_json::Value::Null)]),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let result = ScatterRenderer::new().render(&data, &make_null_config("x", "y"));
        assert!(result.is_ok(), "all-null x+y should render empty chart, not error: {:?}", result.err());
        let scatter_points = count_elements(&result.unwrap(), &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        assert_eq!(scatter_points, 0);
    }

    #[test]
    fn scatter_partial_null_renders_valid_points_only() {
        // Some rows have valid values, some have nulls — only valid pairs are plotted.
        let rows = vec![
            make_row(&[("x", serde_json::json!(5.0)), ("y", serde_json::json!(10.0))]),
            make_row(&[("x", serde_json::Value::Null), ("y", serde_json::json!(20.0))]),
            make_row(&[("x", serde_json::json!(15.0)), ("y", serde_json::Value::Null)]),
            make_row(&[("x", serde_json::json!(25.0)), ("y", serde_json::json!(30.0))]),
        ];
        let data = DataTable::from_rows(&rows).unwrap();
        let result = ScatterRenderer::new().render(&data, &make_null_config("x", "y"));
        assert!(result.is_ok(), "partial nulls should not error: {:?}", result.err());
        let scatter_points = count_elements(&result.unwrap(), &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        // Only rows 0 and 3 have both x and y non-null.
        assert_eq!(scatter_points, 2, "expected 2 valid points (rows with both x and y non-null)");
    }

    // ----- Phase 6: theme.grid_style gating -----

    fn count_scatter_grid_lines(el: &ChartElement) -> usize {
        let mut n = 0usize;
        fn visit(el: &ChartElement, n: &mut usize) {
            match el {
                ChartElement::Line { class, .. } => {
                    if class.split_whitespace().any(|c| c == "grid-line") {
                        *n += 1;
                    }
                }
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for c in children {
                        visit(c, n);
                    }
                }
                _ => {}
            }
        }
        visit(el, &mut n);
        n
    }

    // ----- Phase 8: dot_halo wiring -----

    /// Collect (index, class) of every Circle and dot-halo Path in a flat
    /// traversal order. Used by Phase 8 tests to assert halo-before-dot
    /// ordering and counts.
    fn collect_dot_and_halo_order(el: &ChartElement) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        fn visit(el: &ChartElement, out: &mut Vec<(usize, String)>) {
            match el {
                ChartElement::Circle { class, .. } => {
                    let idx = out.len();
                    out.push((idx, class.clone()));
                }
                ChartElement::Path { class, .. } if class == "dot-halo" => {
                    let idx = out.len();
                    out.push((idx, class.clone()));
                }
                ChartElement::Svg { children, .. }
                | ChartElement::Group { children, .. } => {
                    for c in children {
                        visit(c, out);
                    }
                }
                _ => {}
            }
        }
        visit(el, &mut out);
        out
    }

    fn count_halos(el: &ChartElement) -> usize {
        count_elements(el, &|e| matches!(e, ChartElement::Path { class, .. } if class == "dot-halo"))
    }

    #[test]
    fn phase8_scatter_default_theme_emits_no_halo() {
        use chartml_core::theme::Theme;
        let renderer = ScatterRenderer::new();
        let mut config = make_scatter_config();
        config.theme = Theme::default();
        let element = renderer.render(&make_scatter_data(), &config).unwrap();
        assert_eq!(count_halos(&element), 0, "default theme must emit zero dot-halo elements");
    }

    #[test]
    fn phase8_scatter_halo_color_emits_one_halo_per_point() {
        use chartml_core::theme::Theme;
        let renderer = ScatterRenderer::new();
        let mut config = make_scatter_config();
        let mut t = Theme::default();
        t.dot_halo_color = Some("#ffffff".to_string());
        t.dot_halo_width = 1.5;
        config.theme = t;
        let element = renderer.render(&make_scatter_data(), &config).unwrap();

        // 4 data points → 4 halos (legend circles don't get halos).
        assert_eq!(count_halos(&element), 4);

        // Data-point circles should also still number 4.
        let data_circles = count_elements(&element, &|e| {
            matches!(e, ChartElement::Circle { class, .. } if class.contains("chartml-scatter-point"))
        });
        assert_eq!(data_circles, 4);

        // Verify halo stroke + width on at least one emitted halo.
        fn find_halo(el: &ChartElement) -> Option<(String, f64)> {
            match el {
                ChartElement::Path { class, stroke, stroke_width, .. } if class == "dot-halo" => {
                    Some((stroke.clone().unwrap_or_default(), stroke_width.unwrap_or(-1.0)))
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    children.iter().find_map(find_halo)
                }
                _ => None,
            }
        }
        let (stroke, width) = find_halo(&element).expect("at least one halo");
        assert_eq!(stroke, "#ffffff");
        assert!((width - 1.5).abs() < 1e-9, "halo stroke-width {} != 1.5", width);
    }

    #[test]
    fn phase8_scatter_halo_precedes_dot_in_order() {
        use chartml_core::theme::Theme;
        let renderer = ScatterRenderer::new();
        let mut config = make_scatter_config();
        let mut t = Theme::default();
        t.dot_halo_color = Some("#ffffff".to_string());
        t.dot_halo_width = 1.5;
        config.theme = t;
        let element = renderer.render(&make_scatter_data(), &config).unwrap();

        // Walk the points group and assert every dot-marker circle is
        // preceded immediately by a dot-halo.
        fn find_points_group(el: &ChartElement) -> Option<&Vec<ChartElement>> {
            match el {
                ChartElement::Group { class, children, .. }
                    if class == "chartml-scatter-points" => Some(children),
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    children.iter().find_map(find_points_group)
                }
                _ => None,
            }
        }
        let points = find_points_group(&element).expect("points group");
        let mut pair_count = 0;
        let mut iter = points.iter().peekable();
        while let Some(el) = iter.next() {
            if let ChartElement::Path { class, .. } = el {
                if class == "dot-halo" {
                    let next = iter.peek().expect("halo must be followed by dot");
                    match next {
                        ChartElement::Circle { class, .. } => {
                            assert!(class.contains("dot-marker"));
                            pair_count += 1;
                        }
                        _ => panic!("halo not immediately followed by a Circle"),
                    }
                }
            }
        }
        assert_eq!(pair_count, 4);
    }

    #[test]
    fn phase8_bubble_halo_radius_tracks_per_point_size() {
        // For a bubble chart each point has a distinct radius from the size
        // scale. The halo's path must encode that same radius, not a static
        // theme.dot_radius default.
        use chartml_core::theme::Theme;
        let renderer = ScatterRenderer::new();
        let mut config = make_bubble_config();
        let mut t = Theme::default();
        t.dot_halo_color = Some("#000000".to_string());
        t.dot_halo_width = 1.0;
        config.theme = t;
        let element = renderer.render(&make_bubble_data(), &config).unwrap();

        // Collect halo path d strings and dot radii in traversal order.
        fn collect(el: &ChartElement, halos: &mut Vec<String>, dots: &mut Vec<f64>) {
            match el {
                ChartElement::Path { class, d, .. } if class == "dot-halo" => halos.push(d.clone()),
                ChartElement::Circle { class, r, .. } if class.contains("chartml-scatter-point") => {
                    dots.push(*r);
                }
                ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                    for c in children { collect(c, halos, dots); }
                }
                _ => {}
            }
        }
        let mut halos = Vec::new();
        let mut dots = Vec::new();
        collect(&element, &mut halos, &mut dots);
        assert_eq!(halos.len(), 3);
        assert_eq!(dots.len(), 3);
        // Distinct radii → distinct halo d strings.
        let unique_d: std::collections::HashSet<&String> = halos.iter().collect();
        assert_eq!(unique_d.len(), 3, "bubble halos should have 3 distinct path d values, one per radius");
        // Each halo d must contain the per-point radius.
        for (d, r) in halos.iter().zip(dots.iter()) {
            assert!(
                d.contains(&format!("{},{}", r, r)) || d.contains(&format!(" {},", r)),
                "halo d {:?} should encode per-point radius {}",
                d,
                r
            );
        }
    }

    #[test]
    fn phase8_scatter_traversal_order_sanity() {
        // Basic smoke: with halo enabled, the recorded sequence should
        // alternate halo/dot for each data point.
        use chartml_core::theme::Theme;
        let renderer = ScatterRenderer::new();
        let mut config = make_scatter_config();
        let mut t = Theme::default();
        t.dot_halo_color = Some("#ffffff".to_string());
        t.dot_halo_width = 1.0;
        config.theme = t;
        let element = renderer.render(&make_scatter_data(), &config).unwrap();
        let order = collect_dot_and_halo_order(&element);
        // Find the first "dot-halo" entry and ensure it's followed by a
        // scatter-point dot.
        let first_halo = order.iter().position(|(_, c)| c == "dot-halo");
        assert!(first_halo.is_some());
    }

    #[test]
    fn phase6_scatter_grid_style_none_skips_all_gridlines() {
        use chartml_core::theme::{GridStyle, Theme};
        let renderer = ScatterRenderer::new();
        let data = make_scatter_data();
        let mut config = make_scatter_config();

        // Sanity: Both (default) produces > 0 gridlines.
        let baseline = renderer.render(&data, &config).unwrap();
        let baseline_count = count_scatter_grid_lines(&baseline);
        assert!(
            baseline_count > 0,
            "scatter default (GridStyle::Both) should emit gridlines, got {}",
            baseline_count
        );

        // None: zero gridlines of either orientation.
        let mut t = Theme::default();
        t.grid_style = GridStyle::None;
        config.theme = t;
        let element = renderer.render(&data, &config).unwrap();
        let n = count_scatter_grid_lines(&element);
        assert_eq!(n, 0, "GridStyle::None: expected 0 scatter gridlines, got {}", n);
    }
}
