/// A single stacked data point with baseline and top values.
#[derive(Debug, Clone)]
pub struct StackedPoint {
    /// Category (e.g., month name)
    pub key: String,
    /// Series name (e.g., product line)
    pub series: String,
    /// Bottom of stack (baseline)
    pub y0: f64,
    /// Top of stack (y0 + value)
    pub y1: f64,
    /// The value used for stacking (raw value, or normalized proportion when using Normalize offset)
    pub value: f64,
}

/// Order in which series are stacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackOrder {
    /// Use input order
    None,
    /// Smallest series first (bottom)
    Ascending,
    /// Largest series first (bottom)
    Descending,
}

/// Offset mode for the stack baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackOffset {
    /// Zero baseline (standard stacking)
    None,
    /// Normalize to 0-1 (100% stacked)
    Normalize,
}

/// Computes stacked layout from grouped data.
/// Equivalent to D3's `d3.stack()`.
pub struct StackLayout {
    order: StackOrder,
    offset: StackOffset,
}

impl StackLayout {
    /// Create a new StackLayout with default settings (StackOrder::None, StackOffset::None).
    pub fn new() -> Self {
        Self {
            order: StackOrder::None,
            offset: StackOffset::None,
        }
    }

    /// Set the stacking order.
    pub fn order(mut self, order: StackOrder) -> Self {
        self.order = order;
        self
    }

    /// Set the stacking offset.
    pub fn offset(mut self, offset: StackOffset) -> Self {
        self.offset = offset;
        self
    }

    /// Compute stacked layout.
    ///
    /// # Arguments
    /// - `keys`: the categories (x-axis values), e.g., \["Jan", "Feb", "Mar"\]
    /// - `series_names`: the series (stacking groups), e.g., \["Hardware", "Software", "Services"\]
    /// - `values`: a 2D structure where `values[series_index][key_index]` = the value
    ///   (outer = series, inner = keys)
    ///
    /// # Returns
    /// A `Vec<StackedPoint>` for every (key, series) combination.
    pub fn layout(
        &self,
        keys: &[String],
        series_names: &[String],
        values: &[Vec<f64>],
    ) -> Vec<StackedPoint> {
        if keys.is_empty() || series_names.is_empty() || values.is_empty() {
            return Vec::new();
        }

        let num_series = series_names.len();
        let num_keys = keys.len();

        // Determine the order of series indices
        let ordered_indices = self.compute_order(series_names, values, num_keys);

        // Compute totals per key (needed for Normalize)
        let totals: Vec<f64> = (0..num_keys)
            .map(|k| {
                (0..num_series)
                    .map(|s| {
                        values
                            .get(s)
                            .and_then(|v| v.get(k))
                            .copied()
                            .unwrap_or(0.0)
                    })
                    .sum()
            })
            .collect();

        let mut results = Vec::with_capacity(num_series * num_keys);

        for k in 0..num_keys {
            let mut y_base = 0.0;

            for &s in &ordered_indices {
                let raw_value = values
                    .get(s)
                    .and_then(|v| v.get(k))
                    .copied()
                    .unwrap_or(0.0);

                let value = match self.offset {
                    StackOffset::None => raw_value,
                    StackOffset::Normalize => {
                        let total = totals[k];
                        if total == 0.0 {
                            0.0
                        } else {
                            raw_value / total
                        }
                    }
                };

                let y0 = y_base;
                let y1 = y_base + value;
                y_base = y1;

                results.push(StackedPoint {
                    key: keys[k].clone(),
                    series: series_names[s].clone(),
                    y0,
                    y1,
                    value,
                });
            }
        }

        results
    }

