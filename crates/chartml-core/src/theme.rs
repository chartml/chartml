//! Default theme colors using CSS custom properties with light-mode fallbacks.
//!
//! All chart renderers use these constants for axis text, lines, ticks, and grids.
//! In a browser DOM, the CSS variables resolve from the consumer's stylesheet.
//! In static SVG or PNG output, the fallback values apply.

/// Tick label and axis label text color.
pub const TEXT_SECONDARY: &str = "var(--chartml-text-secondary, #6b7280)";

/// Axis line color (the main horizontal/vertical axis strokes).
pub const AXIS_LINE: &str = "var(--chartml-axis-line, #374151)";

/// Tick mark color (the small marks at each tick position).
pub const TICK: &str = "var(--chartml-axis-line, #374151)";

/// Grid line color.
pub const GRID: &str = "var(--chartml-grid, #e0e0e0)";

/// Background-aware stroke for element separators (pie slice borders, dot outlines).
/// Matches the chart background so borders blend rather than standing out.
pub const BG_STROKE: &str = "var(--chartml-bg, #ffffff)";
