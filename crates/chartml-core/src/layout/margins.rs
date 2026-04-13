/// Chart margins in pixels.
#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Margins {
    pub fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self { top, right, bottom, left }
    }

    /// Calculate the inner chart width after margins.
    pub fn inner_width(&self, total_width: f64) -> f64 {
        (total_width - self.left - self.right).max(0.0)
    }

    /// Calculate the inner chart height after margins.
    pub fn inner_height(&self, total_height: f64) -> f64 {
        (total_height - self.top - self.bottom).max(0.0)
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self { top: 20.0, right: 30.0, bottom: 40.0, left: 70.0 }
    }
}

/// Configuration for margin calculation.
pub struct MarginConfig {
    pub has_title: bool,
    pub has_x_axis_label: bool,
    pub has_y_axis_label: bool,
    pub has_right_axis: bool,
    /// Actual legend height in pixels (0.0 when no legend is present).
    /// This should be pre-computed via `calculate_legend_layout().total_height`
    /// so that multi-row legends get proper bottom margin space.
    pub legend_height: f64,
    pub y_tick_labels: Vec<String>,
    pub right_tick_labels: Vec<String>,
    pub x_label_strategy_margin: f64,
    pub max_left_margin: f64,
    pub max_right_margin: f64,
    /// Total SVG height — used to scale bottom margin for small charts.
    pub chart_height: f64,
    /// Text metrics used to measure numeric tick labels (left/right axes).
    /// Defaults to the legacy 12px sans calibration; override via
    /// `TextMetrics::from_theme_tick_value(theme)` when the theme overrides
    /// numeric typography.
    pub tick_value_metrics: super::labels::TextMetrics,
    /// Text metrics used to reserve room for the rotated Y-axis label.
    /// Defaults to legacy.
    pub axis_label_metrics: super::labels::TextMetrics,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            has_title: false,
            has_x_axis_label: false,
            has_y_axis_label: false,
            has_right_axis: false,
            legend_height: 0.0,
            y_tick_labels: Vec::new(),
            right_tick_labels: Vec::new(),
            x_label_strategy_margin: 0.0,
            max_left_margin: 250.0,
            max_right_margin: 250.0,
            chart_height: 400.0,
            tick_value_metrics: super::labels::TextMetrics::default(),
            axis_label_metrics: super::labels::TextMetrics::default(),
        }
    }
}

