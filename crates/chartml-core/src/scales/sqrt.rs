use super::{ContinuousScale, tick_step, round_to_precision};

/// Square root scale for bubble sizes. Maps continuous domain to continuous range
/// via sqrt transformation. Equivalent to D3's `scaleSqrt()`.
/// This is a power scale with exponent 0.5.
pub struct ScaleSqrt {
    domain: (f64, f64),
    range: (f64, f64),
}

impl ScaleSqrt {
    /// Create a new sqrt scale with the given domain and range.
    pub fn new(domain: (f64, f64), range: (f64, f64)) -> Self {
        Self { domain, range }
    }

    /// Map a domain value to a range value using sqrt interpolation.
    /// Negative domain values are clamped to 0 before sqrt.
    pub fn map(&self, value: f64) -> f64 {
        let (d0, d1) = self.domain;
        let (r0, r1) = self.range;
        let sqrt_d0 = d0.max(0.0).sqrt();
        let sqrt_d1 = d1.max(0.0).sqrt();
        let sqrt_val = value.max(0.0).sqrt();
        let sqrt_span = sqrt_d1 - sqrt_d0;
        if sqrt_span == 0.0 {
            return (r0 + r1) / 2.0;
        }
        r0 + (sqrt_val - sqrt_d0) / sqrt_span * (r1 - r0)
    }

    /// Inverse mapping: range value back to domain value.
    pub fn invert(&self, value: f64) -> f64 {
        let (d0, d1) = self.domain;
        let (r0, r1) = self.range;
        let sqrt_d0 = d0.max(0.0).sqrt();
        let sqrt_d1 = d1.max(0.0).sqrt();
        let range_span = r1 - r0;
        if range_span == 0.0 {
            return (d0 + d1) / 2.0;
        }
        let sqrt_val = sqrt_d0 + (value - r0) / range_span * (sqrt_d1 - sqrt_d0);
        sqrt_val * sqrt_val
    }

    /// Generate ticks by computing them in sqrt-transformed space,
    /// then squaring back to the original domain.
    pub fn ticks(&self, count: usize) -> Vec<f64> {
        if count == 0 {
            return vec![];
        }
        let (d0, d1) = self.domain;
        let sqrt_min = d0.max(0.0).sqrt().min(d1.max(0.0).sqrt());
        let sqrt_max = d0.max(0.0).sqrt().max(d1.max(0.0).sqrt());
        if sqrt_min == sqrt_max {
            return vec![sqrt_min * sqrt_min];
        }

        let step = tick_step(sqrt_min, sqrt_max, count);
        if step == 0.0 || !step.is_finite() {
            return vec![];
        }

        let mut ticks = Vec::new();
        let start = (sqrt_min / step).ceil();
        let stop = (sqrt_max / step).floor();

        let mut i = start;
        while i <= stop {
            let sqrt_tick = i * step;
            let tick = round_to_precision(sqrt_tick * sqrt_tick, step * step);
            ticks.push(tick);
            i += 1.0;
        }

        ticks
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

impl ContinuousScale for ScaleSqrt {
    fn map(&self, value: f64) -> f64 {
        ScaleSqrt::map(self, value)
    }

    fn domain(&self) -> (f64, f64) {
        ScaleSqrt::domain(self)
    }

    fn range(&self) -> (f64, f64) {
        ScaleSqrt::range(self)
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        ScaleSqrt::ticks(self, count)
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
    use super::*;

    #[test]
    fn sqrt_scale_maps_zero() {
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        assert!((scale.map(0.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn sqrt_scale_maps_max() {
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        assert!((scale.map(100.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn sqrt_scale_nonlinear() {
        // sqrt(25)/sqrt(100) = 5/10 = 0.5, so map(25) should be 50
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        assert!(
            (scale.map(25.0) - 50.0).abs() < 1e-10,
            "map(25) should be 50, got {}",
            scale.map(25.0)
        );
    }

    #[test]
    fn sqrt_scale_inverts() {
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        let x = 42.0;
        let mapped = scale.map(x);
        let inverted = scale.invert(mapped);
        assert!(
            (inverted - x).abs() < 1e-10,
            "invert(map({})) should be {}, got {}",
            x,
            x,
            inverted
        );
    }

    #[test]
    fn sqrt_scale_ticks() {
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        let ticks = scale.ticks(5);
        assert!(!ticks.is_empty(), "should generate at least one tick");
        assert!(
            ticks.len() <= 15,
            "should not generate too many ticks, got {}",
            ticks.len()
        );
        // All ticks should be non-negative and within domain
        for tick in &ticks {
            assert!(*tick >= 0.0, "tick {} should be >= 0", tick);
            assert!(*tick <= 100.0 + 1e-10, "tick {} should be <= 100", tick);
        }
    }

    #[test]
    fn sqrt_scale_negative_clamped() {
        let scale = ScaleSqrt::new((0.0, 100.0), (0.0, 100.0));
        // Negative values should be clamped to 0 before sqrt, so map(-10) == map(0)
        assert!(
            (scale.map(-10.0) - scale.map(0.0)).abs() < 1e-10,
            "negative values should be clamped to 0"
        );
    }
}
