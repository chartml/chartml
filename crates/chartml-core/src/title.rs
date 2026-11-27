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
use crate::layout::labels::{truncate_label_with_metrics, TextMetrics, TEXT_ASCENT_FRACTION};
use crate::theme::Theme;

/// Space above the title text, in px.
const TOP_PAD: f64 = 8.0;
/// Space between the title text and the chart, in px.
const BOTTOM_PAD: f64 = 8.0;
/// Class of the title text. The SVG title's font attributes are presentation
/// attributes, so a host's `--chartml-title-*` rule on this class wins over
/// them; the HTML heading of a table carries inline style and does not.
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
    /// The size as CSS (`14px`), formatted from the theme's own `f32`.
    font_size_css: String,
    font_family: String,
    font_weight: u16,
    fill: String,
}

impl TitleBand {
    /// Lay out `title` for a chart `width` px wide in the resolved `theme`'s
    /// title font. `None` for a title that is empty after trimming.
    ///
    /// The band is at most `max_height` tall (the title is never dropped: in
    /// a chart too short for it, the text sits in the band that fits).
    pub fn new(title: &str, width: f64, max_height: f64, theme: &Theme) -> Option<Self> {
        let full_text = title.trim();
        if full_text.is_empty() {
            return None;
        }
        let font_size = f64::from(theme.title_font_size);
        let font_size_css = format!("{}px", theme.title_font_size);
        let metrics = TextMetrics::from_theme_title(theme);
        let text = truncate_label_with_metrics(full_text, width, &metrics);
        Some(Self {
            height: (TOP_PAD + font_size + BOTTOM_PAD).min(max_height),
            text,
            full_text: full_text.to_string(),
            font_size,
            font_size_css,
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
            y: ((TOP_PAD + self.font_size * TEXT_ASCENT_FRACTION).min(self.height) * 10.0).round() / 10.0,
            content: self.text.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_family: Some(self.font_family.clone()),
            font_size: Some(self.font_size_css.clone()),
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
            ("font-size", self.font_size_css.clone()),
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
    /// than `height`, so the result is as tall as requested.
    pub fn wrap(&self, chart: ChartElement, height: f64) -> ChartElement {
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
                    // Fill the box the host gives the table (its own root
                    // fills it the same way), not a pixel size that could
                    // disagree with it.
                    ("width", "100%".to_string()),
                    ("height", "100%".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_written_as_the_theme_has_it() {
        let theme = Theme { title_font_size: 13.3, ..Theme::default() };
        let band = TitleBand::new("Revenue", 400.0, 200.0, &theme).expect("band");
        let ChartElement::Group { children, .. } = band.svg_element() else { panic!("group") };
        let ChartElement::Text { font_size, .. } = &children[0] else { panic!("text") };
        assert_eq!(font_size.as_deref(), Some("13.3px"));
    }

    #[test]
    fn a_title_in_a_chart_too_short_for_it_still_draws_in_a_band_that_fits() {
        let band = TitleBand::new("Revenue", 400.0, 10.0, &Theme::default()).expect("band");
        assert_eq!(band.height, 10.0);
        let ChartElement::Group { children, .. } = band.svg_element() else { panic!("group") };
        let ChartElement::Text { y, .. } = &children[0] else { panic!("text") };
        assert!(*y <= 10.0, "baseline {y} inside the band");
    }

    #[test]
    fn a_blank_title_draws_nothing() {
        assert!(TitleBand::new("   ", 400.0, 200.0, &Theme::default()).is_none());
    }
}
