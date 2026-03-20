use chartml_core::data::{get_f64, get_string, unique_values, group_by, Row};
use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::layout::stack::StackLayout;
use chartml_core::plugin::ChartConfig;
use chartml_core::layout::adaptive_tick_count;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::AreaGenerator;

use crate::helpers::{generate_x_axis, generate_y_axis_numeric, generate_legend, get_color_field, get_field_name, get_y_format, offset_element};

pub fn render_area(data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
    let category_field = get_field_name(&config.visualize.columns)?;
    let value_field = get_field_name(&config.visualize.rows)?;

    let categories = unique_values(data, &category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let color_field = get_color_field(config);
    let is_stacked = matches!(config.visualize.mode, Some(chartml_core::spec::ChartMode::Stacked));
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();

    // Calculate margins
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        has_legend: color_field.is_some(),
        ..Default::default()
    };
    let margins = calculate_margins(&margin_config);

    let inner_width = margins.inner_width(config.width);
    let inner_height = margins.inner_height(config.height);

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let bandwidth = band.bandwidth();

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

    let area_gen = AreaGenerator::new();
    let mut area_elements = Vec::new();

    if let Some(ref color_f) = color_field {
        let series_names = unique_values(data, color_f);
        let groups = group_by(data, color_f);

        if is_stacked && series_names.len() > 1 {
            // Build values matrix for stacking
            let mut values_matrix: Vec<Vec<f64>> = Vec::new();
            for series in &series_names {
                let rows = groups.get(series);
                let mut series_vals = Vec::new();
                for cat in &categories {
                    let val = rows
                        .map(|rs| {
                            rs.iter()
                                .find(|r| get_string(r, &category_field).as_deref() == Some(cat.as_str()))
                                .and_then(|r| get_f64(r, &value_field))
                                .unwrap_or(0.0)
                        })
                        .unwrap_or(0.0);
                    series_vals.push(val);
                }
                values_matrix.push(series_vals);
            }

            let stack = StackLayout::new();
            let stacked_points = stack.layout(&categories, &series_names, &values_matrix);

            let value_max = stacked_points
                .iter()
                .map(|p| p.y1)
                .fold(0.0_f64, f64::max);
            let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
            let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));

            // Group stacked points by series
            for (series_idx, series_name) in series_names.iter().enumerate() {
                let series_points: Vec<(f64, f64, f64)> = categories
                    .iter()
                    .enumerate()
                    .filter_map(|(_cat_idx, cat)| {
                        let point = stacked_points.iter().find(|p| {
                            p.key == *cat && p.series == *series_name
                        })?;
                        let x = band.map(cat)? + bandwidth / 2.0;
                        let y0 = linear.map(point.y0);
                        let y1 = linear.map(point.y1);
                        Some((x, y0, y1))
                    })
                    .collect();

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
                    fill: Some(color),
                    stroke: None,
                    stroke_width: None,
                    stroke_dasharray: None,
                    class: "area".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });
            }

            // Axes
            let x_axis_elements =
                generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height);
            let y_axis_elements =
                generate_y_axis_numeric((0.0, value_max), (inner_height, 0.0), margins.left, y_fmt_ref, adaptive_tick_count(inner_height));

            children.push(ChartElement::Group {
                class: "axes".to_string(),
                transform: None,
                children: {
                    let mut axes = Vec::new();
                    axes.extend(
                        x_axis_elements
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
            let values: Vec<f64> = data
                .iter()
                .filter_map(|row| get_f64(row, &value_field))
                .collect();
            let value_max = values.iter().cloned().fold(0.0_f64, f64::max);
            let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
            let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));
            let baseline = linear.map(0.0);

            for (series_idx, series_name) in series_names.iter().enumerate() {
                let series_rows = match groups.get(series_name) {
                    Some(rows) => rows,
                    None => continue,
                };

                let points: Vec<(f64, f64, f64)> = categories
                    .iter()
                    .filter_map(|cat| {
                        let row = series_rows.iter().find(|r| {
                            get_string(r, &category_field).as_deref() == Some(cat.as_str())
                        })?;
                        let x = band.map(cat)? + bandwidth / 2.0;
                        let y = linear.map(get_f64(row, &value_field)?);
                        Some((x, baseline, y))
                    })
                    .collect();

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
                    fill: Some(color),
                    stroke: None,
                    stroke_width: None,
                    stroke_dasharray: None,
                    class: "area".to_string(),
                    data: Some(ElementData::new(series_name, "").with_series(series_name)),
                });
            }

            // Axes
            let x_axis_elements =
                generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height);
            let y_axis_elements =
                generate_y_axis_numeric((0.0, value_max), (inner_height, 0.0), margins.left, y_fmt_ref, adaptive_tick_count(inner_height));

            children.push(ChartElement::Group {
                class: "axes".to_string(),
                transform: None,
                children: {
                    let mut axes = Vec::new();
                    axes.extend(
                        x_axis_elements
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
        let series_names_for_legend = unique_values(data, color_f);
        let legend_elements = generate_legend(
            &series_names_for_legend,
            &config.colors,
            config.width,
            config.height - 10.0,
        );
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else {
        // Single series area
        let values: Vec<f64> = data
            .iter()
            .filter_map(|row| get_f64(row, &value_field))
            .collect();
        let value_max = values.iter().cloned().fold(0.0_f64, f64::max);
        let value_max = if value_max <= 0.0 { 1.0 } else { value_max };
        let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));
        let baseline = linear.map(0.0);

        let points: Vec<(f64, f64, f64)> = categories
            .iter()
            .filter_map(|cat| {
                let row = data.iter().find(|r| {
                    get_string(r, &category_field).as_deref() == Some(cat.as_str())
                })?;
                let x = band.map(cat)? + bandwidth / 2.0;
                let y = linear.map(get_f64(row, &value_field)?);
                Some((x, baseline, y))
            })
            .collect();

        if !points.is_empty() {
            let path_d = area_gen.generate(&points);
            let color = config
                .colors
                .first()
                .cloned()
                .unwrap_or_else(|| "#2E7D9A".to_string());

            area_elements.push(ChartElement::Path {
                d: path_d,
                fill: Some(color),
                stroke: None,
                stroke_width: None,
                stroke_dasharray: None,
                class: "area".to_string(),
                data: None,
            });
        }

        // Axes
        let x_axis_elements =
            generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height);
        let y_axis_elements =
            generate_y_axis_numeric((0.0, value_max), (inner_height, 0.0), margins.left, y_fmt_ref, adaptive_tick_count(inner_height));

        children.push(ChartElement::Group {
            class: "axes".to_string(),
            transform: None,
            children: {
                let mut axes = Vec::new();
                axes.extend(
                    x_axis_elements
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

    Ok(ChartElement::Svg {
        viewbox: ViewBox::new(0.0, 0.0, config.width, config.height),
        width: Some(config.width),
        height: Some(config.height),
        class: "chartml-area".to_string(),
        children,
    })
}

