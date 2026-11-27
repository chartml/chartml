use crate::element::{ChartElement, TextAnchor, TextRole, TextStyle};
use crate::layout::labels::{measure_text, TextMetrics};

/// Legend symbol type — matches the JS renderSymbol() function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LegendMark {
    Rect,    // bar, area, pie — rounded rectangle
    Line,    // line charts — horizontal line
    Circle,  // scatter, bubble — circle
}

/// A positioned legend item.
#[derive(Debug, Clone)]
pub struct LegendItem {
    pub index: usize,
    pub label: String,
    pub color: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub row: usize,
    pub visible: bool,
}

/// Legend layout configuration.
#[derive(Debug, Clone)]
pub struct LegendConfig {
    pub symbol_size: f64,
    pub symbol_text_gap: f64,
    pub item_padding: f64,
    pub row_height: f64,
    pub max_rows: usize,
    pub max_label_chars: usize,
    pub alignment: LegendAlignment,
    /// Text shaping metrics for legend labels. Defaults to the legacy
    /// calibration, so existing callers get byte-identical layout.
    pub text_metrics: TextMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LegendAlignment {
    Left,
    Center,
    Right,
}

impl Default for LegendConfig {
    fn default() -> Self {
        Self {
            symbol_size: 12.0,
            symbol_text_gap: 6.0,
            item_padding: 12.0,
            row_height: 20.0,
            max_rows: 3,
            max_label_chars: 20,
            alignment: LegendAlignment::Center,
            text_metrics: TextMetrics::default(),
        }
    }
}

/// Result of legend layout computation.
#[derive(Debug, Clone)]
pub struct LegendLayoutResult {
    pub items: Vec<LegendItem>,
    pub total_height: f64,
    pub overflow_count: usize,
}

/// Compute legend layout for the given labels and colors.
///
/// Algorithm (matches JS legendUtils.js):
/// 1. Calculate each item width: symbol_size + symbol_text_gap + text_width + item_padding
/// 2. Fit items into rows left-to-right
/// 3. Start new row when item doesn't fit and we haven't exceeded max_rows
/// 4. Mark items beyond max_rows as not visible
/// 5. Apply alignment offset to each row
pub fn calculate_legend_layout(
    labels: &[String],
    colors: &[String],
    available_width: f64,
    config: &LegendConfig,
) -> LegendLayoutResult {
    let metrics = &config.text_metrics;
    let measure = |s: &str| measure_text(s, metrics);

    let mut items = Vec::with_capacity(labels.len());
    let mut current_x = 0.0;
    let mut current_row = 0_usize;
    let mut overflow_count = 0;

    // Track rows for alignment: (start_index, end_index, row_width)
    let mut rows: Vec<(usize, usize, f64)> = vec![(0, 0, 0.0)];

    for (i, label) in labels.iter().enumerate() {
        // First, compute full-label width to decide row placement
        let full_text_width = measure(label);
        let full_item_width = config.symbol_size + config.symbol_text_gap + full_text_width + config.item_padding;

        // Check if full item fits on current row; if not, wrap to next row
        if current_x + full_item_width > available_width && current_x > 0.0 {
            current_row += 1;
            current_x = 0.0;
            if current_row < config.max_rows {
                rows.push((i, i, 0.0));
            }
        }

        // Now truncate only if the label doesn't fit in the remaining row width
        let remaining_width = available_width - current_x;
        let non_text_width = config.symbol_size + config.symbol_text_gap + config.item_padding;
        let max_text_width = remaining_width - non_text_width;

        let display_label = if full_text_width > max_text_width {
            // Progressively truncate until it fits
            let char_count = label.chars().count();
            let mut truncated_count = char_count.min(config.max_label_chars);
            loop {
                if truncated_count == 0 {
                    break "\u{2026}".to_string();
                }
                let candidate: String = label.chars().take(truncated_count).collect();
                let candidate_with_ellipsis = format!("{}\u{2026}", candidate);
                if measure(&candidate_with_ellipsis) <= max_text_width {
                    break candidate_with_ellipsis;
                }
                truncated_count -= 1;
            }
        } else {
            label.clone()
        };

        let text_width = measure(&display_label);
        let item_width = config.symbol_size + config.symbol_text_gap + text_width + config.item_padding;

        let visible = current_row < config.max_rows;
        if !visible {
            overflow_count += 1;
        }

        let y = current_row as f64 * config.row_height;

        items.push(LegendItem {
            index: i,
            label: display_label,
            color: colors.get(i).cloned().unwrap_or_default(),
            x: current_x,
            y,
            width: item_width,
            row: current_row,
            visible,
        });

        if visible {
            if let Some(row) = rows.last_mut() {
                row.1 = i;
                row.2 = current_x + item_width;
            }
        }

        current_x += item_width;
    }

    // Apply alignment offset
    if config.alignment != LegendAlignment::Left {
        for &(start, end, row_width) in &rows {
            let offset = match config.alignment {
                LegendAlignment::Center => (available_width - row_width) / 2.0,
                LegendAlignment::Right => available_width - row_width,
                LegendAlignment::Left => 0.0,
            };
            if offset > 0.0 {
                for item in items.iter_mut() {
                    if item.index >= start && item.index <= end && item.visible {
                        item.x += offset;
                    }
                }
            }
        }
    }

    let total_rows = (current_row + 1).min(config.max_rows);
    let total_height = total_rows as f64 * config.row_height;

    LegendLayoutResult {
        items,
        total_height,
        overflow_count,
    }
}

/// Generate legend SVG elements for the given series, laid out in rows
/// across `chart_width` with the first row's top at `y_position` (the
/// default bottom placement). Renders nothing for one series or none.
pub fn generate_legend_elements(
    series_names: &[String],
    colors: &[String],
    chart_width: f64,
    y_position: f64,
    mark: LegendMark,
    theme: &crate::theme::Theme,
) -> Vec<ChartElement> {
    if series_names.len() <= 1 {
        return Vec::new();
    }

    let config = LegendConfig {
        text_metrics: TextMetrics::from_theme_legend(theme),
        ..LegendConfig::default()
    };
    let result = calculate_legend_layout(series_names, colors, chart_width, &config);
    legend_item_elements(&result, &config, 0.0, y_position, |_| mark, theme)
}

/// Symbol and label elements for every visible item of `layout`, with the
/// layout's origin at (`x0`, `y0`). `mark_for` gives each item's symbol by
/// item index.
pub fn legend_item_elements(
    layout: &LegendLayoutResult,
    config: &LegendConfig,
    x0: f64,
    y0: f64,
    mark_for: impl Fn(usize) -> LegendMark,
    theme: &crate::theme::Theme,
) -> Vec<ChartElement> {
    let mut elements = Vec::new();

    for item in &layout.items {
        if !item.visible {
            continue;
        }

        let x = x0 + item.x;
        let sym_y = y0 + item.y;

        match mark_for(item.index) {
            LegendMark::Line => {
                elements.push(ChartElement::Line {
                    x1: x,
                    y1: sym_y + config.symbol_size / 2.0,
                    x2: x + config.symbol_size,
                    y2: sym_y + config.symbol_size / 2.0,
                    stroke: item.color.clone(),
                    stroke_width: Some(2.5),
                    stroke_dasharray: None, opacity: None,
                    class: "legend-symbol legend-line".to_string(),
                });
            }
            LegendMark::Circle => {
                elements.push(ChartElement::Circle {
                    cx: x + config.symbol_size / 2.0,
                    cy: sym_y + config.symbol_size / 2.0,
                    r: config.symbol_size / 2.0 - 1.0,
                    fill: item.color.clone(),
                    stroke: None,
                    class: "legend-symbol legend-circle".to_string(),
                    data: None,
                });
            }
            LegendMark::Rect => {
                elements.push(ChartElement::Rect {
                    x,
                    y: sym_y,
                    width: config.symbol_size,
                    height: config.symbol_size,
                    fill: item.color.clone(),
                    stroke: None,
                    rx: None,
                    ry: None,
                    class: "legend-symbol".to_string(),
                    data: None,
                    animation_origin: None,
                });
            }
        }

        let ts = TextStyle::for_role(theme, TextRole::LegendLabel);
        elements.push(ChartElement::Text {
            x: x + config.symbol_size + config.symbol_text_gap,
            y: sym_y + 10.0,
            content: item.label.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_family: ts.font_family,
            font_size: ts.font_size,
            font_weight: ts.font_weight,
            letter_spacing: ts.letter_spacing,
            text_transform: ts.text_transform,
            fill: Some(theme.text_secondary.clone()),
            class: "legend-label".to_string(),
            data: None,
        });
    }

    elements
}

// =============================================================================
// Legend placement: `style.legend.position` / `style.legend.orientation`.
// =============================================================================

/// Where a chart's legend goes (`style.legend.position`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendPosition {
    Top,
    Right,
    Bottom,
    Left,
    /// No legend, and no room kept for one.
    None,
}

