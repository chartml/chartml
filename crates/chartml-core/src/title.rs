//! The chart title on paths where the host draws no heading (CHA-59).
//!
//! The Leptos component draws the title as an HTML heading above the SVG and
//! asks the engine for none (`title_heading`). Everywhere else the SVG is shown
//! alone (`renderToSvg` for @chartml/core, React, markdown-it and
//! markdown-react; `render_to_png`; the test runner), so the engine draws the
//! title itself, inside the requested size: the title band takes its height
//! from the top of the chart, and the chart renders into what is left. The
//! SVG's width and height are exactly what the host asked for, with or
//! without a title.

use std::collections::HashMap;

use crate::element::{ChartElement, TextAnchor, Transform, ViewBox};
use crate::layout::labels::{truncate_label_with_metrics, TextMetrics};
use crate::theme::Theme;

/// Space above the title text, in px.
const TOP_PAD: f64 = 8.0;
/// Space between the title text and the chart, in px.
const BOTTOM_PAD: f64 = 8.0;
/// Class of the title text; hosts restyle it through the `--chartml-title-*`
/// custom properties.
pub const TITLE_CLASS: &str = "chart-title";

/// A title laid out for a given width, ready to be drawn above a chart.
#[derive(Debug, Clone)]
pub struct TitleBand {
    /// Height the band takes from the top of the chart.
    pub height: f64,
    /// The text, shortened with an ellipsis when it does not fit the width.
    pub text: String,
    /// The full title, to show as the `<title>` tooltip when `text` was shortened.
    pub full_text: String,
    font_size: f64,
    font_family: String,
    font_weight: u16,
    fill: String,
}

impl TitleBand {
    /// Lay out `title` for a chart `width` px wide in the resolved `theme`'s
    /// title font. `None` for a title that is empty after trimming.
    pub fn new(title: &str, width: f64, theme: &Theme) -> Option<Self> {
        let full_text = title.trim();
        if full_text.is_empty() {
            return None;
        }
        let font_size = f64::from(theme.title_font_size);
        let metrics = TextMetrics::from_theme_title(theme);
        let text = truncate_label_with_metrics(full_text, width, &metrics);
        Some(Self {
            height: TOP_PAD + font_size + BOTTOM_PAD,
            text,
            full_text: full_text.to_string(),
            font_size,
            font_family: theme.title_font_family.clone(),
            font_weight: theme.title_font_weight,
            fill: theme.title_color.clone().unwrap_or_else(|| theme.text_strong.clone()),
        })
    }

    /// The title as SVG: a group holding the text and, when the text was
    /// shortened, the full title as its `<title>`.
    pub fn svg_element(&self) -> ChartElement {
        let mut children = vec![ChartElement::Text {
            x: 0.0,
            y: ((TOP_PAD + self.font_size * 0.8) * 10.0).round() / 10.0,
            content: self.text.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_family: Some(self.font_family.clone()),
            font_size: Some(format!("{}px", self.font_size)),
            font_weight: Some(self.font_weight.to_string()),
            letter_spacing: None,
            text_transform: None,
            fill: Some(self.fill.clone()),
            class: TITLE_CLASS.to_string(),
            data: None,
        }];
        if self.text != self.full_text {
            children.push(ChartElement::Title { content: self.full_text.clone() });
        }
        ChartElement::Group { class: "chart-title-group".to_string(), transform: None, children }
    }

    /// The title as an HTML heading, for a chart whose root is HTML (the table).
    fn html_element(&self) -> ChartElement {
        let style: HashMap<String, String> = [
            ("flex", "none".to_string()),
            ("height", format!("{}px", self.height)),
            ("box-sizing", "border-box".to_string()),
            ("padding-top", format!("{TOP_PAD}px")),
            ("white-space", "nowrap".to_string()),
            ("overflow", "hidden".to_string()),
            ("text-overflow", "ellipsis".to_string()),
            ("font-family", self.font_family.clone()),
            ("font-size", format!("{}px", self.font_size)),
            ("font-weight", self.font_weight.to_string()),
            ("color", self.fill.clone()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        ChartElement::Div {
            class: TITLE_CLASS.to_string(),
            style,
            children: vec![ChartElement::Span {
                class: String::new(),
                style: HashMap::new(),
                content: self.full_text.clone(),
            }],
        }
    }

    /// Draw the title above `chart`, which was rendered `self.height` shorter
    /// than `height`, so the result is `width` by `height` as requested.
    pub fn wrap(&self, chart: ChartElement, width: f64, height: f64) -> ChartElement {
        match chart {
            ChartElement::Svg { viewbox, width: svg_width, class, children, .. } => {
                let body = ChartElement::Group {
                    class: "chart-body".to_string(),
                    transform: Some(Transform::Translate(0.0, self.height)),
                    children,
                };
                ChartElement::Svg {
                    viewbox: ViewBox::new(viewbox.x, viewbox.y, viewbox.width, height),
                    width: svg_width,
                    height: Some(height),
                    class,
                    children: vec![self.svg_element(), body],
                }
            }
            other => {
                let style: HashMap<String, String> = [
                    ("display", "flex".to_string()),
                    ("flex-direction", "column".to_string()),
                    ("width", format!("{width}px")),
                    ("height", format!("{height}px")),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
                let body_style: HashMap<String, String> =
                    [("flex", "1".to_string()), ("min-height", "0".to_string())]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect();
                ChartElement::Div {
                    class: "chart-titled".to_string(),
                    style,
                    children: vec![
                        self.html_element(),
                        ChartElement::Div {
                            class: "chart-body".to_string(),
                            style: body_style,
                            children: vec![other],
                        },
                    ],
                }
            }
        }
    }
}