/// Calculate chart margins based on configuration.
///
/// Algorithm:
/// - Top: 20px (title is rendered as HTML outside the SVG)
/// - Left: max(tick_label_widths) + 15px buffer, or tick_width + 33px when y-axis
///   label is present (to fit rotated label + gap), capped at max_left_margin
/// - Right: 30px base, or max(right_label_widths) + 44px if right axis present,
///   capped at max_right_margin
/// - Bottom: 40px base + x_label_strategy_margin (rotation) + 20px if x-axis label
///   + legend_height + 8px gap when legend is present
pub fn calculate_margins(config: &MarginConfig) -> Margins {
    use super::labels::measure_text;

    // Top margin — matches JS d3CartesianChart.js marginTop=20 (title is rendered as HTML outside SVG)
    let top = 20.0;

    // Left margin: based on Y-axis tick label widths
    let max_y_label_width = config.y_tick_labels.iter()
        .map(|l| measure_text(l, &config.tick_value_metrics))
        .fold(0.0_f64, f64::max);
    let tick_padding = 15.0;
    let left_base = if max_y_label_width > 0.0 {
        max_y_label_width + tick_padding
    } else {
        70.0 // matches JS d3CartesianChart.js default marginLeft
    };
    // When a y-axis label is present (rotated -90°), ensure the left margin
    // accommodates: tick_label_max_width + tick_padding (15px) + gap (4px)
    // + axis_label_width (rotated text height ≈ font size + padding).
    // This prevents the rotated axis label from overlapping the tick labels.
    let left = if config.has_y_axis_label {
        // Base legacy allowance is 14px; when the theme scales the axis
        // label font up, grow the reservation in proportion so the rotated
        // label still clears the tick labels.
        let axis_label_width = if config.axis_label_metrics.is_legacy_default() {
            14.0_f64
        } else {
            (config.axis_label_metrics.font_size_px + 2.0).max(14.0)
        };
        let gap = 4.0_f64;
        let min_with_label = max_y_label_width + tick_padding + gap + axis_label_width;
        left_base.max(min_with_label)
    } else {
        left_base
    }.min(config.max_left_margin);

    // Right margin
    let right = if config.has_right_axis {
        let max_right_width = config.right_tick_labels.iter()
            .map(|l| measure_text(l, &config.tick_value_metrics))
            .fold(0.0_f64, f64::max);
        // 24px for tick+gap, +20px for axis title label
        (max_right_width + 24.0 + 20.0)
            .min(config.max_right_margin)
    } else {
        30.0 // matches JS d3CartesianChart.js default marginRight
    };

    // Bottom margin: 40px base covers tick marks (+5px) and horizontal label text (+18px)
    // with some padding.  For small charts (< 300px tall), scale proportionally to
    // avoid the base consuming too much of the chart height.
    // The rotation margin from x_label_strategy_margin adds the vertical descent of
    // rotated labels beyond the horizontal baseline.
    let base_bottom = if config.chart_height < 300.0 {
        // Scale: at 150px tall => ~25px base; at 300px => 40px base.
        // This keeps horizontal labels visible while leaving room for the plot.
        (config.chart_height * 0.16).clamp(20.0, 40.0)
    } else {
        40.0
    };
    let bottom = base_bottom
        + config.x_label_strategy_margin
        + if config.has_x_axis_label { 20.0 } else { 0.0 }
        + if config.legend_height > 0.0 { config.legend_height + 8.0 } else { 0.0 };

    Margins { top, right, bottom, left }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_margins() {
        let m = Margins::default();
        assert_eq!(m.top, 20.0);
        assert_eq!(m.right, 30.0);
        assert_eq!(m.bottom, 40.0);
        assert_eq!(m.left, 70.0);
    }

    #[test]
    fn margins_inner_dimensions() {
        let m = Margins::new(10.0, 20.0, 30.0, 40.0);
        assert!((m.inner_width(800.0) - 740.0).abs() < f64::EPSILON);
        assert!((m.inner_height(600.0) - 560.0).abs() < f64::EPSILON);
    }

    #[test]
    fn margins_inner_dimensions_clamp_to_zero() {
        let m = Margins::new(300.0, 300.0, 300.0, 300.0);
        assert_eq!(m.inner_width(100.0), 0.0);
        assert_eq!(m.inner_height(100.0), 0.0);
    }

    #[test]
    fn margins_with_title() {
        let config = MarginConfig {
            has_title: true,
            ..Default::default()
        };
        let m = calculate_margins(&config);
        assert_eq!(m.top, 20.0); // title is rendered as HTML outside SVG — no extra margin needed
    }

    #[test]
    fn margins_without_title() {
        let config = MarginConfig::default();
        let m = calculate_margins(&config);
        assert_eq!(m.top, 20.0);
    }

    #[test]
    fn margins_with_single_row_legend() {
        let config = MarginConfig {
            legend_height: 20.0, // single row
            ..Default::default()
        };
        let m = calculate_margins(&config);
        assert_eq!(m.bottom, 68.0); // 40 + 20 + 8
    }

    #[test]
    fn margins_with_multi_row_legend() {
        let config = MarginConfig {
            legend_height: 60.0, // 3 rows × 20px
            ..Default::default()
        };
        let m = calculate_margins(&config);
        assert_eq!(m.bottom, 108.0); // 40 + 60 + 8
    }

    #[test]
    fn margins_with_y_labels() {
        use crate::layout::labels::approximate_text_width;
        let config = MarginConfig {
            y_tick_labels: vec!["100,000".into(), "1,000,000".into()],
            ..Default::default()
        };
        let m = calculate_margins(&config);
        // Left margin should be max label width + 15px buffer
        let expected_max_width = approximate_text_width("1,000,000");
        let expected_left = expected_max_width + 15.0;
        assert!((m.left - expected_left).abs() < f64::EPSILON,
            "Expected left margin ~{}, got {}", expected_left, m.left);
    }

    #[test]
    fn margins_capped() {
        let config = MarginConfig {
            y_tick_labels: vec!["A".repeat(100)], // very wide label
            max_left_margin: 250.0,
            ..Default::default()
        };
        let m = calculate_margins(&config);
        assert!(m.left <= 250.0, "Left margin {} exceeds cap of 250", m.left);
    }
}
