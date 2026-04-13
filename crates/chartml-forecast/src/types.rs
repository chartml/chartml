use serde::{Deserialize, Serialize};

/// Minimum number of data points required for ETS forecasting.
pub const MIN_DATA_POINTS: usize = 4;

/// A single time series of (timestamp, value) pairs.
/// Timestamps are days since 1970-01-01 (Unix epoch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub timestamps: Vec<i32>,
    pub values: Vec<f64>,
}

impl TimeSeries {
    /// Returns the number of data points.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if the series has no data points.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the last timestamp, or None if empty.
    pub fn last_timestamp(&self) -> Option<i32> {
        self.timestamps.last().copied()
    }
}

/// The result of a forecasting operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastResult {
    /// Future timestamps (days since epoch).
    pub timestamps: Vec<i32>,
    /// Point forecasts.
    pub forecasts: Vec<f64>,
    /// Lower prediction interval bounds.
    pub lower_bounds: Vec<f64>,
    /// Upper prediction interval bounds.
    pub upper_bounds: Vec<f64>,
}

/// Detect the most common interval (in days) between consecutive timestamps.
/// Returns 1 (daily) if fewer than 2 data points or no clear interval.
pub fn detect_interval(timestamps: &[i32]) -> i32 {
    if timestamps.len() < 2 {
        return 1;
    }

    // Calculate all consecutive differences
    let mut diffs: Vec<i32> = timestamps.windows(2).map(|w| w[1] - w[0]).collect();
    diffs.sort_unstable();

    // Return the mode (most common difference)
    let mut best_val = diffs[0];
    let mut best_count = 1usize;
    let mut current_val = diffs[0];
    let mut current_count = 1usize;

    for &d in &diffs[1..] {
        if d == current_val {
            current_count += 1;
        } else {
            if current_count > best_count {
                best_count = current_count;
                best_val = current_val;
            }
            current_val = d;
            current_count = 1;
        }
    }
    if current_count > best_count {
        best_val = current_val;
    }

    // Sanity check: interval must be positive
    if best_val <= 0 {
        return 1;
    }

    best_val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_interval_daily() {
        let ts = vec![100, 101, 102, 103, 104];
        assert_eq!(detect_interval(&ts), 1);
    }

    #[test]
    fn test_detect_interval_weekly() {
        let ts = vec![100, 107, 114, 121, 128];
        assert_eq!(detect_interval(&ts), 7);
    }

    #[test]
    fn test_detect_interval_monthly_approx() {
        // ~30 day intervals with some variance; mode should win
        let ts = vec![100, 130, 161, 191, 222];
        let interval = detect_interval(&ts);
        assert!((30..=31).contains(&interval));
    }

    #[test]
    fn test_detect_interval_single_point() {
        assert_eq!(detect_interval(&[100]), 1);
    }

    #[test]
    fn test_detect_interval_empty() {
        assert_eq!(detect_interval(&[]), 1);
    }
}