/// How legend items are arranged (`style.legend.orientation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendOrientation {
    /// Items side by side, wrapping onto further rows.
    Horizontal,
    /// One item per row, in a single column.
    Vertical,
}

/// A chart's legend position and orientation.
///
/// The default is a horizontal legend at the bottom. When a style sets a
/// position but no orientation, a legend at the left or right is vertical and
/// one at the top or bottom is horizontal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegendPlacement {
    pub position: LegendPosition,
    pub orientation: LegendOrientation,
}

impl Default for LegendPlacement {
    fn default() -> Self {
        Self { position: LegendPosition::Bottom, orientation: LegendOrientation::Horizontal }
    }
}

/// Space between a legend and the chart edge, and between a top or side
/// legend and the plot.
pub const LEGEND_GAP: f64 = 8.0;

/// The widest a left or right legend may be, as a fraction of the chart
/// width. Longer labels are truncated with an ellipsis to fit.
pub const SIDE_LEGEND_MAX_WIDTH_FRACTION: f64 = 0.3;

impl LegendPlacement {
    /// Read a placement from a style's `legend`. An unknown position or
    /// orientation is an error message naming the field, the accepted values
    /// and the value given.
    pub fn from_spec(spec: Option<&crate::spec::LegendSpec>) -> Result<Self, String> {
        let Some(spec) = spec else {
            return Ok(Self::default());
        };
        let position = match spec.position.as_deref() {
            None | Some("bottom") => LegendPosition::Bottom,
            Some("top") => LegendPosition::Top,
            Some("right") => LegendPosition::Right,
            Some("left") => LegendPosition::Left,
            Some("none") => LegendPosition::None,
            Some(other) => {
                return Err(format!(
                    "position must be one of top, right, bottom, left or none; got '{other}'"
                ))
            }
        };
        let orientation = match spec.orientation.as_deref() {
            None => match position {
                LegendPosition::Left | LegendPosition::Right => LegendOrientation::Vertical,
                _ => LegendOrientation::Horizontal,
            },
            Some("horizontal") => LegendOrientation::Horizontal,
            Some("vertical") => LegendOrientation::Vertical,
            Some(other) => {
                return Err(format!("orientation must be horizontal or vertical; got '{other}'"))
            }
        };
        Ok(Self { position, orientation })
    }

