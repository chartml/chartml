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
    render_to_png_with_background(chartml, yaml, width, height, density, WHITE)
}

/// Render a ChartML YAML spec to PNG bytes (synchronous) with an explicit
/// canvas background color.
///
/// Identical to [`render_to_png`] except the pixmap is filled with
/// `background` instead of white. Use this when the PNG will be placed on
/// a non-white surface (e.g. a dark-mode email card) — the theme set on
/// `chartml` should use matching colors or chart text will be illegible.
///
/// # Arguments
/// * `chartml` — configured ChartML instance with renderers registered
/// * `yaml` — ChartML YAML specification string
/// * `width` — chart width in CSS pixels
/// * `height` — chart height in CSS pixels
/// * `density` — DPI (72 = 1x, 144 = 2x for PDF)
/// * `background` — canvas fill color as `[r, g, b]`
#[cfg(feature = "rasterize")]
pub fn render_to_png_with_background(
    chartml: &ChartML,
    yaml: &str,
    width: u32,
    height: u32,
    density: u32,
    background: [u8; 3],
) -> Result<Vec<u8>, RenderError> {
    let element = chartml.render_from_yaml_with_size(
        yaml,
        Some(width as f64),
        Some(height as f64),
    )?;

    let svg_str = element_to_svg(&element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, background)
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
    render_to_png_with_background_async(chartml, yaml, width, height, density, WHITE).await
}

/// Render a ChartML YAML spec to PNG bytes (async) with an explicit canvas
/// background color.
///
/// Identical to [`render_to_png_async`] except the pixmap is filled with
/// `background` instead of white. See [`render_to_png_with_background`].
///
/// # Arguments
/// * `chartml` — configured ChartML instance with renderers and transform middleware registered
/// * `yaml` — ChartML YAML specification string
/// * `width` — chart width in CSS pixels
/// * `height` — chart height in CSS pixels
/// * `density` — DPI (72 = 1x, 144 = 2x for PDF)
/// * `background` — canvas fill color as `[r, g, b]`
#[cfg(feature = "rasterize")]
pub async fn render_to_png_with_background_async(
    chartml: &ChartML,
    yaml: &str,
    width: u32,
    height: u32,
    density: u32,
    background: [u8; 3],
) -> Result<Vec<u8>, RenderError> {
    let element = chartml.render_from_yaml_with_params_async(
        yaml,
        Some(width as f64),
        Some(height as f64),
        None,
    ).await?;

    let svg_str = element_to_svg(&element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, background)
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
    element_to_png_with_background(element, width, height, density, WHITE)
}

/// Render a pre-built ChartElement tree to PNG bytes with an explicit
/// canvas background color. See [`render_to_png_with_background`].
#[cfg(feature = "rasterize")]
pub fn element_to_png_with_background(
    element: &chartml_core::ChartElement,
    width: u32,
    height: u32,
    density: u32,
    background: [u8; 3],
) -> Result<Vec<u8>, RenderError> {
    let svg_str = element_to_svg(element, width as f64, height as f64);
    let svg_str = strip_dashoffset_for_static(&svg_str);
    svg_to_png(&svg_str, width, height, density, DEFAULT_PADDING, background)
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

    #[cfg(feature = "rasterize")]
    #[test]
    fn element_to_png_with_background_fills_canvas() {
        let element = chartml_core::ChartElement::Svg {
            viewbox: chartml_core::element::ViewBox {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            width: Some(100.0),
            height: Some(50.0),
            class: String::new(),
            children: vec![],
        };

        let bg = [36u8, 32, 30]; // #24201E — a dark surface
        let png_bytes =
            element_to_png_with_background(&element, 100, 50, 72, bg).expect("render succeeds");

        let decoder = png::Decoder::new(&png_bytes[..]);
        let mut reader = decoder.read_info().expect("valid PNG");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decodable frame");

        // Canvas = chart size + DEFAULT_PADDING on each side at 1x density.
        assert_eq!(info.width, 100 + 2 * DEFAULT_PADDING);
        assert_eq!(info.height, 50 + 2 * DEFAULT_PADDING);

        // The empty chart draws nothing, so every pixel is the background.
        // Check the first and last pixels (RGBA output).
        assert_eq!(&buf[0..3], &bg);
        let last = buf.len() - 4;
        assert_eq!(&buf[last..last + 3], &bg);
    }

    #[cfg(feature = "rasterize")]
    #[test]
    fn element_to_png_defaults_to_white_background() {
        let element = chartml_core::ChartElement::Svg {
            viewbox: chartml_core::element::ViewBox {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            width: Some(100.0),
            height: Some(50.0),
            class: String::new(),
            children: vec![],
        };

        let png_bytes = element_to_png(&element, 100, 50, 72).expect("render succeeds");

        let decoder = png::Decoder::new(&png_bytes[..]);
        let mut reader = decoder.read_info().expect("valid PNG");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        reader.next_frame(&mut buf).expect("decodable frame");

        assert_eq!(&buf[0..3], &WHITE);
    }
}
