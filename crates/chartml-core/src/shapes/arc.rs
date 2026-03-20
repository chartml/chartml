/// Generates SVG path `d` strings for pie/doughnut slices.
/// Equivalent to D3's `d3.arc()`.

use super::line::fmt;
use std::f64::consts::PI;

/// Generates SVG path `d` strings for arc segments (pie/doughnut slices).
pub struct ArcGenerator {
    inner_radius: f64,
    outer_radius: f64,
    corner_radius: f64,
}

impl ArcGenerator {
    /// Create a new ArcGenerator.
    /// Use inner_radius=0 for pie charts, >0 for doughnut charts.
    pub fn new(inner_radius: f64, outer_radius: f64) -> Self {
        Self {
            inner_radius,
            outer_radius,
            corner_radius: 0.0,
        }
    }

    /// Set the corner radius for rounded corners (not yet implemented, stored for future use).
    pub fn corner_radius(mut self, r: f64) -> Self {
        self.corner_radius = r;
        self
    }

    /// Generate an SVG path for an arc from start_angle to end_angle.
    ///
    /// Angles are in radians, 0 = 12 o'clock, clockwise.
    /// The arc is centered at the origin.
    pub fn generate(&self, start_angle: f64, end_angle: f64) -> String {
        let delta = (end_angle - start_angle).abs();

        // Handle near-full circle: use two half-arcs to avoid SVG arc rendering issues
        if delta >= 2.0 * PI - 1e-6 {
            return self.generate_full_circle(start_angle);
        }

        let outer_start = angle_to_point(start_angle, self.outer_radius);
        let outer_end = angle_to_point(end_angle, self.outer_radius);
        let large_arc = if delta > PI { 1 } else { 0 };

        if self.inner_radius > 0.0 {
            // Doughnut segment
            let inner_start = angle_to_point(start_angle, self.inner_radius);
            let inner_end = angle_to_point(end_angle, self.inner_radius);
            // Counter-sweep for inner arc (going back)
            let inner_large_arc = large_arc;

            format!(
                "M{},{} A{},{} 0 {},1 {},{} L{},{} A{},{} 0 {},0 {},{} Z",
                fmt(outer_start.0), fmt(outer_start.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                large_arc,
                fmt(outer_end.0), fmt(outer_end.1),
                fmt(inner_end.0), fmt(inner_end.1),
                fmt(self.inner_radius), fmt(self.inner_radius),
                inner_large_arc,
                fmt(inner_start.0), fmt(inner_start.1),
            )
        } else {
            // Pie segment (no inner radius)
            format!(
                "M{},{} A{},{} 0 {},1 {},{} L0,0 Z",
                fmt(outer_start.0), fmt(outer_start.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                large_arc,
                fmt(outer_end.0), fmt(outer_end.1),
            )
        }
    }

    /// Generate a full circle (or near-full) using two half-arcs.
    fn generate_full_circle(&self, start_angle: f64) -> String {
        let mid_angle = start_angle + PI;
        let outer_start = angle_to_point(start_angle, self.outer_radius);
        let outer_mid = angle_to_point(mid_angle, self.outer_radius);

        if self.inner_radius > 0.0 {
            let inner_start = angle_to_point(start_angle, self.inner_radius);
            let inner_mid = angle_to_point(mid_angle, self.inner_radius);

            format!(
                "M{},{} A{},{} 0 1,1 {},{} A{},{} 0 1,1 {},{} M{},{} A{},{} 0 1,0 {},{} A{},{} 0 1,0 {},{}Z",
                fmt(outer_start.0), fmt(outer_start.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                fmt(outer_mid.0), fmt(outer_mid.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                fmt(outer_start.0), fmt(outer_start.1),
                fmt(inner_start.0), fmt(inner_start.1),
                fmt(self.inner_radius), fmt(self.inner_radius),
                fmt(inner_mid.0), fmt(inner_mid.1),
                fmt(self.inner_radius), fmt(self.inner_radius),
                fmt(inner_start.0), fmt(inner_start.1),
            )
        } else {
            format!(
                "M{},{} A{},{} 0 1,1 {},{} A{},{} 0 1,1 {},{}Z",
                fmt(outer_start.0), fmt(outer_start.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                fmt(outer_mid.0), fmt(outer_mid.1),
                fmt(self.outer_radius), fmt(self.outer_radius),
                fmt(outer_start.0), fmt(outer_start.1),
            )
        }
    }
}

/// Convert an angle (0 = 12 o'clock, clockwise) to cartesian coordinates centered at origin.
/// x = r * sin(angle), y = -r * cos(angle)
fn angle_to_point(angle: f64, radius: f64) -> (f64, f64) {
    (radius * angle.sin(), -radius * angle.cos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn arc_quarter_circle() {
        let gen = ArcGenerator::new(0.0, 100.0);
        let path = gen.generate(0.0, FRAC_PI_2);
        assert!(path.starts_with("M"), "Path should start with M, got: {}", path);
        assert!(path.contains("A"), "Path should contain A command, got: {}", path);
        assert!(path.ends_with("Z"), "Path should end with Z, got: {}", path);
    }

    #[test]
    fn arc_full_circle() {
        let gen = ArcGenerator::new(0.0, 100.0);
        let path = gen.generate(0.0, 2.0 * PI);
        assert!(path.starts_with("M"), "Path should start with M, got: {}", path);
        assert!(path.contains("A"), "Path should contain A command, got: {}", path);
        // Full circle uses two half-arcs
        let arc_count = path.matches("A").count();
        assert!(arc_count >= 2, "Full circle should have at least 2 arc commands, got: {}", arc_count);
    }

    #[test]
    fn arc_doughnut() {
        let gen = ArcGenerator::new(50.0, 100.0);
        let path = gen.generate(0.0, FRAC_PI_2);
        // Doughnut segment should contain two arc commands (outer and inner)
        let arc_count = path.matches("A").count();
        assert!(arc_count >= 2, "Doughnut should have at least 2 arc commands, got: {} in path: {}", arc_count, path);
    }
}
