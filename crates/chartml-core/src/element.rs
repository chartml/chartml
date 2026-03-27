use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The output of any ChartRenderer. Framework adapters walk this tree
/// and produce framework-specific DOM/view output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChartElement {
    Svg {
        viewbox: ViewBox,
        width: Option<f64>,
        height: Option<f64>,
        class: String,
        children: Vec<ChartElement>,
    },
    Group {
        class: String,
        transform: Option<Transform>,
        children: Vec<ChartElement>,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill: String,
        stroke: Option<String>,
        class: String,
        data: Option<ElementData>,
    },
    #[serde(rename_all = "camelCase")]
    Path {
        d: String,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<f64>,
        stroke_dasharray: Option<String>,
        opacity: Option<f64>,
        class: String,
        data: Option<ElementData>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: String,
        stroke: Option<String>,
        class: String,
        data: Option<ElementData>,
    },
    #[serde(rename_all = "camelCase")]
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: String,
        stroke_width: Option<f64>,
        stroke_dasharray: Option<String>,
        class: String,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        x: f64,
        y: f64,
        content: String,
        anchor: TextAnchor,
        dominant_baseline: Option<String>,
        transform: Option<Transform>,
        font_size: Option<String>,
        font_weight: Option<String>,
        fill: Option<String>,
        class: String,
        data: Option<ElementData>,
    },
    /// Non-SVG container (e.g., metric card uses div-based layout)
    Div {
        class: String,
        style: HashMap<String, String>,
        children: Vec<ChartElement>,
    },
    /// Raw text node (for metric values, labels in div-based charts)
    Span {
        class: String,
        style: HashMap<String, String>,
        content: String,
    },
}

/// Data attached to interactive elements for tooltips.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementData {
    pub label: String,
    pub value: String,
    pub series: Option<String>,
    pub raw: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transform {
    Translate(f64, f64),
    Rotate(f64, f64, f64),
    Multiple(Vec<Transform>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: Option<f64>,
    pub height: f64,
}

impl ViewBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Format as SVG viewBox attribute string: "x y width height"
    pub fn to_svg_string(&self) -> String {
        format!("{} {} {} {}", self.x, self.y, self.width, self.height)
    }
}

impl std::fmt::Display for ViewBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {} {}", self.x, self.y, self.width, self.height)
    }
}

impl Transform {
    /// Format as SVG transform attribute string.
    pub fn to_svg_string(&self) -> String {
        match self {
            Transform::Translate(x, y) => format!("translate({},{})", x, y),
            Transform::Rotate(angle, cx, cy) => format!("rotate({},{},{})", angle, cx, cy),
            Transform::Multiple(transforms) => {
                transforms.iter().map(|t| t.to_svg_string()).collect::<Vec<_>>().join(" ")
            }
        }
    }
}

impl std::fmt::Display for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_svg_string())
    }
}

impl std::fmt::Display for TextAnchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextAnchor::Start => write!(f, "start"),
            TextAnchor::Middle => write!(f, "middle"),
            TextAnchor::End => write!(f, "end"),
        }
    }
}

impl ElementData {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            series: None,
            raw: HashMap::new(),
        }
    }

    pub fn with_series(mut self, series: impl Into<String>) -> Self {
        self.series = Some(series.into());
        self
    }
}

impl Dimensions {
    pub fn new(height: f64) -> Self {
        Self { width: None, height }
    }

    pub fn with_width(mut self, width: f64) -> Self {
        self.width = Some(width);
        self
    }
}

