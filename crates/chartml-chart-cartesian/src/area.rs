use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, ElementData, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::layout::stack::{StackLayout, StackOffset};
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::AreaGenerator;

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig};

use chartml_core::layout::legend::{calculate_legend_layout, LegendConfig};

use crate::helpers::{GridConfig, generate_annotations, generate_x_axis, generate_y_axis_numeric, generate_legend, get_color_field, get_field_name, get_x_format, get_y_format, offset_element};

pub fn render_area(data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
    let category_field = get_field_name(&config.visualize.columns)?;
    let value_field = get_field_name(&config.visualize.rows)?;

    let categories = data.unique_values(&category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let color_field = get_color_field(config);
    let is_stacked = matches!(config.visualize.mode, Some(chartml_core::spec::ChartMode::Stacked));
    let is_normalized = matches!(config.visualize.mode, Some(chartml_core::spec::ChartMode::Normalized));
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let grid = GridConfig::from_config(config);
    let left_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_deref());

    // Step 1: Compute label strategy for margin estimation
    let estimated_width = config.width - 80.0;
    let x_format = get_x_format(config);
    let formatted_for_strategy = crate::helpers::format_display_labels(&categories, x_format.as_deref());
    let x_strategy = LabelStrategy::determine(&formatted_for_strategy, estimated_width, &LabelStrategyConfig::default());
    let x_extra_margin = match &x_strategy {
        LabelStrategy::Rotated { margin, .. } => *margin,
        _ => 0.0,
    };

    // Step 1b: Pre-compute domain for left margin estimation (matches JS two-pass approach).
    let prelim_values: Vec<f64> = (0..data.num_rows()).filter_map(|i| data.get_f64(i, &value_field)).collect();
    let prelim_min = prelim_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let prelim_max = prelim_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let prelim_max = if prelim_max <= 0.0 { 1.0 } else { prelim_max };
    // When data has negative values, use the actual minimum; otherwise anchor at 0
    let prelim_domain_min = if prelim_min < 0.0 { prelim_min } else { 0.0 };
    let (prelim_nice_min, prelim_nice_max) = crate::helpers::nice_domain(prelim_domain_min, prelim_max, 5);
    let area_prelim_fmt = if is_normalized { Some(".0%") } else { y_fmt_ref };
    let area_prelim_labels = vec![
        crate::helpers::format_value(prelim_nice_min, area_prelim_fmt),
        crate::helpers::format_value(prelim_nice_max, area_prelim_fmt),
    ];

    // Pre-compute legend height so the bottom margin accounts for multi-row legends.
    let legend_height = if let Some(ref color_f) = color_field {
        let series_names = data.unique_values(color_f);
        let legend_config = LegendConfig::default();
        calculate_legend_layout(&series_names, &config.colors, config.width, &legend_config).total_height
    } else {
        0.0
    };

    // Step 2: Calculate margins including rotation
    let has_x_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.x.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let has_y_axis_label = config.visualize.axes.as_ref()
        .and_then(|a| a.left.as_ref())
        .and_then(|a| a.label.as_ref())
        .is_some();
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        legend_height,
        has_x_axis_label,
        has_y_axis_label,
        x_label_strategy_margin: x_extra_margin,
        y_tick_labels: area_prelim_labels,
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let bandwidth = band.bandwidth();

    let mut children = Vec::new();

    // Title is rendered as HTML outside the SVG — not added here.

    let area_gen = AreaGenerator::new().curve(chartml_core::shapes::CurveType::MonotoneX);
    let mut area_elements = Vec::new();

    // Track Y domain for annotations (set inside each branch below)
    let y_domain_min: f64;
    let y_domain_max: f64;

    if let Some(ref color_f) = color_field {
        let series_names = data.unique_values(color_f);
        let groups = data.group_by(color_f);

        if (is_stacked || is_normalized) && series_names.len() > 1 {
            // Build values matrix for stacking
            let mut values_matrix: Vec<Vec<f64>> = Vec::new();
            for series in &series_names {
                let series_data = groups.get(series);
                let mut series_vals = Vec::new();
                for cat in &categories {
                    let val = series_data
                        .map(|sd| {
                            (0..sd.num_rows())
                                .find_map(|i| {
                                    if sd.get_string(i, &category_field).as_deref() == Some(cat.as_str()) {
                                        sd.get_f64(i, &value_field)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0.0)
                        })
                        .unwrap_or(0.0);
                    series_vals.push(val);
                }
                values_matrix.push(series_vals);
            }

            let stack = if is_normalized {
                StackLayout::new().offset(StackOffset::Normalize)
            } else {
                StackLayout::new()
            };
            let stacked_points = stack.layout(&categories, &series_names, &values_matrix);

            // For normalized mode, the stack values are 0-1; Y-axis domain is 0-1
            // with ".0%" format (NumberFormatter multiplies by 100 and appends %).
            // For regular stacked mode, use the raw stacked max with nice rounding.
            let (value_min, value_max, y_axis_fmt): (f64, f64, Option<&str>) = if is_normalized {
                (0.0, 1.0, Some(".0%"))
            } else {
                let raw_value_max = stacked_points
                    .iter()
                    .map(|p| p.y1)
                    .fold(0.0_f64, f64::max);
                let raw_value_max = if raw_value_max <= 0.0 { 1.0 } else { raw_value_max };
                // Match JS: yLeft.nice() uses default count=10 for domain rounding.
                let (_, nice_max) = crate::helpers::nice_domain(0.0, raw_value_max, 5);
                (0.0, nice_max, y_fmt_ref)
            };
            let linear = ScaleLinear::new((value_min, value_max), (inner_height, 0.0));
            y_domain_min = value_min;
            y_domain_max = value_max;

            // Group stacked points by series
            for (series_idx, series_name) in series_names.iter().enumerate() {
                let mut series_points: Vec<(f64, f64, f64)> = Vec::new();
                let mut dot_data: Vec<(String, f64, f64)> = Vec::new(); // (category, y1_value, y1_pixel)

                for cat in &categories {
                    let point = match stacked_points.iter().find(|p| {
                        p.key == *cat && p.series == *series_name
                    }) {
                        Some(p) => p,
                        None => continue,
                    };
                    let x = match band.map(cat) {
                        Some(x) => x + bandwidth / 2.0,
                        None => continue,
                    };
                    let y0 = linear.map(point.y0);
                    let y1 = linear.map(point.y1);
                    series_points.push((x, y0, y1));
                    // The individual series value is y1 - y0 (unstacked)
                    let series_val = point.y1 - point.y0;
                    dot_data.push((cat.clone(), series_val, y1));
                }

                if series_points.is_empty() {
                    continue;
                }

                let path_d = area_gen.generate(&series_points);
                let color = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                area_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: Some(color.clone()),
                    stroke: None,
                    stroke_width: None,
                    stroke_dasharray: None,
                    opacity: Some(0.6),
                    class: "chartml-area-path".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });

                // Stroke line along the top edge of the area
                let line_d = area_gen.generate_line(&series_points);
                area_elements.push(ChartElement::Path {
                    d: line_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(2.0),
                    stroke_dasharray: None,
                    opacity: None,
                    class: "chartml-line-path".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });

                // Area charts do not show dots by default (matches JS reference behaviour)
            }

            // Axes
            let bottom_axis_label = config.visualize.axes.as_ref()
                .and_then(|a| a.x.as_ref())
                .and_then(|a| a.label.as_deref());
            let x_axis_result =
                generate_x_axis(&crate::helpers::XAxisParams {
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
            let y_axis_elements =
                generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
                    domain: (value_min, value_max),
                    range: (inner_height, 0.0),
                    x_position: margins.left,
                    fmt: y_axis_fmt,
                    tick_count: 5,
                    chart_width: Some(inner_width),
                    grid: &grid,
                    axis_label: left_axis_label,
                    theme: &config.theme,
                });

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
        } else {
            // Multiple series, non-stacked: each area from baseline
            let values: Vec<f64> = (0..data.num_rows())
                .filter_map(|i| data.get_f64(i, &value_field))
                .collect();
            let raw_value_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let raw_value_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let raw_value_max = if raw_value_max <= 0.0 { 1.0 } else { raw_value_max };
            // When data has negative values, use the actual minimum; otherwise anchor at 0
            let domain_min = if raw_value_min < 0.0 { raw_value_min } else { 0.0 };
            let (value_min, value_max) = crate::helpers::nice_domain(domain_min, raw_value_max, 5);
            let linear = ScaleLinear::new((value_min, value_max), (inner_height, 0.0));
            y_domain_min = value_min;
            y_domain_max = value_max;
            let baseline = linear.map(0.0);

            for (series_idx, series_name) in series_names.iter().enumerate() {
                let series_data = match groups.get(series_name) {
                    Some(d) => d,
                    None => continue,
                };

                let mut points: Vec<(f64, f64, f64)> = Vec::new();
                let mut dot_data: Vec<(String, f64)> = Vec::new();

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
                    points.push((x, baseline, y));
                    dot_data.push((cat.clone(), val));
                }

                if points.is_empty() {
                    continue;
                }

                let path_d = area_gen.generate(&points);
                let color = config
                    .colors
                    .get(series_idx)
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                area_elements.push(ChartElement::Path {
                    d: path_d,
                    fill: Some(color.clone()),
                    stroke: None,
                    stroke_width: None,
                    stroke_dasharray: None,
                    opacity: Some(0.6),
                    class: "chartml-area-path".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });

                // Stroke line along the top edge of the area
                let line_d = area_gen.generate_line(&points);
                area_elements.push(ChartElement::Path {
                    d: line_d,
                    fill: None,
                    stroke: Some(color.clone()),
                    stroke_width: Some(2.0),
                    stroke_dasharray: None,
                    opacity: None,
                    class: "chartml-line-path".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });

            }

            // Axes
            let bottom_axis_label = config.visualize.axes.as_ref()
                .and_then(|a| a.x.as_ref())
                .and_then(|a| a.label.as_deref());
            let x_axis_result =
                generate_x_axis(&crate::helpers::XAxisParams {
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
            let y_axis_elements =
                generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
                    domain: (value_min, value_max),
                    range: (inner_height, 0.0),
                    x_position: margins.left,
                    fmt: None,
                    tick_count: 5,
                    chart_width: Some(inner_width),
                    grid: &grid,
                    axis_label: left_axis_label,
                    theme: &config.theme,
                });

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
        }

        // Legend
        let series_names_for_legend = data.unique_values(color_f);
        let legend_config = LegendConfig::default();
        let legend_layout = calculate_legend_layout(&series_names_for_legend, &config.colors, config.width, &legend_config);
        let legend_y = config.height - legend_layout.total_height - 8.0;
        let legend_elements = generate_legend(
            &series_names_for_legend,
            &config.colors,
            config.width,
            legend_y,
            &config.theme,
        );
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else {
        // Single series area
        let values: Vec<f64> = (0..data.num_rows())
            .filter_map(|i| data.get_f64(i, &value_field))
            .collect();
        let raw_value_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let raw_value_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let raw_value_max = if raw_value_max <= 0.0 { 1.0 } else { raw_value_max };
        // When data has negative values, use the actual minimum; otherwise anchor at 0
        let domain_min = if raw_value_min < 0.0 { raw_value_min } else { 0.0 };
        let (value_min, value_max) = crate::helpers::nice_domain(domain_min, raw_value_max, 5);
        let linear = ScaleLinear::new((value_min, value_max), (inner_height, 0.0));
        y_domain_min = value_min;
        y_domain_max = value_max;
        let baseline = linear.map(0.0);

        let mut points: Vec<(f64, f64, f64)> = Vec::new();

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
            points.push((x, baseline, y));
        }

        if !points.is_empty() {
            let path_d = area_gen.generate(&points);
            let color = config
                .colors
                .first()
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            area_elements.push(ChartElement::Path {
                d: path_d,
                fill: Some(color.clone()),
                stroke: None,
                stroke_width: None,
                stroke_dasharray: None,
                opacity: Some(0.6),
                class: "chartml-area-path".to_string(),
                data: None,
            });

            // Stroke line along the top edge of the area
            let line_d = area_gen.generate_line(&points);
            area_elements.push(ChartElement::Path {
                d: line_d,
                fill: None,
                stroke: Some(color.clone()),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                opacity: None,
                class: "chartml-line-path".to_string(),
                data: None,
            });

            // Area charts do not show dots by default (matches JS reference behaviour)
        }

        // Axes
        let bottom_axis_label = config.visualize.axes.as_ref()
            .and_then(|a| a.x.as_ref())
            .and_then(|a| a.label.as_deref());
        let x_axis_result =
            generate_x_axis(&crate::helpers::XAxisParams {
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
        let y_axis_elements =
            generate_y_axis_numeric(&crate::helpers::YAxisNumericParams {
                domain: (value_min, value_max),
                range: (inner_height, 0.0),
                x_position: margins.left,
                fmt: None,
                tick_count: 5,
                chart_width: Some(inner_width),
                grid: &grid,
                axis_label: left_axis_label,
                theme: &config.theme,
            });

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
    }

    children.push(ChartElement::Group {
        class: "areas".to_string(),
        transform: Some(Transform::Translate(margins.left, margins.top)),
        children: area_elements,
    });

    // Annotations — rendered on top of marks, in inner coordinate space
    if let Some(annotations) = config.visualize.annotations.as_deref() {
        if !annotations.is_empty() {
            use chartml_core::scales::ScaleLinear;
            let ann_scale = ScaleLinear::new((y_domain_min, y_domain_max), (inner_height, 0.0));
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

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-area".to_string(),
        children,
    })
}

