use chartml_core::data::{get_f64, get_string, unique_values, group_by, Row};
use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::plugin::ChartConfig;
use chartml_core::layout::adaptive_tick_count;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::LineGenerator;

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig};

use crate::helpers::{GridConfig, LegendMark, format_value, generate_annotations, generate_x_axis, generate_y_axis_numeric, generate_legend_with_mark, get_color_field, get_field_name, get_x_format, get_y_format, offset_element};

pub fn render_line(data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
    use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec};

    let category_field = get_field_name(&config.visualize.columns)?;

    let categories = unique_values(data, &category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    // Detect multi-field rows (e.g., [{field: revenue, color: ...}, {field: target, ...}])
    let multi_fields: Vec<FieldSpec> = match &config.visualize.rows {
        Some(FieldRef::Multiple(items)) => items.iter().filter_map(|item| match item {
            FieldRefItem::Detailed(spec) => Some(spec.clone()),
            FieldRefItem::Simple(name) => Some(FieldSpec {
                field: name.clone(), mark: None, axis: None, label: None,
                color: None, format: None, data_labels: None,
            }),
        }).collect(),
        _ => vec![],
    };
    let is_multi_field = !multi_fields.is_empty();
    let value_field = if is_multi_field {
        multi_fields[0].field.clone()
    } else {
        get_field_name(&config.visualize.rows)?
    };

    let color_field = get_color_field(config);
    let has_series = color_field.is_some() || is_multi_field;

    // Step 1: Compute label strategy for margin estimation
    let estimated_width = config.width - 80.0;
    let x_format = get_x_format(config);
    let x_strategy = LabelStrategy::determine(&categories, estimated_width, &LabelStrategyConfig::default());
    let x_extra_margin = match &x_strategy {
        LabelStrategy::Rotated { margin, .. } => *margin,
        _ => 0.0,
    };

    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        has_legend: has_series,
        x_label_strategy_margin: x_extra_margin,
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    // Find value extent across ALL fields
    let all_value_fields: Vec<String> = if is_multi_field {
        multi_fields.iter().map(|f| f.field.clone()).collect()
    } else {
        vec![value_field.clone()]
    };

    let mut all_values: Vec<f64> = Vec::new();
    for field in &all_value_fields {
        for row in data {
            if let Some(v) = get_f64(row, field) {
                all_values.push(v);
            }
        }
    }
    let value_min = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let value_max = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Apply explicit axis bounds
    let axes_bounds = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref());
    let domain_min = axes_bounds.and_then(|a| a.min).unwrap_or(if value_min >= 0.0 { 0.0 } else { value_min });
    let domain_max = axes_bounds.and_then(|a| a.max).unwrap_or(if value_max <= 0.0 { 1.0 } else { value_max });

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let linear = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));

    let mut children = Vec::new();

    // Title
    if let Some(ref title) = config.title {
        children.push(ChartElement::Text {
            x: 10.0,
            y: 20.0,
            content: title.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_size: Some("14px".to_string()),
            font_weight: Some("bold".to_string()),
            fill: Some("#333".to_string()),
            class: "chart-title".to_string(),
            data: None,
        });
    }

    // Axes — read format string from spec
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let grid = GridConfig::from_config(config);

    // Apply D3-style nice domain rounding for clean tick intervals and headroom (Regressions 2 & 3).
    // Only when no explicit axis bounds are set.
    let axes_bounds_min = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref()).and_then(|a| a.min);
    let axes_bounds_max = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref()).and_then(|a| a.max);
    let (domain_min, domain_max) = if axes_bounds_min.is_none() && axes_bounds_max.is_none() {
        // Match JS: yLeft.nice() uses default count=10 for domain rounding.
        crate::helpers::nice_domain(domain_min, domain_max, 10)
    } else {
        (domain_min, domain_max)
    };

    let x_axis_result = generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height, inner_width, x_format.as_deref(), Some(inner_height), &grid);
    let y_axis_elements = generate_y_axis_numeric(
        (domain_min, domain_max),
        (inner_height, 0.0),
        margins.left,
        None,
        adaptive_tick_count(inner_height),
        Some(inner_width),
        &grid,
    );

    children.push(ChartElement::Group {
        class: "axes".to_string(),
        transform: None,
        children: {
            let mut axes = Vec::new();
            axes.extend(
                x_axis_result.elements
                    .into_iter()
                    .map(|e| offset_element(e, margins.left, 0.0)),
            );
            axes.extend(
                y_axis_elements
                    .into_iter()
                    .map(|e| offset_element(e, 0.0, margins.top)),
            );
            axes
        },
    });

    // Annotations — rendered below the line (added before line elements)
    if let Some(annotations) = config.visualize.annotations.as_deref() {
        if !annotations.is_empty() {
            let ann_scale = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));
            let ann_elements = generate_annotations(
                annotations,
                &ann_scale,
                0.0,
                inner_width,
                inner_height,
                Some(&categories),
            );
            if !ann_elements.is_empty() {
                children.push(ChartElement::Group {
                    class: "annotations".to_string(),
                    transform: Some(Transform::Translate(margins.left, margins.top)),
                    children: ann_elements,
                });
            }
        }
    }

    // Line paths
    let line_gen = LineGenerator::new().curve(chartml_core::shapes::CurveType::MonotoneX);
    let bandwidth = band.bandwidth();
    let mut line_elements = Vec::new();

    if is_multi_field {
        // Multi-field rows: each field spec is a separate line series
        let mut series_names = Vec::new();
        let mut series_colors = Vec::new();

        for (field_idx, field_spec) in multi_fields.iter().enumerate() {
            let field_name = &field_spec.field;
            let color = field_spec.color.clone()
                .unwrap_or_else(|| config.colors.get(field_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string()));
            let label = field_spec.label.clone().unwrap_or_else(|| field_name.clone());

            let mut points: Vec<(f64, f64)> = Vec::new();
            let mut point_data: Vec<(String, f64)> = Vec::new();

            for cat in &categories {
                // Find the row for this category
                let row = match data.iter().find(|r| get_string(r, &category_field).as_deref() == Some(cat.as_str())) {
                    Some(r) => r,
                    None => continue,
                };
                let val = match get_f64(row, field_name) {
                    Some(v) => v,
                    None => continue,
                };
                let x = match band.map(cat) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                let y = linear.map(val);
                points.push((x, y));
                point_data.push((cat.clone(), val));
            }

            if points.is_empty() {
                continue;
            }

            let path_d = line_gen.generate(&points);

            line_elements.push(ChartElement::Path {
                d: path_d,
                fill: None,
                stroke: Some(color.clone()),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                class: "line".to_string(),
                data: Some(ElementData::new(&label, "").with_series(&label)),
            });

            // Hover dots
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                line_elements.push(ChartElement::Circle {
                    cx: px, cy: py, r: 5.0,
                    fill: color.clone(),
                    stroke: Some("#fff".to_string()),
                    class: "chartml-line-dot".to_string(),
                    data: Some(ElementData::new(cat, format_value(val, y_fmt_ref)).with_series(&label)),
                });
            }

            // Data labels (if configured on this field spec)
            if let Some(ref dl) = field_spec.data_labels {
                if dl.show == Some(true) {
                    let dl_fmt = dl.format.as_deref().or(y_fmt_ref);
                    for (i, &(px, py)) in points.iter().enumerate() {
                        let (_, val) = &point_data[i];
                        let label_y = match dl.position.as_deref() {
                            Some("bottom") => py + 15.0,
                            _ => py - 10.0,
                        };
                        line_elements.push(ChartElement::Text {
                            x: px, y: label_y,
                            content: format_value(*val, dl_fmt),
                            anchor: TextAnchor::Middle,
                            dominant_baseline: None,
                            transform: None,
                            font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "11px".to_string())),
                            font_weight: None,
                            fill: Some(dl.color.clone().unwrap_or_else(|| color.clone())),
                            class: "data-label".to_string(),
                            data: None,
                        });
                    }
                }
            }

            series_names.push(label);
            series_colors.push(color);
        }

        // Legend
        let legend_elements = generate_legend_with_mark(&series_names, &series_colors, config.width, config.height - 10.0, LegendMark::Line);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else if let Some(ref color_f) = color_field {
        let series_names = unique_values(data, color_f);
        let groups = group_by(data, color_f);

        for (series_idx, series_name) in series_names.iter().enumerate() {
            let series_rows = match groups.get(series_name) {
                Some(rows) => rows,
                None => continue,
            };

            let mut points: Vec<(f64, f64)> = Vec::new();
            let mut point_data: Vec<(String, f64)> = Vec::new();

            for cat in &categories {
                let row = match series_rows.iter().find(|r| {
                    get_string(r, &category_field).as_deref() == Some(cat.as_str())
                }) {
                    Some(r) => r,
                    None => continue,
                };
                let val = match get_f64(row, &value_field) {
                    Some(v) => v,
                    None => continue,
                };
                let x = match band.map(cat) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                let y = linear.map(val);
                points.push((x, y));
                point_data.push((cat.clone(), val));
            }

            if points.is_empty() {
                continue;
            }

            let path_d = line_gen.generate(&points);
            let color = config
                .colors
                .get(series_idx)
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            line_elements.push(ChartElement::Path {
                d: path_d,
                fill: None,
                stroke: Some(color.clone()),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                class: "line".to_string(),
                data: Some(ElementData::new(series_name, "").with_series(series_name)),
            });

            // Hover dots at each data point
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                line_elements.push(ChartElement::Circle {
                    cx: px,
                    cy: py,
                    r: 5.0,
                    fill: color.clone(),
                    stroke: Some("#fff".to_string()),
                    class: "chartml-line-dot".to_string(),
                    data: Some(ElementData::new(cat, format_value(val, y_fmt_ref)).with_series(series_name)),
                });
            }
        }

        // Legend
        let legend_elements =
            generate_legend_with_mark(&series_names, &config.colors, config.width, config.height - 10.0, LegendMark::Line);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else {
        // Single series
        let mut points: Vec<(f64, f64)> = Vec::new();
        let mut point_data: Vec<(String, f64)> = Vec::new();

        for cat in &categories {
            let row = match data.iter().find(|r| {
                get_string(r, &category_field).as_deref() == Some(cat.as_str())
            }) {
                Some(r) => r,
                None => continue,
            };
            let val = match get_f64(row, &value_field) {
                Some(v) => v,
                None => continue,
            };
            let x = match band.map(cat) {
                Some(x) => x + bandwidth / 2.0,
                None => continue,
            };
            let y = linear.map(val);
            points.push((x, y));
            point_data.push((cat.clone(), val));
        }

        if !points.is_empty() {
            let path_d = line_gen.generate(&points);
            let color = config
                .colors
                .first()
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            line_elements.push(ChartElement::Path {
                d: path_d,
                fill: None,
                stroke: Some(color.clone()),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                class: "line".to_string(),
                data: None,
            });

            // Hover dots at each data point
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                line_elements.push(ChartElement::Circle {
                    cx: px,
                    cy: py,
                    r: 5.0,
                    fill: color.clone(),
                    stroke: Some("#fff".to_string()),
                    class: "chartml-line-dot".to_string(),
                    data: Some(ElementData::new(cat, format_value(val, y_fmt_ref))),
                });
            }
        }
    }

    children.push(ChartElement::Group {
        class: "lines".to_string(),
        transform: Some(Transform::Translate(margins.left, margins.top)),
        children: line_elements,
    });

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-line".to_string(),
        children,
    })
}