    /// Compute the order of series indices based on the configured StackOrder.
    fn compute_order(
        &self,
        series_names: &[String],
        values: &[Vec<f64>],
        num_keys: usize,
    ) -> Vec<usize> {
        let num_series = series_names.len();
        let mut indices: Vec<usize> = (0..num_series).collect();

        match self.order {
            StackOrder::None => {
                // Keep input order
            }
            StackOrder::Ascending => {
                let sums: Vec<f64> = (0..num_series)
                    .map(|s| {
                        (0..num_keys)
                            .map(|k| {
                                values
                                    .get(s)
                                    .and_then(|v| v.get(k))
                                    .copied()
                                    .unwrap_or(0.0)
                            })
                            .sum()
                    })
                    .collect();
                indices.sort_by(|&a, &b| sums[a].partial_cmp(&sums[b]).unwrap_or(std::cmp::Ordering::Equal));
            }
            StackOrder::Descending => {
                let sums: Vec<f64> = (0..num_series)
                    .map(|s| {
                        (0..num_keys)
                            .map(|k| {
                                values
                                    .get(s)
                                    .and_then(|v| v.get(k))
                                    .copied()
                                    .unwrap_or(0.0)
                            })
                            .sum()
                    })
                    .collect();
                indices.sort_by(|&a, &b| sums[b].partial_cmp(&sums[a]).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        indices
    }
}

impl Default for StackLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn stack_basic() {
        let keys = vec!["Jan".to_string(), "Feb".to_string(), "Mar".to_string()];
        let series = vec!["A".to_string(), "B".to_string()];
        let values = vec![
            vec![10.0, 20.0, 30.0], // Series A
            vec![5.0, 10.0, 15.0],  // Series B
        ];

        let layout = StackLayout::new();
        let points = layout.layout(&keys, &series, &values);

        assert_eq!(points.len(), 6);

        // Jan: A(0-10), B(10-15)
        assert_eq!(points[0].key, "Jan");
        assert_eq!(points[0].series, "A");
        assert_eq!(points[0].y0, 0.0);
        assert_eq!(points[0].y1, 10.0);
        assert_eq!(points[0].value, 10.0);

        assert_eq!(points[1].key, "Jan");
        assert_eq!(points[1].series, "B");
        assert_eq!(points[1].y0, 10.0);
        assert_eq!(points[1].y1, 15.0);
        assert_eq!(points[1].value, 5.0);

        // Feb: A(0-20), B(20-30)
        assert_eq!(points[2].y0, 0.0);
        assert_eq!(points[2].y1, 20.0);
        assert_eq!(points[3].y0, 20.0);
        assert_eq!(points[3].y1, 30.0);

        // Mar: A(0-30), B(30-45)
        assert_eq!(points[4].y0, 0.0);
        assert_eq!(points[4].y1, 30.0);
        assert_eq!(points[5].y0, 30.0);
        assert_eq!(points[5].y1, 45.0);
    }

    #[test]
    fn stack_y0_y1_chain() {
        let keys = vec!["X".to_string()];
        let series = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let values = vec![vec![10.0], vec![20.0], vec![30.0]];

        let layout = StackLayout::new();
        let points = layout.layout(&keys, &series, &values);

        assert_eq!(points.len(), 3);
        // Each series y0 equals previous series y1
        assert_eq!(points[0].y0, 0.0);
        assert_eq!(points[0].y1, 10.0);
        assert_eq!(points[1].y0, points[0].y1);
        assert_eq!(points[1].y1, 30.0);
        assert_eq!(points[2].y0, points[1].y1);
        assert_eq!(points[2].y1, 60.0);
    }

    #[test]
    fn stack_normalize() {
        let keys = vec!["Jan".to_string(), "Feb".to_string()];
        let series = vec!["A".to_string(), "B".to_string()];
        let values = vec![
            vec![30.0, 40.0], // Series A
            vec![70.0, 60.0], // Series B
        ];

        let layout = StackLayout::new().offset(StackOffset::Normalize);
        let points = layout.layout(&keys, &series, &values);

        assert_eq!(points.len(), 4);

        // Jan: total=100, A=0.3, B=0.7 → top should be 1.0
        assert!((points[0].y0 - 0.0).abs() < 1e-10);
        assert!((points[0].y1 - 0.3).abs() < 1e-10);
        assert!((points[1].y0 - 0.3).abs() < 1e-10);
        assert!((points[1].y1 - 1.0).abs() < 1e-10);

        // Feb: total=100, A=0.4, B=0.6 → top should be 1.0
        assert!((points[2].y0 - 0.0).abs() < 1e-10);
        assert!((points[2].y1 - 0.4).abs() < 1e-10);
        assert!((points[3].y0 - 0.4).abs() < 1e-10);
        assert!((points[3].y1 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn stack_empty() {
        let layout = StackLayout::new();

        let points = layout.layout(&[], &[], &[]);
        assert!(points.is_empty());

        let points = layout.layout(&["A".to_string()], &[], &[]);
        assert!(points.is_empty());

        let points = layout.layout(&[], &["S".to_string()], &[vec![1.0]]);
        assert!(points.is_empty());
    }

    #[test]
    fn stack_single_series() {
        let keys = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let series = vec!["Only".to_string()];
        let values = vec![vec![10.0, 20.0, 30.0]];

        let layout = StackLayout::new();
        let points = layout.layout(&keys, &series, &values);

        assert_eq!(points.len(), 3);
        for point in &points {
            assert_eq!(point.y0, 0.0);
            assert_eq!(point.series, "Only");
        }
        assert_eq!(points[0].y1, 10.0);
        assert_eq!(points[1].y1, 20.0);
        assert_eq!(points[2].y1, 30.0);
    }
}