    /// The placement a renderer draws with, from its chart's resolved
    /// `visualize.style.legend`.
    pub fn from_style(
        style: Option<&crate::spec::ChartStyleSpec>,
    ) -> Result<Self, crate::error::ChartError> {
        Self::from_spec(style.and_then(|s| s.legend.as_ref()))
            .map_err(|msg| crate::error::ChartError::InvalidSpec(format!("visualize.style.legend.{msg}")))
    }
}

/// A legend laid out for its placement: items positioned relative to the
/// block's top-left corner, and the block's size.
#[derive(Debug, Clone)]
pub struct LegendBlock {
    pub placement: LegendPlacement,
    pub layout: LegendLayoutResult,
    pub config: LegendConfig,
    /// Block width. A top or bottom legend spans the chart width (its rows
    /// are aligned within it); a side legend is as wide as its widest row.
    pub width: f64,
    /// Block height.
    pub height: f64,
}

/// Room a legend takes from each side of the chart, before [`LEGEND_GAP`]s.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LegendReserve {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl LegendReserve {
    /// How far a right legend pushes in content anchored to the chart's right
    /// edge (a right axis title): its width plus a gap either side.
    pub fn right_inset(&self) -> f64 {
        if self.right > 0.0 { self.right + 2.0 * LEGEND_GAP } else { 0.0 }
    }

    /// Set the legend fields of `margins` (`legend_height`,
    /// `legend_top_height`, `legend_left_width`, `legend_right_width`) from
    /// this reserve.
    pub fn apply_to(&self, margins: &mut crate::layout::MarginConfig) {
        margins.legend_height = self.bottom;
        margins.legend_top_height = self.top;
        margins.legend_left_width = self.left;
        margins.legend_right_width = self.right;
    }
}

