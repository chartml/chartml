use crate::theme::{TextTransform, Theme};

/// Text-shaping parameters that influence the rendered width of a label.
///
/// `TextMetrics` is how layout code feeds theme typography into the width
/// estimator so margins, tick spacing, label strategy, legend packing, and
/// truncation all see the same width the SVG renderer will eventually paint.
///
/// The default is calibrated to `Theme::default()`: a 12px sans-serif face
/// with no letter spacing and no text transform. `measure_text(text, default)`
/// is therefore guaranteed to return exactly the legacy
/// `approximate_text_width(text)` value — this is the backward-compatibility
/// contract that keeps the pre-theme-hooks golden snapshots byte-identical.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    /// Rendered font size in pixels.
    pub font_size_px: f64,
    /// Extra CSS `letter-spacing` in pixels applied between glyphs.
    pub letter_spacing_px: f64,
    /// `text-transform` that will be applied by the renderer.
    pub text_transform: TextTransform,
    /// Whether the rendered font is a monospace face. Monospace glyphs are
    /// noticeably wider than the sans-serif calibration; this flag lets the
    /// measurement apply a per-character width correction.
    pub monospace: bool,
}

impl Default for TextMetrics {
    /// Default metrics match the `Theme::default()` legacy assumptions
    /// (12px sans-serif, no letter spacing, no transform). Feeding default
    /// metrics to `measure_text` produces output byte-identical to the legacy
    /// `approximate_text_width`.
    fn default() -> Self {
        Self {
            font_size_px: 12.0,
            letter_spacing_px: 0.0,
            text_transform: TextTransform::None,
            monospace: false,
        }
    }
}

impl TextMetrics {
    /// True when these metrics exactly match the legacy calibration. When
    /// true, `measure_text` short-circuits to `approximate_text_width` so
    /// layout output is bit-for-bit identical to the pre-3.1 behavior.
    #[inline]
    pub fn is_legacy_default(&self) -> bool {
        // Exact-equals on f64 is safe here: the sentinel values come from
        // `Default` literals, never from arithmetic.
        self.font_size_px == 12.0
            && self.letter_spacing_px == 0.0
            && !self.monospace
            && matches!(self.text_transform, TextTransform::None)
    }

    /// Build `TextMetrics` for numeric tick labels from a theme.
    ///
    /// Returns the legacy-default metrics (byte-identical to
    /// `approximate_text_width`) when every relevant field on `theme` matches
    /// `Theme::default()`. Any divergence flips the layout path to the full
    /// `measure_text` measurement.
    pub fn from_theme_tick_value(theme: &Theme) -> Self {
        let default = Theme::default();
        let size = theme.numeric_font_size as f64;
        let letter_spacing = theme.label_letter_spacing as f64;
        let transform = theme.label_text_transform.clone();
        let family_changed = theme.numeric_font_family != default.numeric_font_family;
        let monospace = family_changed && family_is_monospace(&theme.numeric_font_family);
        if (size - default.numeric_font_size as f64).abs() < f64::EPSILON
            && letter_spacing == 0.0
            && matches!(transform, TextTransform::None)
            && !family_changed
        {
            return Self::default();
        }
        Self {
            font_size_px: size,
            letter_spacing_px: letter_spacing,
            text_transform: transform,
            monospace,
        }
    }

    /// Build `TextMetrics` for axis/category labels from a theme.
    pub fn from_theme_axis_label(theme: &Theme) -> Self {
        let default = Theme::default();
        let size = theme.label_font_size as f64;
        let letter_spacing = theme.label_letter_spacing as f64;
        let transform = theme.label_text_transform.clone();
        let family_changed = theme.label_font_family != default.label_font_family;
        let monospace = family_changed && family_is_monospace(&theme.label_font_family);
        if (size - default.label_font_size as f64).abs() < f64::EPSILON
            && letter_spacing == 0.0
            && matches!(transform, TextTransform::None)
            && !family_changed
        {
            return Self::default();
        }
        Self {
            font_size_px: size,
            letter_spacing_px: letter_spacing,
            text_transform: transform,
            monospace,
        }
    }

