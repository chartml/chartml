use crate::element::{ChartElement, TextAnchor};

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
pub struct LegendConfig {
    pub symbol_size: f64,
    pub symbol_text_gap: f64,
    pub item_padding: f64,
    pub row_height: f64,
    pub max_rows: usize,
    pub max_label_chars: usize,
    pub alignment: LegendAlignment,
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
    use super::labels::approximate_text_width;

    let mut items = Vec::with_capacity(labels.len());
    let mut current_x = 0.0;
    let mut current_row = 0_usize;
    let mut overflow_count = 0;

    // Track rows for alignment: (start_index, end_index, row_width)
    let mut rows: Vec<(usize, usize, f64)> = vec![(0, 0, 0.0)];

    for (i, label) in labels.iter().enumerate() {
        // First, compute full-label width to decide row placement
        let full_text_width = approximate_text_width(label);
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
                if approximate_text_width(&candidate_with_ellipsis) <= max_text_width {
                    break candidate_with_ellipsis;
                }
                truncated_count -= 1;
            }
        } else {
            label.clone()
        };

        let text_width = approximate_text_width(&display_label);
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

/// Generate legend SVG elements for the given series.
/// Computes layout, then renders symbol + label elements for each visible item.
pub fn generate_legend_elements(
    series_names: &[String],
    colors: &[String],
    chart_width: f64,
    y_position: f64,
    mark: LegendMark,
) -> Vec<ChartElement> {
    if series_names.len() <= 1 {
        return Vec::new();
    }

    let config = LegendConfig::default();
    let result = calculate_legend_layout(series_names, colors, chart_width, &config);

    let mut elements = Vec::new();

    for item in &result.items {
        if !item.visible {
            continue;
        }

        let x = item.x;
        let sym_y = y_position + item.y;

        match mark {
            LegendMark::Line => {
                elements.push(ChartElement::Line {
                    x1: x,
                    y1: sym_y + config.symbol_size / 2.0,
                    x2: x + config.symbol_size,
                    y2: sym_y + config.symbol_size / 2.0,
                    stroke: item.color.clone(),
                    stroke_width: Some(2.5),
                    stroke_dasharray: None,
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
                    class: "legend-symbol".to_string(),
                    data: None,
                });
            }
        }

        elements.push(ChartElement::Text {
            x: x + config.symbol_size + config.symbol_text_gap,
            y: sym_y + 10.0,
            content: item.label.clone(),
            anchor: TextAnchor::Start,
            dominant_baseline: None,
            transform: None,
            font_size: Some("11px".to_string()),
            font_weight: None,
            fill: Some("#333".to_string()),
            class: "legend-label".to_string(),
            data: None,
        });
    }

    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_labels(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn make_colors(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("#{:02x}{:02x}{:02x}", i * 30, i * 50, i * 70)).collect()
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
        // Use a narrow width to force truncation (truncation is based on pixel width, not char count)
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