impl LegendBlock {
    /// Lay out `labels` for `placement` on a `chart_width` x `chart_height`
    /// chart. `None` when the placement is [`LegendPosition::None`].
    ///
    /// `config` supplies the item metrics and the alignment of a top or
    /// bottom legend's rows; a side legend's rows align left. A horizontal
    /// legend wraps onto at most `config.max_rows` rows. A vertical legend has
    /// one item per row; at the top or bottom it shows at most
    /// `config.max_rows` items, at a side as many as fit the chart height.
    pub fn new(
        labels: &[String],
        colors: &[String],
        placement: LegendPlacement,
        chart_width: f64,
        chart_height: f64,
        config: LegendConfig,
    ) -> Option<Self> {
        let side = match placement.position {
            LegendPosition::None => return None,
            LegendPosition::Left | LegendPosition::Right => true,
            LegendPosition::Top | LegendPosition::Bottom => false,
        };
        let available_width = if side {
            chart_width * SIDE_LEGEND_MAX_WIDTH_FRACTION
        } else {
            chart_width
        };
        let mut config = config;
        if side {
            config.alignment = LegendAlignment::Left;
        }
        let layout = match placement.orientation {
            LegendOrientation::Horizontal => {
                calculate_legend_layout(labels, colors, available_width, &config)
            }
            LegendOrientation::Vertical => {
                let max_rows = if side {
                    (((chart_height - 2.0 * LEGEND_GAP) / config.row_height).floor() as usize).max(1)
                } else {
                    config.max_rows
                };
                let mut layout =
                    calculate_vertical_legend_layout(labels, colors, available_width, max_rows, &config);
                if !side {
                    // A column above or below the plot is aligned across the
                    // chart width like a row is.
                    let column_width = layout
                        .items
                        .iter()
                        .filter(|i| i.visible)
                        .map(|i| i.width)
                        .fold(0.0_f64, f64::max);
                    let offset = match config.alignment {
                        LegendAlignment::Left => 0.0,
                        LegendAlignment::Center => ((chart_width - column_width) / 2.0).max(0.0),
                        LegendAlignment::Right => (chart_width - column_width).max(0.0),
                    };
                    for item in &mut layout.items {
                        item.x += offset;
                    }
                }
                layout
            }
        };
        let rows_width = layout
            .items
            .iter()
            .filter(|i| i.visible)
            .map(|i| i.x + i.width)
            .fold(0.0_f64, f64::max);
        let width = if side { rows_width } else { chart_width };
        let height = layout.total_height;
        Some(Self { placement, layout, config, width, height })
    }

    /// Room the legend takes from the side it sits on.
    pub fn reserve(&self) -> LegendReserve {
        let mut r = LegendReserve::default();
        match self.placement.position {
            LegendPosition::Top => r.top = self.height,
            LegendPosition::Bottom => r.bottom = self.height,
            LegendPosition::Left => r.left = self.width,
            LegendPosition::Right => r.right = self.width,
            LegendPosition::None => {}
        }
        r
    }

