//! Server-side ChartML rendering: spec → ChartElement → SVG → PNG.
//!
//! This crate provides the final rendering step for chartml-rs,
//! converting ChartElement trees into static PNG images without
//! requiring a browser, DOM, or JavaScript runtime.
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
pub mod rasterize;
pub mod svg;

pub use error::RenderError;
pub use rasterize::svg_to_png;
pub use svg::element_to_svg;

use chartml_core::ChartML;

/// Default padding in CSS pixels around the chart.
const DEFAULT_PADDING: u32 = 16;

/// White background color.
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
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, WHITE)
}

/// Render a pre-built ChartElement tree to PNG bytes.
///
/// Use this when you already have a ChartElement (e.g. from a custom rendering pipeline).
pub fn element_to_png(
    element: &chartml_core::ChartElement,
    width: u32,
    height: u32,
    density: u32,
) -> Result<Vec<u8>, RenderError> {
    let svg_str = element_to_svg(element, width as f64, height as f64);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, WHITE)
}
