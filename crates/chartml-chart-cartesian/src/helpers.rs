use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform};
use chartml_core::error::ChartError;
use chartml_core::format::NumberFormatter;
use chartml_core::format::{detect_date_format, reformat_date_label};
use chartml_core::layout::labels::{LabelStrategy, LabelStrategyConfig, truncate_label};
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::{ScaleBand, ScaleLinear};
use chartml_core::spec::{FieldRef, FieldRefItem, MarkEncoding};

/// Extract the field name from a FieldRef (Simple, Detailed, or Multiple).
pub fn get_field_name(field_ref: &Option<FieldRef>) -> Result<String, ChartError> {
    match field_ref {
        Some(FieldRef::Simple(name)) => Ok(name.clone()),
        Some(FieldRef::Detailed(spec)) => Ok(spec.field.clone()),
        Some(FieldRef::Multiple(items)) => match items.first() {
            Some(FieldRefItem::Simple(name)) => Ok(name.clone()),
            Some(FieldRefItem::Detailed(spec)) => Ok(spec.field.clone()),
            None => Err(ChartError::MissingField("rows/columns field".into())),
        },
        None => Err(ChartError::MissingField("rows/columns field".into())),
    }
}

/// Extract color/series field from marks.
pub fn get_color_field(config: &ChartConfig) -> Option<String> {
    config
        .visualize
        .marks
        .as_ref()?
        .color
        .as_ref()
        .map(|enc| match enc {
            MarkEncoding::Simple(name) => name.clone(),
            MarkEncoding::Detailed(spec) => spec.field.clone(),
        })
}

/// Extract the y-axis (rows/left) format string from the spec.
pub fn get_y_format(config: &ChartConfig) -> Option<String> {
    config.visualize.axes.as_ref().and_then(|axes| {
        axes.left.as_ref().or(axes.x.as_ref()).and_then(|a| a.format.clone())
    })
}

/// Extract the x-axis (columns) format string from the spec.
pub fn get_x_format(config: &ChartConfig) -> Option<String> {
    config.visualize.axes.as_ref().and_then(|axes| {
        axes.x.as_ref().and_then(|a| a.format.clone())
    })
}

/// Format a numeric value using a format string, or a sensible default.
pub fn format_value(value: f64, format_str: Option<&str>) -> String {
    match format_str {
        Some(fmt) => NumberFormatter::new(fmt).format(value),
        None => default_format_value(value),
    }
}

/// Default numeric formatting: integers without decimals, floats with 1 decimal.
fn default_format_value(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        // Use comma separator for large integers
        let abs = value.abs() as u64;
        let formatted = insert_commas(abs);
        if value < 0.0 {
            format!("-{}", formatted)
        } else {
            formatted
        }
    } else {
        format!("{:.1}", value)
    }
}