    /// The block's top-left corner on a `chart_width` x `chart_height`
    /// chart, `edge_gap` in from the edge it sits on. A side legend is
    /// centred vertically.
    pub fn origin(&self, chart_width: f64, chart_height: f64, edge_gap: f64) -> (f64, f64) {
        let side_y = ((chart_height - self.height) / 2.0).max(0.0);
        match self.placement.position {
            LegendPosition::Bottom | LegendPosition::None => (0.0, chart_height - self.height - edge_gap),
            LegendPosition::Top => (0.0, edge_gap),
            LegendPosition::Left => (edge_gap, side_y),
            LegendPosition::Right => (chart_width - self.width - edge_gap, side_y),
        }
    }

    /// The legend's elements with its corner at `origin`.
    pub fn elements(
        &self,
        origin: (f64, f64),
        mark_for: impl Fn(usize) -> LegendMark,
        theme: &crate::theme::Theme,
    ) -> Vec<ChartElement> {
        legend_item_elements(&self.layout, &self.config, origin.0, origin.1, mark_for, theme)
    }
}

/// Lay out a single-column legend: one item per row, left-aligned, labels
/// truncated with an ellipsis to fit `available_width`, and items past
/// `max_rows` hidden.
pub fn calculate_vertical_legend_layout(
    labels: &[String],
    colors: &[String],
    available_width: f64,
    max_rows: usize,
    config: &LegendConfig,
) -> LegendLayoutResult {
    let metrics = &config.text_metrics;
    let non_text_width = config.symbol_size + config.symbol_text_gap + config.item_padding;
    let max_text_width = available_width - non_text_width;
    let mut items = Vec::with_capacity(labels.len());
    let mut overflow_count = 0;
    for (i, label) in labels.iter().enumerate() {
        let display_label = truncate_to_width(label, max_text_width, config.max_label_chars, metrics);
        let item_width = non_text_width + measure_text(&display_label, metrics);
        let visible = i < max_rows;
        if !visible {
            overflow_count += 1;
        }
        items.push(LegendItem {
            index: i,
            label: display_label,
            color: colors.get(i).cloned().unwrap_or_default(),
            x: 0.0,
            y: i as f64 * config.row_height,
            width: item_width,
            row: i,
            visible,
        });
    }
    let rows = labels.len().min(max_rows);
    LegendLayoutResult { items, total_height: rows as f64 * config.row_height, overflow_count }
}

