/// Generates SVG path `d` strings for area charts.
/// Equivalent to D3's `d3.area()`.

use super::line::{CurveType, LineGenerator};

/// Generates SVG path `d` strings for area charts.
pub struct AreaGenerator {
    curve: CurveType,
}

impl AreaGenerator {
    /// Create a new AreaGenerator with Linear curve type.
    pub fn new() -> Self {
        Self {
            curve: CurveType::Linear,
        }
    }

    /// Set the curve interpolation type.
    pub fn curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
        self
    }

    /// Generate an SVG path from a series of (x, y0, y1) points.
    ///
    /// The area is formed by:
    /// 1. Forward path along (x, y1) -- the top line
    /// 2. Reverse path along (x, y0) -- the bottom line (reversed)
    /// 3. Close with "Z"
    ///
    /// For non-stacked areas, y0 is typically the baseline (e.g., the bottom of the chart).
    pub fn generate(&self, points: &[(f64, f64, f64)]) -> String {
        if points.is_empty() {
            return String::new();
        }

        // Top line: (x, y1)
        let top_points: Vec<(f64, f64)> = points.iter().map(|&(x, _y0, y1)| (x, y1)).collect();
        // Bottom line reversed: (x, y0)
        let bottom_points: Vec<(f64, f64)> = points.iter().rev().map(|&(x, y0, _y1)| (x, y0)).collect();

        let line_gen = LineGenerator::new().curve(self.curve);
        let top_path = line_gen.generate(&top_points);
        let bottom_path = line_gen.generate(&bottom_points);

        // The bottom path starts with "M..." but we need "L..." to connect from the top path.
        let bottom_continuation = if bottom_path.len() > 1 {
            format!("L{}", &bottom_path[1..])
        } else {
            String::new()
        };

        format!("{}{}Z", top_path, bottom_continuation)
    }
}

impl Default for AreaGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_basic() {
        let gen = AreaGenerator::new();
        let path = gen.generate(&[
            (0.0, 100.0, 10.0),
            (50.0, 100.0, 20.0),
            (100.0, 100.0, 5.0),
        ]);
        assert!(path.starts_with("M"), "Path should start with M, got: {}", path);
        assert!(path.contains("L"), "Path should contain L commands, got: {}", path);
        assert!(path.ends_with("Z"), "Path should end with Z, got: {}", path);
    }

    #[test]
    fn area_empty() {
        let gen = AreaGenerator::new();
        let path = gen.generate(&[]);
        assert_eq!(path, "");
    }
}
