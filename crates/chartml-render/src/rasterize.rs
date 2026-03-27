//! SVG string → PNG bytes rasterization using resvg.
//!
//! Uses resvg (pure Rust) for SVG parsing and rasterization.
//! Supports density/DPI scaling, white background, and configurable padding.

use crate::error::RenderError;
use std::sync::{Arc, LazyLock};

/// Shared font database — loaded once, reused across all renders.
static FONT_DB: LazyLock<Arc<fontdb::Database>> = LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    // Load system fonts (Liberation Sans, DejaVu, etc.)
    db.load_system_fonts();
    Arc::new(db)
});

/// Rasterize an SVG string to PNG bytes.
///
/// # Arguments
/// * `svg` — SVG XML string
/// * `width` — target width in CSS pixels
/// * `height` — target height in CSS pixels
/// * `density` — DPI scale factor (72 = 1x, 144 = 2x for crisp PDF output)
/// * `padding` — padding in CSS pixels around the chart
/// * `background` — RGB background color (e.g. `[255, 255, 255]` for white)
pub fn svg_to_png(
    svg: &str,
    width: u32,
    height: u32,
    density: u32,
    padding: u32,
    background: [u8; 3],
) -> Result<Vec<u8>, RenderError> {
    let scale = density as f32 / 72.0;

    // Parse SVG with font database
    let options = usvg::Options {
        font_family: "Inter, Liberation Sans, Arial, sans-serif".to_string(),
        font_size: 12.0,
        dpi: density as f32,
        fontdb: FONT_DB.clone(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|e| RenderError::SvgParse(e.to_string()))?;

    // Calculate output dimensions with padding
    let total_width = ((width + 2 * padding) as f32 * scale).ceil() as u32;
    let total_height = ((height + 2 * padding) as f32 * scale).ceil() as u32;

    // Create pixel buffer with background
    let mut pixmap = tiny_skia::Pixmap::new(total_width, total_height)
        .ok_or_else(|| RenderError::Rasterize("Failed to create pixmap".into()))?;

    // Fill with background color
    let bg = tiny_skia::Color::from_rgba8(background[0], background[1], background[2], 255);
    pixmap.fill(bg);

    // Render SVG into the pixmap with padding offset and scale
    let padding_px = padding as f32 * scale;
    let chart_width = width as f32 * scale;
    let chart_height = height as f32 * scale;

    // Scale the SVG to fit within the chart area (excluding padding)
    let svg_size = tree.size();
    let scale_x = chart_width / svg_size.width();
    let scale_y = chart_height / svg_size.height();

    let transform = tiny_skia::Transform::from_translate(padding_px, padding_px)
        .post_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Encode to PNG
    let png_data = pixmap.encode_png()
        .map_err(|e| RenderError::PngEncode(e.to_string()))?;

    Ok(png_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_svg() -> String {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100">"#,
            r#"<rect x="10" y="10" width="180" height="80" fill="steelblue"/>"#,
            r#"</svg>"#,
        ).to_string()
    }

    fn circle_svg() -> String {
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">"#,
            r#"<circle cx="50" cy="50" r="40" fill="red"/>"#,
            r#"</svg>"#,
        ).to_string()
    }

    #[test]
    fn renders_simple_svg_to_png() {
        let svg = simple_svg();
        let result = svg_to_png(&svg, 200, 100, 72, 16, [255, 255, 255]);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
        let png = result.unwrap();
        // PNG magic bytes
        assert_eq!(&png[0..4], &[0x89, 0x50, 0x4E, 0x47]);
        assert!(png.len() > 100);
    }

    #[test]
    fn density_2x_produces_larger_output() {
        let svg = circle_svg();
        let png_1x = svg_to_png(&svg, 100, 100, 72, 0, [255, 255, 255]).unwrap();
        let png_2x = svg_to_png(&svg, 100, 100, 144, 0, [255, 255, 255]).unwrap();
        assert!(png_2x.len() > png_1x.len());
    }

    #[test]
    fn padding_increases_dimensions() {
        let svg = circle_svg();
        let no_pad = svg_to_png(&svg, 100, 100, 72, 0, [255, 255, 255]).unwrap();
        let with_pad = svg_to_png(&svg, 100, 100, 72, 16, [255, 255, 255]).unwrap();
        assert!(with_pad.len() > no_pad.len());
    }

    #[test]
    fn invalid_svg_returns_error() {
        let result = svg_to_png("not valid svg", 100, 100, 72, 0, [255, 255, 255]);
        assert!(result.is_err());
    }

    #[test]
    fn svg_with_text_renders() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100">"#,
            r#"<text x="100" y="50" text-anchor="middle" font-family="Liberation Sans, Arial, sans-serif" font-size="16">Hello World</text>"#,
            r#"</svg>"#,
        );
        let result = svg_to_png(svg, 200, 100, 72, 0, [255, 255, 255]);
        assert!(result.is_ok(), "text render failed: {:?}", result.err());
    }
}
