use super::{ContinuousScale, tick_step, round_to_precision};

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
    /// Uses D3's iterative algorithm (up to 10 passes) so the domain expands
    /// until the tick step stabilises — matching d3-scale's linearish.nice().
    /// Preserves domain direction (reversed domains stay reversed).
    pub fn nice(self, count: usize) -> Self {
        if count == 0 {
            return self;
        }
        let (d0, d1) = self.domain;
        let reversed = d0 > d1;
        let mut start = d0.min(d1);
        let mut stop  = d0.max(d1);
        if start == stop {
            return self;
        }

        let mut prestep = f64::NAN;
        let mut max_iter = 10i32;
        while max_iter > 0 {
            max_iter -= 1;
            let step = tick_step(start, stop, count);
            if step == 0.0 || !step.is_finite() {
                break;
            }
            if step == prestep {
                // Stable — done.
                break;
            }
            // D3 also handles step < 0 (sub-unit steps use reciprocal encoding),
            // but tick_step always returns positive for our data ranges.
            start = (start / step).floor() * step;
            stop  = (stop  / step).ceil()  * step;
            prestep = step;
        }

        let domain = if reversed {
            (stop, start)
        } else {
            (start, stop)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
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
        let expected = [0.0, 20.0, 40.0, 60.0, 80.0, 100.0];
        assert_eq!(ticks.len(), expected.len(), "tick count mismatch: got {:?}", ticks);
        for (a, b) in ticks.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-10, "tick mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn linear_scale_ticks_non_round() {
        let scale = ScaleLinear::new((0.0, 1.0), (0.0, 500.0));
        let ticks = scale.ticks(5);
        let expected = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
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
