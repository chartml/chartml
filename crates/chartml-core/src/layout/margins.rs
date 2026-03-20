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
        Self { top: 30.0, right: 20.0, bottom: 40.0, left: 60.0 }
    }
}

/// Configuration for margin calculation.
pub struct MarginConfig {
    pub has_title: bool,
    pub has_x_axis_label: bool,
    pub has_y_axis_label: bool,
    pub has_right_axis: bool,
    pub has_legend: bool,
    pub y_tick_labels: Vec<String>,
    pub right_tick_labels: Vec<String>,
    pub x_label_strategy_margin: f64,
    pub max_left_margin: f64,
    pub max_right_margin: f64,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            has_title: false,
            has_x_axis_label: false,
            has_y_axis_label: false,
            has_right_axis: false,
            has_legend: false,
            y_tick_labels: Vec::new(),
            right_tick_labels: Vec::new(),
            x_label_strategy_margin: 0.0,
            max_left_margin: 250.0,
            max_right_margin: 250.0,
        }
    }
}

/// Calculate chart margins based on configuration.
///
/// Algorithm (matches JS d3CartesianChart.js):
/// - Top: 30px base + 25px if title present
/// - Left: max(y-axis label widths) + 15px buffer, capped at max_left_margin
///   + 20px if y-axis label present
/// - Right: 20px base, or max(right-axis label widths) + 24px if right axis present,
///   capped at max_right_margin
/// - Bottom: 40px base + x_label_strategy_margin (rotation) + 20px if x-axis label
///   + 30px if legend present
pub fn calculate_margins(config: &MarginConfig) -> Margins {
    use super::labels::approximate_text_width;

    // Top margin
    let top = 30.0 + if config.has_title { 25.0 } else { 0.0 };

    // Left margin: based on Y-axis tick label widths
    let max_y_label_width = config.y_tick_labels.iter()
        .map(|l| approximate_text_width(l))
        .fold(0.0_f64, f64::max);
    let left_base = if max_y_label_width > 0.0 {
        max_y_label_width + 15.0
    } else {
        60.0 // default
    };
    let left = (left_base + if config.has_y_axis_label { 20.0 } else { 0.0 })
        .min(config.max_left_margin);

    // Right margin
    let right = if config.has_right_axis {
        let max_right_width = config.right_tick_labels.iter()
            .map(|l| approximate_text_width(l))
            .fold(0.0_f64, f64::max);
        (max_right_width + 24.0)
            .min(config.max_right_margin)
    } else {
        20.0
    };

    // Bottom margin
    let bottom = 40.0
        + config.x_label_strategy_margin
        + if config.has_x_axis_label { 20.0 } else { 0.0 }
        + if config.has_legend { 30.0 } else { 0.0 };

    Margins { top, right, bottom, left }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_margins() {
        let m = Margins::default();
        assert_eq!(m.top, 30.0);
        assert_eq!(m.right, 20.0);
        assert_eq!(m.bottom, 40.0);
        assert_eq!(m.left, 60.0);
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
        assert_eq!(m.top, 55.0); // 30 + 25
    }

    #[test]
    fn margins_without_title() {
        let config = MarginConfig::default();
        let m = calculate_margins(&config);
        assert_eq!(m.top, 30.0);
    }

    #[test]
    fn margins_with_legend() {
        let config = MarginConfig {
            has_legend: true,
            ..Default::default()
        };
        let m = calculate_margins(&config);
        assert_eq!(m.bottom, 70.0); // 40 + 30
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