    /// Build `TextMetrics` for legend labels from a theme.
    pub fn from_theme_legend(theme: &Theme) -> Self {
        let default = Theme::default();
        let size = theme.legend_font_size as f64;
        let letter_spacing = theme.label_letter_spacing as f64;
        let transform = theme.label_text_transform.clone();
        let family_changed = theme.legend_font_family != default.legend_font_family;
        let monospace = family_changed && family_is_monospace(&theme.legend_font_family);
        if (size - default.legend_font_size as f64).abs() < f64::EPSILON
            && letter_spacing == 0.0
            && matches!(transform, TextTransform::None)
            && !family_changed
        {
            return Self::default();
        }
        Self {
            font_size_px: size,
            letter_spacing_px: letter_spacing,
            text_transform: transform,
            monospace,
        }
    }

    /// Build `TextMetrics` for the chart title from a theme.
    pub fn from_theme_title(theme: &Theme) -> Self {
        let default = Theme::default();
        let size = theme.title_font_size as f64;
        let family_changed = theme.title_font_family != default.title_font_family;
        let monospace = family_changed && family_is_monospace(&theme.title_font_family);
        if (size - default.title_font_size as f64).abs() < f64::EPSILON && !family_changed {
            return Self::default();
        }
        Self {
            font_size_px: size,
            letter_spacing_px: 0.0,
            text_transform: TextTransform::None,
            monospace,
        }
    }
}

/// Heuristic check for monospace-family CSS font stacks. Matches any of the
/// common monospace tokens (`monospace`, `mono`, `code`, or widely-known
/// monospaced faces like `Geist Mono`, `Menlo`, `Consolas`, …). False
/// positives are extremely rare because the tokens are specific; false
/// negatives only affect unrecognised monospaced fonts and fall back to the
/// sans-serif calibration, which is the legacy behavior.
fn family_is_monospace(family: &str) -> bool {
    let s = family.to_ascii_lowercase();
    s.contains("monospace")
        || s.contains(" mono")
        || s.contains(",mono")
        || s.ends_with(" mono")
        || s.starts_with("mono")
        || s.contains("ui-monospace")
        || s.contains("menlo")
        || s.contains("consolas")
        || s.contains("courier")
        || s.contains("sf mono")
        || s.contains("jetbrains mono")
        || s.contains("fira code")
        || s.contains("fira mono")
        || s.contains("source code pro")
}

/// Apply a `TextTransform` to a text string without allocating when the
/// transform is `None`.
fn apply_transform<'a>(text: &'a str, transform: &TextTransform) -> std::borrow::Cow<'a, str> {
    match transform {
        TextTransform::None => std::borrow::Cow::Borrowed(text),
        TextTransform::Uppercase => std::borrow::Cow::Owned(text.to_uppercase()),
        TextTransform::Lowercase => std::borrow::Cow::Owned(text.to_lowercase()),
    }
}

/// Measure the rendered width of a text string under the given metrics.
///
/// This is the self-correcting text width estimator used by every layout
/// decision: margins, tick strategy, legend packing, truncation, and label
/// rotation. It accounts for:
///
/// * `font_size_px` — linear scaling from the 12px calibration baseline.
/// * `text_transform` — transforms the text before measuring so uppercase
///   labels are measured as uppercase (wider than the original).
/// * `letter_spacing_px` — CSS letter-spacing adds a constant per glyph.
/// * `monospace` — monospace faces are measured with a fixed per-character
///   advance that is wider than the sans-serif default.
///
/// Backward compatibility: when `metrics.is_legacy_default()` is true, the
/// result is identical (to the bit) to `approximate_text_width(text)`. This
/// is the invariant that keeps the pre-theme-hooks golden snapshots
/// byte-identical under `Theme::default()`.
pub fn measure_text(text: &str, metrics: &TextMetrics) -> f64 {
    if metrics.is_legacy_default() {
        return approximate_text_width(text);
    }

    let transformed = apply_transform(text, &metrics.text_transform);

    // Base width in the 12px calibration.
    let base = if metrics.monospace {
        // Monospace average advance ~7.2px at 12px is too narrow — glyphs in
        // real monospace faces (Geist Mono, Menlo, Consolas, SF Mono) sit
        // around 7.5–7.8 at 12px. Use 7.7 as the calibration so that e.g.
        // "1,234,567" measures wider than the sans fallback would claim.
        transformed.chars().count() as f64 * 7.7
    } else {
        transformed.chars().map(char_width).sum::<f64>()
    };

    // Scale for font size.
    let size_ratio = metrics.font_size_px / 12.0;
    let mut width = base * size_ratio;

    // Uppercase letters are meaningfully wider than the average in the
    // sans-serif calibration table (which is tuned for mixed case). Apply a
    // 1.10× correction on top of the scaled width when the renderer will
    // uppercase the glyphs.
    if matches!(metrics.text_transform, TextTransform::Uppercase) {
        width *= 1.10;
    }

    // CSS letter-spacing is additive between glyphs; it also affects the
    // trailing edge in the conservative direction, so we count all glyphs.
    let char_count = transformed.chars().count() as f64;
    if metrics.letter_spacing_px != 0.0 && char_count > 0.0 {
        width += char_count * metrics.letter_spacing_px;
    }

    width
}

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
    /// Text shaping metrics used to measure the rendered width of each
    /// candidate label. Defaults to the legacy calibration so callers that
    /// do not pass theme metrics see byte-identical layout behavior.
    pub text_metrics: TextMetrics,
}

