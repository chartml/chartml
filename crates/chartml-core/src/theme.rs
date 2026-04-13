//! Chart theme — colors, typography, and shape defaults for chart chrome.
//!
//! The `Theme` struct provides all chrome properties used by chart renderers:
//! colors (axes, grid, text, background), typography (fonts and sizes for
//! titles, labels, and numeric tick labels), and shape defaults (stroke
//! weights, dot radii, corner radii, grid styling).
//!
//! Plain values are written directly into SVG attributes, ensuring
//! compatibility with every SVG renderer (browsers, resvg, Inkscape, etc.).
//!
//! ## Browser theming
//!
//! When charts are rendered in a browser, a `<style>` block inside the SVG
//! maps element classes to CSS custom properties:
//!
//! ```css
//! .axis-line { stroke: var(--chartml-axis-line) }
//! .grid-line { stroke: var(--chartml-grid) }
//! ```
//!
//! CSS specificity means these override the inline attribute defaults, so
//! consuming apps can set `--chartml-axis-line: #9ca3af` on a parent element
//! and charts respond instantly — no re-render needed.
//!
//! ## Server-side rendering
//!
//! For server-side rendering (e.g. `render_to_png()`), pass a `Theme` that
//! matches your application's current appearance. The same `Theme` used
//! server-side should match the CSS custom properties set browser-side,
//! ensuring visual parity between both rendering paths.
//!
//! ## Example
//!
//! ```rust
//! use chartml_core::theme::Theme;
//!
//! // Light mode (default)
//! let light = Theme::default();
//!
//! // Dark mode
//! let dark = Theme::dark();
//!
//! // Custom theme
//! let custom = Theme {
//!     axis_line: "#9ca3af".into(),
//!     grid: "#374151".into(),
//!     ..Theme::dark()
//! };
//! ```

/// Grid line style — controls which gridlines are drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum GridStyle {
    /// Draw both horizontal and vertical gridlines (current default behavior).
    Both,
    /// Draw only horizontal gridlines.
    HorizontalOnly,
    /// Draw only vertical gridlines.
    VerticalOnly,
    /// Do not draw gridlines.
    None,
}

/// Text transform applied to label text (tick labels, axis labels, legend).
#[derive(Debug, Clone, PartialEq)]
pub enum TextTransform {
    /// No transform — render text as-is.
    None,
    /// Transform to uppercase.
    Uppercase,
    /// Transform to lowercase.
    Lowercase,
}

/// Specification for the zero-line (baseline) overlay on value axes.
#[derive(Debug, Clone, PartialEq)]
pub struct ZeroLineSpec {
    /// Stroke color for the zero line.
    pub color: String,
    /// Stroke width in pixels.
    pub width: f32,
}

