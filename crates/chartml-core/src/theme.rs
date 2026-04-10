//! Chart theme — colors for axes, grid lines, text, and backgrounds.
//!
//! The `Theme` struct provides all chrome colors used by chart renderers.
//! Plain hex values are written directly into SVG attributes, ensuring
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

/// Chart theme colors.
///
/// All fields are CSS color strings (typically hex like `"#374151"`).
/// These values are written directly into SVG `stroke` and `fill` attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
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
}

impl Default for Theme {
    /// Light mode theme — matches the `chartml.css` light-mode custom properties.
    fn default() -> Self {
        Self {
            text: "#374151".into(),
            text_secondary: "#6b7280".into(),
            text_strong: "#1f2937".into(),
            axis_line: "#374151".into(),
            tick: "#374151".into(),
            grid: "#e0e0e0".into(),
            bg: "#ffffff".into(),
        }
    }
}

impl Theme {
    /// Dark mode theme — matches the `chartml.css` dark-mode custom properties.
    pub fn dark() -> Self {
        Self {
            text: "#e5e7eb".into(),
            text_secondary: "#9ca3af".into(),
            text_strong: "#f3f4f6".into(),
            axis_line: "#9ca3af".into(),
            tick: "#9ca3af".into(),
            grid: "#374151".into(),
            bg: "#1f2937".into(),
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
}
