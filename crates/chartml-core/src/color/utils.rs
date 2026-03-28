/// Validate hex color format (#RRGGBB).
pub fn is_valid_hex_color(hex: &str) -> bool {
    if hex.len() != 7 {
        return false;
    }
    if !hex.starts_with('#') {
        return false;
    }
    hex[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse a hex color string (#RRGGBB) into RGB components.
fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    if !is_valid_hex_color(hex) {
        return None;
    }
    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;
    Some((r, g, b))
}

/// Convert RGB to HSL. Returns (h, s, l) where all values are in [0, 1].
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r_norm = r as f64 / 255.0;
    let g_norm = g as f64 / 255.0;
    let b_norm = b as f64 / 255.0;

    let max = r_norm.max(g_norm).max(b_norm);
    let min = r_norm.min(g_norm).min(b_norm);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r_norm).abs() < f64::EPSILON {
        let offset = if g_norm < b_norm { 6.0 } else { 0.0 };
        ((g_norm - b_norm) / d + offset) / 6.0
    } else if (max - g_norm).abs() < f64::EPSILON {
        ((b_norm - r_norm) / d + 2.0) / 6.0
    } else {
        ((r_norm - g_norm) / d + 4.0) / 6.0
    };

    (h, s, l)
}

/// Helper for HSL to RGB conversion.
fn hue2rgb(p: f64, q: f64, t: f64) -> f64 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Convert HSL to RGB. h, s, l are all in [0, 1].
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s.abs() < f64::EPSILON {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let r = (hue2rgb(p, q, h + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue2rgb(p, q, h) * 255.0).round() as u8;
    let b = (hue2rgb(p, q, h - 1.0 / 3.0) * 255.0).round() as u8;

    (r, g, b)
}

/// Convert RGB components to a hex color string (#RRGGBB).
fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Generate a desaturated fallback color from a hex color.
///
/// Algorithm:
/// 1. Parse hex to RGB
/// 2. Convert RGB to HSL
/// 3. Reduce saturation by 40% (S *= 0.6)
/// 4. Shift luminosity toward mid-range (L = L * 0.7 + 0.15)
/// 5. Convert back to RGB
/// 6. Return as hex string
pub fn generate_fallback_color(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex).unwrap_or((128, 128, 128));
    let (h, s, l) = rgb_to_hsl(r, g, b);

    let s_new = s * 0.6;
    let l_new = l * 0.7 + 0.15;

    let (r2, g2, b2) = hsl_to_rgb(h, s_new, l_new);
    rgb_to_hex(r2, g2, b2)
}

/// Generate 12 fallback colors from a base palette.
pub fn generate_fallback_colors(base_colors: &[&str]) -> Vec<String> {
    base_colors.iter().map(|c| generate_fallback_color(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_rgb_valid() {
        assert_eq!(hex_to_rgb("#2E7D9A"), Some((46, 125, 154)));
    }

    #[test]
    fn hex_to_rgb_invalid() {
        assert_eq!(hex_to_rgb("invalid"), None);
        assert_eq!(hex_to_rgb("#GGG"), None);
        assert_eq!(hex_to_rgb(""), None);
    }

    #[test]
    fn rgb_to_hex_roundtrip() {
        let original = "#2E7D9A";
        let (r, g, b) = hex_to_rgb(original).unwrap();
        let result = rgb_to_hex(r, g, b);
        assert_eq!(result, original);
    }

    #[test]
    fn fallback_color_is_desaturated() {
        let original = "#2E7D9A";
        let fallback = generate_fallback_color(original);
        assert_ne!(fallback, original);
    }

    #[test]
    fn fallback_color_is_valid_hex() {
        let fallback = generate_fallback_color("#D4A445");
        assert!(is_valid_hex_color(&fallback));
    }

    #[test]
    fn is_valid_hex_color_valid() {
        assert!(is_valid_hex_color("#2E7D9A"));
        assert!(is_valid_hex_color("#000000"));
        assert!(is_valid_hex_color("#FFFFFF"));
        assert!(is_valid_hex_color("#abcdef"));
    }

    #[test]
    fn is_valid_hex_color_invalid() {
        assert!(!is_valid_hex_color(""));
        assert!(!is_valid_hex_color("2E7D9A"));
        assert!(!is_valid_hex_color("#2E7D9"));
        assert!(!is_valid_hex_color("#GGGGGG"));
        assert!(!is_valid_hex_color("#2E7D9A00"));
    }

    #[test]
    fn generate_fallback_colors_count() {
        let base = &[
            "#2E7D9A", "#D4A445", "#4A7C59", "#D66B5B",
            "#8B6BA8", "#9BB85A", "#A85A6B", "#5A6BA8",
            "#B87D5A", "#5A9B9B", "#759B75", "#A8758B",
        ];
        let fallbacks = generate_fallback_colors(base);
        assert_eq!(fallbacks.len(), 12);
        for fb in &fallbacks {
            assert!(is_valid_hex_color(fb));
        }
    }

    #[test]
    fn hsl_roundtrip_primary_colors() {
        // Pure red
        let (h, s, l) = rgb_to_hsl(255, 0, 0);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert_eq!((r, g, b), (255, 0, 0));

        // Pure green
        let (h, s, l) = rgb_to_hsl(0, 128, 0);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert_eq!((r, g, b), (0, 128, 0));

        // Grey (achromatic)
        let (h, s, l) = rgb_to_hsl(128, 128, 128);
        assert!((h - 0.0).abs() < f64::EPSILON);
        assert!((s - 0.0).abs() < f64::EPSILON);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert_eq!((r, g, b), (128, 128, 128));
    }
}
