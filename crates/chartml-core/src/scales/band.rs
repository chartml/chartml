/// Maps discrete domain values (categories) to continuous range positions.
/// Equivalent to D3's `scaleBand()`, used for bar chart x-axes.
pub struct ScaleBand {
    domain: Vec<String>,
    range: (f64, f64),
    padding_inner: f64,
    padding_outer: f64,
    step: f64,
    bandwidth: f64,
}

impl ScaleBand {
    /// Create a new band scale with the given domain and range.
    /// Default padding: inner=0.1, outer=0.1.
    pub fn new(domain: Vec<String>, range: (f64, f64)) -> Self {
        let mut scale = Self {
            domain,
            range,
            padding_inner: 0.1,
            padding_outer: 0.1,
            step: 0.0,
            bandwidth: 0.0,
        };
        scale.recalculate();
        scale
    }

    /// Set inner padding (between bands). Returns self for chaining.
    pub fn padding_inner(mut self, padding: f64) -> Self {
        self.padding_inner = padding;
        self.recalculate();
        self
    }

    /// Set outer padding (before first and after last band). Returns self for chaining.
    pub fn padding_outer(mut self, padding: f64) -> Self {
        self.padding_outer = padding;
        self.recalculate();
        self
    }

    /// Set both inner and outer padding to the same value.
    pub fn padding(mut self, padding: f64) -> Self {
        self.padding_inner = padding;
        self.padding_outer = padding;
        self.recalculate();
        self
    }

    /// Map a domain value to its range position (the start of the band).
    /// Returns None if the value is not in the domain.
    pub fn map(&self, value: &str) -> Option<f64> {
        let index = self.domain.iter().position(|d| d == value)?;
        let start = self.range.0.min(self.range.1);
        Some(start + self.padding_outer * self.step + index as f64 * self.step)
    }

    /// Get the width of each band.
    pub fn bandwidth(&self) -> f64 {
        self.bandwidth
    }

    /// Get the step size (band + inner padding).
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Get the domain values.
    pub fn domain(&self) -> &[String] {
        &self.domain
    }

    /// Get the range extent.
    pub fn range(&self) -> (f64, f64) {
        self.range
    }

    /// Recalculate step and bandwidth from current domain/range/padding.
    fn recalculate(&mut self) {
        let n = self.domain.len() as f64;
        let range_size = (self.range.1 - self.range.0).abs();

        if n == 0.0 {
            self.step = 0.0;
            self.bandwidth = 0.0;
            return;
        }

        self.step = range_size / (n - self.padding_inner + 2.0 * self.padding_outer).max(1.0);
        self.bandwidth = self.step * (1.0 - self.padding_inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain_abc() -> Vec<String> {
        vec!["A".to_string(), "B".to_string(), "C".to_string()]
    }

    #[test]
    fn band_scale_basic() {
        let scale = ScaleBand::new(domain_abc(), (0.0, 300.0));
        let a = scale.map("A").unwrap();
        let c = scale.map("C").unwrap();
        // A should be the first position, C should be the last
        assert!(a < c, "A position {} should be less than C position {}", a, c);
        // A should start after outer padding
        assert!(a > 0.0, "A should have some outer padding offset");
        // C + bandwidth should be close to but not exceed 300
        assert!(c + scale.bandwidth() <= 300.0 + 1e-10);
    }

    #[test]
    fn band_scale_bandwidth() {
        let scale = ScaleBand::new(domain_abc(), (0.0, 300.0));
        // With 3 items in 300px, bandwidth should be reasonable (not 0, not 300)
        assert!(scale.bandwidth() > 0.0);
        assert!(scale.bandwidth() < 300.0);
        // Bandwidth should be less than step
        assert!(scale.bandwidth() <= scale.step());
    }

    #[test]
    fn band_scale_unknown_value() {
        let scale = ScaleBand::new(domain_abc(), (0.0, 300.0));
        assert!(scale.map("D").is_none());
    }

    #[test]
    fn band_scale_empty_domain() {
        let scale = ScaleBand::new(vec![], (0.0, 300.0));
        assert_eq!(scale.bandwidth(), 0.0);
        assert_eq!(scale.step(), 0.0);
    }

    #[test]
    fn band_scale_single_item() {
        let scale = ScaleBand::new(vec!["A".to_string()], (0.0, 300.0));
        let a = scale.map("A").unwrap();
        assert!(a >= 0.0);
        assert!(scale.bandwidth() > 0.0);
        assert!(a + scale.bandwidth() <= 300.0 + 1e-10);
    }

    #[test]
    fn band_scale_custom_padding() {
        let scale_default = ScaleBand::new(domain_abc(), (0.0, 300.0));
        let scale_padded = ScaleBand::new(domain_abc(), (0.0, 300.0)).padding(0.2);
        // More padding means smaller bandwidth
        assert!(
            scale_padded.bandwidth() < scale_default.bandwidth(),
            "padded bandwidth {} should be less than default {}",
            scale_padded.bandwidth(),
            scale_default.bandwidth()
        );
    }

    #[test]
    fn band_scale_no_padding() {
        let scale = ScaleBand::new(domain_abc(), (0.0, 300.0))
            .padding_inner(0.0)
            .padding_outer(0.0);
        // With no padding, bandwidth == step == range / n
        let expected = 300.0 / 3.0;
        assert!(
            (scale.bandwidth() - expected).abs() < 1e-10,
            "bandwidth should be {} but got {}",
            expected,
            scale.bandwidth()
        );
        assert!(
            (scale.step() - expected).abs() < 1e-10,
            "step should be {} but got {}",
            expected,
            scale.step()
        );
        // First item should start at 0
        let a = scale.map("A").unwrap();
        assert!((a - 0.0).abs() < 1e-10, "A should be at 0, got {}", a);
    }

    #[test]
    fn band_scale_step() {
        let scale = ScaleBand::new(domain_abc(), (0.0, 300.0));
        assert!(
            scale.step() >= scale.bandwidth(),
            "step {} should be >= bandwidth {}",
            scale.step(),
            scale.bandwidth()
        );
    }
}
