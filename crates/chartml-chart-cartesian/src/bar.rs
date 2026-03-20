use chartml_core::data::{get_f64, get_string, unique_values, group_by, Row};
use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::layout::stack::StackLayout;
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::layout::adaptive_tick_count;
use chartml_core::spec::{ChartMode, Orientation};

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig};

use crate::helpers::{GridConfig, format_value, generate_x_axis, generate_x_axis_numeric, generate_y_axis, generate_y_axis_numeric, generate_y_axis_numeric_right, generate_legend, get_color_field, get_data_labels_config, get_field_name, get_x_format, get_y_axis_bounds, get_y_format, offset_element};

pub fn render_bar(data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
    use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec};
    use chartml_core::shapes::LineGenerator;

    // Detect multi-field rows (combo chart pattern)
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

    if !multi_fields.is_empty() {
        return render_combo(data, config, &multi_fields);
    }

    let category_field = get_field_name(&config.visualize.columns)?;
    let value_field = get_field_name(&config.visualize.rows)?;

    let categories = unique_values(data, &category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let color_field = get_color_field(config);
    let is_horizontal = matches!(config.visualize.orientation, Some(Orientation::Horizontal));
    let is_stacked = matches!(config.visualize.mode, Some(ChartMode::Stacked));
    let is_grouped = matches!(config.visualize.mode, Some(ChartMode::Grouped));

    // Step 1: Compute label strategy for margin estimation (only for vertical bars)
    let x_format = get_x_format(config);
    let x_extra_margin = if !is_horizontal {
        let estimated_width = config.width - 80.0;
        let x_strategy = LabelStrategy::determine(&categories, estimated_width, &LabelStrategyConfig::default());
        match &x_strategy {
            LabelStrategy::Rotated { margin, .. } => *margin,
            _ => 0.0,
        }
    } else {
        0.0
    };

    // Step 2: Calculate margins including rotation
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        has_legend: color_field.is_some(),
        x_label_strategy_margin: x_extra_margin,
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let mut children = Vec::new();

    // Title
    if let Some(ref title) = config.title {
        children.push(ChartElement::Text {
            x: config.width / 2.0,
            y: 20.0,
            content: title.clone(),
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: None,
            font_size: Some("14px".to_string()),
            fill: Some("#333".to_string()),
            class: "chart-title".to_string(),
            data: None,
        });
    }

    // Determine value domain
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let grid = GridConfig::from_config(config);

    // Pre-read axis bounds (needed by bar rendering for correct scale)
    let (axis_min, axis_max) = get_y_axis_bounds(config);

    let (value_max, bar_elements) = if let Some(ref color_f) = color_field {
        let (vm, els) = render_multi_series_bars(
            data,
            config,
            &category_field,
            &value_field,
            color_f,
            &categories,
            &margins,
            inner_width,
            inner_height,
            is_stacked,
            is_grouped,
            is_horizontal,
            y_fmt_ref,
            axis_min.unwrap_or(0.0),
            axis_max.unwrap_or(f64::MAX),
        )?;
        (vm, els)
    } else {
        let (vm, els) = render_single_series_bars(
            data,
            config,
            &category_field,
            &value_field,
            &categories,
            &margins,
            inner_width,
            inner_height,
            is_horizontal,
            y_fmt_ref,
            axis_min.unwrap_or(0.0),
            axis_max.unwrap_or(f64::MAX),
        )?;
        (vm, els)
    };

    let domain_min = axis_min.unwrap_or(0.0);
    let domain_max = axis_max.unwrap_or(value_max);

    // Axes (use domain_min/domain_max instead of 0.0/value_max)
    let axis_elements = if is_horizontal {
        // Category y-axis: generate at x=0 relative, then offset by margins.left
        let x_axis = generate_y_axis(&categories, (0.0, inner_height), 0.0, None);
        let y_axis = generate_x_axis_numeric((domain_min, domain_max), (0.0, inner_width), margins.top + inner_height, y_fmt_ref, adaptive_tick_count(inner_width), Some(inner_height), &grid);
        let mut axes = Vec::new();
        axes.extend(x_axis.into_iter().map(|e| offset_element(e, margins.left, margins.top)));
        axes.extend(y_axis.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
        axes
    } else {
        let x_axis_result = generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height, inner_width, x_format.as_deref(), Some(inner_height), &grid);
        let y_axis = generate_y_axis_numeric((domain_min, domain_max), (inner_height, 0.0), margins.left, y_fmt_ref, adaptive_tick_count(inner_height), Some(inner_width), &grid);
        let mut axes = Vec::new();
        axes.extend(x_axis_result.elements.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
        axes.extend(y_axis.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
        axes
    };

    children.push(ChartElement::Group {
        class: "axes".to_string(),
        transform: None,
        children: axis_elements,
    });

    children.push(ChartElement::Group {
        class: "bars".to_string(),
        transform: Some(Transform::Translate(margins.left, margins.top)),
        children: bar_elements,
    });

    // Legend
    if let Some(ref color_f) = color_field {
        let series_names = unique_values(data, color_f);
        let legend_elements = generate_legend(&series_names, &config.colors, config.width, config.height - 10.0);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    }

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-bar".to_string(),
        children,
    })
}

