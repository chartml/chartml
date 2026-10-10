/// Generates SVG path `d` strings for line charts.
/// Equivalent to D3's `d3.line()`.
/// Curve interpolation strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurveType {
    /// Straight line segments between points.
    Linear,
    /// Monotone cubic Hermite interpolation in x (smooth, no overshooting).
    MonotoneX,
    /// Step interpolation (horizontal-then-vertical at midpoint between x values).
    /// Equivalent to D3's `curveStep` with t=0.5.
    Step,
}

/// Generates SVG path `d` strings from a series of (x, y) points.
pub struct LineGenerator {
    curve: CurveType,
}

impl LineGenerator {
    /// Create a new LineGenerator with Linear curve type.
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

    /// Generate an SVG path from a series of (x, y) points.
    pub fn generate(&self, points: &[(f64, f64)]) -> String {
        if points.is_empty() {
            return String::new();
        }

        match self.curve {
            CurveType::Linear => generate_linear(points),
            CurveType::MonotoneX => generate_monotone_x(points),
            CurveType::Step => generate_step(points),
        }
    }

    /// Generate an SVG path AND its total length (for animation dasharray).
    ///
    /// Returns `(path_d, length)`. The length is an upper-bound estimate
    /// suitable for `stroke-dasharray`/`stroke-dashoffset` draw animation.
    pub fn generate_with_length(&self, points: &[(f64, f64)]) -> (String, f64) {
        let path = self.generate(points);
        let length = self.compute_length(points);
        (path, length)
    }

    /// Compute an upper-bound estimate of the path length for the given points.
    fn compute_length(&self, points: &[(f64, f64)]) -> f64 {
        if points.len() < 2 {
            return 0.0;
        }
        match self.curve {
            CurveType::Linear => {
                points
                    .windows(2)
                    .map(|w| {
                        let dx = w[1].0 - w[0].0;
                        let dy = w[1].1 - w[0].1;
                        (dx * dx + dy * dy).sqrt()
                    })
                    .sum()
            }
            CurveType::Step => {
                // Step path traces: M(x0,y0), then for each subsequent point
                // L(x_mid,prev_y) L(x_mid,y), ending with L(last_x,last_y).
                // Track the cursor's x position to measure each horizontal.
                let mut length = 0.0;
                let mut cursor_x = points[0].0;
                let mut prev_y = points[0].1;
                let mut raw_prev_x = points[0].0;
                for &(x, y) in &points[1..] {
                    let x_mid = (raw_prev_x + x) * 0.5;
                    length += (x_mid - cursor_x).abs();
                    length += (y - prev_y).abs();
                    cursor_x = x_mid;
                    prev_y = y;
                    raw_prev_x = x;
                }
                length += (points[points.len() - 1].0 - cursor_x).abs();
                length
            }
            CurveType::MonotoneX => {
                // Control polygon length approximation for cubic bezier segments.
                // The true arc length <= control polygon length, so multiply
                // by 1.05 safety factor to guarantee >= true length.
                let n = points.len();
                if n == 2 {
                    // Falls back to linear for 2 points.
                    let dx = points[1].0 - points[0].0;
                    let dy = points[1].1 - points[0].1;
                    return (dx * dx + dy * dy).sqrt();
                }

                // Reconstruct tangents (same algorithm as generate_monotone_x)
                let mut secants = Vec::with_capacity(n - 1);
                for i in 0..n - 1 {
                    let dx = points[i + 1].0 - points[i].0;
                    if dx == 0.0 {
                        secants.push(0.0);
                    } else {
                        secants.push((points[i + 1].1 - points[i].1) / dx);
                    }
                }
                let mut tangents = vec![0.0; n];
                tangents[0] = secants[0];
                tangents[n - 1] = secants[n - 2];
                for i in 1..n - 1 {
                    if secants[i - 1].signum() != secants[i].signum() {
                        tangents[i] = 0.0;
                    } else {
                        tangents[i] = (secants[i - 1] + secants[i]) / 2.0;
                    }
                }
                for i in 0..n - 1 {
                    if secants[i] == 0.0 {
                        tangents[i] = 0.0;
                        tangents[i + 1] = 0.0;
                    } else {
                        let alpha = tangents[i] / secants[i];
                        let beta = tangents[i + 1] / secants[i];
                        let sum_sq = alpha * alpha + beta * beta;
                        if sum_sq > 9.0 {
                            let tau = 3.0 / sum_sq.sqrt();
                            tangents[i] = tau * alpha * secants[i];
                            tangents[i + 1] = tau * beta * secants[i];
                        }
                    }
                }

                // Sum control polygon lengths for each cubic segment
                let mut total = 0.0;
                for i in 0..n - 1 {
                    let dx = points[i + 1].0 - points[i].0;
                    let p0 = points[i];
                    let p1 = (points[i].0 + dx / 3.0, points[i].1 + tangents[i] * dx / 3.0);
                    let p2 = (points[i + 1].0 - dx / 3.0, points[i + 1].1 - tangents[i + 1] * dx / 3.0);
                    let p3 = points[i + 1];

                    let d01 = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
                    let d12 = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
                    let d23 = ((p3.0 - p2.0).powi(2) + (p3.1 - p2.1).powi(2)).sqrt();
                    total += d01 + d12 + d23;
                }
                total * 1.05
            }
        }
    }
}

