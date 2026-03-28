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
    pub min_label_spacing: f64,   // Default: 4.0 px
    pub max_label_width: f64,     // Default: 120.0 px for truncation
    pub max_rotation_margin: f64, // Default: 150.0 px
    pub rotation_angle_deg: f64,  // Default: 45.0 degrees
}

impl Default for LabelStrategyConfig {
    fn default() -> Self {
        Self {
            min_label_spacing: 4.0,
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
    /// 2. Rotated: if <= 40 labels, rotate -45 degrees (post-rotation truncation
    ///    is applied later in generate_x_axis to guarantee no overlap)
    /// 3. Truncated: if truncated labels fit and <= 50 labels
    /// 4. Sampled: show an evenly-distributed subset
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

        // Strategy 2: Rotated -- rotate -45 degrees if not too many labels.
        // Post-rotation truncation is handled in generate_x_axis to ensure
        // rotated labels don't collide even when they are long.
        if label_count <= 40 {
            let angle_rad = config.rotation_angle_deg.to_radians();
            let skip_factor = compute_skip_factor(labels, available_width, config.rotation_angle_deg);

            // Mirror the post-rotation truncation from generate_x_axis:
            // visible labels are capped so their rotated horizontal projection
            // fits the available space.  The effective label width after truncation
            // determines the actual vertical descent used for the margin.
            let visible_count = match skip_factor {
                Some(f) if f > 1 => (0..label_count).filter(|i| i % f == 0).count(),
                _ => label_count,
            };
            let cos_a = angle_rad.cos(); // ~0.707 for 45 deg
            let available_per_visible = if visible_count > 0 {
                available_width / visible_count as f64
            } else {
                available_width
            };
            let spacing = 6.0;
            let overlap_width = (available_per_visible - spacing) / cos_a;

            // Effective width: cap each label at the overlap-free width
            // derived from the per-label spacing. This scales naturally with
            // chart width and label count — no special-case boost needed.
            let effective_width = if overlap_width > 0.0 {
                max_width.min(overlap_width)
            } else {
                max_width
            };
            let required_vertical = effective_width * angle_rad.sin();
            // Rotated labels are placed at y_position + 10, so total space
            // needed below the axis line is 10 + vertical_descent + padding.
            // The base bottom margin (40px) already covers some of that.
            // Match the JS labelUtils.js padding of 15px.
            let total_needed = 10.0 + required_vertical + 15.0;
            let base_bottom = 40.0;
            let margin = (total_needed - base_bottom).max(0.0).ceil().min(config.max_rotation_margin);
            return LabelStrategy::Rotated { margin, skip_factor };
        }

        // Strategy 3: Truncated -- if truncated labels would fit
        if config.max_label_width + config.min_label_spacing <= available_per_label && label_count <= 50 {
            return LabelStrategy::Truncated { max_width: config.max_label_width };
        }

        // Strategy 4: Sampled -- show a subset
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
/// Calibrated for ~12px font. For other sizes, use `approximate_text_width_at`.
pub fn approximate_text_width(text: &str) -> f64 {
    text.chars().map(char_width).sum()
}

/// Approximate text width scaled for a specific font size.
pub fn approximate_text_width_at(text: &str, font_size_px: f64) -> f64 {
    approximate_text_width(text) * (font_size_px / 12.0)
}

/// Format a numeric tick value with SI suffixes for large magnitudes.
/// Returns compact labels like "1.5M", "200K", "3B" based on the tick step.
pub fn format_tick_value_si(value: f64, tick_step: f64) -> String {
    let (scaled, suffix) = if tick_step >= 1_000_000_000.0 {
        (value / 1_000_000_000.0, "B")
    } else if tick_step >= 1_000_000.0 {
        (value / 1_000_000.0, "M")
    } else if tick_step >= 1_000.0 {
        (value / 1_000.0, "K")
    } else {
        // No SI suffix — use standard formatting
        let precision = if tick_step.abs() < 1e-15 {
            0usize
        } else {
            ((-tick_step.abs().log10().floor()) as i64).max(0) as usize
        };
        return format!("{:.prec$}", value, prec = precision);
    };

    // Use integer form if value is whole, otherwise one decimal
    if (scaled - scaled.round()).abs() < 1e-9 {
        format!("{}{}", scaled.round() as i64, suffix)
    } else {
        format!("{:.1}{}", scaled, suffix)
    }
}

#[cfg(test)]
mod si_tests {
    use super::format_tick_value_si;

    #[test]
    fn si_millions() {
        assert_eq!(format_tick_value_si(1_000_000.0, 1_000_000.0), "1M");
        assert_eq!(format_tick_value_si(7_200_000.0, 1_000_000.0), "7.2M");
        assert_eq!(format_tick_value_si(0.0, 1_000_000.0), "0M");
    }

    #[test]
    fn si_thousands() {
        assert_eq!(format_tick_value_si(1_000.0, 1_000.0), "1K");
        assert_eq!(format_tick_value_si(200_000.0, 100_000.0), "200K");
        assert_eq!(format_tick_value_si(1_500.0, 1_000.0), "1.5K");
    }

    #[test]
    fn si_billions() {
        assert_eq!(format_tick_value_si(2_000_000_000.0, 1_000_000_000.0), "2B");
    }

    #[test]
    fn no_si_small_values() {
        assert_eq!(format_tick_value_si(42.0, 10.0), "42");
        assert_eq!(format_tick_value_si(3.5, 0.5), "3.5");
    }

    #[test]
    fn zero_tick_step() {
        // Should not panic or produce absurd output
        assert_eq!(format_tick_value_si(5.0, 0.0), "5");
    }

    #[test]
    fn negative_values() {
        assert_eq!(format_tick_value_si(-2_000_000.0, 1_000_000.0), "-2M");
    }
}

/// After rotation, check if labels still overlap and compute skip factor.
///
/// Two-pronged approach:
/// 1. **Physical overlap**: When rotated labels overlap, the renderer truncates
///    them. Only skip when truncation would make labels too short to read
///    (below `min_readable_width`).
/// 2. **Readability thinning**: When there are many rotated labels (> 14) that
///    fill most of their allotted horizontal space, thin for visual clarity
///    even though there is no physical overlap.
pub fn compute_skip_factor(
    labels: &[String],
    available_width: f64,
    rotation_angle_deg: f64,
) -> Option<usize> {
    if labels.len() <= 8 {
        return None;
    }
    let label_count = labels.len();
    let available_per_label = available_width / label_count as f64;
    let cos_angle = rotation_angle_deg.to_radians().cos();

    // Use actual average label width for the overlap check (post-rotation
    // horizontal projection) rather than a fixed minimum.
    let widths: Vec<f64> = labels.iter().map(|l| approximate_text_width(l)).collect();
    let avg_width = widths.iter().sum::<f64>() / widths.len() as f64;
    let avg_rotated = avg_width * cos_angle;

    // Check 1: Physical overlap after rotation.
    // When the rotated projection exceeds the per-label slot, the renderer
    // applies post-rotation truncation. Only skip if truncation would make
    // labels unreadably short (< min_readable_width unrotated).
    let min_gap = 2.0;
    if avg_rotated + min_gap > available_per_label {
        let max_unrotated = (available_per_label - min_gap).max(0.0) / cos_angle;
        let min_readable_width = 30.0; // ~4 chars + ellipsis
        if max_unrotated < min_readable_width {
            let needed_per = min_readable_width * cos_angle + min_gap;
            let skip = (needed_per / available_per_label).ceil() as usize;
            return Some(skip.max(2));
        }
        // Truncation keeps labels readable; no skip needed.
        return None;
    }

    // Check 2: Readability thinning.
    // Many rotated labels (> 14) look cluttered when there is meaningful gap
    // between them (> 5px) but the density is still high. When the gap is tiny
    // (< 5px), labels are in "barely fits" territory and truncation alone
    // handles the layout — thinning would over-reduce.
    let gap = available_per_label - avg_rotated;
    if label_count > 14 && gap > 5.0 {
        return Some(2);
    }

    None
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
    fn strategy_rotated_when_dense_axis() {
        // 20 labels that don't fit horizontally should be Rotated (<=40 labels)
        let labels: Vec<String> = (0..20)
            .map(|i| format!("Category {}", i))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 200.0, &LabelStrategyConfig::default());
        assert!(matches!(strategy, LabelStrategy::Rotated { .. }),
            "Expected Rotated, got {:?}", strategy);
    }

    #[test]
    fn strategy_rotated_for_monthly_labels() {
        // 18 monthly labels should be Rotated (<=40 labels)
        let labels: Vec<String> = (0..18)
            .map(|i| format!("Jan {:02}", i + 1))
            .collect();
        let strategy = LabelStrategy::determine(&labels, 560.0, &LabelStrategyConfig::default());
        assert!(matches!(strategy, LabelStrategy::Rotated { .. }),
            "Expected Rotated, got {:?}", strategy);
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