fn render_single_series_bars(
    data: &[Row],
    config: &ChartConfig,
    category_field: &str,
    value_field: &str,
    categories: &[String],
    _margins: &chartml_core::layout::margins::Margins,
    inner_width: f64,
    inner_height: f64,
    is_horizontal: bool,
    y_fmt_ref: Option<&str>,
    domain_min: f64,
    domain_max: f64,
) -> Result<(f64, Vec<ChartElement>), ChartError> {
    // Find the max value
    let values: Vec<f64> = data
        .iter()
        .filter_map(|row| get_f64(row, value_field))
        .collect();
    let value_max = values.iter().cloned().fold(0.0_f64, f64::max);
    let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
    // Use explicit axis max if provided, otherwise data max
    let effective_max = if domain_max < f64::MAX { domain_max } else { value_max };

    let mut elements = Vec::new();
    let fill = config.colors.first().cloned().unwrap_or_else(|| "#2E7D9A".to_string());

    if is_horizontal {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_height));
        let linear = ScaleLinear::new((domain_min, effective_max), (0.0, inner_width));

        for row in data {
            let cat = match get_string(row, category_field) {
                Some(c) => c,
                None => continue,
            };
            let val = get_f64(row, value_field).unwrap_or(0.0);
            let y = match band.map(&cat) {
                Some(y) => y,
                None => continue,
            };
            let bar_width = linear.map(val);

            elements.push(ChartElement::Rect {
                x: 0.0,
                y,
                width: bar_width,
                height: band.bandwidth(),
                fill: fill.clone(),
                stroke: None,
                class: "bar".to_string(),
                data: Some(ElementData::new(&cat, format_value(val, y_fmt_ref))),
            });
        }
    } else {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((domain_min, effective_max), (inner_height, 0.0));

        for row in data {
            let cat = match get_string(row, category_field) {
                Some(c) => c,
                None => continue,
            };
            let val = get_f64(row, value_field).unwrap_or(0.0);
            let x = match band.map(&cat) {
                Some(x) => x,
                None => continue,
            };
            let bar_top = linear.map(val);
            let bar_bottom = linear.map(0.0);
            let bar_height = (bar_bottom - bar_top).abs();

            elements.push(ChartElement::Rect {
                x,
                y: bar_top,
                width: band.bandwidth(),
                height: bar_height,
                fill: fill.clone(),
                stroke: None,
                class: "bar".to_string(),
                data: Some(ElementData::new(&cat, format_value(val, y_fmt_ref))),
            });

            // Data label above bar (if configured)
            if let Some(dl) = get_data_labels_config(config) {
                if dl.show == Some(true) {
                    let label_fmt = dl.format.as_deref().or(y_fmt_ref);
                    let label_y = match dl.position.as_deref() {
                        Some("center") => bar_top + bar_height / 2.0,
                        Some("bottom") => bar_bottom - 5.0,
                        _ => bar_top - 5.0, // "top" or default
                    };
                    elements.push(ChartElement::Text {
                        x: x + band.bandwidth() / 2.0,
                        y: label_y,
                        content: format_value(val, label_fmt),
                        anchor: TextAnchor::Middle,
                        dominant_baseline: None,
                        transform: None,
                        font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "11px".to_string())),
                        fill: Some(dl.color.clone().unwrap_or_else(|| "#333".to_string())),
                        class: "data-label".to_string(),
                        data: None,
                    });
                }
            }
        }
    }

    Ok((value_max, elements))
}