fn insert_commas(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    if len <= 3 {
        return s;
    }
    let mut result = String::with_capacity(len + len / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Result of x-axis generation, including the computed label strategy.
pub struct XAxisResult {
    pub elements: Vec<ChartElement>,
    /// Additional bottom margin needed (e.g., from label rotation).
    pub extra_bottom_margin: f64,
}

/// Generate x-axis elements with smart label handling.
/// Applies LabelStrategy (horizontal/rotated/truncated/sampled) based on available space.
/// Auto-detects date labels and reformats them if no explicit format is provided.
pub fn generate_x_axis(
    labels: &[String],
    range: (f64, f64),
    y_position: f64,
    available_width: f64,
    x_format: Option<&str>,
) -> XAxisResult {
    let band = ScaleBand::new(labels.to_vec(), range);
    let bandwidth = band.bandwidth();

    // Step 1: Format labels (date detection or explicit format)
    let display_labels: Vec<String> = if let Some(fmt) = x_format {
        labels.iter().map(|l| reformat_date_label(l, fmt)).collect()
    } else if let Some(detected_fmt) = detect_date_format(labels) {
        labels.iter().map(|l| reformat_date_label(l, &detected_fmt)).collect()
    } else {
        labels.to_vec()
    };

    // Step 2: Determine label strategy
    let strategy = LabelStrategy::determine(&display_labels, available_width, &LabelStrategyConfig::default());

    let mut elements = Vec::new();
    let extra_bottom_margin;

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

    // Step 3: Apply strategy
    match &strategy {
        LabelStrategy::Horizontal => {
            extra_bottom_margin = 0.0;
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &labels[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label
                elements.push(ChartElement::Text {
                    x, y: y_position + 18.0,
                    content: label.clone(),
                    anchor: TextAnchor::Middle,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    fill: Some("#666".to_string()),
                    class: "tick-label".to_string(),
                    data: None,
                });
            }
        }

        LabelStrategy::Rotated { margin, skip_factor } => {
            extra_bottom_margin = *margin;
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &labels[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark (always shown)
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label — skip if skip_factor says so
                let should_show = match skip_factor {
                    Some(factor) => i % factor == 0,
                    None => true,
                };
                if should_show {
                    elements.push(ChartElement::Text {
                        x, y: y_position + 10.0,
                        content: label.clone(),
                        anchor: TextAnchor::End,
                        dominant_baseline: None,
                        transform: Some(Transform::Rotate(-45.0, x, y_position + 10.0)),
                        font_size: Some("11px".to_string()),
                        fill: Some("#666".to_string()),
                        class: "tick-label".to_string(),
                        data: None,
                    });
                }
            }
        }

        LabelStrategy::Truncated { max_width } => {
            extra_bottom_margin = 0.0;
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &labels[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                let truncated = truncate_label(label, *max_width);
                let is_truncated = truncated != *label;
                elements.push(ChartElement::Text {
                    x, y: y_position + 18.0,
                    content: truncated,
                    anchor: TextAnchor::Middle,
                    dominant_baseline: None,
                    transform: None,
                    font_size: Some("11px".to_string()),
                    fill: Some("#666".to_string()),
                    class: "tick-label".to_string(),
                    data: if is_truncated {
                        Some(ElementData::new(label.clone(), ""))
                    } else {
                        None
                    },
                });
            }
        }

        LabelStrategy::Sampled { indices } => {
            extra_bottom_margin = 0.0;
            for (i, label) in display_labels.iter().enumerate() {
                let orig_label = &labels[i];
                let x = match band.map(orig_label) {
                    Some(x) => x + bandwidth / 2.0,
                    None => continue,
                };
                // Tick mark for all
                elements.push(ChartElement::Line {
                    x1: x, y1: y_position, x2: x, y2: y_position + 5.0,
                    stroke: "#999".to_string(), stroke_width: Some(1.0),
                    stroke_dasharray: None, class: "tick".to_string(),
                });
                // Label only for sampled indices
                if indices.contains(&i) {
                    elements.push(ChartElement::Text {
                        x, y: y_position + 18.0,
                        content: label.clone(),
                        anchor: TextAnchor::Middle,
                        dominant_baseline: None,
                        transform: None,
                        font_size: Some("11px".to_string()),
                        fill: Some("#666".to_string()),
                        class: "tick-label".to_string(),
                        data: None,
                    });
                }
            }
        }
    }

    XAxisResult { elements, extra_bottom_margin }
}

/// Generate y-axis elements for category data (used in horizontal bar charts).
pub fn generate_y_axis(
    labels: &[String],
    range: (f64, f64),
    x_position: f64,
    _formatter: Option<&str>,
) -> Vec<ChartElement> {
    let band = ScaleBand::new(labels.to_vec(), range);
    let bandwidth = band.bandwidth();
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

    for label in labels {
        let y = match band.map(label) {
            Some(y) => y + bandwidth / 2.0,
            None => continue,
        };

        // Tick mark
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

        // Label
        elements.push(ChartElement::Text {
            x: x_position - 8.0,
            y,
            content: label.clone(),
            anchor: TextAnchor::End,
            dominant_baseline: Some("middle".to_string()),
            transform: None,
            font_size: Some("11px".to_string()),
            fill: Some("#666".to_string()),
            class: "tick-label".to_string(),
            data: None,
        });
    }

    elements
}

/// Generate y-axis elements for numeric data (used by bar, line, and area charts).
/// If `chart_width` is provided, horizontal grid lines extend across the chart.
pub fn generate_y_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    x_position: f64,
    fmt: Option<&str>,
    tick_count: usize,
    chart_width: Option<f64>,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(tick_count);
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
        let label = format_value(*val, fmt);

        // Horizontal grid line (if chart_width provided)
        if let Some(cw) = chart_width {
            elements.push(ChartElement::Line {
                x1: x_position,
                y1: y,
                x2: x_position + cw,
                y2: y,
                stroke: "#e0e0e0".to_string(),
                stroke_width: Some(1.0),
                stroke_dasharray: None,
                class: "grid-line".to_string(),
            });
        }

        // Tick mark
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
            data: None,
        });
    }

    elements
}

/// Generate x-axis elements for numeric data (used by horizontal bar charts).
pub fn generate_x_axis_numeric(
    domain: (f64, f64),
    range: (f64, f64),
    y_position: f64,
    fmt: Option<&str>,
    tick_count: usize,
) -> Vec<ChartElement> {
    let scale = ScaleLinear::new(domain, range);
    let ticks = scale.ticks(tick_count);
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
        let label = format_value(*val, fmt);

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
            data: None,
        });
    }

    elements
}

/// Generate legend elements for multi-series charts.
pub fn generate_legend(
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
            data: None,
        });

        x_offset += 80.0;
    }

    elements
}

/// Offset an element's position by wrapping it in a Group with a Translate transform.
pub fn offset_element(element: ChartElement, dx: f64, dy: f64) -> ChartElement {
    if dx == 0.0 && dy == 0.0 {
        return element;
    }
    ChartElement::Group {
        class: String::new(),
        transform: Some(Transform::Translate(dx, dy)),
        children: vec![element],
    }
}
