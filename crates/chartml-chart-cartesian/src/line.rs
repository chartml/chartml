use chartml_core::data::{get_f64, get_string, unique_values, group_by, Row};
use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform, ViewBox};
use chartml_core::error::ChartError;
use chartml_core::layout::margins::{calculate_margins, MarginConfig};
use chartml_core::plugin::ChartConfig;
use chartml_core::layout::adaptive_tick_count;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::shapes::LineGenerator;

use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig};

use crate::helpers::{generate_x_axis, generate_y_axis_numeric, generate_legend, get_color_field, get_field_name, get_x_format, get_y_format, offset_element};

pub fn render_line(data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
    let category_field = get_field_name(&config.visualize.columns)?;
    let value_field = get_field_name(&config.visualize.rows)?;

    let categories = unique_values(data, &category_field);
    if categories.is_empty() {
        return Err(ChartError::DataError("No category values found".into()));
    }

    let color_field = get_color_field(config);

    // Step 1: Compute label strategy for margin estimation
    let estimated_width = config.width - 80.0;
    let x_format = get_x_format(config);
    let x_strategy = LabelStrategy::determine(&categories, estimated_width, &LabelStrategyConfig::default());
    let x_extra_margin = match &x_strategy {
        LabelStrategy::Rotated { margin, .. } => *margin,
        _ => 0.0,
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

    // Find value extent across all data
    let values: Vec<f64> = data
        .iter()
        .filter_map(|row| get_f64(row, &value_field))
        .collect();
    let value_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let value_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let (domain_min, domain_max) = if value_min >= 0.0 {
        (0.0, if value_max <= 0.0 { 1.0 } else { value_max })
    } else {
        (value_min, if value_max <= value_min { value_min + 1.0 } else { value_max })
    };

    let band = ScaleBand::new(categories.clone(), (0.0, inner_width));
    let linear = ScaleLinear::new((domain_min, domain_max), (inner_height, 0.0));

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

    // Axes — read format string from spec
    let y_fmt = get_y_format(config);
    let y_fmt_ref = y_fmt.as_deref();
    let x_axis_result = generate_x_axis(&categories, (0.0, inner_width), margins.top + inner_height, inner_width, x_format.as_deref());
    let y_axis_elements = generate_y_axis_numeric(
        (domain_min, domain_max),
        (inner_height, 0.0),
        margins.left,
        y_fmt_ref,
        adaptive_tick_count(inner_height),
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

    // Line paths
    let line_gen = LineGenerator::new();
    let bandwidth = band.bandwidth();
    let mut line_elements = Vec::new();

    if let Some(ref color_f) = color_field {
        let series_names = unique_values(data, color_f);
        let groups = group_by(data, color_f);

        for (series_idx, series_name) in series_names.iter().enumerate() {
            let series_rows = match groups.get(series_name) {
                Some(rows) => rows,
                None => continue,
            };

            let points: Vec<(f64, f64)> = categories
                .iter()
                .filter_map(|cat| {
                    let row = series_rows.iter().find(|r| {
                        get_string(r, &category_field).as_deref() == Some(cat.as_str())
                    })?;
                    let x = band.map(cat)? + bandwidth / 2.0;
                    let y = linear.map(get_f64(row, &value_field)?);
                    Some((x, y))
                })
                .collect();

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
                stroke: Some(color),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                class: "line".to_string(),
                data: Some(ElementData::new(series_name, "").with_series(series_name)),
            });
        }

        // Legend
        let legend_elements =
            generate_legend(&series_names, &config.colors, config.width, config.height - 10.0);
        children.push(ChartElement::Group {
            class: "legend".to_string(),
            transform: None,
            children: legend_elements,
        });
    } else {
        // Single series
        let points: Vec<(f64, f64)> = categories
            .iter()
            .filter_map(|cat| {
                let row = data.iter().find(|r| {
                    get_string(r, &category_field).as_deref() == Some(cat.as_str())
                })?;
                let x = band.map(cat)? + bandwidth / 2.0;
                let y = linear.map(get_f64(row, &value_field)?);
                Some((x, y))
            })
            .collect();

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
                stroke: Some(color),
                stroke_width: Some(2.0),
                stroke_dasharray: None,
                class: "line".to_string(),
                data: None,
            });
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

