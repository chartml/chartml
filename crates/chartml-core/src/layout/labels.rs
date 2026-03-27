/// The strategy selected for rendering labels.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelStrategy {
    /// Labels displayed horizontally (no transformation needed).
    Horizontal,
    /// Labels rotated -45 degrees. Contains the additional bottom margin needed
    /// and an optional skip factor for label sampling after rotation.
    Rotated { margin: f64, skip_factor: Option<usize> },
    /// Labels truncated to max_width with ellipsis.
    Truncated { max_width: f64 },
    /// Only a subset of labels shown (evenly sampled).
    Sampled { indices: Vec<usize> },
}

/// Configuration for label strategy determination.
pub struct LabelStrategyConfig {
    pub min_label_spacing: f64,   // Default: 10.0 px
    pub max_label_width: f64,     // Default: 120.0 px for truncation
    pub max_rotation_margin: f64, // Default: 150.0 px
    pub rotation_angle_deg: f64,  // Default: 45.0 degrees
}

impl Default for LabelStrategyConfig {
    fn default() -> Self {
        Self {
            min_label_spacing: 10.0,
            max_label_width: 120.0,
            max_rotation_margin: 150.0,
            rotation_angle_deg: 45.0,
        }
    }
}

impl LabelStrategy {
    /// Determine the best label strategy based on available space and label measurements.
    ///
    /// Algorithm (cascading priority):
    /// 1. Horizontal: if labels fit without overlap
    /// 2. Sampled-horizontal: if > 12 labels don't fit, sample to ~10 labels shown horizontally
    /// 3. Rotated: if <= 40 labels, rotate -45 degrees
    /// 4. Truncated: if truncated labels fit and <= 50 labels
    /// 5. Sampled: show an evenly-distributed subset
    ///
    /// Parameters:
    /// - labels: the label strings
    /// - available_width: total width available for the axis (chart width)
    /// - config: strategy configuration
    pub fn determine(
        labels: &[String],
        available_width: f64,
        config: &LabelStrategyConfig,
    ) -> Self {
        let label_count = labels.len();
        if label_count == 0 {
            return LabelStrategy::Horizontal;
        }

        let available_per_label = available_width / label_count as f64;

        // Measure label widths using character approximation
        let widths: Vec<f64> = labels.iter().map(|l| approximate_text_width(l)).collect();
        let avg_width = widths.iter().sum::<f64>() / widths.len() as f64;
        let max_width = widths.iter().cloned().fold(0.0_f64, f64::max);

        // Strategy 1: Horizontal -- labels fit without overlap
        if avg_width + config.min_label_spacing <= available_per_label {
            return LabelStrategy::Horizontal;
        }

        // Strategy 2: Sampled-horizontal -- for dense axes (> 12 labels), prefer
        // sampling over rotation. Compute a stride so ~10 labels are shown, keeping
        // first and last. This avoids cluttered rotated text on temporal axes.
        if label_count > 12 {
            let target_count = 10usize.min(label_count);
            let indices = strategic_indices(label_count, target_count);
            return LabelStrategy::Sampled { indices };
        }

        // Strategy 3: Rotated -- rotate -45 degrees if not too many labels
        if label_count <= 40 {
            let angle_rad = config.rotation_angle_deg.to_radians();
            let required_vertical = max_width * angle_rad.sin();
            let margin = (required_vertical.ceil() + 15.0).min(config.max_rotation_margin);
            let skip_factor = compute_skip_factor(labels, available_width, config.rotation_angle_deg);
            return LabelStrategy::Rotated { margin, skip_factor };
        }

        // Strategy 4: Truncated -- if truncated labels would fit
        if config.max_label_width + config.min_label_spacing <= available_per_label && label_count <= 50 {
            return LabelStrategy::Truncated { max_width: config.max_label_width };
        }

        // Strategy 5: Sampled -- show a subset
        let target_count = ((available_width / 120.0).floor() as usize).max(5);
        let indices = strategic_indices(label_count, target_count);
        LabelStrategy::Sampled { indices }
    }
}

/// Approximate width of a single character in pixels at default font size (~12px).
fn char_width(ch: char) -> f64 {
    match ch {
        'M' | 'W' | 'm' | 'w' => 9.0,
        'i' | 'l' | 'j' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 4.0,
        'f' | 'r' | 't' => 5.0,
        ' ' => 4.0,
        _ => 7.0,
    }
}

/// Approximate text width in pixels using a character-width table.
pub fn approximate_text_width(text: &str) -> f64 {
    text.chars().map(char_width).sum()
}

/// After rotation, check if labels still overlap and compute skip factor.
/// Matches JS: if overlapRatio > 1.5 && labelCount > 8, skip = ceil(overlapRatio / 2)
pub fn compute_skip_factor(
    labels: &[String],
    available_width: f64,
    rotation_angle_deg: f64,
) -> Option<usize> {
    if labels.len() <= 8 {
        return None;
    }
    let available_per_label = available_width / labels.len() as f64;
    let max_width = labels.iter().map(|l| approximate_text_width(l)).fold(0.0_f64, f64::max);
    let rotated_width = max_width * rotation_angle_deg.to_radians().cos();
    let overlap_ratio = (rotated_width + 6.0) / available_per_label;
    if overlap_ratio > 1.5 {
        Some((overlap_ratio / 2.0).ceil() as usize)
    } else {
        None
    }
}

