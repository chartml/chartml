use super::ContinuousScale;

/// Maps a continuous domain to a continuous range via linear interpolation.
/// Equivalent to D3's `scaleLinear()`.
pub struct ScaleLinear {
    domain: (f64, f64),
    range: (f64, f64),
}

impl ScaleLinear {
    /// Create a new linear scale with the given domain and range.
    pub fn new(domain: (f64, f64), range: (f64, f64)) -> Self {
        Self { domain, range }
    }

    /// Map a domain value to a range value using linear interpolation.
    pub fn map(&self, value: f64) -> f64 {
        let (d0, d1) = self.domain;
        let (r0, r1) = self.range;
        let domain_span = d1 - d0;
        if domain_span == 0.0 {
            // When domain is a single point, return the midpoint of the range.
            return (r0 + r1) / 2.0;
        }
        r0 + (value - d0) / domain_span * (r1 - r0)
    }

    /// Inverse mapping: range value back to domain value.
    pub fn invert(&self, value: f64) -> f64 {
        let (d0, d1) = self.domain;
        let (r0, r1) = self.range;
        let range_span = r1 - r0;
        if range_span == 0.0 {
            return (d0 + d1) / 2.0;
        }
        d0 + (value - r0) / range_span * (d1 - d0)
    }

    /// Generate approximately `count` nice tick values using the D3 "nice numbers" algorithm.
    /// Ticks are returned in the same order as the domain (descending if domain is reversed).
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        if count == 0 {
            return vec![];
        }
        let (d0, d1) = self.domain;
        let reversed = d0 > d1;
        let min = d0.min(d1);
        let max = d0.max(d1);
        if min == max {
            return vec![min];
        }

        let step = tick_step(min, max, count);
        if step == 0.0 || !step.is_finite() {
            return vec![];
        }

        let mut ticks = Vec::new();
        let start = (min / step).ceil();
        let stop = (max / step).floor();

        let mut i = start;
        while i <= stop {
            let tick = i * step;
            let tick = round_to_precision(tick, step);
            ticks.push(tick);
            i += 1.0;
        }

        if reversed {
            ticks.reverse();
        }

        ticks
    }

    /// Extend the domain to nice round numbers (like D3's `.nice()`).
    /// Preserves domain direction (reversed domains stay reversed).
    pub fn nice(self, count: usize) -> Self {
        if count == 0 {
            return self;
        }
        let (d0, d1) = self.domain;
        let reversed = d0 > d1;
        let min = d0.min(d1);
        let max = d0.max(d1);
        if min == max {
            return self;
        }

        let step = tick_step(min, max, count);
        if step == 0.0 || !step.is_finite() {
            return self;
        }

        let nice_min = round_to_precision((min / step).floor() * step, step);
        let nice_max = round_to_precision((max / step).ceil() * step, step);

        let domain = if reversed {
            (nice_max, nice_min)
        } else {
            (nice_min, nice_max)
        };

        Self {
            domain,
            range: self.range,
        }
    }

    /// Get the domain extent.
    pub fn domain(&self) -> (f64, f64) {
        self.domain
    }

    /// Get the range extent.
    pub fn range(&self) -> (f64, f64) {
        self.range
    }
}

impl ContinuousScale for ScaleLinear {
    fn map(&self, value: f64) -> f64 {
        ScaleLinear::map(self, value)
    }

    fn domain(&self) -> (f64, f64) {
        ScaleLinear::domain(self)
    }

    fn range(&self) -> (f64, f64) {
        ScaleLinear::range(self)
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        ScaleLinear::ticks(self, count)
    }

    fn clamp(&self, value: f64) -> f64 {
        let (d0, d1) = self.domain;
        let min = d0.min(d1);
        let max = d0.max(d1);
        value.clamp(min, max)
    }
}