/// Chart theme — colors, typography, and shape defaults.
///
/// Color fields are CSS color strings (typically hex like `"#374151"`) that
/// are written directly into SVG `stroke` and `fill` attributes.
///
/// ## Line weight audit (Phase 2)
///
/// Defaults for the stroke-weight fields were chosen by auditing every
/// hardcoded `stroke_width: Some(X.0)` across the renderer crates and
/// categorizing each by role. The audit found:
///
/// - **axis_line_weight = 1.0** — universal across
///   `chartml-chart-cartesian/src/helpers.rs` (lines 419, 684, 756, 866, 957)
///   and `chartml-chart-scatter/src/lib.rs` (lines 237, 242). All axis lines
///   currently use 1.0. (Tick marks also use 1.0 today and reuse this field.)
/// - **grid_line_weight = 1.0** — universal across
///   `chartml-chart-cartesian/src/helpers.rs` (lines 434, 780, 982) and
///   `chartml-chart-scatter/src/lib.rs` (lines 168, 211).
/// - **series_line_weight = 2.0** — majority value in
///   `chartml-chart-cartesian/src/{line.rs:454, 563, 635}`, `area.rs:{212, 343, 481}`,
///   and `bar.rs:1258` (combo line).
///   Outlier: legend line symbols use 2.5 in `bar.rs:1352` and
///   `chartml-core/src/layout/legend.rs:220` — this is a legend-specific glyph
///   weight, not a series weight, and is intentionally not folded in.
///   NOT included: `chartml-chart-pie/src/lib.rs:75` (pie slice border). The
///   pie slice border uses `theme.bg` as its color — it is a background-colored
///   separator gap between slices, not a series mark, and must NOT be wired to
///   `series_line_weight` in later phases.
/// - **annotation_line_weight = 1.0** — annotations currently read their
///   stroke width from the spec (`ann.stroke_width` in `helpers.rs:1348, 1382`);
///   no hardcoded default exists. 1.0 is chosen as the natural fallback for
///   a future "annotation default" path (reference lines, brackets, etc.).
/// - **dot_radius = 5.0** — matches `chartml-chart-scatter/src/lib.rs:106`
///   (default when no size field) and line endpoint markers in
///   `chartml-chart-cartesian/src/line.rs:{466, 577, 649}` and
///   `bar.rs:1268` (combo line dots).
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    // ----- Chrome colors -----
    /// Primary text color (metric values, param controls).
    pub text: String,
    /// Secondary text color (tick labels, axis labels, legend labels).
    pub text_secondary: String,
    /// Strong/emphasized text color (chart titles).
    pub text_strong: String,
    /// Axis line strokes (the main horizontal/vertical axis lines).
    pub axis_line: String,
    /// Tick mark strokes (small marks at each tick position).
    pub tick: String,
    /// Grid line strokes.
    pub grid: String,
    /// Background-aware stroke for element separators
    /// (pie slice borders, dot outlines). Should match the chart background.
    pub bg: String,

    // ----- Typography: title -----
    /// Font family for chart titles.
    pub title_font_family: String,
    /// Font size (px) for chart titles.
    pub title_font_size: f32,
    /// Font weight for chart titles.
    pub title_font_weight: u16,
    /// Font style (`"normal"` / `"italic"`) for chart titles.
    pub title_font_style: String,

    // ----- Typography: labels (tick labels, axis labels, data labels) -----
    /// Font family for tick and axis labels.
    pub label_font_family: String,
    /// Font size (px) for tick and axis labels.
    pub label_font_size: f32,
    /// Font weight for tick and axis labels.
    pub label_font_weight: u16,
    /// Extra letter spacing (px) applied to labels.
    pub label_letter_spacing: f32,
    /// Text transform applied to labels.
    pub label_text_transform: TextTransform,

    // ----- Typography: numeric tick labels -----
    /// Font family for numeric tick labels (e.g. a tabular/monospaced face).
    pub numeric_font_family: String,
    /// Font size (px) for numeric tick labels.
    pub numeric_font_size: f32,

    // ----- Typography: legend -----
    /// Font family for legend labels.
    pub legend_font_family: String,
    /// Font size (px) for legend labels.
    pub legend_font_size: f32,
    /// Font weight for legend labels.
    pub legend_font_weight: u16,

    // ----- Shape / stroke -----
    /// Stroke width for axis lines. See audit in struct-level doc.
    pub axis_line_weight: f32,
    /// Stroke width for grid lines. See audit in struct-level doc.
    pub grid_line_weight: f32,
    /// Stroke width for series marks (line paths, area outlines, combo line).
    /// Pie slice borders are NOT series marks — they use `theme.bg` as a
    /// background-colored separator. See audit in struct-level doc.
    pub series_line_weight: f32,
    /// Stroke width for annotation lines (reference lines, brackets).
    /// See audit in struct-level doc.
    pub annotation_line_weight: f32,
    /// Corner radius (px) for bar rects. When `0.0`, renderers MUST NOT emit
    /// any `rx`/`ry` attribute at all (to preserve byte-identical output).
    pub bar_corner_radius: f32,
    /// Default radius (px) for scatter points and line endpoint markers.
    pub dot_radius: f32,
    /// Optional halo/outline color for dots. When `None`, no halo is drawn.
    pub dot_halo_color: Option<String>,
    /// Halo stroke width (px). Only used when `dot_halo_color` is `Some`.
    pub dot_halo_width: f32,

    // ----- Grid + baseline -----
    /// Which gridlines to draw (both, horizontal-only, vertical-only, none).
    pub grid_style: GridStyle,
    /// Optional emphasized zero line on the value axis. `None` = no zero line.
    pub zero_line: Option<ZeroLineSpec>,
}