impl Default for LineGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a float value for SVG output, trimming unnecessary trailing zeros.
pub(crate) fn fmt(v: f64) -> String {
    if v == v.round() && v.abs() < 1e10 {
        format!("{}", v as i64)
    } else {
        let s = format!("{:.6}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn generate_linear(points: &[(f64, f64)]) -> String {
    let mut path = String::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            path.push_str(&format!("M{},{}", fmt(x), fmt(y)));
        } else {
            path.push_str(&format!("L{},{}", fmt(x), fmt(y)));
        }
    }
    path
}

/// Step interpolation (D3 `curveStep`, t = 0.5).
///
/// For each consecutive pair of points, draws:
///   1. A horizontal segment to the midpoint of their x-coordinates.
///   2. A vertical segment to the new y value.
///
/// This produces a staircase where each step transitions at the midpoint
/// between adjacent x values.
fn generate_step(points: &[(f64, f64)]) -> String {
    let n = points.len();
    if n == 0 {
        return String::new();
    }
    if n == 1 {
        return format!("M{},{}", fmt(points[0].0), fmt(points[0].1));
    }

    // D3 step with t = 0.5 (center):
    // First point: moveTo(x0, y0)
    // For each subsequent point (x, y):
    //   x_mid = prev_x * 0.5 + x * 0.5
    //   lineTo(x_mid, prev_y)
    //   lineTo(x_mid, y)
    // After the last point, the lineEnd adds lineTo(x_last, y_last)
    // because 0 < t < 1 and point == 2.
    let mut path = format!("M{},{}", fmt(points[0].0), fmt(points[0].1));

    let mut prev_x = points[0].0;
    let mut prev_y = points[0].1;

    for &(x, y) in &points[1..] {
        let x_mid = prev_x * 0.5 + x * 0.5;
        path.push_str(&format!("L{},{}", fmt(x_mid), fmt(prev_y)));
        path.push_str(&format!("L{},{}", fmt(x_mid), fmt(y)));
        prev_x = x;
        prev_y = y;
    }

    // D3's lineEnd: when 0 < t < 1 and point == 2, it adds lineTo(x, y)
    // for the last stored point. This ensures the path extends all the way
    // to the final data point's x coordinate.
    path.push_str(&format!("L{},{}", fmt(prev_x), fmt(prev_y)));

    path
}

