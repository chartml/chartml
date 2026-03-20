use chartml_core::data::{get_f64, get_string, unique_values, group_by, Row};
use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::layout::stack::StackLayout;
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::spec::{ChartMode, Orientation};

use crate::helpers::{generate_x_axis, generate_y_axis, get_color_field, get_field_name};

pub fn render_bar(data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
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

    // Calculate margins
    let margin_config = MarginConfig {
        has_title: config.title.is_some(),
        has_legend: color_field.is_some(),
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
        });
    }

    // Determine value domain
    let (value_max, bar_elements) = if let Some(ref color_f) = color_field {
        render_multi_series_bars(
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
        )?
    } else {
        render_single_series_bars(
            data,
            config,
            &category_field,
            &value_field,
            &categories,
            &margins,
            inner_width,
            inner_height,
            is_horizontal,
        )?
    };

    // Axes
    let axis_elements = if is_horizontal {
        let x_axis = generate_y_axis(&categories, (0.0, inner_height), margins.left, None);
        let y_axis = generate_x_axis_numeric((0.0, value_max), (0.0, inner_width), margins.top + inner_height);
        let mut axes = Vec::new();
        axes.extend(x_axis.into_iter().map(|e| offset_element(e, margins.left, margins.top)));
        axes.extend(y_axis.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
        axes
    } else {
        let x_axis = generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height);
        let y_axis = generate_y_axis_numeric((0.0, value_max), (inner_height, 0.0), margins.left);
        let mut axes = Vec::new();
        axes.extend(x_axis.into_iter().map(|e| offset_element(e, margins.left, 0.0)));
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
) -> Result<(f64, Vec<ChartElement>), ChartError> {
    // Find the max value
    let values: Vec<f64> = data
        .iter()
        .filter_map(|row| get_f64(row, value_field))
        .collect();
    let value_max = values.iter().cloned().fold(0.0_f64, f64::max);
    let value_max = if value_max <= 0.0 { 1.0 } else { value_max };

    let mut elements = Vec::new();
    let fill = config.colors.first().cloned().unwrap_or_else(|| "#2E7D9A".to_string());

    if is_horizontal {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_height));
        let linear = ScaleLinear::new((0.0, value_max), (0.0, inner_width));

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
                data: Some(ElementData::new(&cat, format!("{}", val))),
            });
        }
    } else {
        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));

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
                data: Some(ElementData::new(&cat, format!("{}", val))),
            });
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

        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));

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
                    ElementData::new(&point.key, format!("{}", point.value))
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

        let band = ScaleBand::new(categories.to_vec(), (0.0, inner_width));
        let linear = ScaleLinear::new((0.0, value_max), (inner_height, 0.0));

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
                    ElementData::new(&cat, format!("{}", val)).with_series(&series),
                ),
            });
        }

        Ok((value_max, elements))
    }
}

fn generate_x_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    y_position: f64,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(5);
    let mut elements = Vec::new();

    // Axis line
    elements.push(ChartElement::Line {
        x1: range.0,
        y1: y_position,
        x2: range.1,
        y2: y_position,
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    for val in &ticks {
        let x = scale.map(*val);
        let label = if *val == val.floor() && val.abs() < 1e15 {
            format!("{}", *val as i64)
        } else {
            format!("{:.1}", val)
        };

        elements.push(ChartElement::Line {
            x1: x,
            y1: y_position,
            x2: x,
            y2: y_position + 5.0,
            stroke: "#999".to_string(),
            stroke_width: Some(1.0),
            stroke_dasharray: None,
            class: "tick".to_string(),
        });

        elements.push(ChartElement::Text {
            x,
            y: y_position + 18.0,
            content: label,
            anchor: TextAnchor::Middle,
            dominant_baseline: None,
            transform: None,
            font_size: Some("11px".to_string()),
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
        });
    }

    elements
}

fn generate_y_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    x_position: f64,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(5);
    let mut elements = Vec::new();

    // Axis line
    elements.push(ChartElement::Line {
        x1: x_position,
        y1: range.0.min(range.1),
        x2: x_position,
        y2: range.0.max(range.1),
        stroke: "#ccc".to_string(),
        stroke_width: Some(1.0),
        stroke_dasharray: None,
        class: "axis-line".to_string(),
    });

    for val in &ticks {
        let y = scale.map(*val);
        let label = if *val == val.floor() && val.abs() < 1e15 {
            format!("{}", *val as i64)
        } else {
            format!("{:.1}", val)
        };

        elements.push(ChartElement::Line {
            x1: x_position - 5.0,
            y1: y,
            x2: x_position,
            y2: y,
            stroke: "#999".to_string(),
            stroke_width: Some(1.0),
            stroke_dasharray: None,
            class: "tick".to_string(),
        });

        elements.push(ChartElement::Text {
            x: x_position - 8.0,
            y,
            content: label,
            anchor: TextAnchor::End,
            dominant_baseline: Some("middle".to_string()),
            transform: None,
            font_size: Some("11px".to_string()),
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
        });
    }

    elements
}

fn generate_legend(
    series_names: &[String],
    colors: &[String],
    chart_width: f64,
    y_position: f64,
) -> Vec<ChartElement> {
    let mut elements = Vec::new();
    let mut x_offset = chart_width / 2.0 - (series_names.len() as f64 * 60.0) / 2.0;

    for (i, name) in series_names.iter().enumerate() {
        let color = colors
            .get(i)
            .cloned()
            .unwrap_or_else(|| "#999".to_string());

        elements.push(ChartElement::Rect {
            x: x_offset,
            y: y_position,
            width: 12.0,
            height: 12.0,
            fill: color,
            stroke: None,
            class: "legend-symbol".to_string(),
            data: None,
        });

        elements.push(ChartElement::Text {
            x: x_offset + 16.0,
            y: y_position + 10.0,
            content: name.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_size: Some("11px".to_string()),
            fill: Some("#333".to_string()),
            class: "legend-label".to_string(),
        });

        x_offset += 80.0;
    }

    elements
}

fn offset_element(element: ChartElement, dx: f64, dy: f64) -> ChartElement {
    if dx == 0.0 && dy == 0.0 {
        return element;
    }
    ChartElement::Group {
        class: String::new(),
        transform: Some(Transform::Translate(dx, dy)),
        children: vec![element],
    }
}