fn render_multi_series_bars(
    data: &[Row],
    config: &ChartConfig,
    category_field: &str,
    value_field: &str,
    color_field: &str,
    categories: &[String],
    _margins: &chartml_core::layout::margins::Margins,
    inner_width: f64,
    inner_height: f64,
    is_stacked: bool,
    _is_grouped: bool,
    _is_horizontal: bool,
    y_fmt_ref: Option<&str>,
    domain_min: f64,
    domain_max: f64,
) -> Result<(f64, Vec<ChartElement>), ChartError> {
    let series_names = unique_values(data, color_field);
    let groups = group_by(data, color_field);

    let mut elements = Vec::new();

    if is_stacked {
        // Build values matrix: values[series_idx][category_idx]
        let mut values_matrix: Vec<Vec<f64>> = Vec::new();
        for series in &series_names {
            let mut series_vals = Vec::new();
            let rows = groups.get(series);
            for cat in categories {
                let val = rows
                    .map(|rs| {
                        rs.iter()
                            .find(|r| get_string(r, category_field).as_deref() == Some(cat.as_str()))
                            .and_then(|r| get_f64(r, value_field))
                            .unwrap_or(0.0)
                    })
                    .unwrap_or(0.0);
                series_vals.push(val);
            }
            values_matrix.push(series_vals);
        }

        let stack = StackLayout::new();
        let stacked_points = stack.layout(categories, &series_names, &values_matrix);

        // Find max y1
        let value_max = stacked_points
            .iter()
            .map(|p| p.y1)
            .fold(0.0_f64, f64::max);
        let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
        let effective_max = if domain_max < f64::MAX { domain_max } else { value_max };

        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((domain_min, effective_max), (inner_height, 0.0));

        for point in &stacked_points {
            let x = match band.map(&point.key) {
                Some(x) => x,
                None => continue,
            };
            let y_top = linear.map(point.y1);
            let y_bottom = linear.map(point.y0);
            let bar_height = (y_bottom - y_top).abs();

            let series_idx = series_names.iter().position(|s| s == &point.series).unwrap_or(0);
            let fill = config
                .colors
                .get(series_idx)
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            elements.push(ChartElement::Rect {
                x,
                y: y_top,
                width: band.bandwidth(),
                height: bar_height,
                fill,
                stroke: None,
                class: "bar".to_string(),
                data: Some(
                    ElementData::new(&point.key, format_value(point.value, y_fmt_ref))
                        .with_series(&point.series),
                ),
            });
        }

        Ok((value_max, elements))
    } else {
        // Grouped (or default multi-series)
        // Find overall max value
        let value_max = data
            .iter()
            .filter_map(|row| get_f64(row, value_field))
            .fold(0.0_f64, f64::max);
        let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
        let effective_max = if domain_max < f64::MAX { domain_max } else { value_max };

        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((domain_min, effective_max), (inner_height, 0.0));

        let num_series = series_names.len().max(1);
        let sub_band_width = band.bandwidth() / num_series as f64;

        for row in data {
            let cat = match get_string(row, category_field) {
                Some(c) => c,
                None => continue,
            };
            let series = match get_string(row, color_field) {
                Some(s) => s,
                None => continue,
            };
            let val = get_f64(row, value_field).unwrap_or(0.0);

            let x_base = match band.map(&cat) {
                Some(x) => x,
                None => continue,
            };
            let series_idx = series_names.iter().position(|s| s == &series).unwrap_or(0);
            let x = x_base + series_idx as f64 * sub_band_width;

            let bar_top = linear.map(val);
            let bar_bottom = linear.map(0.0);
            let bar_height = (bar_bottom - bar_top).abs();

            let fill = config
                .colors
                .get(series_idx)
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            elements.push(ChartElement::Rect {
                x,
                y: bar_top,
                width: sub_band_width,
                height: bar_height,
                fill,
                stroke: None,
                class: "bar".to_string(),
                data: Some(
                    ElementData::new(&cat, format_value(val, y_fmt_ref)).with_series(&series),
                ),
            });
        }

        Ok((value_max, elements))
    }
}