impl Default for LabelStrategyConfig {
    fn default() -> Self {
        Self {
            min_label_spacing: 4.0,
            max_label_width: 120.0,
            max_rotation_margin: 150.0,
            rotation_angle_deg: 45.0,
            text_metrics: TextMetrics::default(),
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

        // Measure label widths using theme-aware metrics. With default
        // metrics this matches the legacy `approximate_text_width`.
        let widths: Vec<f64> = labels.iter().map(|l| measure_text(l, &config.text_metrics)).collect();
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
            let skip_factor = compute_skip_factor(labels, available_width, config.rotation_angle_deg, &config.text_metrics);

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
    #![allow(clippy::unwrap_used)]
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
    metrics: &TextMetrics,
) -> Option<usize> {
    if labels.len() <= 8 {
        return None;
    }
    let label_count = labels.len();
    let available_per_label = available_width / label_count as f64;
    let cos_angle = rotation_angle_deg.to_radians().cos();

    // Use actual average label width for the overlap check (post-rotation
    // horizontal projection) rather than a fixed minimum.
    let widths: Vec<f64> = labels.iter().map(|l| measure_text(l, metrics)).collect();
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
        // Truncation keeps labels readable — fall through to readability
        // thinning check (many truncated labels are still cluttered).
    }

    // Check 2: Readability thinning.
    // Many rotated labels (> 14) look cluttered regardless of overlap state.
    // Thin by 2 to improve visual clarity.
    if label_count > 14 {
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
///
/// Uses the legacy calibration. Call `truncate_label_with_metrics` when the
/// theme has overridden font size, letter spacing, or text transform.
pub fn truncate_label(label: &str, max_width: f64) -> String {
    truncate_label_with_metrics(label, max_width, &TextMetrics::default())
}

/// Truncate a label to fit within `max_width`, adding ellipsis, measuring
/// glyphs under the provided `metrics`.
pub fn truncate_label_with_metrics(label: &str, max_width: f64, metrics: &TextMetrics) -> String {
    let full_width = measure_text(label, metrics);
    if full_width <= max_width {
        return label.to_string();
    }

    let ellipsis_width = measure_text("\u{2026}", metrics);
    let target_width = max_width - ellipsis_width;
    if target_width <= 0.0 {
        return "\u{2026}".to_string();
    }

    // Progressively shrink the label's char prefix until its measured width
    // (with the transform, size, and letter-spacing applied) fits.
    let chars: Vec<(usize, char)> = label.char_indices().collect();
    let mut end_chars = chars.len();
    while end_chars > 0 {
        let end_byte = chars[end_chars - 1].0 + chars[end_chars - 1].1.len_utf8();
        let slice = &label[..end_byte];
        if measure_text(slice, metrics) <= target_width {
            return format!("{}\u{2026}", slice);
        }
        end_chars -= 1;
    }
    "\u{2026}".to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
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

    // ---- TextMetrics / measure_text tests ----

    #[test]
    fn measure_text_default_matches_legacy() {
        // The byte-identity contract: default metrics must reproduce the
        // legacy approximate_text_width exactly for every input.
        let samples = ["", "A", "Hello, world!", "1,234,567", "Category 42", "\u{2026}"];
        for s in samples {
            let legacy = approximate_text_width(s);
            let measured = measure_text(s, &TextMetrics::default());
            assert!((legacy - measured).abs() < f64::EPSILON,
                "measure_text default must equal approximate_text_width for {s:?} (legacy={legacy}, measured={measured})");
        }
    }

    #[test]
    fn measure_text_font_size_scales_linearly() {
        let base = measure_text("Hello", &TextMetrics::default());
        let m = TextMetrics { font_size_px: 24.0, ..TextMetrics::default() };
        // Not legacy-default (font_size != 12.0), so full path runs.
        let big = measure_text("Hello", &m);
        assert!((big - base * 2.0).abs() < 1e-9);
    }

    #[test]
    fn measure_text_uppercase_is_wider() {
        // Even for all-uppercase input, applying the Uppercase transform
        // activates the width-correction boost (1.10×), so the measurement
        // exceeds the raw char-table sum.
        let text = "HELLO";
        let none = measure_text(text, &TextMetrics::default());
        let m = TextMetrics {
            text_transform: crate::theme::TextTransform::Uppercase,
            ..TextMetrics::default()
        };
        let upper = measure_text(text, &m);
        assert!(upper > none,
            "uppercase measurement must exceed default (upper={upper}, none={none})");
    }

    #[test]
    fn measure_text_letter_spacing_adds_space() {
        let m = TextMetrics { letter_spacing_px: 2.0, ..TextMetrics::default() };
        let base = approximate_text_width("Hello");
        let spaced = measure_text("Hello", &m);
        let expected = base + 5.0 * 2.0;
        assert!((spaced - expected).abs() < 1e-9,
            "letter_spacing should add char_count * spacing (expected={expected}, got={spaced})");
    }

    #[test]
    fn measure_text_monospace_is_wider_than_sans() {
        let sans = measure_text("1,234,567", &TextMetrics::default());
        let m = TextMetrics { monospace: true, font_size_px: 12.0, ..TextMetrics::default() };
        let mono = measure_text("1,234,567", &m);
        assert!(mono > sans,
            "monospace should measure wider than the sans calibration (sans={sans}, mono={mono})");
    }

    #[test]
    fn theme_tick_metrics_default_is_legacy() {
        use crate::theme::Theme;
        let t = Theme::default();
        let m = TextMetrics::from_theme_tick_value(&t);
        assert!(m.is_legacy_default(),
            "Theme::default() must produce legacy-default tick metrics for byte-identity");
    }

    #[test]
    fn theme_axis_label_metrics_default_is_legacy() {
        use crate::theme::Theme;
        assert!(TextMetrics::from_theme_axis_label(&Theme::default()).is_legacy_default());
    }

    #[test]
    fn theme_legend_metrics_default_is_legacy() {
        use crate::theme::Theme;
        assert!(TextMetrics::from_theme_legend(&Theme::default()).is_legacy_default());
    }

    #[test]
    fn theme_title_metrics_default_is_legacy() {
        use crate::theme::Theme;
        assert!(TextMetrics::from_theme_title(&Theme::default()).is_legacy_default());
    }

    #[test]
    fn theme_tick_metrics_picks_up_override() {
        use crate::theme::{Theme, TextTransform};
        let t = Theme {
            numeric_font_size: 11.0,
            numeric_font_family: "Geist Mono, monospace".into(),
            label_letter_spacing: 1.2,
            label_text_transform: TextTransform::Uppercase,
            ..Theme::default()
        };
        let m = TextMetrics::from_theme_tick_value(&t);
        assert!(!m.is_legacy_default());
        assert!((m.font_size_px - 11.0).abs() < 1e-6);
        assert!((m.letter_spacing_px - 1.2).abs() < 1e-6);
        assert!(m.monospace);
        assert!(matches!(m.text_transform, TextTransform::Uppercase));
    }

    #[test]
    fn family_is_monospace_recognises_common_stacks() {
        assert!(family_is_monospace("Geist Mono, monospace"));
        assert!(family_is_monospace("'JetBrains Mono', monospace"));
        assert!(family_is_monospace("ui-monospace, Menlo, monospace"));
        assert!(family_is_monospace("Consolas, Courier New, monospace"));
        assert!(!family_is_monospace("system-ui, sans-serif"));
        assert!(!family_is_monospace("Inter, Liberation Sans, Arial, sans-serif"));
    }
}
