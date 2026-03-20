use chartml_core::plugin::{ChartRenderer, ChartConfig};
use chartml_core::data::{Row, get_f64, get_string, extent, unique_values};
use chartml_core::element::*;
use chartml_core::error::ChartError;
use chartml_core::scales::{ScaleLinear, ScaleSqrt};
use chartml_core::spec::{VisualizeSpec, FieldRef, MarkEncoding};
use chartml_core::layout::margins::Margins;

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
    fn render(&self, data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError> {
        let x_field = get_field_name(&config.visualize.columns)?;
        let y_field = get_field_name(&config.visualize.rows)?;
        let color_field = get_color_field(config);
        let size_field = get_size_field(config);

        let width = config.width;
        let height = config.height;

        let margins = Margins::default();
        let inner_width = margins.inner_width(width);
        let inner_height = margins.inner_height(height);

        // Compute domains
        let x_extent = extent(data, &x_field)
            .ok_or_else(|| ChartError::DataError(format!("No numeric data for field '{}'", x_field)))?;
        let y_extent = extent(data, &y_field)
            .ok_or_else(|| ChartError::DataError(format!("No numeric data for field '{}'", y_field)))?;

        // Default: include 0 in both axes (matching JS D3 behavior)
        let x_domain = (x_extent.0.min(0.0), x_extent.1);
        let y_domain = (y_extent.0.min(0.0), y_extent.1);
        let x_scale = ScaleLinear::new(x_domain, (margins.left, margins.left + inner_width)).nice(5);
        let y_scale = ScaleLinear::new(y_domain, (margins.top + inner_height, margins.top)).nice(5); // inverted for SVG

        // Size scale (if marks.size present)
        let size_scale = size_field.as_ref().and_then(|f| {
            extent(data, f).map(|ext| ScaleSqrt::new(ext, (3.0, 20.0))) // radius 3-20px
        });

        // Color mapping
        let color_categories: Vec<String> = if let Some(ref cf) = color_field {
            unique_values(data, cf)
        } else {
            vec![]
        };

        // Generate scatter points
        let mut point_elements = Vec::new();
        for row in data {
            let x_val = get_f64(row, &x_field);
            let y_val = get_f64(row, &y_field);

            if let (Some(x), Some(y)) = (x_val, y_val) {
                let cx = x_scale.map(x);
                let cy = y_scale.map(y);

                let r = match (&size_field, &size_scale) {
                    (Some(sf), Some(ss)) => {
                        get_f64(row, sf).map(|v| ss.map(v)).unwrap_or(5.0)
                    }
                    _ => 5.0,
                };

                let color_idx = if let Some(ref cf) = color_field {
                    get_string(row, cf)
                        .and_then(|v| color_categories.iter().position(|c| c == &v))
                        .unwrap_or(0)
                } else {
                    0
                };
                let fill = config.colors.get(color_idx % config.colors.len())
                    .cloned()
                    .unwrap_or_else(|| "#2E7D9A".to_string());

                let label = get_string(row, &x_field).unwrap_or_default();
                let value = format!("{}", y);
                let data = ElementData::new(label, value);

                point_elements.push(ChartElement::Circle {
                    cx,
                    cy,
                    r,
                    fill,
                    stroke: Some("#fff".to_string()),
                    class: "chartml-scatter-point".to_string(),
                    data: Some(data),
                });
            }
        }

        // Build SVG
        let mut children = Vec::new();

        // Grid lines + axes
        let x_ticks = x_scale.ticks(((inner_width / 50.0).floor() as usize).clamp(4, 10));
        let y_ticks = y_scale.ticks(((inner_height / 50.0).floor() as usize).clamp(4, 10));
        let mut axis_elements = Vec::new();

        // Horizontal grid lines + y-axis ticks
        for &val in &y_ticks {
            let y = y_scale.map(val);
            // Grid line
            axis_elements.push(ChartElement::Line {
                x1: margins.left, y1: y, x2: margins.left + inner_width, y2: y,
                stroke: "#e0e0e0".to_string(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "grid-line".to_string(),
            });
            // Tick
            axis_elements.push(ChartElement::Line {
                x1: margins.left - 5.0, y1: y, x2: margins.left, y2: y,
                stroke: "#999".to_string(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "tick".to_string(),
            });
            // Label
            let label = if val == val.floor() && val.abs() < 1e15 { format!("{}", val as i64) } else { format!("{:.1}", val) };
            axis_elements.push(ChartElement::Text {
                x: margins.left - 8.0, y,
                content: label, anchor: TextAnchor::End,
                dominant_baseline: Some("middle".to_string()),
                transform: None, font_size: Some("11px".to_string()),
                fill: Some("#666".to_string()), class: "tick-label".to_string(), data: None,
            });
        }

        // Vertical grid lines + x-axis ticks
        let x_axis_y = margins.top + inner_height;
        for &val in &x_ticks {
            let x = x_scale.map(val);
            // Grid line
            axis_elements.push(ChartElement::Line {
                x1: x, y1: margins.top, x2: x, y2: x_axis_y,
                stroke: "#e0e0e0".to_string(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "grid-line".to_string(),
            });
            // Tick
            axis_elements.push(ChartElement::Line {
                x1: x, y1: x_axis_y, x2: x, y2: x_axis_y + 5.0,
                stroke: "#999".to_string(), stroke_width: Some(1.0),
                stroke_dasharray: None, class: "tick".to_string(),
            });
            // Label
            let label = if val == val.floor() && val.abs() < 1e15 { format!("{}", val as i64) } else { format!("{:.1}", val) };
            axis_elements.push(ChartElement::Text {
                x, y: x_axis_y + 18.0,
                content: label, anchor: TextAnchor::Middle,
                dominant_baseline: None, transform: None,
                font_size: Some("11px".to_string()), fill: Some("#666".to_string()),
                class: "tick-label".to_string(), data: None,
            });
        }

        // Axis lines
        axis_elements.push(ChartElement::Line {
            x1: margins.left, y1: margins.top, x2: margins.left, y2: x_axis_y,
            stroke: "#ccc".to_string(), stroke_width: Some(1.0),
            stroke_dasharray: None, class: "axis-line".to_string(),
        });
        axis_elements.push(ChartElement::Line {
            x1: margins.left, y1: x_axis_y, x2: margins.left + inner_width, y2: x_axis_y,
            stroke: "#ccc".to_string(), stroke_width: Some(1.0),
            stroke_dasharray: None, class: "axis-line".to_string(),
        });

        children.push(ChartElement::Group {
            class: "axes".to_string(),
            transform: None,
            children: axis_elements,
        });

        // Title
        if let Some(title) = &config.title {
            children.push(ChartElement::Text {
                x: width / 2.0,
                y: 20.0,
                content: title.clone(),
                anchor: TextAnchor::Middle,
                dominant_baseline: None,
                transform: None,
                font_size: Some("16px".to_string()),
                fill: Some("#333".to_string()),
                class: "chartml-title".to_string(),
                data: None,
            });
        }

        // Points group
        children.push(ChartElement::Group {
            class: "chartml-scatter-points".to_string(),
            transform: None,
            children: point_elements,
        });

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use chartml_core::spec::{VisualizeSpec, MarksSpec, MarkEncoding};

    fn make_row(pairs: &[(&str, serde_json::Value)]) -> Row {
        let mut map = HashMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v.clone());
        }
        map
    }

    fn make_scatter_data() -> Vec<Row> {
        vec![
            make_row(&[("price", serde_json::json!(10.0)), ("units", serde_json::json!(100.0)), ("category", serde_json::json!("A"))]),
            make_row(&[("price", serde_json::json!(20.0)), ("units", serde_json::json!(200.0)), ("category", serde_json::json!("B"))]),
            make_row(&[("price", serde_json::json!(30.0)), ("units", serde_json::json!(150.0)), ("category", serde_json::json!("A"))]),
            make_row(&[("price", serde_json::json!(40.0)), ("units", serde_json::json!(300.0)), ("category", serde_json::json!("B"))]),
        ]
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
        }
    }

    fn make_bubble_data() -> Vec<Row> {
        vec![
            make_row(&[("x", serde_json::json!(5.0)), ("y", serde_json::json!(10.0)), ("size", serde_json::json!(100.0))]),
            make_row(&[("x", serde_json::json!(15.0)), ("y", serde_json::json!(20.0)), ("size", serde_json::json!(400.0))]),
            make_row(&[("x", serde_json::json!(25.0)), ("y", serde_json::json!(15.0)), ("size", serde_json::json!(200.0))]),
        ]
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
        }
    }

    #[test]
    fn scatter_chart_renders() {
        let renderer = ScatterRenderer::new();
        let result = renderer.render(&make_scatter_data(), &make_scatter_config());
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let element = result.unwrap();
        let circle_count = count_elements(&element, &|e| matches!(e, ChartElement::Circle { .. }));
        assert_eq!(circle_count, 4); // 4 data points
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
    fn scatter_empty_data_errors() {
        let renderer = ScatterRenderer::new();
        let result = renderer.render(&[], &make_scatter_config());
        assert!(result.is_err());
    }
}
