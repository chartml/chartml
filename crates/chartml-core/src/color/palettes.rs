/// autumn_forest -- Default palette. Warm earth tones with strategic contrast.
pub const AUTUMN_FOREST: [&str; 12] = [
    "#2E7D9A", // Ocean
    "#D4A445", // Amber
    "#4A7C59", // Forest
    "#D66B5B", // Coral
    "#8B6BA8", // Violet
    "#9BB85A", // Chartreuse
    "#A85A6B", // Burgundy
    "#5A6BA8", // Indigo
    "#B87D5A", // Sienna
    "#5A9B9B", // Teal
    "#759B75", // Sage
    "#A8758B", // Mauve
];

/// spectrum_pro -- Warmer tones, Tableau-inspired.
pub const SPECTRUM_PRO: [&str; 12] = [
    "#4285F4", // Azure
    "#E8710A", // Tangerine
    "#34A853", // Seafoam
    "#DC3545", // Crimson
    "#9B59B6", // Orchid
    "#F1C40F", // Marigold
    "#607D8B", // Steel
    "#00897B", // Jade
    "#8B0000", // Burgundy
    "#6C63FF", // Periwinkle
    "#8BC34A", // Chartreuse
    "#5C6BC0", // Slate Blue
];

/// horizon_suite -- Deeper saturation, Looker-inspired.
pub const HORIZON_SUITE: [&str; 12] = [
    "#1A73E8", // Cobalt
    "#0D652D", // Emerald
    "#E8710A", // Sunset
    "#9334E6", // Lavender
    "#F9AB00", // Gold
    "#0097A7", // Teal
    "#C2185B", // Berry
    "#558B2F", // Moss
    "#FF7043", // Peach
    "#3F51B5", // Indigo
    "#2E7D32", // Pine
    "#D81B60", // Rose
];

/// Get a palette by name. Returns the default (autumn_forest) for unknown names.
pub fn get_palette(name: &str) -> &'static [&'static str; 12] {
    match name {
        "autumn_forest" => &AUTUMN_FOREST,
        "spectrum_pro" => &SPECTRUM_PRO,
        "horizon_suite" => &HORIZON_SUITE,
        _ => &AUTUMN_FOREST,
    }
}

/// List of available palette names.
pub fn palette_names() -> Vec<&'static str> {
    vec!["autumn_forest", "spectrum_pro", "horizon_suite"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::utils::is_valid_hex_color;

    #[test]
    fn get_default_palette() {
        let palette = get_palette("autumn_forest");
        assert_eq!(palette.len(), 12);
    }

    #[test]
    fn get_palette_by_name() {
        let af = get_palette("autumn_forest");
        assert_eq!(af[0], "#2E7D9A");

        let sp = get_palette("spectrum_pro");
        assert_eq!(sp[0], "#4285F4");

        let hs = get_palette("horizon_suite");
        assert_eq!(hs[0], "#1A73E8");
    }

    #[test]
    fn palette_colors_are_valid_hex() {
        for name in palette_names() {
            let palette = get_palette(name);
            for color in palette.iter() {
                assert!(
                    is_valid_hex_color(color),
                    "Invalid hex color '{}' in palette '{}'",
                    color,
                    name
                );
            }
        }
    }
}