/// Render a combo chart: multiple fields with different marks (bar/line) and optional dual axis.
fn render_combo(
    data: &[Row],
    config: &ChartConfig,
    fields: &[chartml_core::spec::FieldSpec],
) -> Result<ChartElement, ChartError> {
    use chartml_core::shapes::LineGenerator;

    let category_field = get_field_name(&config.visualize.columns)?;
    let categories = unique_values(data, &category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let grid = GridConfig::from_config(config);
    let x_format = get_x_format(config);

    // Margins — account for right axis if present
    let has_right = fields.iter().any(|f| f.axis.as_deref() == Some("right"));
    let right_fmt = config.visualize.axes.as_ref()
        .and_then(|a| a.right.as_ref())
        .and_then(|a| a.format.as_deref());

    // Pre-compute right tick labels to measure their width
    let right_tick_labels: Vec<String> = if has_right {
        // Estimate right-axis values for label width measurement
        let right_max = fields.iter()
            .filter(|f| f.axis.as_deref() == Some("right"))
            .flat_map(|f| data.iter().filter_map(|r| get_f64(r, &f.field)))
            .fold(0.0_f64, f64::max);
        let right_domain_max = config.visualize.axes.as_ref()
            .and_then(|a| a.right.as_ref())
            .and_then(|a| a.max)
            .unwrap_or(if right_max <= 0.0 { 1.0 } else { right_max });
        let tmp_scale = ScaleLinear::new((0.0, right_domain_max), (0.0, 100.0));
        tmp_scale.ticks(5).iter().map(|v| format_value(*v, right_fmt)).collect()
    } else {
        vec![]
    };

    let has_left_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let has_right_label = config.visualize.axes.as_ref()
        .and_then(|a| a.right.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();

    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        has_legend: fields.len() > 1,
        has_y_axis_label: has_left_label,
        has_right_axis: has_right,
        right_tick_labels,
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);
    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let bandwidth = band.bandwidth();

    // Separate fields by axis
    let left_fields: Vec<&chartml_core::spec::FieldSpec> = fields.iter()
        .filter(|f| f.axis.as_deref() != Some("right"))
        .collect();
    let right_fields: Vec<&chartml_core::spec::FieldSpec> = fields.iter()
        .filter(|f| f.axis.as_deref() == Some("right"))
        .collect();

    // Compute left-axis domain
    let left_max = left_fields.iter()
        .flat_map(|f| data.iter().filter_map(|r| get_f64(r, &f.field)))
        .fold(0.0_f64, f64::max);
    let axes_left = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref());
    let left_domain_min = axes_left.and_then(|a| a.min).unwrap_or(0.0);
    let left_domain_max = axes_left.and_then(|a| a.max).unwrap_or(if left_max <= 0.0 { 1.0 } else { left_max });
    let left_scale = ScaleLinear::new((left_domain_min, left_domain_max), (inner_height, 0.0));

    // Compute right-axis domain
    let right_scale = if !right_fields.is_empty() {
        let right_max = right_fields.iter()
            .flat_map(|f| data.iter().filter_map(|r| get_f64(r, &f.field)))
            .fold(0.0_f64, f64::max);
        let axes_right = config.visualize.axes.as_ref().and_then(|a| a.right.as_ref());
        let right_domain_min = axes_right.and_then(|a| a.min).unwrap_or(0.0);
        let right_domain_max = axes_right.and_then(|a| a.max).unwrap_or(if right_max <= 0.0 { 1.0 } else { right_max });
        Some(ScaleLinear::new((right_domain_min, right_domain_max), (inner_height, 0.0)))
    } else {
        None
    };

    let mut children = Vec::new();

    // Title
    if let Some(ref title) = config.title {
        children.push(ChartElement::Text {
            x: config.width / 2.0, y: 20.0,
            content: title.clone(),
            anchor: TextAnchor::Middle, dominant_baseline: None,
            transform: None, font_size: Some("16px".to_string()),
            fill: Some("#333".to_string()), class: "chart-title".to_string(), data: None,
        });
    }

    // Axes
    let x_axis_result = generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height, inner_width, x_format.as_deref(), Some(inner_height), &grid);
    let y_axis_left = generate_y_axis_numeric(
        (left_domain_min, left_domain_max), (inner_height, 0.0), margins.left,
        y_fmt_ref, adaptive_tick_count(inner_height), Some(inner_width), &grid,
    );

    let mut axis_elements = Vec::new();
    axis_elements.extend(x_axis_result.elements.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
    axis_elements.extend(y_axis_left.into_iter().map(|e| offset_element(e, 0.0, margins.top)));

    // Right axis — ticks and labels on the right side
    if let Some(ref rs) = right_scale {
        let right_fmt = config.visualize.axes.as_ref()
            .and_then(|a| a.right.as_ref())
            .and_then(|a| a.format.as_deref());
        let right_axis = generate_y_axis_numeric_right(
            rs.domain(), (inner_height, 0.0), margins.left + inner_width,
            right_fmt, adaptive_tick_count(inner_height),
        );
        axis_elements.extend(right_axis.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
    }

    // Axis title labels
    if let Some(label) = config.visualize.axes.as_ref().and_then(|a| a.left.as_ref()).and_then(|a| a.label.clone()) {
        axis_elements.push(ChartElement::Text {
            x: 12.0,
            y: margins.top + inner_height / 2.0,
            content: label,
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: Some(Transform::Rotate(-90.0, 12.0, margins.top + inner_height / 2.0)),
            font_size: Some("12px".to_string()),
            fill: Some("#666".to_string()),
            class: "axis-label".to_string(),
            data: None,
        });
    }
    if let Some(label) = config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.label.clone()) {
        let rx = config.width - 12.0;
        axis_elements.push(ChartElement::Text {
            x: rx,
            y: margins.top + inner_height / 2.0,
            content: label,
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: Some(Transform::Rotate(90.0, rx, margins.top + inner_height / 2.0)),
            font_size: Some("12px".to_string()),
            fill: Some("#666".to_string()),
            class: "axis-label".to_string(),
            data: None,
        });
    }

    children.push(ChartElement::Group {
        class: "axes".to_string(), transform: None, children: axis_elements,
    });

    // Render each field
    let mut mark_elements = Vec::new();
    let line_gen = LineGenerator::new().curve(chartml_core::shapes::CurveType::MonotoneX);

    // Count bar fields for grouped subdivision
    let num_bar_fields = fields.iter()
        .filter(|f| f.mark.as_deref().unwrap_or("bar") == "bar")
        .count()
        .max(1);
    let sub_bar_width = bandwidth / num_bar_fields as f64;
    let mut bar_field_idx = 0_usize;
    let mut series_names = Vec::new();
    let mut series_colors = Vec::new();
    let mut series_marks = Vec::new();

    for (field_idx, field_spec) in fields.iter().enumerate() {
        let field_name = &field_spec.field;
        let is_right = field_spec.axis.as_deref() == Some("right");
        let scale = if is_right { right_scale.as_ref().unwrap_or(&left_scale) } else { &left_scale };
        let mark = field_spec.mark.as_deref().unwrap_or("bar");
        let color = field_spec.color.clone()
            .unwrap_or_else(|| config.colors.get(field_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string()));
        let label = field_spec.label.clone().unwrap_or_else(|| field_name.clone());
        let fmt_ref = if is_right {
            config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.format.as_deref())
        } else {
            y_fmt_ref
        };

        match mark {
            "bar" => {
                let this_bar_idx = bar_field_idx;
                bar_field_idx += 1;

                for row in data {
                    let cat = match get_string(row, &category_field) { Some(c) => c, None => continue };
                    let val = get_f64(row, field_name).unwrap_or(0.0);
                    let x = match band.map(&cat) { Some(x) => x, None => continue };
                    let bar_x = x + this_bar_idx as f64 * sub_bar_width;
                    let bar_top = scale.map(val);
                    let bar_bottom = scale.map(0.0);
                    let bar_height = (bar_bottom - bar_top).abs();

                    mark_elements.push(ChartElement::Rect {
                        x: bar_x + margins.left, y: bar_top + margins.top,
                        width: sub_bar_width, height: bar_height,
                        fill: color.clone(), stroke: None,
                        class: "bar".to_string(),
                        data: Some(ElementData::new(&cat, format_value(val, fmt_ref)).with_series(&label)),
                    });

                    // Data labels
                    if let Some(ref dl) = field_spec.data_labels {
                        if dl.show == Some(true) {
                            let dl_fmt = dl.format.as_deref().or(fmt_ref);
                            mark_elements.push(ChartElement::Text {
                                x: bar_x + sub_bar_width / 2.0 + margins.left,
                                y: bar_top + margins.top - 5.0,
                                content: format_value(val, dl_fmt),
                                anchor: TextAnchor::Middle, dominant_baseline: None,
                                transform: None,
                                font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "11px".to_string())),
                                fill: Some(dl.color.clone().unwrap_or_else(|| "#333".to_string())),
                                class: "data-label".to_string(), data: None,
                            });
                        }
                    }
                }
            }
            "line" | _ => {
                let mut points = Vec::new();
                let mut point_data = Vec::new();
                for cat in &categories {
                    let row = match data.iter().find(|r| get_string(r, &category_field).as_deref() == Some(cat.as_str())) {
                        Some(r) => r, None => continue,
                    };
                    let val = match get_f64(row, field_name) { Some(v) => v, None => continue };
                    let x = match band.map(cat) { Some(x) => x + bandwidth / 2.0, None => continue };
                    let y = scale.map(val);
                    points.push((x + margins.left, y + margins.top));
                    point_data.push((cat.clone(), val));
                }

                if !points.is_empty() {
                    let path_d = line_gen.generate(&points);
                    mark_elements.push(ChartElement::Path {
                        d: path_d, fill: None, stroke: Some(color.clone()),
                        stroke_width: Some(2.0), stroke_dasharray: None,
                        class: "line".to_string(),
                        data: Some(ElementData::new(&label, "").with_series(&label)),
                    });

                    // Dots
                    for (i, &(px, py)) in points.iter().enumerate() {
                        let (ref cat, val) = point_data[i];
                        mark_elements.push(ChartElement::Circle {
                            cx: px, cy: py, r: 5.0,
                            fill: color.clone(), stroke: Some("#fff".to_string()),
                            class: "chartml-line-dot".to_string(),
                            data: Some(ElementData::new(cat, format_value(val, fmt_ref)).with_series(&label)),
                        });
                    }

                    // Data labels
                    if let Some(ref dl) = field_spec.data_labels {
                        if dl.show == Some(true) {
                            let dl_fmt = dl.format.as_deref().or(fmt_ref);
                            for (i, &(px, py)) in points.iter().enumerate() {
                                let (_, val) = &point_data[i];
                                mark_elements.push(ChartElement::Text {
                                    x: px, y: py - 10.0,
                                    content: format_value(*val, dl_fmt),
                                    anchor: TextAnchor::Middle, dominant_baseline: None,
                                    transform: None,
                                    font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "11px".to_string())),
                                    fill: Some(dl.color.clone().unwrap_or_else(|| color.clone())),
                                    class: "data-label".to_string(), data: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        series_names.push(label);
        series_colors.push(color);
        series_marks.push(mark.to_string());
    }

    children.push(ChartElement::Group {
        class: "marks".to_string(), transform: None, children: mark_elements,
    });

    // Legend with mixed marks
    if series_names.len() > 1 {
        let mut legend_elements = Vec::new();
        let mut x_offset = 0.0;
        let total_w: f64 = series_names.iter().enumerate().map(|(i, name)| {
            let tw = chartml_core::layout::labels::approximate_text_width(name);
            12.0 + 6.0 + tw + 16.0
        }).sum();
        x_offset = (config.width - total_w).max(0.0) / 2.0;

        for (i, name) in series_names.iter().enumerate() {
            let color = &series_colors[i];
            let mark = series_marks[i].as_str();
            let y = config.height - 10.0;

            match mark {
                "line" => {
                    legend_elements.push(ChartElement::Line {
                        x1: x_offset, y1: y + 6.0, x2: x_offset + 12.0, y2: y + 6.0,
                        stroke: color.clone(), stroke_width: Some(2.5),
                        stroke_dasharray: None, class: "legend-symbol legend-line".to_string(),
                    });
                }
                _ => {
                    legend_elements.push(ChartElement::Rect {
                        x: x_offset, y, width: 12.0, height: 12.0,
                        fill: color.clone(), stroke: None,
                        class: "legend-symbol".to_string(), data: None,
                    });
                }
            }

            legend_elements.push(ChartElement::Text {
                x: x_offset + 18.0, y: y + 10.0, content: name.clone(),
                anchor: TextAnchor::Start, dominant_baseline: None,
                transform: None, font_size: Some("11px".to_string()),
                fill: Some("#333".to_string()), class: "legend-label".to_string(), data: None,
            });

            let tw = chartml_core::layout::labels::approximate_text_width(name);
            x_offset += 12.0 + 6.0 + tw + 16.0;
        }

        children.push(ChartElement::Group {
            class: "legend".to_string(), transform: None, children: legend_elements,
        });
    }

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-bar chartml-combo".to_string(),
        children,
    })
}