/// Select strategic indices for sampled label display.
/// Always includes first and last; evenly distributes the rest.
pub fn strategic_indices(total: usize, target: usize) -> Vec<usize> {
    if total == 0 {
        return vec![];
    }
    if target >= total {
        return (0..total).collect();
    }
    if target <= 1 {
        return if total == 1 { vec![0] } else { vec![0, total - 1] };
    }
    if target == 2 {
        return vec![0, total - 1];
    }

    let mut indices = Vec::with_capacity(target);
    let step = (total - 1) as f64 / (target - 1) as f64;
    for i in 0..target {
        let idx = (i as f64 * step).round() as usize;
        indices.push(idx.min(total - 1));
    }
    // Deduplicate while preserving order
    indices.dedup();
    indices
}

/// Truncate a label to fit within max_width, adding ellipsis.
pub fn truncate_label(label: &str, max_width: f64) -> String {
    let full_width = approximate_text_width(label);
    if full_width <= max_width {
        return label.to_string();
    }

    let ellipsis_width = approximate_text_width("\u{2026}");
    let target_width = max_width - ellipsis_width;

    let mut width = 0.0;
    let mut end_idx = 0;
    for (i, ch) in label.char_indices() {
        let cw = char_width(ch);
        if width + cw > target_width {
            break;
        }
        width += cw;
        end_idx = i + ch.len_utf8();
    }

    format!("{}\u{2026}", &label[..end_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_horizontal_when_fits() {
        let labels: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        let strategy = LabelStrategy::determine(&labels, 800.0, &LabelStrategyConfig::default());
        assert_eq!(strategy, LabelStrategy::Horizontal);
    }

    #[test]
    fn strategy_rotated_when_moderate() {
        // Use <= 12 labels that don't fit horizontally to get Rotated
        // (> 12 labels now prefer Sampled over Rotated)
        let labels: Vec<String> = (0..10)
            .map(|i| format!("Category {}", i))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 200.0, &LabelStrategyConfig::default());
        assert!(matches!(strategy, LabelStrategy::Rotated { .. }),
            "Expected Rotated, got {:?}", strategy);
    }

    #[test]
    fn strategy_sampled_when_dense_axis() {
        // > 12 labels that don't fit horizontally should be Sampled
        let labels: Vec<String> = (0..20)
            .map(|i| format!("Category {}", i))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 200.0, &LabelStrategyConfig::default());
        match &strategy {
            LabelStrategy::Sampled { indices } => {
                assert!(indices.contains(&0), "Should include first index");
                assert!(indices.contains(&19), "Should include last index");
                assert!(indices.len() <= 10, "Should show at most 10 labels, got {}", indices.len());
            }
            other => panic!("Expected Sampled, got {:?}", other),
        }
    }

    #[test]
    fn strategy_sampled_preserves_first_and_last() {
        // 18 monthly labels (the temporal_x_axis_monthly case)
        let labels: Vec<String> = (0..18)
            .map(|i| format!("Jan {:02}", i + 1))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 560.0, &LabelStrategyConfig::default());
        match &strategy {
            LabelStrategy::Sampled { indices } => {
                assert!(indices.contains(&0), "Should include first index");
                assert!(indices.contains(&17), "Should include last index");
                assert!(indices.len() <= 10, "Should show at most 10 labels, got {}", indices.len());
            }
            other => panic!("Expected Sampled, got {:?}", other),
        }
    }

    #[test]
    fn strategy_sampled_when_many() {
        let labels: Vec<String> = (0..100)
            .map(|i| format!("Long Category Name {}", i))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 400.0, &LabelStrategyConfig::default());
        assert!(matches!(strategy, LabelStrategy::Sampled { .. }),
            "Expected Sampled, got {:?}", strategy);
    }

    #[test]
    fn strategy_empty_labels() {
        let labels: Vec<String> = vec![];
        let strategy = LabelStrategy::determine(&labels, 800.0, &LabelStrategyConfig::default());
        assert_eq!(strategy, LabelStrategy::Horizontal);
    }

    #[test]
    fn strategic_indices_basic() {
        let indices = strategic_indices(10, 5);
        assert!(indices.contains(&0), "Should include first index");
        assert!(indices.contains(&9), "Should include last index");
        assert!(indices.len() <= 5, "Should have at most 5 indices");
    }

    #[test]
    fn strategic_indices_all() {
        let indices = strategic_indices(5, 10);
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn truncate_short_label() {
        let result = truncate_label("Hi", 100.0);
        assert_eq!(result, "Hi");
    }

    #[test]
    fn truncate_long_label() {
        let result = truncate_label("This is a very long label that should be truncated", 50.0);
        assert!(result.ends_with('\u{2026}'), "Should end with ellipsis, got '{}'", result);
        assert!(result.len() < "This is a very long label that should be truncated".len(),
            "Should be shorter than original");
    }

    #[test]
    fn approximate_text_width_basic() {
        let width = approximate_text_width("Hello");
        assert!(width > 0.0, "Width should be non-zero for non-empty string");
    }
}