/// Calculate a nice step size for the given range and approximate tick count.
/// Uses D3's tick step algorithm.
fn tick_step(min: f64, max: f64, count: usize) -> f64 {
    let raw_step = (max - min) / count as f64;
    let magnitude = 10_f64.powf(raw_step.log10().floor());
    let error = raw_step / magnitude;

    if error >= 50_f64.sqrt() {
        10.0 * magnitude
    } else if error >= 10_f64.sqrt() {
        5.0 * magnitude
    } else if error >= 2_f64.sqrt() {
        2.0 * magnitude
    } else {
        magnitude
    }
}

/// Round a value to remove floating point artifacts based on step precision.
fn round_to_precision(value: f64, step: f64) -> f64 {
    if step == 0.0 {
        return value;
    }
    // Determine the number of decimal places in the step
    let decimals = (-step.log10().floor()).max(0.0) as i32;
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_scale_maps_midpoint() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        assert!((scale.map(50.0) - 250.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_maps_endpoints() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        assert!((scale.map(0.0) - 0.0).abs() < 1e-10);
        assert!((scale.map(100.0) - 500.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_inverts() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        assert!((scale.invert(250.0) - 50.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_reversed_range() {
        let scale = ScaleLinear::new((0.0, 100.0), (500.0, 0.0));
        assert!((scale.map(0.0) - 500.0).abs() < 1e-10);
        assert!((scale.map(100.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_ticks() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        let ticks = scale.ticks(5);
        let expected = vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0];
        assert_eq!(ticks.len(), expected.len(), "tick count mismatch: got {:?}", ticks);
        for (a, b) in ticks.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-10, "tick mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn linear_scale_ticks_non_round() {
        let scale = ScaleLinear::new((0.0, 1.0), (0.0, 500.0));
        let ticks = scale.ticks(5);
        let expected = vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
        assert_eq!(ticks.len(), expected.len(), "tick count mismatch: got {:?}", ticks);
        for (a, b) in ticks.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-10, "tick mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn linear_scale_nice() {
        let scale = ScaleLinear::new((0.5, 9.7), (0.0, 500.0)).nice(10);
        let (d0, d1) = scale.domain();
        assert!((d0 - 0.0).abs() < 1e-10, "nice min should be 0, got {}", d0);
        assert!((d1 - 10.0).abs() < 1e-10, "nice max should be 10, got {}", d1);
    }

    #[test]
    fn linear_scale_single_value_domain() {
        let scale = ScaleLinear::new((5.0, 5.0), (0.0, 500.0));
        assert!((scale.map(5.0) - 250.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_negative_domain() {
        let scale = ScaleLinear::new((-100.0, 100.0), (0.0, 1000.0));
        assert!((scale.map(0.0) - 500.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_reversed_domain_ticks() {
        let scale = ScaleLinear::new((100.0, 0.0), (0.0, 500.0));
        let ticks = scale.ticks(5);
        // Ticks should be in descending order for reversed domain
        assert!(ticks[0] > ticks[ticks.len() - 1]);
        assert!((ticks[0] - 100.0).abs() < 1e-10);
        assert!((ticks[ticks.len() - 1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_reversed_domain_nice() {
        let scale = ScaleLinear::new((9.7, 0.5), (0.0, 500.0)).nice(10);
        let (d0, d1) = scale.domain();
        // nice() should preserve reversed direction: d0 > d1
        assert!(d0 > d1, "reversed domain should stay reversed: ({}, {})", d0, d1);
        assert!((d0 - 10.0).abs() < 1e-10);
        assert!((d1 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_invert_reversed_range() {
        let scale = ScaleLinear::new((0.0, 100.0), (500.0, 0.0));
        assert!((scale.invert(250.0) - 50.0).abs() < 1e-10);
        assert!((scale.invert(500.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn linear_scale_ticks_zero() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        assert!(scale.ticks(0).is_empty());
    }

    #[test]
    fn linear_scale_ticks_one() {
        let scale = ScaleLinear::new((0.0, 100.0), (0.0, 500.0));
        let ticks = scale.ticks(1);
        // Should produce at least one tick
        assert!(!ticks.is_empty());
    }
}
