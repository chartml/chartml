/// Computes start/end angles for pie slices from data values.
/// Equivalent to D3's `d3.pie()`.
use std::f64::consts::PI;

/// A computed pie slice with angular position.
#[derive(Debug, Clone)]
pub struct PieSlice {
    /// Index of the original data value.
    pub index: usize,
    /// The original data value.
    pub value: f64,
    /// Start angle in radians.
    pub start_angle: f64,
    /// End angle in radians.
    pub end_angle: f64,
}

/// Computes pie layout angles from data values.
pub struct PieLayout {
    start_angle: f64,
    end_angle: f64,
    sort: bool,
}

impl PieLayout {
    /// Create a new PieLayout with default settings (full circle, unsorted).
    pub fn new() -> Self {
        Self {
            start_angle: 0.0,
            end_angle: 2.0 * PI,
            sort: false,
        }
    }

    /// Set the start angle in radians.
    pub fn start_angle(mut self, angle: f64) -> Self {
        self.start_angle = angle;
        self
    }

    /// Set the end angle in radians.
    pub fn end_angle(mut self, angle: f64) -> Self {
        self.end_angle = angle;
        self
    }

    /// Set whether to sort slices by value (descending).
    pub fn sort(mut self, sort: bool) -> Self {
        self.sort = sort;
        self
    }

    /// Compute pie slices from data values.
    /// Each value gets a proportional angle within [start_angle, end_angle].
    pub fn layout(&self, values: &[f64]) -> Vec<PieSlice> {
        if values.is_empty() {
            return Vec::new();
        }

        let total: f64 = values.iter().sum();
        let angle_span = self.end_angle - self.start_angle;

        // Build indexed values
        let mut indexed: Vec<(usize, f64)> = values.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        if self.sort {
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        let mut slices = Vec::with_capacity(indexed.len());
        let mut current_angle = self.start_angle;

        for &(index, value) in &indexed {
            let slice_angle = if total > 0.0 {
                value / total * angle_span
            } else {
                0.0
            };
            let start = current_angle;
            let end = current_angle + slice_angle;
            slices.push(PieSlice {
                index,
                value,
                start_angle: start,
                end_angle: end,
            });
            current_angle = end;
        }

        slices
    }
}

impl Default for PieLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn pie_basic() {
        let layout = PieLayout::new();
        let slices = layout.layout(&[1.0, 2.0, 3.0]);
        assert_eq!(slices.len(), 3);
        // Angles should sum to 2*PI
        let total_angle: f64 = slices.iter().map(|s| s.end_angle - s.start_angle).sum();
        assert!((total_angle - 2.0 * PI).abs() < 1e-10, "Total angle should be 2*PI, got: {}", total_angle);
        // First slice should be 1/6 of the circle
        let first_angle = slices[0].end_angle - slices[0].start_angle;
        assert!((first_angle - 2.0 * PI / 6.0).abs() < 1e-10);
    }

    #[test]
    fn pie_single_value() {
        let layout = PieLayout::new();
        let slices = layout.layout(&[1.0]);
        assert_eq!(slices.len(), 1);
        assert!((slices[0].start_angle - 0.0).abs() < 1e-10);
        assert!((slices[0].end_angle - 2.0 * PI).abs() < 1e-10);
    }

    #[test]
    fn pie_equal_values() {
        let layout = PieLayout::new();
        let slices = layout.layout(&[1.0, 1.0, 1.0]);
        assert_eq!(slices.len(), 3);
        let expected_angle = 2.0 * PI / 3.0;
        for slice in &slices {
            let angle = slice.end_angle - slice.start_angle;
            assert!((angle - expected_angle).abs() < 1e-10, "Each slice should be 2*PI/3, got: {}", angle);
        }
    }

    #[test]
    fn pie_sorted_descending() {
        let layout = PieLayout::new().sort(true);
        let slices = layout.layout(&[1.0, 3.0, 2.0]);
        assert_eq!(slices.len(), 3);
        // Sorted descending: first slice should be value 3.0 (index 1)
        assert_eq!(slices[0].index, 1);
        assert!((slices[0].value - 3.0).abs() < 1e-10);
        // Second should be value 2.0 (index 2)
        assert_eq!(slices[1].index, 2);
        assert!((slices[1].value - 2.0).abs() < 1e-10);
        // Third should be value 1.0 (index 0)
        assert_eq!(slices[2].index, 0);
    }

    #[test]
    fn pie_custom_angle_range() {
        // Half circle (semicircle)
        let layout = PieLayout::new().start_angle(0.0).end_angle(PI);
        let slices = layout.layout(&[1.0, 1.0]);
        assert_eq!(slices.len(), 2);
        let total_angle: f64 = slices.iter().map(|s| s.end_angle - s.start_angle).sum();
        assert!((total_angle - PI).abs() < 1e-10, "Total angle should be PI, got: {}", total_angle);
        // Each slice should be PI/2
        let each = PI / 2.0;
        assert!((slices[0].end_angle - slices[0].start_angle - each).abs() < 1e-10);
    }

    #[test]
    fn pie_with_zero() {
        let layout = PieLayout::new();
        let slices = layout.layout(&[0.0, 1.0, 2.0]);
        assert_eq!(slices.len(), 3);
        // Zero-value slice should have start == end angle
        assert!((slices[0].end_angle - slices[0].start_angle).abs() < 1e-10,
            "Zero-value slice should have zero angle span");
    }
}