/// Count elements in the tree matching a predicate.
pub fn count_elements<F>(element: &ChartElement, predicate: &F) -> usize
where
    F: Fn(&ChartElement) -> bool,
{
    let mut count = if predicate(element) { 1 } else { 0 };
    match element {
        ChartElement::Svg { children, .. }
        | ChartElement::Group { children, .. }
        | ChartElement::Div { children, .. } => {
            for child in children {
                count += count_elements(child, predicate);
            }
        }
        _ => {}
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewbox_display() {
        let vb = ViewBox::new(0.0, 0.0, 800.0, 400.0);
        assert_eq!(vb.to_string(), "0 0 800 400");
    }

    #[test]
    fn transform_translate_display() {
        let t = Transform::Translate(10.0, 20.0);
        assert_eq!(t.to_string(), "translate(10,20)");
    }

    #[test]
    fn transform_rotate_display() {
        let t = Transform::Rotate(45.0, 100.0, 200.0);
        assert_eq!(t.to_string(), "rotate(45,100,200)");
    }

    #[test]
    fn transform_multiple_display() {
        let t = Transform::Multiple(vec![
            Transform::Translate(10.0, 20.0),
            Transform::Rotate(45.0, 0.0, 0.0),
        ]);
        assert_eq!(t.to_string(), "translate(10,20) rotate(45,0,0)");
    }

    #[test]
    fn text_anchor_display() {
        assert_eq!(TextAnchor::Start.to_string(), "start");
        assert_eq!(TextAnchor::Middle.to_string(), "middle");
        assert_eq!(TextAnchor::End.to_string(), "end");
    }

    #[test]
    fn element_data_builder() {
        let data = ElementData::new("Jan", "1234")
            .with_series("Revenue");
        assert_eq!(data.label, "Jan");
        assert_eq!(data.value, "1234");
        assert_eq!(data.series, Some("Revenue".to_string()));
    }

    #[test]
    fn count_rects_in_tree() {
        let tree = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "chart".to_string(),
            children: vec![
                ChartElement::Group {
                    class: "bars".to_string(),
                    transform: None,
                    children: vec![
                        ChartElement::Rect {
                            x: 0.0, y: 0.0, width: 50.0, height: 100.0,
                            fill: "red".to_string(), stroke: None,
                            class: "bar".to_string(), data: None,
                        },
                        ChartElement::Rect {
                            x: 60.0, y: 0.0, width: 50.0, height: 150.0,
                            fill: "blue".to_string(), stroke: None,
                            class: "bar".to_string(), data: None,
                        },
                    ],
                },
                ChartElement::Text {
                    x: 400.0, y: 20.0, content: "Title".to_string(),
                    anchor: TextAnchor::Middle, dominant_baseline: None,
                    transform: None, font_size: None, font_weight: None, fill: None,
                    class: "title".to_string(),
                    data: None,
                },
            ],
        };
        let rect_count = count_elements(&tree, &|e| matches!(e, ChartElement::Rect { .. }));
        assert_eq!(rect_count, 2);
    }

    #[test]
    fn dimensions_builder() {
        let dims = Dimensions::new(400.0).with_width(800.0);
        assert_eq!(dims.height, 400.0);
        assert_eq!(dims.width, Some(800.0));
    }

    #[test]
    fn serde_round_trip_chart_element_tree() {
        let tree = ChartElement::Svg {
            viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
            width: Some(800.0),
            height: Some(400.0),
            class: "chart".to_string(),
            children: vec![
                ChartElement::Group {
                    class: "bars".to_string(),
                    transform: Some(Transform::Translate(50.0, 10.0)),
                    children: vec![
                        ChartElement::Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 50.0,
                            height: 100.0,
                            fill: "red".to_string(),
                            stroke: None,
                            class: "bar".to_string(),
                            data: Some(
                                ElementData::new("Jan", "1234").with_series("Revenue"),
                            ),
                        },
                        ChartElement::Path {
                            d: "M0,0 L10,10".to_string(),
                            fill: None,
                            stroke: Some("blue".to_string()),
                            stroke_width: Some(2.0),
                            stroke_dasharray: Some("4,2".to_string()),
                            opacity: Some(0.8),
                            class: "line".to_string(),
                            data: None,
                        },
                    ],
                },
                ChartElement::Line {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 100.0,
                    y2: 100.0,
                    stroke: "black".to_string(),
                    stroke_width: Some(1.0),
                    stroke_dasharray: None,
                    class: "axis".to_string(),
                },
                ChartElement::Text {
                    x: 400.0,
                    y: 20.0,
                    content: "Title".to_string(),
                    anchor: TextAnchor::Middle,
                    dominant_baseline: Some("central".to_string()),
                    transform: Some(Transform::Rotate(45.0, 400.0, 20.0)),
                    font_size: Some("14px".to_string()),
                    font_weight: Some("bold".to_string()),
                    fill: Some("black".to_string()),
                    class: "title".to_string(),
                    data: None,
                },
                ChartElement::Circle {
                    cx: 50.0,
                    cy: 50.0,
                    r: 5.0,
                    fill: "green".to_string(),
                    stroke: None,
                    class: "dot".to_string(),
                    data: None,
                },
                ChartElement::Div {
                    class: "metric-card".to_string(),
                    style: HashMap::from([
                        ("display".to_string(), "flex".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "value".to_string(),
                        style: HashMap::from([
                            ("font-size".to_string(), "24px".to_string()),
                        ]),
                        content: "$1,234".to_string(),
                    }],
                },
            ],
        };

        let json = serde_json::to_string(&tree).expect("serialize");
        let deserialized: ChartElement =
            serde_json::from_str(&json).expect("deserialize");

        // Re-serialize to confirm structural equality
        let json2 = serde_json::to_string(&deserialized).expect("re-serialize");
        assert_eq!(json, json2);

        // Verify the tag format is correct
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("parse as Value");
        assert_eq!(value["type"], "svg");
        assert_eq!(value["children"][0]["type"], "group");
        assert_eq!(value["children"][0]["children"][1]["type"], "path");
        assert_eq!(
            value["children"][0]["children"][1]["strokeWidth"],
            serde_json::json!(2.0)
        );
        assert_eq!(
            value["children"][2]["dominantBaseline"],
            serde_json::json!("central")
        );
    }
}
