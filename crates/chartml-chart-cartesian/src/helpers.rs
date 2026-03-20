use chartml_core::element::{ChartElement, TextAnchor};
use chartml_core::error::ChartError;
use chartml_core::plugin::ChartConfig;
use chartml_core::scales::ScaleBand;
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

/// Generate x-axis elements (tick marks and labels) for category data.
pub fn generate_x_axis(
    labels: &[String],
    range: (f64, f64),
    y_position: f64,
) -> Vec<ChartElement> {
    let band = ScaleBand::new(labels.to_vec(), range);
    let bandwidth = band.bandwidth();
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

    for label in labels {
        let x = match band.map(label) {
            Some(x) => x + bandwidth / 2.0,
            None => continue,
        };

        // Tick mark
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

        // Label
        elements.push(ChartElement::Text {
            x,
            y: y_position + 18.0,
            content: label.clone(),
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
        });
    }

    elements
}