impl Default for Theme {
    /// Light mode theme — matches the `chartml.css` light-mode custom properties
    /// and all currently hardcoded renderer defaults.
    fn default() -> Self {
        Self {
            // Chrome colors
            text: "#374151".into(),
            text_secondary: "#6b7280".into(),
            text_strong: "#1f2937".into(),
            axis_line: "#374151".into(),
            tick: "#374151".into(),
            grid: "#e0e0e0".into(),
            bg: "#ffffff".into(),

            // Title typography
            title_font_family: "system-ui, sans-serif".into(),
            title_font_size: 14.0,
            title_font_weight: 700,
            title_font_style: "normal".into(),

            // Label typography
            label_font_family: "system-ui, sans-serif".into(),
            label_font_size: 12.0,
            label_font_weight: 400,
            label_letter_spacing: 0.0,
            label_text_transform: TextTransform::None,

            // Numeric typography
            numeric_font_family: "system-ui, sans-serif".into(),
            numeric_font_size: 12.0,

            // Legend typography
            legend_font_family: "system-ui, sans-serif".into(),
            legend_font_size: 12.0,
            legend_font_weight: 400,

            // Shape / stroke — see struct-level audit
            axis_line_weight: 1.0,
            grid_line_weight: 1.0,
            series_line_weight: 2.0,
            annotation_line_weight: 1.0,
            bar_corner_radius: 0.0,
            dot_radius: 5.0,
            dot_halo_color: None,
            dot_halo_width: 0.0,

            // Grid + baseline
            grid_style: GridStyle::Both,
            zero_line: None,
        }
    }
}

