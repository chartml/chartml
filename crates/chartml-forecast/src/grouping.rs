use serde::{Deserialize, Serialize};

use crate::types::TimeSeries;

/// A time series associated with a specific group key.
///
/// For example, with `group_by := ['region', 'product']`, one GroupedTimeSeries
/// might have `group_columns = ["region", "product"]` and
/// `group_key = ["US", "Widget"]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedTimeSeries {
    /// The values for each group column, e.g. ["US", "Widget"].
    pub group_key: Vec<String>,
    /// The column names for each group, e.g. ["region", "product"].
    /// Retained for future use.
    pub group_columns: Vec<String>,
    /// The time series data for this group.
    pub series: TimeSeries,
}

/// A single output row from grouped seasonality detection.
///
/// Flat struct for easy row-by-row emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSeasonalityRow {
    /// Values for each group column (same order as group_columns).
    pub group_values: Vec<String>,
    /// Detected seasonal period length.
    pub period: i32,
    /// Strength of the seasonal component (0.0 to 1.0).
    pub strength: f64,
}

/// A single output row from a grouped forecast.
///
/// Flat struct for easy row-by-row emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupForecastRow {
    /// Values for each group column (same order as group_columns).
    pub group_values: Vec<String>,
    /// Forecast timestamp (days since epoch).
    pub timestamp: i32,
    /// Point forecast value.
    pub forecast: f64,
    /// Lower prediction interval bound.
    pub lower_bound: f64,
    /// Upper prediction interval bound.
    pub upper_bound: f64,
}
