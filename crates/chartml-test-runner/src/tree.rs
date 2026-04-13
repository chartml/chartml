//! ChartElement → JSON serialization for the evaluator agent.
//!
//! Produces a structured JSON tree that the evaluator can inspect
//! for structural, geometric, and proportional assertions.

use chartml_core::element::{ChartElement, ElementData, TextAnchor, Transform};
use serde_json::{json, Value};

/// Convert a ChartElement tree to a JSON Value.
pub fn element_to_json(element: &ChartElement) -> Value {
    match element {
        ChartElement::Svg { viewbox, width, height, class, children } => {
            json!({
                "type": "svg",
                "viewBox": {
                    "x": viewbox.x,
                    "y": viewbox.y,
                    "width": viewbox.width,
                    "height": viewbox.height
                },
                "width": width,
                "height": height,
                "class": class,
                "children": children.iter().map(element_to_json).collect::<Vec<_>>()
            })
        }

        ChartElement::Group { class, transform, children } => {
            json!({
                "type": "group",
                "class": class,
                "transform": transform.as_ref().map(transform_to_json),
                "children": children.iter().map(element_to_json).collect::<Vec<_>>()
            })
        }

        ChartElement::Rect { x, y, width, height, fill, stroke, class, data } => {
            json!({
                "type": "rect",
                "x": x,
                "y": y,
                "width": width,
                "height": height,
                "fill": fill,
                "stroke": stroke,
                "class": class,
                "data": data.as_ref().map(data_to_json)
            })
        }

        ChartElement::Path { d, fill, stroke, stroke_width, stroke_dasharray, opacity, class, data } => {
            json!({
                "type": "path",
                "d": d,
                "fill": fill,
                "stroke": stroke,
                "strokeWidth": stroke_width,
                "strokeDasharray": stroke_dasharray,
                "opacity": opacity,
                "class": class,
                "data": data.as_ref().map(data_to_json)
            })
        }

        ChartElement::Circle { cx, cy, r, fill, stroke, class, data } => {
            json!({
                "type": "circle",
                "cx": cx,
                "cy": cy,
                "r": r,
                "fill": fill,
                "stroke": stroke,
                "class": class,
                "data": data.as_ref().map(data_to_json)
            })
        }

        ChartElement::Line { x1, y1, x2, y2, stroke, stroke_width, stroke_dasharray, class } => {
            json!({
                "type": "line",
                "x1": x1,
                "y1": y1,
                "x2": x2,
                "y2": y2,
                "stroke": stroke,
                "strokeWidth": stroke_width,
                "strokeDasharray": stroke_dasharray,
                "class": class
            })
        }

        ChartElement::Text { x, y, content, anchor, dominant_baseline, transform, font_size, font_weight, fill, class, data, .. } => {
            json!({
                "type": "text",
                "x": x,
                "y": y,
                "content": content,
                "anchor": match anchor {
                    TextAnchor::Start => "start",
                    TextAnchor::Middle => "middle",
                    TextAnchor::End => "end",
                },
                "dominantBaseline": dominant_baseline,
                "transform": transform.as_ref().map(transform_to_json),
                "fontSize": font_size,
                "fontWeight": font_weight,
                "fill": fill,
                "class": class,
                "data": data.as_ref().map(data_to_json)
            })
        }

        ChartElement::Div { class, style, children } => {
            json!({
                "type": "div",
                "class": class,
                "style": style,
                "children": children.iter().map(element_to_json).collect::<Vec<_>>()
            })
        }

        ChartElement::Span { class, style, content } => {
            json!({
                "type": "span",
                "class": class,
                "style": style,
                "content": content
            })
        }
    }
}

fn transform_to_json(t: &Transform) -> Value {
    match t {
        Transform::Translate(x, y) => json!({"translate": [x, y]}),
        Transform::Rotate(angle, cx, cy) => json!({"rotate": [angle, cx, cy]}),
        Transform::Multiple(transforms) => {
            json!({"multiple": transforms.iter().map(transform_to_json).collect::<Vec<_>>()})
        }
    }
}

fn data_to_json(d: &ElementData) -> Value {
    json!({
        "label": d.label,
        "value": d.value,
        "series": d.series,
        "raw": d.raw
    })
}

/// Extract width/height from a root SVG element.
pub fn extract_dimensions(element: &ChartElement) -> (f64, f64) {
    match element {
        ChartElement::Svg { width, height, viewbox, .. } => {
            (width.unwrap_or(viewbox.width), height.unwrap_or(viewbox.height))
        }
        _ => (800.0, 400.0),
    }
}
