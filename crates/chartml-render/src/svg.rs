//! ChartElement tree → SVG string serialization.
//!
//! Walks the ChartElement tree recursively and emits SVG XML.
//! Follows the same pattern as `chartml-leptos/src/element.rs` but
//! produces a string instead of Leptos views.

use chartml_core::element::{ChartElement, ElementData};
use std::fmt::Write;

/// Default font family for SVG text elements.
const DEFAULT_FONT_FAMILY: &str = "Inter, Liberation Sans, Arial, sans-serif";

/// Convert a ChartElement tree to an SVG string.
///
/// The root element should be a `ChartElement::Svg`. If it's a `Div` (e.g. metric cards),
/// the output wraps it in an SVG with a `<foreignObject>`.
pub fn element_to_svg(element: &ChartElement, width: f64, height: f64) -> String {
    let mut buf = String::with_capacity(4096);

    match element {
        ChartElement::Svg { .. } => {
            write_element(&mut buf, element);
        }
        // Non-SVG root (metric cards, multi-chart grids) — wrap in SVG + foreignObject
        _ => {
            write!(
                &mut buf,
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
                width, height, width, height,
            ).unwrap();
            buf.push_str(r#"<foreignObject x="0" y="0" width="100%" height="100%">"#);
            write_element(&mut buf, element);
            buf.push_str("</foreignObject>");
            buf.push_str("</svg>");
        }
    }

    buf
}

/// Recursively write a ChartElement to the buffer as SVG/HTML.
fn write_element(buf: &mut String, element: &ChartElement) {
    match element {
        ChartElement::Svg { viewbox, width, height, class, children } => {
            write!(
                buf,
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{}""##,
                viewbox.to_svg_string(),
            ).unwrap();
            if let Some(w) = width {
                write!(buf, r#" width="{}""#, w).unwrap();
            }
            if let Some(h) = height {
                write!(buf, r#" height="{}""#, h).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            buf.push('>');
            for child in children {
                write_element(buf, child);
            }
            buf.push_str("</svg>");
        }

        ChartElement::Group { class, transform, children } => {
            buf.push_str("<g");
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            if let Some(t) = transform {
                write!(buf, r#" transform="{}""#, t.to_svg_string()).unwrap();
            }
            buf.push('>');
            for child in children {
                write_element(buf, child);
            }
            buf.push_str("</g>");
        }

        ChartElement::Rect { x, y, width, height, fill, stroke, rx, ry, class, data } => {
            // Bar animation: transform-origin depends on orientation.
            // Vertical bars: bottom-center (scaleY grows upward from baseline).
            // Horizontal bars: left-center (scaleX grows rightward from y-axis).
            // Heuristic: bars wider than tall are horizontal.
            let (origin_x, origin_y) = if width > height {
                (*x, y + height / 2.0) // left-center for horizontal
            } else {
                (x + width / 2.0, y + height) // bottom-center for vertical
            };
            write!(
                buf,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" style="transform-origin: {}px {}px;""#,
                x, y, width, height, xml_escape(fill), origin_x, origin_y,
            ).unwrap();
            if let Some(s) = stroke {
                write!(buf, r#" stroke="{}""#, xml_escape(s)).unwrap();
            }
            if let Some(r) = rx {
                write!(buf, r#" rx="{}""#, r).unwrap();
            }
            if let Some(r) = ry {
                write!(buf, r#" ry="{}""#, r).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            write_data_attrs(buf, data);
            buf.push_str("/>");
        }

        ChartElement::Path { d, fill, stroke, stroke_width, stroke_dasharray, opacity, class, data } => {
            write!(buf, r#"<path d="{}""#, xml_escape(d)).unwrap();
            match fill.as_deref() {
                Some(f) => write!(buf, r#" fill="{}""#, xml_escape(f)).unwrap(),
                None => buf.push_str(r#" fill="none""#),
            }
            match stroke.as_deref() {
                Some(s) => write!(buf, r#" stroke="{}""#, xml_escape(s)).unwrap(),
                None => buf.push_str(r#" stroke="none""#),
            }
            if let Some(sw) = stroke_width {
                write!(buf, r#" stroke-width="{}""#, sw).unwrap();
            }
            if let Some(sda) = stroke_dasharray {
                write!(buf, r#" stroke-dasharray="{}""#, xml_escape(sda)).unwrap();
            }
            if let Some(op) = opacity {
                write!(buf, r#" opacity="{}""#, op).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            write_data_attrs(buf, data);
            buf.push_str("/>");
        }

        ChartElement::Circle { cx, cy, r, fill, stroke, class, data } => {
            write!(
                buf,
                r#"<circle cx="{}" cy="{}" r="{}" fill="{}""#,
                cx, cy, r, xml_escape(fill),
            ).unwrap();
            if let Some(s) = stroke {
                write!(buf, r#" stroke="{}""#, xml_escape(s)).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            write_data_attrs(buf, data);
            buf.push_str("/>");
        }

        ChartElement::Line { x1, y1, x2, y2, stroke, stroke_width, stroke_dasharray, class } => {
            write!(
                buf,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}""#,
                x1, y1, x2, y2, xml_escape(stroke),
            ).unwrap();
            if let Some(sw) = stroke_width {
                write!(buf, r#" stroke-width="{}""#, sw).unwrap();
            }
            if let Some(sda) = stroke_dasharray {
                write!(buf, r#" stroke-dasharray="{}""#, xml_escape(sda)).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            buf.push_str("/>");
        }

        ChartElement::Text {
            x, y, content, anchor, dominant_baseline, transform,
            font_family, font_size, font_weight, letter_spacing, text_transform,
            fill, class, ..
        } => {
            // Per-element font-family overrides the hardcoded default. When
            // the Text carries no family, fall back to the legacy default
            // (matches pre-Phase-4 baseline output byte-for-byte).
            let family = font_family.as_deref().unwrap_or(DEFAULT_FONT_FAMILY);
            write!(
                buf,
                r#"<text x="{}" y="{}" text-anchor="{}" font-family="{}""#,
                x, y, anchor, xml_escape(family),
            ).unwrap();
            if let Some(db) = dominant_baseline {
                write!(buf, r#" dominant-baseline="{}""#, xml_escape(db)).unwrap();
            }
            if let Some(t) = transform {
                write!(buf, r#" transform="{}""#, t.to_svg_string()).unwrap();
            }
            if let Some(fs) = font_size {
                write!(buf, r#" font-size="{}""#, xml_escape(fs)).unwrap();
            }
            if let Some(fw) = font_weight {
                write!(buf, r#" font-weight="{}""#, xml_escape(fw)).unwrap();
            }
            if let Some(ls) = letter_spacing {
                write!(buf, r#" letter-spacing="{}""#, xml_escape(ls)).unwrap();
            }
            if let Some(tt) = text_transform {
                write!(buf, r#" text-transform="{}""#, xml_escape(tt)).unwrap();
            }
            if let Some(f) = fill {
                write!(buf, r#" fill="{}""#, xml_escape(f)).unwrap();
            }
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            buf.push('>');
            buf.push_str(&xml_escape(content));
            buf.push_str("</text>");
        }

        ChartElement::Div { class, style, children } => {
            buf.push_str(r#"<div xmlns="http://www.w3.org/1999/xhtml""#);
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            let style_str = style_map_to_string(style);
            if !style_str.is_empty() {
                write!(buf, r#" style="{}""#, xml_escape(&style_str)).unwrap();
            }
            buf.push('>');
            for child in children {
                write_element(buf, child);
            }
            buf.push_str("</div>");
        }

        ChartElement::Span { class, style, content } => {
            buf.push_str("<span");
            if !class.is_empty() {
                write!(buf, r#" class="{}""#, xml_escape(class)).unwrap();
            }
            let style_str = style_map_to_string(style);
            if !style_str.is_empty() {
                write!(buf, r#" style="{}""#, xml_escape(&style_str)).unwrap();
            }
            buf.push('>');
            buf.push_str(&xml_escape(content));
            buf.push_str("</span>");
        }
    }
}

/// Convert a style HashMap to a CSS string (sorted for deterministic output).
fn style_map_to_string(style: &std::collections::HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = style.iter().collect();
    pairs.sort_by_key(|(k, _)| (*k).clone());
    pairs.iter()
        .map(|(k, v)| format!("{}: {}", k, v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Write data-* attributes for interactive tooltip support.
fn write_data_attrs(buf: &mut String, data: &Option<ElementData>) {
    if let Some(d) = data {
        if !d.label.is_empty() {
            write!(buf, r#" data-label="{}""#, xml_escape(&d.label)).unwrap();
        }
        if !d.value.is_empty() {
            write!(buf, r#" data-value="{}""#, xml_escape(&d.value)).unwrap();
        }
        if let Some(ref s) = d.series {
            write!(buf, r#" data-series="{}""#, xml_escape(s)).unwrap();
        }
    }
}

/// XML-escape a string for use in attributes and text content.
fn xml_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chartml_core::element::{ViewBox, Transform, TextAnchor, ElementData};
    use std::collections::HashMap;

    #[test]
    fn simple_svg_with_rect() {
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "chart".to_string(),
            children: vec![
                ChartElement::Rect {
                    x: 10.0, y: 20.0, width: 50.0, height: 100.0,
                    fill: "#ff0000".to_string(), stroke: None,
                    rx: None, ry: None,
                    class: "bar".to_string(), data: None,
                },
            ],
        };

        let svg = element_to_svg(&element, 800.0, 400.0);
        assert!(svg.contains(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
        assert!(svg.contains(r#"viewBox="0 0 800 400""#));
        assert!(svg.contains(r##"fill="#ff0000""##));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn group_with_transform() {
        let element = ChartElement::Group {
            class: "bars".to_string(),
            transform: Some(Transform::Translate(10.0, 20.0)),
            children: vec![
                ChartElement::Rect {
                    x: 0.0, y: 0.0, width: 50.0, height: 100.0,
                    fill: "blue".to_string(), stroke: Some("black".to_string()),
                    rx: None, ry: None,
                    class: "".to_string(), data: None,
                },
            ],
        };

        let svg = element_to_svg(&element, 800.0, 400.0);
        // Non-SVG root gets wrapped in SVG + foreignObject... actually Group is SVG
        // but not an <svg> root. Let me check — it should get wrapped.
        assert!(svg.contains(r#"transform="translate(10,20)""#));
    }

    #[test]
    fn text_element_with_font() {
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "".to_string(),
            children: vec![
                ChartElement::Text {
                    x: 400.0, y: 20.0,
                    content: "Revenue & Costs".to_string(),
                    anchor: TextAnchor::Middle,
                    dominant_baseline: None,
                    transform: None,
                    font_family: None,
                    font_size: Some("16".to_string()),
                    font_weight: Some("bold".to_string()),
                    letter_spacing: None,
                    text_transform: None,
                    fill: Some("#333".to_string()),
                    class: "title".to_string(),
                    data: None,
                },
            ],
        };

        let svg = element_to_svg(&element, 800.0, 400.0);
        assert!(svg.contains(r#"font-family="Inter, Liberation Sans, Arial, sans-serif""#));
        assert!(svg.contains("Revenue &amp; Costs"));
        assert!(svg.contains(r#"text-anchor="middle""#));
    }

    #[test]
    fn path_element() {
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 100.0, 100.0),
            width: Some(100.0),
            height: Some(100.0),
            class: "".to_string(),
            children: vec![
                ChartElement::Path {
                    d: "M0,0 L100,100".to_string(),
                    fill: None,
                    stroke: Some("#000".to_string()),
                    stroke_width: Some(2.0),
                    stroke_dasharray: Some("5,3".to_string()),
                    opacity: Some(0.5),
                    class: "line".to_string(),
                    data: None,
                },
            ],
        };

        let svg = element_to_svg(&element, 100.0, 100.0);
        assert!(svg.contains(r#"d="M0,0 L100,100""#));
        assert!(svg.contains(r#"fill="none""#));
        assert!(svg.contains(r##"stroke="#000""##));
        assert!(svg.contains(r#"stroke-width="2""#));
        assert!(svg.contains(r#"stroke-dasharray="5,3""#));
        assert!(svg.contains(r#"opacity="0.5""#));
    }

    #[test]
    fn circle_element() {
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 100.0, 100.0),
            width: Some(100.0),
            height: Some(100.0),
            class: "".to_string(),
            children: vec![
                ChartElement::Circle {
                    cx: 50.0, cy: 50.0, r: 5.0,
                    fill: "red".to_string(), stroke: None,
                    class: "dot".to_string(), data: None,
                },
            ],
        };

        let svg = element_to_svg(&element, 100.0, 100.0);
        assert!(svg.contains(r#"<circle cx="50" cy="50" r="5" fill="red""#));
    }

    #[test]
    fn line_element() {
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 100.0, 100.0),
            width: Some(100.0),
            height: Some(100.0),
            class: "".to_string(),
            children: vec![
                ChartElement::Line {
                    x1: 0.0, y1: 0.0, x2: 100.0, y2: 100.0,
                    stroke: "red".to_string(),
                    stroke_width: Some(1.0),
                    stroke_dasharray: None,
                    class: "grid".to_string(),
                },
            ],
        };

        let svg = element_to_svg(&element, 100.0, 100.0);
        assert!(svg.contains(r##"stroke="red""##));
    }

    #[test]
    fn div_span_metric_card() {
        let element = ChartElement::Div {
            class: "metric".to_string(),
            style: HashMap::from([
                ("font-size".to_string(), "36px".to_string()),
                ("color".to_string(), "#333".to_string()),
            ]),
            children: vec![
                ChartElement::Span {
                    class: "value".to_string(),
                    style: HashMap::new(),
                    content: "$1,234".to_string(),
                },
            ],
        };

        let svg = element_to_svg(&element, 200.0, 100.0);
        // Non-SVG root should be wrapped in SVG + foreignObject
        assert!(svg.contains(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
        assert!(svg.contains("<foreignObject"));
        assert!(svg.contains(r#"<div xmlns="http://www.w3.org/1999/xhtml""#));
        assert!(svg.contains("$1,234"));
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<script>"), "&lt;script&gt;");
        assert_eq!(xml_escape(r#"say "hi""#), "say &quot;hi&quot;");
    }

    #[test]
    fn interactive_data_ignored_for_svg() {
        // ElementData is for interactive tooltips — ignored in static SVG
        let element = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 100.0, 100.0),
            width: Some(100.0),
            height: Some(100.0),
            class: "".to_string(),
            children: vec![
                ChartElement::Rect {
                    x: 0.0, y: 0.0, width: 50.0, height: 50.0,
                    fill: "blue".to_string(), stroke: None,
                    rx: None, ry: None,
                    class: "".to_string(),
                    data: Some(ElementData::new("Jan", "1234")),
                },
            ],
        };

        let svg = element_to_svg(&element, 100.0, 100.0);
        // Should render the rect with data attributes for tooltip support
        assert!(svg.contains(r#"<rect x="0" y="0""#));
        assert!(svg.contains(r#"data-label="Jan""#));
        assert!(svg.contains(r#"data-value="1234""#));
    }
}
