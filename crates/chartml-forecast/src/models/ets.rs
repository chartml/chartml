use augurs_core::{Fit, Predict};
use augurs_ets::AutoETS;

use crate::seasonality::detect_seasonality;
use crate::types::{detect_interval, ForecastResult, TimeSeries, MIN_DATA_POINTS};
use crate::ForecastError;

/// Run ETS forecasting on a time series.
///
/// Fits an automatic ETS model to the values, generates point forecasts
/// for `horizon` steps, and computes prediction intervals at the given
/// confidence level.
pub fn forecast_ets(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
) -> Result<ForecastResult, ForecastError> {
    if series.len() < MIN_DATA_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_DATA_POINTS,
            actual: series.len(),
            context: "ETS forecasting".to_string(),
        });
    }

    // Fit non-seasonal ETS (season_length=1, "ZZN").
    // For seasonal ETS, use forecast_ets_seasonal() instead.
    let model = AutoETS::non_seasonal();
    let fitted = model
        .fit(&series.values)
        .map_err(|e| ForecastError::ModelFit(format!("ETS model fitting failed: {}", e)))?;

    // Generate forecasts with prediction intervals
    let forecast = fitted
        .predict(horizon, confidence_level)
        .map_err(|e| ForecastError::Prediction(format!("ETS prediction failed: {}", e)))?;

    // Detect the time interval between data points
    let interval_days = detect_interval(&series.timestamps);
    let last_ts = series
        .last_timestamp()
        .ok_or(ForecastError::EmptyTimeSeries)?;

    // Generate future timestamps
    let timestamps: Vec<i32> = (1..=horizon as i32)
        .map(|i| last_ts + i * interval_days)
        .collect();

    // Extract intervals (fall back to point forecast if no intervals available)
    let (lower_bounds, upper_bounds) = match forecast.intervals {
        Some(intervals) => (intervals.lower, intervals.upper),
        None => {
            // No intervals available - use point forecast as both bounds
            (forecast.point.clone(), forecast.point.clone())
        }
    };

    Ok(ForecastResult {
        timestamps,
        forecasts: forecast.point,
        lower_bounds,
        upper_bounds,
    })
}

/// Run ETS forecasting with a known seasonal period.
///
/// Same as `forecast_ets` but uses `AutoETS::new(season_length, "ZZZ")` to search
/// over all error/trend/seasonality component combinations.
pub fn forecast_ets_seasonal(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
    season_length: usize,
) -> Result<ForecastResult, ForecastError> {
    if series.len() < MIN_DATA_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_DATA_POINTS,
            actual: series.len(),
            context: "ETS forecasting".to_string(),
        });
    }

    let model = AutoETS::new(season_length, "ZZZ")
        .map_err(|e| ForecastError::ModelFit(format!("Failed to create seasonal ETS model: {}", e)))?;
    let fitted = model
        .fit(&series.values)
        .map_err(|e| ForecastError::ModelFit(format!("Seasonal ETS model fitting failed: {}", e)))?;

    let forecast = fitted
        .predict(horizon, confidence_level)
        .map_err(|e| ForecastError::Prediction(format!("Seasonal ETS prediction failed: {}", e)))?;

    let interval_days = detect_interval(&series.timestamps);
    let last_ts = series
        .last_timestamp()
        .ok_or(ForecastError::EmptyTimeSeries)?;

    let timestamps: Vec<i32> = (1..=horizon as i32)
        .map(|i| last_ts + i * interval_days)
        .collect();

    let (lower_bounds, upper_bounds) = match forecast.intervals {
        Some(intervals) => (intervals.lower, intervals.upper),
        None => (forecast.point.clone(), forecast.point.clone()),
    };

    Ok(ForecastResult {
        timestamps,
        forecasts: forecast.point,
        lower_bounds,
        upper_bounds,
    })
}

/// Detect the best seasonal period length from the data.
///
/// Returns the period with highest strength if strength > 0.3 (meaningful
/// seasonality), otherwise returns 1 (non-seasonal).
pub(crate) fn detect_best_season_length(values: &[f64]) -> usize {
    let results = match detect_seasonality(values) {
        Ok(r) => r,
        Err(_) => return 1,
    };
    match results.first() {
        Some(best) if best.strength > 0.3 => best.period as usize,
        _ => 1,
    }
}
