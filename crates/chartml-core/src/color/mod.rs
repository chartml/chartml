pub mod palettes;
pub mod utils;

pub use palettes::{get_palette, palette_names, AUTUMN_FOREST, HORIZON_SUITE, SPECTRUM_PRO};
pub use utils::{generate_fallback_color, generate_fallback_colors, is_valid_hex_color};

/// Get colors for a chart based on series count.
///
/// - 1-12 series: base palette colors
/// - 13-24 series: base + desaturated fallbacks
/// - 25+ series: cycle through combined 24-color palette
pub fn get_chart_colors(series_count: usize, base_palette: &[&str]) -> Vec<String> {
    let fallbacks = generate_fallback_colors(base_palette);

    // Build combined palette: base colors + fallback colors
    let combined: Vec<String> = base_palette
        .iter()
        .map(|c| c.to_string())
        .chain(fallbacks)
        .collect();

    let combined_len = combined.len();

    (0..series_count)
        .map(|i| combined[i % combined_len].clone())
        .collect()
}

/// Get a color by index from a palette with fallback support.
pub fn get_color_at_index(index: usize, base_palette: &[&str]) -> String {
    let fallbacks = generate_fallback_colors(base_palette);

    let combined: Vec<String> = base_palette
        .iter()
        .map(|c| c.to_string())
        .chain(fallbacks)
        .collect();

    let combined_len = combined.len();
    combined[index % combined_len].clone()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn chart_colors_basic() {
        let colors = get_chart_colors(5, &AUTUMN_FOREST);
        assert_eq!(colors.len(), 5);
        assert_eq!(colors[0], AUTUMN_FOREST[0]);
        assert_eq!(colors[4], AUTUMN_FOREST[4]);
    }

    #[test]
    fn chart_colors_12() {
        let colors = get_chart_colors(12, &AUTUMN_FOREST);
        assert_eq!(colors.len(), 12);
        for (i, color) in colors.iter().enumerate() {
            assert_eq!(color, AUTUMN_FOREST[i]);
        }
    }

    #[test]
    fn chart_colors_13_plus() {
        let colors = get_chart_colors(13, &AUTUMN_FOREST);
        assert_eq!(colors.len(), 13);
        // First 12 are base palette
        for i in 0..12 {
            assert_eq!(colors[i], AUTUMN_FOREST[i]);
        }
        // 13th is a fallback color (desaturated version of first base color)
        assert_ne!(colors[12], AUTUMN_FOREST[0]);
        assert!(is_valid_hex_color(&colors[12]));
    }

    #[test]
    fn chart_colors_25_plus() {
        let colors = get_chart_colors(30, &AUTUMN_FOREST);
        assert_eq!(colors.len(), 30);
        // All colors should be valid hex
        for color in &colors {
            assert!(is_valid_hex_color(color));
        }
        // Color at index 24 should cycle back (24 % 24 == 0)
        assert_eq!(colors[24], colors[0]);
    }

    #[test]
    fn get_color_at_index_basic() {
        let color = get_color_at_index(0, &AUTUMN_FOREST);
        assert_eq!(color, AUTUMN_FOREST[0]);
    }

    #[test]
    fn get_color_at_index_fallback() {
        let color = get_color_at_index(15, &AUTUMN_FOREST);
        assert!(is_valid_hex_color(&color));
        // Index 15 is in the fallback range (12-23), should differ from base palette index 3
        assert_ne!(color, AUTUMN_FOREST[15 % 12]);
    }
}