fn generate_monotone_x(points: &[(f64, f64)]) -> String {
    let n = points.len();
    if n == 0 {
        return String::new();
    }
    if n == 1 {
        return format!("M{},{}", fmt(points[0].0), fmt(points[0].1));
    }
    if n == 2 {
        // With only 2 points, fall back to linear.
        return generate_linear(points);
    }

    // Step 1: Calculate secants
    let mut secants = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let dx = points[i + 1].0 - points[i].0;
        if dx == 0.0 {
            secants.push(0.0);
        } else {
            secants.push((points[i + 1].1 - points[i].1) / dx);
        }
    }

    // Step 2: Calculate tangents using Fritsch-Carlson method
    let mut tangents = vec![0.0; n];
    tangents[0] = secants[0];
    tangents[n - 1] = secants[n - 2];
    for i in 1..n - 1 {
        if secants[i - 1].signum() != secants[i].signum() {
            tangents[i] = 0.0;
        } else {
            tangents[i] = (secants[i - 1] + secants[i]) / 2.0;
        }
    }

    // Step 3: Adjust for monotonicity
    for i in 0..n - 1 {
        if secants[i] == 0.0 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
        } else {
            let alpha = tangents[i] / secants[i];
            let beta = tangents[i + 1] / secants[i];
            let sum_sq = alpha * alpha + beta * beta;
            if sum_sq > 9.0 {
                let tau = 3.0 / sum_sq.sqrt();
                tangents[i] = tau * alpha * secants[i];
                tangents[i + 1] = tau * beta * secants[i];
            }
        }
    }

    // Step 4: Generate cubic bezier path
    let mut path = format!("M{},{}", fmt(points[0].0), fmt(points[0].1));
    for i in 0..n - 1 {
        let dx = points[i + 1].0 - points[i].0;
        let cp1x = points[i].0 + dx / 3.0;
        let cp1y = points[i].1 + tangents[i] * dx / 3.0;
        let cp2x = points[i + 1].0 - dx / 3.0;
        let cp2y = points[i + 1].1 - tangents[i + 1] * dx / 3.0;
        path.push_str(&format!(
            "C{},{} {},{} {},{}",
            fmt(cp1x),
            fmt(cp1y),
            fmt(cp2x),
            fmt(cp2y),
            fmt(points[i + 1].0),
            fmt(points[i + 1].1),
        ));
    }
    path
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn line_linear_basic() {
        let gen = LineGenerator::new();
        let path = gen.generate(&[(0.0, 10.0), (50.0, 20.0), (100.0, 5.0)]);
        assert_eq!(path, "M0,10L50,20L100,5");
    }

    #[test]
    fn line_linear_single_point() {
        let gen = LineGenerator::new();
        let path = gen.generate(&[(0.0, 10.0)]);
        assert_eq!(path, "M0,10");
    }

    #[test]
    fn line_linear_two_points() {
        let gen = LineGenerator::new();
        let path = gen.generate(&[(0.0, 10.0), (50.0, 20.0)]);
        assert_eq!(path, "M0,10L50,20");
    }

    #[test]
    fn line_linear_empty() {
        let gen = LineGenerator::new();
        let path = gen.generate(&[]);
        assert_eq!(path, "");
    }

    #[test]
    fn line_step_basic() {
        let gen = LineGenerator::new().curve(CurveType::Step);
        let path = gen.generate(&[(0.0, 10.0), (50.0, 20.0), (100.0, 5.0)]);
        // Step with t=0.5: midpoints at x=25 and x=75
        // M0,10 L25,10 L25,20 L75,20 L75,5 L100,5
        assert_eq!(path, "M0,10L25,10L25,20L75,20L75,5L100,5");
        assert!(!path.contains("C"), "Step path should NOT contain C commands");
    }

    #[test]
    fn line_step_single_point() {
        let gen = LineGenerator::new().curve(CurveType::Step);
        let path = gen.generate(&[(42.0, 7.0)]);
        assert_eq!(path, "M42,7");
    }

    #[test]
    fn line_step_two_points() {
        let gen = LineGenerator::new().curve(CurveType::Step);
        let path = gen.generate(&[(0.0, 10.0), (100.0, 20.0)]);
        // Midpoint at x=50
        assert_eq!(path, "M0,10L50,10L50,20L100,20");
    }

    #[test]
    fn line_monotone_basic() {
        let gen = LineGenerator::new().curve(CurveType::MonotoneX);
        let path = gen.generate(&[
            (0.0, 10.0),
            (50.0, 20.0),
            (100.0, 5.0),
            (150.0, 15.0),
        ]);
        assert!(path.starts_with("M"), "Path should start with M, got: {}", path);
        assert!(path.contains("C"), "Path should contain C commands, got: {}", path);
    }

    // ── generate_with_length tests ──

    #[test]
    fn line_linear_length() {
        // 3 points forming a right triangle: (0,0) -> (3,0) -> (3,4)
        // Segment 1: horizontal 3, Segment 2: vertical 4, Total: 7
        let gen = LineGenerator::new();
        let (path, length) = gen.generate_with_length(&[(0.0, 0.0), (3.0, 0.0), (3.0, 4.0)]);
        assert!(!path.is_empty());
        assert!((length - 7.0).abs() < 1e-10, "expected 7.0, got {}", length);
    }

    #[test]
    fn line_step_length() {
        // Two points: (0, 10) -> (100, 20)
        // Step path: M0,10 L50,10 L50,20 L100,20
        // Segments: |50-0|=50 + |20-10|=10 + |100-50|=50 = 110
        let gen = LineGenerator::new().curve(CurveType::Step);
        let (path, length) = gen.generate_with_length(&[(0.0, 10.0), (100.0, 20.0)]);
        assert!(!path.is_empty());
        assert!((length - 110.0).abs() < 1e-10, "expected 110.0, got {}", length);

        // Three points: (0, 10) -> (50, 20) -> (100, 5)
        // Step path: M0,10 L25,10 L25,20 L75,20 L75,5 L100,5
        // Segments: |25-0|=25 + |20-10|=10 + |75-25|=50 + |5-20|=15 + |100-75|=25 = 125
        let (_, length3) = gen.generate_with_length(&[(0.0, 10.0), (50.0, 20.0), (100.0, 5.0)]);
        assert!((length3 - 125.0).abs() < 1e-10, "expected 125.0, got {}", length3);
    }

    #[test]
    fn line_monotone_length() {
        // MonotoneX length should be >= chord length sum (straight-line between consecutive points)
        let gen = LineGenerator::new().curve(CurveType::MonotoneX);
        let points = [(0.0, 10.0), (50.0, 20.0), (100.0, 5.0), (150.0, 15.0)];
        let (path, length) = gen.generate_with_length(&points);
        assert!(!path.is_empty());

        let chord_sum: f64 = points.windows(2).map(|w| {
            let dx = w[1].0 - w[0].0;
            let dy = w[1].1 - w[0].1;
            (dx * dx + dy * dy).sqrt()
        }).sum();
        assert!(
            length >= chord_sum,
            "monotone length {} should be >= chord sum {}",
            length, chord_sum
        );
    }

    #[test]
    fn line_single_point_length() {
        let gen = LineGenerator::new();
        let (_, length) = gen.generate_with_length(&[(42.0, 7.0)]);
        assert!((length - 0.0).abs() < 1e-10, "single point should have length 0.0, got {}", length);
    }

    #[test]
    fn line_empty_length() {
        let gen = LineGenerator::new();
        let (_, length) = gen.generate_with_length(&[]);
        assert!((length - 0.0).abs() < 1e-10, "empty should have length 0.0, got {}", length);
    }
}