/// `label` unchanged when it fits `max_width`, else cut to at most
/// `max_chars` characters and then shortened until it fits with an ellipsis.
fn truncate_to_width(label: &str, max_width: f64, max_chars: usize, metrics: &TextMetrics) -> String {
    if measure_text(label, metrics) <= max_width {
        return label.to_string();
    }
    let mut count = label.chars().count().min(max_chars);
    loop {
        if count == 0 {
            return "\u{2026}".to_string();
        }
        let candidate: String = label.chars().take(count).collect();
        let with_ellipsis = format!("{candidate}\u{2026}");
        if measure_text(&with_ellipsis, metrics) <= max_width {
            return with_ellipsis;
        }
        count -= 1;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn make_labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn make_colors(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("#{:02x}{:02x}{:02x}", i * 30, i * 50, i * 70)).collect()
    }

    /// Phase 4: verify that non-default `legend_*` typography fields on
    /// `Theme` are applied to the emitted `legend-label` text elements.
    #[test]
    fn phase4_legend_text_picks_up_theme_typography() {
        use crate::theme::Theme;

        let labels = make_labels(&["Alpha", "Beta"]);
        let colors = make_colors(2);
        let theme = Theme {
            legend_font_family: "Georgia, serif".into(),
            legend_font_weight: 800,
            legend_font_size: 15.0,
            ..Theme::default()
        };

        let elements = generate_legend_elements(
            &labels,
            &colors,
            400.0,
            100.0,
            LegendMark::Rect,
            &theme,
        );

        let mut label_hits = 0;
        for el in &elements {
            if let ChartElement::Text {
                class,
                font_family,
                font_weight,
                font_size,
                ..
            } = el
            {
                if class == "legend-label" {
                    label_hits += 1;
                    assert_eq!(
                        font_family.as_deref(),
                        Some("Georgia, serif"),
                        "legend-label must carry theme.legend_font_family"
                    );
                    assert_eq!(
                        font_weight.as_deref(),
                        Some("800"),
                        "legend-label must carry theme.legend_font_weight"
                    );
                    assert_eq!(
                        font_size.as_deref(),
                        Some("15px"),
                        "legend-label must carry theme.legend_font_size"
                    );
                }
            }
        }
        assert_eq!(
            label_hits, 2,
            "expected one text per legend item"
        );
    }

    /// Phase 4: with a default theme, the legend-label text must NOT carry
    /// `font-family` / `font-weight` / `letter-spacing` / `text-transform`
    /// overrides and must restate the legacy 11px font-size. This is the
    /// byte-identity guarantee at the unit-test level.
    #[test]
    fn phase4_legend_text_default_theme_preserves_legacy_emission() {
        use crate::theme::Theme;

        let labels = make_labels(&["Alpha", "Beta"]);
        let colors = make_colors(2);
        let theme = Theme::default();

        let elements = generate_legend_elements(
            &labels,
            &colors,
            400.0,
            100.0,
            LegendMark::Rect,
            &theme,
        );

        for el in &elements {
            if let ChartElement::Text {
                class,
                font_family,
                font_weight,
                letter_spacing,
                text_transform,
                font_size,
                ..
            } = el
            {
                if class == "legend-label" {
                    assert!(font_family.is_none(), "default theme must not set font-family");
                    assert!(font_weight.is_none(), "default theme must not set font-weight");
                    assert!(letter_spacing.is_none(), "default theme must not set letter-spacing");
                    assert!(text_transform.is_none(), "default theme must not set text-transform");
                    assert_eq!(
                        font_size.as_deref(),
                        Some("11px"),
                        "default theme must keep the legacy 11px legend size"
                    );
                }
            }
        }
    }

    fn legend_spec(position: Option<&str>, orientation: Option<&str>) -> crate::spec::LegendSpec {
        crate::spec::LegendSpec {
            position: position.map(str::to_string),
            orientation: orientation.map(str::to_string),
        }
    }

    #[test]
    fn placement_defaults_and_orientation_follow_the_position() {
        use LegendOrientation::{Horizontal, Vertical};
        let p = |pos, ori| LegendPlacement::from_spec(Some(&legend_spec(pos, ori))).unwrap();
        assert_eq!(LegendPlacement::from_spec(None).unwrap(), LegendPlacement::default());
        assert_eq!(
            LegendPlacement::default(),
            LegendPlacement { position: LegendPosition::Bottom, orientation: Horizontal }
        );
        assert_eq!(p(Some("left"), None).orientation, Vertical);
        assert_eq!(p(Some("right"), None).orientation, Vertical);
        assert_eq!(p(Some("top"), None).orientation, Horizontal);
        assert_eq!(p(Some("right"), Some("horizontal")).orientation, Horizontal);
        assert_eq!(p(Some("none"), None).position, LegendPosition::None);
        assert!(LegendPlacement::from_spec(Some(&legend_spec(Some("center"), None)))
            .unwrap_err()
            .contains("got 'center'"));
        assert!(LegendPlacement::from_spec(Some(&legend_spec(None, Some("diagonal"))))
            .unwrap_err()
            .contains("got 'diagonal'"));
    }

    #[test]
    fn bottom_horizontal_block_matches_the_row_layout() {
        // The default placement lays out exactly as calculate_legend_layout,
        // so default charts render unchanged.
        let labels = make_labels(&["Alpha", "Beta", "Gamma"]);
        let colors = make_colors(3);
        let rows = calculate_legend_layout(&labels, &colors, 600.0, &LegendConfig::default());
        let block = LegendBlock::new(&labels, &colors, LegendPlacement::default(), 600.0, 400.0, LegendConfig::default())
            .unwrap();
        for (a, b) in rows.items.iter().zip(&block.layout.items) {
            assert_eq!((a.x, a.y, &a.label), (b.x, b.y, &b.label));
        }
        assert_eq!(block.reserve(), LegendReserve { bottom: rows.total_height, ..LegendReserve::default() });
        assert_eq!(block.origin(600.0, 400.0, LEGEND_GAP), (0.0, 400.0 - rows.total_height - LEGEND_GAP));
    }

    #[test]
    fn side_blocks_are_columns_at_the_edge_centred_vertically() {
        let labels = make_labels(&["Alpha", "Beta", "Gamma"]);
        let colors = make_colors(3);
        let right = LegendPlacement { position: LegendPosition::Right, orientation: LegendOrientation::Vertical };
        let block = LegendBlock::new(&labels, &colors, right, 800.0, 400.0, LegendConfig::default()).unwrap();
        assert!(block.layout.items.iter().all(|i| i.x == 0.0));
        assert_eq!(block.height, 60.0);
        assert!(block.width > 0.0 && block.width <= 800.0 * SIDE_LEGEND_MAX_WIDTH_FRACTION);
        assert_eq!(block.reserve(), LegendReserve { right: block.width, ..LegendReserve::default() });
        let (x, y) = block.origin(800.0, 400.0, LEGEND_GAP);
        assert_eq!(x, 800.0 - block.width - LEGEND_GAP);
        assert_eq!(y, (400.0 - 60.0) / 2.0);

        let left = LegendPlacement { position: LegendPosition::Left, orientation: LegendOrientation::Vertical };
        let block = LegendBlock::new(&labels, &colors, left, 800.0, 400.0, LegendConfig::default()).unwrap();
        assert_eq!(block.origin(800.0, 400.0, LEGEND_GAP).0, LEGEND_GAP);

        let none = LegendPlacement { position: LegendPosition::None, orientation: LegendOrientation::Horizontal };
        assert!(LegendBlock::new(&labels, &colors, none, 800.0, 400.0, LegendConfig::default()).is_none());
    }

    #[test]
    fn side_legend_labels_are_truncated_to_its_width_and_overflow_is_hidden() {
        let labels = make_labels(&["A very long series name that cannot fit", "B", "C", "D"]);
        let colors = make_colors(4);
        let right = LegendPlacement { position: LegendPosition::Right, orientation: LegendOrientation::Vertical };
        // 100px tall: (100 - 16) / 20 = 4 rows fit; 60px tall: 2 rows.
        let block = LegendBlock::new(&labels, &colors, right, 300.0, 60.0, LegendConfig::default()).unwrap();
        assert!(block.layout.items[0].label.ends_with('\u{2026}'), "{}", block.layout.items[0].label);
        assert!(block.width <= 300.0 * SIDE_LEGEND_MAX_WIDTH_FRACTION);
        assert_eq!(block.layout.items.iter().filter(|i| i.visible).count(), 2);
        assert_eq!(block.layout.overflow_count, 2);
    }

    #[test]
    fn top_and_bottom_vertical_columns_are_centred() {
        let labels = make_labels(&["Alpha", "Beta"]);
        let colors = make_colors(2);
        let p = LegendPlacement { position: LegendPosition::Top, orientation: LegendOrientation::Vertical };
        let block = LegendBlock::new(&labels, &colors, p, 600.0, 400.0, LegendConfig::default()).unwrap();
        let column = block.layout.items.iter().map(|i| i.width).fold(0.0_f64, f64::max);
        assert!(block.layout.items.iter().all(|i| i.x == (600.0 - column) / 2.0));
        assert_eq!(block.layout.items[1].y, 20.0);
        assert_eq!(block.origin(600.0, 400.0, LEGEND_GAP), (0.0, LEGEND_GAP));
    }

    #[test]
    fn legend_single_row() {
        let labels = make_labels(&["Alpha", "Beta", "Gamma"]);
        let colors = make_colors(3);
        let config = LegendConfig {
            alignment: LegendAlignment::Left,
            ..Default::default()
        };
        let result = calculate_legend_layout(&labels, &colors, 800.0, &config);
        assert_eq!(result.items.len(), 3);
        assert_eq!(result.overflow_count, 0);
        // All items should be on row 0
        for item in &result.items {
            assert_eq!(item.row, 0);
            assert!(item.visible);
        }
        assert!((result.total_height - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn legend_multi_row() {
        // Use many items with a narrow available width to force wrapping
        let labels = make_labels(&["Series One", "Series Two", "Series Three", "Series Four", "Series Five"]);
        let colors = make_colors(5);
        let config = LegendConfig {
            alignment: LegendAlignment::Left,
            ..Default::default()
        };
        // Narrow width: each item is ~(12 + 6 + 70 + 12) = ~100px for "Series One" (10 chars * 7 = 70)
        let result = calculate_legend_layout(&labels, &colors, 220.0, &config);
        assert!(result.items.iter().any(|item| item.row > 0),
            "Expected items to wrap to multiple rows");
        assert_eq!(result.overflow_count, 0); // 5 items should fit in 3 rows
    }

    #[test]
    fn legend_overflow() {
        // Many items, very narrow width, only 1 row allowed
        let labels = make_labels(&["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"]);
        let colors = make_colors(10);
        let config = LegendConfig {
            max_rows: 1,
            alignment: LegendAlignment::Left,
            ..Default::default()
        };
        // Each item ~(12+6+7+12)=37px, 10 items = 370px needed, only 100 available
        let result = calculate_legend_layout(&labels, &colors, 100.0, &config);
        assert!(result.overflow_count > 0,
            "Expected overflow, got 0 overflow items");
    }

    #[test]
    fn legend_empty() {
        let labels: Vec<String> = vec![];
        let colors: Vec<String> = vec![];
        let config = LegendConfig::default();
        let result = calculate_legend_layout(&labels, &colors, 800.0, &config);
        assert!(result.items.is_empty());
        assert_eq!(result.overflow_count, 0);
    }

    #[test]
    fn legend_center_alignment() {
        let labels = make_labels(&["A", "B"]);
        let colors = make_colors(2);
        let config = LegendConfig {
            alignment: LegendAlignment::Center,
            ..Default::default()
        };
        let result = calculate_legend_layout(&labels, &colors, 800.0, &config);
        // With center alignment and wide available width, first item x should be > 0
        assert!(result.items[0].x > 0.0,
            "Expected first item to be offset for centering, got x={}", result.items[0].x);
    }

    #[test]
    fn legend_label_truncation() {
        let labels = vec!["This is a very long label that exceeds the maximum".to_string()];
        let colors = make_colors(1);
        let config = LegendConfig {
            alignment: LegendAlignment::Left,
            ..Default::default()
        };
        // Use a narrow width to force truncation (truncation is pixel-width based)
        let result = calculate_legend_layout(&labels, &colors, 200.0, &config);
        assert!(result.items[0].label.ends_with('\u{2026}'),
            "Expected truncated label with ellipsis, got '{}'", result.items[0].label);
    }

    #[test]
    fn legend_colors_assigned() {
        let labels = make_labels(&["A", "B", "C"]);
        let colors = vec!["red".to_string(), "green".to_string(), "blue".to_string()];
        let config = LegendConfig {
            alignment: LegendAlignment::Left,
            ..Default::default()
        };
        let result = calculate_legend_layout(&labels, &colors, 800.0, &config);
        assert_eq!(result.items[0].color, "red");
        assert_eq!(result.items[1].color, "green");
        assert_eq!(result.items[2].color, "blue");
    }
}
