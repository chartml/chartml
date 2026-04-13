use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, ElementData, TextAnchor, TextRole, TextStyle, Transform, ViewBox, emit_dot_halo_if_enabled};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::plugin::ChartConfig;
use chartml_core::layout::adaptive_tick_count;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::LineGenerator;

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig, TextMetrics};

use chartml_core::layout::legend::{calculate_legend_layout, LegendConfig};

use crate::helpers::{GridConfig, LegendMark, emit_zero_line_if_crosses, format_value, generate_annotations, generate_x_axis, generate_y_axis_numeric, generate_y_axis_numeric_right, generate_legend_with_mark, get_color_field, get_field_name, get_x_format, get_y_format, nice_domain, offset_element};

pub fn render_line(data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
    use chartml_core::spec::{FieldRef, FieldRefItem, FieldSpec};

    let category_field = get_field_name(&config.visualize.columns)?;

    let categories = data.unique_values(&category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    // Detect multi-field rows (e.g., [{field: revenue, color: ...}, {field: target, ...}])
    let multi_fields: Vec<FieldSpec> = match &config.visualize.rows {
        Some(FieldRef::Multiple(items)) => items.iter().map(|item| match item {
            FieldRefItem::Detailed(spec) => spec.as_ref().clone(),
            FieldRefItem::Simple(name) => FieldSpec {
                field: name.clone(), mark: None, axis: None, label: None,
                color: None, format: None, data_labels: None,
                line_style: None, upper: None, lower: None, opacity: None,
            },
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
    let formatted_for_strategy = crate::helpers::format_display_labels(&categories, x_format.as_deref());
    let x_strategy = LabelStrategy::determine(&formatted_for_strategy, estimated_width, &LabelStrategyConfig {
        text_metrics: TextMetrics::from_theme_axis_label(&config.theme),
        ..LabelStrategyConfig::default()
    });
    let x_extra_margin = match &x_strategy {
        LabelStrategy::Rotated { margin, .. } => *margin,
        _ => 0.0,
    };

    // Detect dual-axis: check if any multi-field spec has axis: "right"
    let has_right = is_multi_field && multi_fields.iter().any(|f| f.axis.as_deref() == Some("right"));

    // Step 1b: Pre-compute domain for left margin estimation (matches JS two-pass approach).
    // When dual-axis, only left-axis fields contribute to the left prelim domain.
    let all_value_fields_prelim: Vec<String> = if is_multi_field {
        let mut fields: Vec<String> = multi_fields.iter()
            .filter(|f| f.mark.as_deref() != Some("range"))
            .filter(|f| !has_right || f.axis.as_deref() != Some("right"))
            .map(|f| f.field.clone())
            .collect();
        // Include upper/lower bound fields from range marks for domain calculation
        for f in &multi_fields {
            if f.mark.as_deref() == Some("range") {
                if let Some(ref upper) = f.upper { fields.push(upper.clone()); }
                if let Some(ref lower) = f.lower { fields.push(lower.clone()); }
            }
        }
        fields
    } else {
        vec![value_field.clone()]
    };
    let mut all_values_prelim: Vec<f64> = Vec::new();
    for field in &all_value_fields_prelim {
        for i in 0..data.num_rows() {
            if let Some(v) = data.get_f64(i, field) {
                all_values_prelim.push(v);
            }
        }
    }
    let prelim_value_max = all_values_prelim.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let prelim_value_min = all_values_prelim.iter().cloned().fold(f64::INFINITY, f64::min);
    let prelim_domain_min = if prelim_value_min >= 0.0 { 0.0 } else { prelim_value_min };
    let prelim_domain_max = if prelim_value_max <= 0.0 { 1.0 } else { prelim_value_max };
    let (prelim_domain_min, prelim_domain_max) = nice_domain(prelim_domain_min, prelim_domain_max, 5);
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let prelim_labels = vec![
        format_value(prelim_domain_max, y_fmt_ref),
        format_value(prelim_domain_min, y_fmt_ref),
    ];

    // Pre-compute right tick labels for margin estimation (mirrors bar.rs render_combo)
    let right_fmt = config.visualize.axes.as_ref()
        .and_then(|a| a.right.as_ref())
        .and_then(|a| a.format.as_deref());
    let right_tick_labels: Vec<String> = if has_right {
        let right_max = multi_fields.iter()
            .filter(|f| f.axis.as_deref() == Some("right"))
            .flat_map(|f| (0..data.num_rows()).filter_map(|i| data.get_f64(i, &f.field)))
            .fold(0.0_f64, f64::max);
        let right_domain_max = if right_max <= 0.0 { 1.0 } else { right_max };
        let tmp_scale = ScaleLinear::new((0.0, right_domain_max), (0.0, 100.0));
        tmp_scale.ticks(5).iter().map(|v| format_value(*v, right_fmt)).collect()
    } else {
        vec![]
    };

    // Pre-compute legend height so the bottom margin accounts for multi-row legends.
    let legend_height = if has_series {
        let legend_series_names: Vec<String> = if is_multi_field {
            multi_fields.iter().map(|f| {
                f.label.clone().unwrap_or_else(|| f.field.clone())
            }).collect()
        } else if let Some(ref color_f) = color_field {
            data.unique_values(color_f)
        } else {
            vec![]
        };
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        calculate_legend_layout(&legend_series_names, &config.colors, config.width, &legend_config).total_height
    } else {
        0.0
    };

    let has_y_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let has_x_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        legend_height,
        has_y_axis_label,
        has_x_axis_label,
        x_label_strategy_margin: x_extra_margin,
        y_tick_labels: prelim_labels,
        has_right_axis: has_right,
        right_tick_labels,
        tick_value_metrics: TextMetrics::from_theme_tick_value(&config.theme),
        axis_label_metrics: TextMetrics::from_theme_axis_label(&config.theme),
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    // Find value extent — when dual-axis, compute separate domains for left and right
    let (domain_min, domain_max, right_domain): (f64, f64, Option<(f64, f64)>) = if has_right {
        // Left-axis fields only
        let left_fields: Vec<&str> = multi_fields.iter()
            .filter(|f| f.axis.as_deref() != Some("right") && f.mark.as_deref() != Some("range"))
            .map(|f| f.field.as_str())
            .collect();
        let mut left_vals: Vec<f64> = Vec::new();
        for field in &left_fields {
            for i in 0..data.num_rows() {
                if let Some(v) = data.get_f64(i, field) { left_vals.push(v); }
            }
        }
        let left_min = left_vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let left_max = left_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let left_domain_min = if left_min >= 0.0 { 0.0 } else { left_min };
        let left_domain_max = if left_max <= 0.0 { 1.0 } else { left_max };
        let (left_domain_min, left_domain_max) = nice_domain(left_domain_min, left_domain_max, 5);

        // Right-axis fields only
        let right_fields: Vec<&str> = multi_fields.iter()
            .filter(|f| f.axis.as_deref() == Some("right"))
            .map(|f| f.field.as_str())
            .collect();
        let mut right_vals: Vec<f64> = Vec::new();
        for field in &right_fields {
            for i in 0..data.num_rows() {
                if let Some(v) = data.get_f64(i, field) { right_vals.push(v); }
            }
        }
        let right_min = right_vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let right_max = right_vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let right_domain_min = if right_min >= 0.0 { 0.0 } else { right_min };
        let right_domain_max = if right_max <= 0.0 { 1.0 } else { right_max };
        let (right_domain_min, right_domain_max) = nice_domain(right_domain_min, right_domain_max, 5);

        (left_domain_min, left_domain_max, Some((right_domain_min, right_domain_max)))
    } else {
        // Single-axis: all fields share one domain
        let all_value_fields: Vec<String> = if is_multi_field {
            multi_fields.iter().map(|f| f.field.clone()).collect()
        } else {
            vec![value_field.clone()]
        };
        let mut all_values: Vec<f64> = Vec::new();
        for field in &all_value_fields {
            for i in 0..data.num_rows() {
                if let Some(v) = data.get_f64(i, field) { all_values.push(v); }
            }
        }
        let value_min = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let value_max = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let dm = if value_min >= 0.0 { 0.0 } else { value_min };
        let dx = if value_max <= 0.0 { 1.0 } else { value_max };
        let (dm, dx) = nice_domain(dm, dx, 5);
        (dm, dx, None)
    };

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let linear = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));
    let right_scale = right_domain.map(|(rmin, rmax)| ScaleLinear::new((rmin, rmax), (inner_height, 0.0)));

    let mut children = Vec::new();

    // Title is rendered as HTML outside the SVG — not added here.

    // Axes — read format string from spec
    let grid = GridConfig::from_config(config);

    let bottom_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_deref());
    let x_axis_result = generate_x_axis(&crate::helpers::XAxisParams {
        labels: &categories,
        display_label_overrides: None,
        range: (0.0, inner_width),
        y_position: margins.top + inner_height,
        available_width: inner_width,
        x_format: x_format.as_deref(),
        chart_height: Some(inner_height),
        grid: &grid,
        axis_label: bottom_axis_label,
        theme: &config.theme,
    });
    let left_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_deref());
    let y_axis_elements = generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
        domain: (domain_min, domain_max),
        range: (inner_height, 0.0),
        x_position: margins.left,
        fmt: y_fmt_ref,
        tick_count: adaptive_tick_count(inner_height),
        chart_width: Some(inner_width),
        grid: &grid,
        axis_label: left_axis_label,
        theme: &config.theme,
    });

    let mut axis_elements = Vec::new();
    axis_elements.extend(
        x_axis_result.elements
            .into_iter()
            .map(|e| offset_element(e, margins.left, 0.0)),
    );
    axis_elements.extend(
        y_axis_elements
            .into_iter()
            .map(|e| offset_element(e, 0.0, margins.top)),
    );
    // Zero-line (Phase 7): emitted after the y-axis grid lines so the line
    // series paints over it. No-op when theme.zero_line is None (default) or
    // when the domain doesn't strictly cross zero.
    if let Some(zl) = emit_zero_line_if_crosses(
        &config.theme,
        (domain_min, domain_max),
        inner_width,
        inner_height,
        false,
    ) {
        axis_elements.push(offset_element(zl, margins.left, margins.top));
    }

    // Right axis — ticks and labels on the right side
    if let Some(ref rs) = right_scale {
        let right_axis = generate_y_axis_numeric_right(
            rs.domain(), (inner_height, 0.0), margins.left + inner_width,
            right_fmt, adaptive_tick_count(inner_height),
            None, &config.theme,
        );
        axis_elements.extend(right_axis.into_iter().map(|e| offset_element(e, 0.0, margins.top)));
    }

    // Right axis title label — rendered manually with absolute positioning
    if has_right {
        if let Some(label) = config.visualize.axes.as_ref().and_then(|a| a.right.as_ref()).and_then(|a| a.label.clone()) {
            let rx = config.width - 12.0;
            let ts = TextStyle::for_role(&config.theme, TextRole::AxisLabel);
            axis_elements.push(ChartElement::Text {
                x: rx,
                y: margins.top + inner_height / 2.0,
                content: label,
                anchor: TextAnchor::Middle,
                dominant_baseline: None,
                transform: Some(Transform::Rotate(90.0, rx, margins.top + inner_height / 2.0)),
                font_family: ts.font_family,
                font_size: ts.font_size,
                font_weight: ts.font_weight,
                letter_spacing: ts.letter_spacing,
                text_transform: ts.text_transform,
                fill: Some(config.theme.text_secondary.clone()),
                class: "axis-label".to_string(),
                data: None,
            });
        }
    }

    children.push(ChartElement::Group {
        class: "axes".to_string(),
        transform: None,
        children: axis_elements,
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
                &config.theme,
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

    // Line paths — select curve type from style.curveType
    let curve_type = match config.visualize.style.as_ref().and_then(|s| s.curve_type.as_deref()) {
        Some("step") => chartml_core::shapes::CurveType::Step,
        Some("linear") => chartml_core::shapes::CurveType::Linear,
        _ => chartml_core::shapes::CurveType::MonotoneX,
    };
    let line_gen = LineGenerator::new().curve(curve_type);
    let bandwidth = band.bandwidth();
    let mut line_elements = Vec::new();

    if is_multi_field {
        // Multi-field rows: each field spec is a separate line series
        let mut series_names = Vec::new();
        let mut series_colors = Vec::new();

        for (field_idx, field_spec) in multi_fields.iter().enumerate() {
            let color = field_spec.color.clone()
                .unwrap_or_else(|| config.colors.get(field_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string()));

            // Range mark — render shaded area between upper and lower bounds
            if field_spec.mark.as_deref() == Some("range") {
                if let (Some(ref upper_field), Some(ref lower_field)) = (&field_spec.upper, &field_spec.lower) {
                    let fill_opacity = field_spec.opacity.unwrap_or(0.15);
                    let mut area_points: Vec<(f64, f64, f64)> = Vec::new(); // (x, y_upper, y_lower)

                    for cat in &categories {
                        let row_i = match (0..data.num_rows()).find(|&i| data.get_string(i, &category_field).as_deref() == Some(cat.as_str())) {
                            Some(i) => i,
                            None => continue,
                        };
                        // Skip rows where bounds are null (historical data)
                        let upper_val = match data.get_f64(row_i, upper_field) {
                            Some(v) => v,
                            None => continue,
                        };
                        let lower_val = match data.get_f64(row_i, lower_field) {
                            Some(v) => v,
                            None => continue,
                        };
                        let x = match band.map(cat) {
                            Some(x) => x + bandwidth / 2.0,
                            None => continue,
                        };
                        area_points.push((x, linear.map(upper_val), linear.map(lower_val)));
                    }

                    if !area_points.is_empty() {
                        // Build area path: forward along upper, backward along lower
                        let mut d = String::new();
                        for (i, &(x, y_upper, _)) in area_points.iter().enumerate() {
                            if i == 0 { d.push_str(&format!("M{:.2},{:.2}", x, y_upper)); }
                            else { d.push_str(&format!("L{:.2},{:.2}", x, y_upper)); }
                        }
                        for &(x, _, y_lower) in area_points.iter().rev() {
                            d.push_str(&format!("L{:.2},{:.2}", x, y_lower));
                        }
                        d.push('Z');

                        line_elements.push(ChartElement::Path {
                            d,
                            fill: Some(color.clone()),
                            stroke: None,
                            stroke_width: None,
                            stroke_dasharray: None,
                            opacity: Some(fill_opacity),
                            class: "range-area".to_string(),
                            data: None,
                            animation_origin: None,
                        });
                    }
                }
                continue; // range marks don't render as lines
            }

            let field_name = &field_spec.field;
            let label = field_spec.label.clone().unwrap_or_else(|| field_name.clone());

            // Determine dash pattern from lineStyle
            let dasharray = match field_spec.line_style.as_deref() {
                Some("dashed") => Some("8 4".to_string()),
                Some("dotted") => Some("2 4".to_string()),
                _ => None,
            };

            // Select scale based on axis assignment
            let is_right_axis = field_spec.axis.as_deref() == Some("right");
            let scale_for_field = if is_right_axis {
                right_scale.as_ref().unwrap_or(&linear)
            } else {
                &linear
            };
            let fmt_for_field: Option<&str> = if is_right_axis { right_fmt } else { y_fmt_ref };

            let mut points: Vec<(f64, f64)> = Vec::new();
            let mut point_data: Vec<(String, f64)> = Vec::new();

            for cat in &categories {
                // Find the row for this category
                let row_i = match (0..data.num_rows()).find(|&i| data.get_string(i, &category_field).as_deref() == Some(cat.as_str())) {
                    Some(i) => i,
                    None => continue,
                };
                let val = match data.get_f64(row_i, field_name) {
                    Some(v) => v,
                    None => continue,
                };
                let x = match band.map(cat) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                let y = scale_for_field.map(val);
                points.push((x, y));
                point_data.push((cat.clone(), val));
            }

            if points.is_empty() {
                continue;
            }

            // Only emit a line path when there are 2+ points (a single point
            // produces a degenerate M-only path with no visible line).
            if points.len() > 1 {
                let path_d = line_gen.generate(&points);

                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: dasharray,
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: Some(ElementData::new(&label, "").with_series(&label)),
                    animation_origin: None,
                });
            }

            // Hover dots
            let dot_r = config.theme.dot_radius as f64;
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                if let Some(halo) = emit_dot_halo_if_enabled(&config.theme, px, py, dot_r) {
                    line_elements.push(halo);
                }
                line_elements.push(ChartElement::Circle {
                    cx: px, cy: py, r: dot_r,
                    fill: color.clone(),
                    stroke: Some(config.theme.bg.clone()),
                    class: "chartml-line-dot dot-marker".to_string(),
                    data: Some(ElementData::new(cat, format_value(val, fmt_for_field)).with_series(&label)),
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
                            font_family: None,
                            font_size: Some(dl.font_size.map(|s| format!("{}px", s)).unwrap_or_else(|| "12px".to_string())),
                            font_weight: None,
                            letter_spacing: None,
                            text_transform: None,
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
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        let legend_layout = calculate_legend_layout(&series_names, &series_colors, config.width, &legend_config);
        let legend_y = config.height - legend_layout.total_height - 8.0;
        let legend_elements = generate_legend_with_mark(&series_names, &series_colors, config.width, legend_y, LegendMark::Line, &config.theme);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else if let Some(ref color_f) = color_field {
        let series_names = data.unique_values(color_f);
        let groups = data.group_by(color_f);

        for (series_idx, series_name) in series_names.iter().enumerate() {
            let series_data = match groups.get(series_name) {
                Some(d) => d,
                None => continue,
            };

            let mut points: Vec<(f64, f64)> = Vec::new();
            let mut point_data: Vec<(String, f64)> = Vec::new();

            for cat in &categories {
                let row_i = match (0..series_data.num_rows()).find(|&i| {
                    series_data.get_string(i, &category_field).as_deref() == Some(cat.as_str())
                }) {
                    Some(i) => i,
                    None => continue,
                };
                let val = match series_data.get_f64(row_i, &value_field) {
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

            let color = config
                .colors
                .get(series_idx)
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            if points.len() > 1 {
                let path_d = line_gen.generate(&points);
                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: None,
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                    animation_origin: None,
                });
            }

            // Hover dots at each data point
            let dot_r = config.theme.dot_radius as f64;
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                if let Some(halo) = emit_dot_halo_if_enabled(&config.theme, px, py, dot_r) {
                    line_elements.push(halo);
                }
                line_elements.push(ChartElement::Circle {
                    cx: px,
                    cy: py,
                    r: dot_r,
                    fill: color.clone(),
                    stroke: Some(config.theme.bg.clone()),
                    class: "chartml-line-dot dot-marker".to_string(),
                    data: Some(ElementData::new(cat, format_value(val, y_fmt_ref)).with_series(series_name)),
                });
            }
        }

        // Legend
        let legend_config = LegendConfig {
            text_metrics: TextMetrics::from_theme_legend(&config.theme),
            ..LegendConfig::default()
        };
        let legend_layout = calculate_legend_layout(&series_names, &config.colors, config.width, &legend_config);
        let legend_y = config.height - legend_layout.total_height - 8.0;
        let legend_elements =
            generate_legend_with_mark(&series_names, &config.colors, config.width, legend_y, LegendMark::Line, &config.theme);
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
            let row_i = match (0..data.num_rows()).find(|&i| {
                data.get_string(i, &category_field).as_deref() == Some(cat.as_str())
            }) {
                Some(i) => i,
                None => continue,
            };
            let val = match data.get_f64(row_i, &value_field) {
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
            let color = config
                .colors
                .first()
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            if points.len() > 1 {
                let path_d = line_gen.generate(&points);
                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: None,
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: None,
                    animation_origin: None,
                });
            }

            // Hover dots at each data point
            let dot_r = config.theme.dot_radius as f64;
            for (i, &(px, py)) in points.iter().enumerate() {
                let (ref cat, val) = point_data[i];
                if let Some(halo) = emit_dot_halo_if_enabled(&config.theme, px, py, dot_r) {
                    line_elements.push(halo);
                }
                line_elements.push(ChartElement::Circle {
                    cx: px,
                    cy: py,
                    r: dot_r,
                    fill: color.clone(),
                    stroke: Some(config.theme.bg.clone()),
                    class: "chartml-line-dot dot-marker".to_string(),
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