impl Theme {
    /// Dark mode theme — matches the `chartml.css` dark-mode custom properties.
    ///
    /// Typography and shape defaults are identical to `Theme::default()`;
    /// only the chrome color fields differ.
    pub fn dark() -> Self {
        Self {
            text: "#e5e7eb".into(),
            text_secondary: "#9ca3af".into(),
            text_strong: "#f3f4f6".into(),
            axis_line: "#9ca3af".into(),
            tick: "#9ca3af".into(),
            grid: "#374151".into(),
            bg: "#1f2937".into(),
            ..Theme::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_no_css_var_values() {
        let theme = Theme::default();
        let all_values = [
            &theme.text, &theme.text_secondary, &theme.text_strong,
            &theme.axis_line, &theme.tick, &theme.grid, &theme.bg,
        ];
        for value in all_values {
            assert!(
                !value.contains("var("),
                "Theme value should be plain hex, not CSS var(): {value}"
            );
        }
    }

    #[test]
    fn dark_theme_has_no_css_var_values() {
        let theme = Theme::dark();
        let all_values = [
            &theme.text, &theme.text_secondary, &theme.text_strong,
            &theme.axis_line, &theme.tick, &theme.grid, &theme.bg,
        ];
        for value in all_values {
            assert!(
                !value.contains("var("),
                "Dark theme value should be plain hex, not CSS var(): {value}"
            );
        }
    }

    #[test]
    fn theme_fields_are_customizable() {
        let custom = Theme {
            axis_line: "#ff0000".into(),
            ..Theme::dark()
        };
        assert_eq!(custom.axis_line, "#ff0000");
        assert_eq!(custom.grid, "#374151"); // rest from dark
    }

    // ---- Phase 2: new field default tests ----

    #[test]
    fn default_title_typography() {
        let t = Theme::default();
        assert_eq!(t.title_font_family, "system-ui, sans-serif");
        assert_eq!(t.title_font_size, 14.0);
        assert_eq!(t.title_font_weight, 700);
        assert_eq!(t.title_font_style, "normal");
    }

    #[test]
    fn default_label_typography() {
        let t = Theme::default();
        assert_eq!(t.label_font_family, "system-ui, sans-serif");
        assert_eq!(t.label_font_size, 12.0);
        assert_eq!(t.label_font_weight, 400);
        assert_eq!(t.label_letter_spacing, 0.0);
        assert_eq!(t.label_text_transform, TextTransform::None);
    }

    #[test]
    fn default_numeric_typography() {
        let t = Theme::default();
        assert_eq!(t.numeric_font_family, "system-ui, sans-serif");
        assert_eq!(t.numeric_font_size, 12.0);
    }

    #[test]
    fn default_legend_typography() {
        let t = Theme::default();
        assert_eq!(t.legend_font_family, "system-ui, sans-serif");
        assert_eq!(t.legend_font_size, 12.0);
        assert_eq!(t.legend_font_weight, 400);
    }

    #[test]
    fn default_stroke_weights_match_audit() {
        let t = Theme::default();
        assert_eq!(t.axis_line_weight, 1.0);
        assert_eq!(t.grid_line_weight, 1.0);
        assert_eq!(t.series_line_weight, 2.0);
        assert_eq!(t.annotation_line_weight, 1.0);
    }

    #[test]
    fn default_shape_fields() {
        let t = Theme::default();
        assert_eq!(t.bar_corner_radius, 0.0);
        assert_eq!(t.dot_radius, 5.0);
        assert!(t.dot_halo_color.is_none());
        assert_eq!(t.dot_halo_width, 0.0);
    }

    #[test]
    fn default_grid_style_is_both() {
        assert_eq!(Theme::default().grid_style, GridStyle::Both);
    }

    #[test]
    fn default_zero_line_is_none() {
        assert!(Theme::default().zero_line.is_none());
    }

    #[test]
    fn dark_theme_inherits_typography_and_shape_from_default() {
        let d = Theme::default();
        let k = Theme::dark();
        // Typography
        assert_eq!(d.title_font_size, k.title_font_size);
        assert_eq!(d.label_font_weight, k.label_font_weight);
        assert_eq!(d.numeric_font_family, k.numeric_font_family);
        assert_eq!(d.legend_font_family, k.legend_font_family);
        // Shape
        assert_eq!(d.axis_line_weight, k.axis_line_weight);
        assert_eq!(d.grid_line_weight, k.grid_line_weight);
        assert_eq!(d.series_line_weight, k.series_line_weight);
        assert_eq!(d.dot_radius, k.dot_radius);
        assert_eq!(d.bar_corner_radius, k.bar_corner_radius);
        // Grid + baseline
        assert_eq!(d.grid_style, k.grid_style);
        assert_eq!(d.zero_line, k.zero_line);
    }

    #[test]
    fn custom_theme_can_override_new_fields_individually() {
        let custom = Theme {
            series_line_weight: 3.5,
            bar_corner_radius: 4.0,
            dot_halo_color: Some("#ffffff".into()),
            dot_halo_width: 2.0,
            grid_style: GridStyle::HorizontalOnly,
            zero_line: Some(ZeroLineSpec { color: "#000000".into(), width: 1.5 }),
            label_text_transform: TextTransform::Uppercase,
            ..Theme::default()
        };
        // Overridden
        assert_eq!(custom.series_line_weight, 3.5);
        assert_eq!(custom.bar_corner_radius, 4.0);
        assert_eq!(custom.dot_halo_color.as_deref(), Some("#ffffff"));
        assert_eq!(custom.dot_halo_width, 2.0);
        assert_eq!(custom.grid_style, GridStyle::HorizontalOnly);
        assert_eq!(
            custom.zero_line,
            Some(ZeroLineSpec { color: "#000000".into(), width: 1.5 })
        );
        assert_eq!(custom.label_text_transform, TextTransform::Uppercase);
        // Not overridden — should match Default
        assert_eq!(custom.axis_line_weight, 1.0);
        assert_eq!(custom.dot_radius, 5.0);
        assert_eq!(custom.title_font_size, 14.0);
        assert_eq!(custom.axis_line, "#374151");
    }
}
