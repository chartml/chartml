//! Server-side ChartML rendering: spec → ChartElement → SVG → PNG.
//!
//! This crate provides the final rendering step for chartml-rs,
//! converting ChartElement trees into static PNG images without
//! requiring a browser, DOM, or JavaScript runtime.
//!
//! # Features
//!
//! - **SVG serialization** (always available): converts `ChartElement` trees to SVG strings
//! - **PNG rasterization** (requires `rasterize` feature, enabled by default): converts SVG to PNG
//!
//! # Usage
//!
//! ```rust,no_run
//! use chartml_core::ChartML;
//! use chartml_render::render_to_png;
//!
//! let chartml = ChartML::new();
//! // ... register renderers ...
//!
//! let yaml = r#"
//! type: chart
//! version: 1
//! data:
//!   provider: inline
//!   rows:
//!     - { x: "A", y: 10 }
//!     - { x: "B", y: 20 }
//! visualize:
//!   type: bar
//!   columns: x
//!   rows: y
//! "#;
//!
//! let png_bytes = render_to_png(&chartml, yaml, 800, 400, 72).unwrap();
//! ```

pub mod error;
#[cfg(feature = "rasterize")]
pub mod rasterize;
pub mod svg;

pub use error::RenderError;
#[cfg(feature = "rasterize")]
pub use rasterize::{init_font_database, svg_to_png};
pub use svg::element_to_svg;

#[cfg(feature = "rasterize")]
use chartml_core::ChartML;

/// Default padding in CSS pixels around the chart.
#[cfg(feature = "rasterize")]
const DEFAULT_PADDING: u32 = 16;

/// Strip `stroke-dashoffset` attributes from an SVG string before static
/// rasterization.
///
/// Line charts set both `stroke-dasharray` and `stroke-dashoffset` to the
/// path length for the CSS draw animation — a browser animates the offset
/// to 0, progressively revealing the stroke. In a static rasterizer (resvg)
/// the animation never runs, leaving every line fully offset and invisible;
/// only the circle dot markers remain.
///
/// Removing `stroke-dashoffset` leaves `stroke-dasharray` set to a single
/// number ≥ the path length, which SVG renders as one unbroken dash — a
/// fully visible solid line. Legitimately dashed lines (`"8 4"` etc.) never
/// set `stroke-dashoffset` in chartml, so they are unaffected.
#[cfg(feature = "rasterize")]
fn strip_dashoffset_for_static(svg: &str) -> String {
    const ATTR: &str = " stroke-dashoffset=\"";
    if !svg.contains(ATTR) {
        return svg.to_owned();
    }
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(pos) = rest.find(ATTR) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + ATTR.len()..];
        match after.find('"') {
            Some(end) => rest = &after[end + 1..],
            None => {
                rest = after;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// White background color.
#[cfg(feature = "rasterize")]
const WHITE: [u8; 3] = [255, 255, 255];

/// Render a ChartML YAML spec to PNG bytes (synchronous).
///
/// Runs the full pipeline: parse YAML → render ChartElement → SVG → PNG.
/// Use this for specs with inline data and no async transforms (sql/forecast).
///
/// # Arguments
/// * `chartml` — configured ChartML instance with renderers registered
/// * `yaml` — ChartML YAML specification string
/// * `width` — chart width in CSS pixels
/// * `height` — chart height in CSS pixels
/// * `density` — DPI (72 = 1x, 144 = 2x for PDF)
#[cfg(feature = "rasterize")]
pub fn render_to_png(
    chartml: &ChartML,
    yaml: &str,
    width: u32,
    height: u32,
    density: u32,
) -> Result<Vec<u8>, RenderError> {
    let element = chartml.render_from_yaml_with_size(
        yaml,
        Some(width as f64),
        Some(height as f64),
    )?;

    let svg_str = element_to_svg(&element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, WHITE)
}

/// Render a ChartML YAML spec to PNG bytes (async).
///
/// Runs the full pipeline: parse YAML → transform (DataFusion) → render → SVG → PNG.
/// Use this for specs that require async transforms (sql, aggregate, forecast).
///
/// # Arguments
/// * `chartml` — configured ChartML instance with renderers and transform middleware registered
/// * `yaml` — ChartML YAML specification string
/// * `width` — chart width in CSS pixels
/// * `height` — chart height in CSS pixels
/// * `density` — DPI (72 = 1x, 144 = 2x for PDF)
#[cfg(feature = "rasterize")]
pub async fn render_to_png_async(
    chartml: &ChartML,
    yaml: &str,
    width: u32,
    height: u32,
    density: u32,
) -> Result<Vec<u8>, RenderError> {
    let element = chartml.render_from_yaml_with_params_async(
        yaml,
        Some(width as f64),
        Some(height as f64),
        None,
    ).await?;

    let svg_str = element_to_svg(&element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, WHITE)
}

/// Render a pre-built ChartElement tree to PNG bytes.
///
/// Use this when you already have a ChartElement (e.g. from a custom rendering pipeline).
#[cfg(feature = "rasterize")]
pub fn element_to_png(
    element: &chartml_core::ChartElement,
    width: u32,
    height: u32,
    density: u32,
) -> Result<Vec<u8>, RenderError> {
    let svg_str = element_to_svg(element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, WHITE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_dashoffset_removes_animation_attrs() {
        let svg = r##"<path d="M0,0L100,50" stroke="#D97706" stroke-width="2" stroke-dasharray="112" stroke-dashoffset="112" class="series-line"/>"##;
        let result = strip_dashoffset_for_static(svg);
        assert!(!result.contains("stroke-dashoffset"));
        assert!(result.contains(r#"stroke-dasharray="112""#));
        assert!(result.contains(r##"stroke="#D97706""##));
    }

    #[test]
    fn strip_dashoffset_preserves_dashed_lines() {
        let svg = r#"<path d="M0,0L100,50" stroke-dasharray="8 4" class="dashed"/>"#;
        let result = strip_dashoffset_for_static(svg);
        assert_eq!(result, svg);
    }

    #[test]
    fn strip_dashoffset_handles_multiple_paths() {
        let svg = r#"<path stroke-dashoffset="200"/><path stroke-dashoffset="300"/>"#;
        let result = strip_dashoffset_for_static(svg);
        assert!(!result.contains("stroke-dashoffset"));
        assert_eq!(result, r#"<path/><path/>"#);
    }

    #[test]
    fn strip_dashoffset_no_op_without_attr() {
        let svg = r#"<circle cx="50" cy="50" r="4" fill="red"/>"#;
        let result = strip_dashoffset_for_static(svg);
        assert_eq!(result, svg);
    }
}
