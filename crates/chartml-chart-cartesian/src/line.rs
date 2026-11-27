use chartml_core::data::DataTable;
use chartml_core::element::{paint_class, ChartElement, ElementData, TextAnchor, TextRole, TextStyle, Transform, ViewBox, emit_dot_halo_if_enabled};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::plugin::ChartConfig;
use chartml_core::layout::adaptive_tick_count;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::LineGenerator;

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig, TextMetrics};
use chartml_core::layout::legend::{LegendBlock, LegendConfig};


use crate::helpers::{cartesian_grid, point_data_labels, LegendMark, emit_zero_line_if_crosses, format_value, generate_annotations, generate_x_axis, generate_y_axis_numeric, generate_y_axis_numeric_right, get_color_field, get_field_name, get_x_format, get_y_format, offset_element};

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
                field: Some(name.clone()), mark: None, axis: None, label: None,
                color: None, format: None, data_labels: None,
                line_style: None, upper: None, lower: None, opacity: None,
                width: None, align: None,
            },
        }).collect(),
        _ => vec![],
    };
    let is_multi_field = !multi_fields.is_empty();
    // value_field is unused when all multi-fields are range marks (no scalar
    // series to plot), so falling back to an empty string is fine — data
    // lookups against an unknown field name return None and are benignly
    // skipped by filter_map.
    let value_field = if is_multi_field {
        multi_fields[0].field.clone().unwrap_or_default()
    } else {
        get_field_name(&config.visualize.rows)?
    };

    let color_field = get_color_field(config);

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

    // The y domains, derived once: the margin is estimated over them and the
    // axes are drawn over them. A range mark contributes its upper and lower
    // fields (the band is drawn on the axis its mark names), a dual-axis
    // chart splits its fields between the left and right axes.
    let side_values = |right: Option<bool>| -> Vec<f64> {
        let mut names: Vec<&str> = Vec::new();
        if is_multi_field {
            for f in &multi_fields {
                if let Some(right) = right {
                    if (f.axis.as_deref() == Some("right")) != right {
                        continue;
                    }
                }
                if f.mark.as_deref() == Some("range") {
                    names.extend(f.upper.as_deref());
                    names.extend(f.lower.as_deref());
                } else {
                    names.extend(f.field.as_deref());
                }
            }
        } else {
            names.push(value_field.as_str());
        }
        names.iter()
            .flat_map(|name| (0..data.num_rows()).filter_map(move |i| data.get_f64(i, name)))
            .collect()
    };
    let extent = |values: &[f64]| {
        (
            values.iter().cloned().fold(f64::INFINITY, f64::min),
            values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (domain_min, domain_max, right_domain): (f64, f64, Option<(f64, f64)>) = if has_right {
        let (left_min, left_max) = extent(&side_values(Some(false)));
        let (dm, dx) = crate::helpers::y_axis_domain(left_min, left_max, crate::helpers::get_y_axis_bounds(config));
        let (right_min, right_max) = extent(&side_values(Some(true)));
        let right = crate::helpers::y_axis_domain(right_min, right_max, crate::helpers::get_right_axis_bounds(config));
        (dm, dx, Some(right))
    } else {
        let (value_min, value_max) = extent(&side_values(None));
        let (dm, dx) = crate::helpers::y_axis_domain(value_min, value_max, crate::helpers::get_y_axis_bounds(config));
        (dm, dx, None)
    };
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let prelim_labels = crate::helpers::y_axis_tick_labels((domain_min, domain_max), y_fmt_ref);

    let right_fmt = config.visualize.axes.as_ref()
        .and_then(|a| a.right.as_ref())
        .and_then(|a| a.format.as_deref());
    let right_tick_labels: Vec<String> = match right_domain {
        Some(domain) => crate::helpers::y_axis_tick_labels(domain, right_fmt),
        None => vec![],
    };

    // Lay out the legend first so the margins keep room for it, from the
    // same entries it draws: one per field that draws a line (range marks
    // render as shaded bands and fields without a value draw nothing), in
    // the field's colour; or one per colour series.
    let (legend_names, legend_colors): (Vec<String>, Vec<String>) = if is_multi_field {
        multi_fields.iter().enumerate()
            .filter(|(_, f)| f.mark.as_deref() != Some("range"))
            .filter(|(_, f)| {
                let field = f.field.as_deref().unwrap_or("");
                categories.iter().any(|cat| category_value(data, &category_field, cat, field).is_some())
            })
            .map(|(field_idx, f)| {
                let label = f.label.clone().unwrap_or_else(|| f.field.clone().unwrap_or_default());
                (label, field_color(f, field_idx, config))
            })
            .unzip()
    } else if let Some(ref color_f) = color_field {
        (data.unique_values(color_f), config.colors.clone())
    } else {
        (vec![], vec![])
    };
    let legend = LegendBlock::for_series(config, &legend_names, &legend_colors, LegendConfig::for_theme(&config.theme))?;
    let legend_reserve = legend.as_ref().map(|l| l.reserve()).unwrap_or_default();

    let has_y_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let has_x_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let mut margin_config = MarginConfig {
        has_title: config.title.is_some(),
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
    legend_reserve.apply_to(&mut margin_config);
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let linear = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));
    let right_scale = right_domain.map(|(rmin, rmax)| ScaleLinear::new((rmin, rmax), (inner_height, 0.0)));

    let mut children = Vec::new();

    // The title is not drawn here: the host draws it as a heading, or the engine
    // draws it above this chart (chartml_core::title).

    // Axes — read format string from spec
    let grid = cartesian_grid(config);

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
        margin_left: margins.left,
        margin_bottom: margins.bottom - margin_config.bottom_legend_extent(),
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
            let rx = config.width - 12.0 - legend_reserve.right_inset();
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
                fill: Some(config.theme.axis_text_color().to_string()),
                class: paint_class("axis-label", config.theme.axis_label_color.is_some()),
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
    // `style.showDots: false` draws no point markers on lines (default: drawn).
    let show_dots = config.visualize.style.as_ref().and_then(|s| s.show_dots).unwrap_or(true);
    let bandwidth = band.bandwidth();
    let mut line_elements = Vec::new();

    if is_multi_field {
        // Multi-field rows: each field spec is a separate line series

        for (field_idx, field_spec) in multi_fields.iter().enumerate() {
            let color = field_color(field_spec, field_idx, config);

            // Range mark — render shaded area between upper and lower bounds
            if field_spec.mark.as_deref() == Some("range") {
                if let (Some(ref upper_field), Some(ref lower_field)) = (&field_spec.upper, &field_spec.lower) {
                    let fill_opacity = field_spec.opacity.unwrap_or(0.15);
                    // The band is drawn on the axis its mark names, as its
                    // upper and lower values are part of that axis's domain.
                    let band_scale = if field_spec.axis.as_deref() == Some("right") {
                        right_scale.as_ref().unwrap_or(&linear)
                    } else {
                        &linear
                    };
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
                        area_points.push((x, band_scale.map(upper_val), band_scale.map(lower_val)));
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
                            stroke_dashoffset: None,
                            opacity: Some(fill_opacity),
                            class: "range-area".to_string(),
                            data: None,
                            animation_origin: None,
                        });
                    }
                }
                continue; // range marks don't render as lines
            }

            // Reached only for non-range specs, which always have a `field`.
            let field_name = field_spec.field.as_deref().unwrap_or("");
            let label = field_spec.label.clone().unwrap_or_else(|| field_name.to_string());

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
                // The value the legend counted this field's entry by.
                let val = match category_value(data, &category_field, cat, field_name) {
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
                let (path_d, path_length) = line_gen.generate_with_length(&points);
                // When an explicit lineStyle dasharray is set (dashed/dotted),
                // keep it and skip animation attributes. Otherwise, set
                // dasharray + dashoffset to the computed path length so the
                // CSS draw animation reveals the full line.
                let (final_dasharray, final_dashoffset) = if dasharray.is_some() {
                    (dasharray, None)
                } else {
                    let len = format!("{}", path_length.ceil() as i64);
                    (Some(len.clone()), Some(len))
                };

                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: final_dasharray,
                    stroke_dashoffset: final_dashoffset,
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: Some(ElementData::new(&label, "").with_series(&label)),
                    animation_origin: None,
                });
            }

            // Hover dots
            if show_dots {
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
            }

            // Data labels (if configured on this field spec)
            if let Some(ref dl) = field_spec.data_labels {
                line_elements.extend(point_data_labels(&points, &point_data, dl, y_fmt_ref, &color, config));
            }
        }

        // Legend, in each field's own colour
        if let Some(ref legend) = legend {
            children.push(legend.group(config, |_| LegendMark::Line));
        }
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
                let (path_d, path_length) = line_gen.generate_with_length(&points);
                let len = format!("{}", path_length.ceil() as i64);
                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: Some(len.clone()),
                    stroke_dashoffset: Some(len),
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                    animation_origin: None,
                });
            }

            // Hover dots at each data point
            if show_dots {
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
        }

        // Legend
        if let Some(ref legend) = legend {
            children.push(legend.group(config, |_| LegendMark::Line));
        }
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
                let (path_d, path_length) = line_gen.generate_with_length(&points);
                let len = format!("{}", path_length.ceil() as i64);
                line_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(config.theme.series_line_weight as f64),
                    stroke_dasharray: Some(len.clone()),
                    stroke_dashoffset: Some(len),
                    opacity: None,
                    class: "chartml-line-path series-line".to_string(),
                    data: None,
                    animation_origin: None,
                });
            }

            // Hover dots at each data point
            if show_dots {
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

            // Data labels (`rows: { field, dataLabels }`)
            if let Some(dl) = crate::helpers::get_data_labels_config(config) {
                line_elements.extend(point_data_labels(&points, &point_data, dl, y_fmt_ref, &color, config));
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

/// A field's value for `cat`: from the first row of that category, as a
/// multi-field line plots it. `None` when the category has no row or the
/// value is missing.
fn category_value(data: &DataTable, category_field: &str, cat: &str, field: &str) -> Option<f64> {
    (0..data.num_rows())
        .find(|&i| data.get_string(i, category_field).as_deref() == Some(cat))
        .and_then(|i| data.get_f64(i, field))
}

/// A multi-field line's colour: its own `color`, else the palette colour at
/// its position.
fn field_color(field: &chartml_core::spec::FieldSpec, field_idx: usize, config: &ChartConfig) -> String {
    field.color.clone()
        .unwrap_or_else(|| config.colors.get(field_idx).cloned().unwrap_or_else(|| "#2E7D9A".to_string()))
}
