use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::models::ets::{detect_best_season_length, forecast_ets, forecast_ets_seasonal};
use crate::models::exponential::forecast_exponential;
use crate::models::linear::forecast_linear;
use crate::models::logistic::forecast_logistic;
use crate::types::{ForecastResult, TimeSeries, MIN_DATA_POINTS};
use crate::ForecastError;

/// Compute MSE of a model on held-out data.
///
/// Splits the series into training (first n - holdout) and test (last holdout),
/// fits the model on training data, forecasts holdout steps, and returns MSE.
/// Wraps each model call in `catch_unwind` to safely handle panics from
/// underlying libraries (e.g. nalgebra/levenberg-marquardt in WASM).
fn evaluate_model_mse(
    series: &TimeSeries,
    holdout: usize,
    model_name: &str,
    season_length: usize,
) -> Option<f64> {
    let n = series.len();
    if holdout >= n || n - holdout < MIN_DATA_POINTS {
        return None;
    }

    let train = TimeSeries {
        timestamps: series.timestamps[..n - holdout].to_vec(),
        values: series.values[..n - holdout].to_vec(),
    };
    let actual = &series.values[n - holdout..];

    // Use a fixed confidence level for CV (doesn't affect point forecasts)
    let confidence = 0.95;

    // Wrap in catch_unwind to prevent panics from crashing the WASM process
    let fit_result = catch_unwind(AssertUnwindSafe(|| -> Option<ForecastResult> {
        match model_name {
            "seasonal_ets" => {
                if season_length <= 1 {
                    return None;
                }
                forecast_ets_seasonal(&train, holdout, confidence, season_length).ok()
            }
            "ets" => forecast_ets(&train, holdout, confidence).ok(),
            "exponential" => forecast_exponential(&train, holdout, confidence).ok(),
            "logistic" => forecast_logistic(&train, holdout, confidence).ok(),
            "linear" => forecast_linear(&train, holdout, confidence).ok(),
            _ => None,
        }
    }));

    let result = match fit_result {
        Ok(Some(r)) => r,
        Ok(None) => return None,
        Err(_) => {
            eprintln!(
                "chartml-forecast: auto CV: model={} panicked during evaluation, skipping",
                model_name
            );
            return None;
        }
    };

    if result.forecasts.len() != holdout {
        return None;
    }

    // Compute MSE
    let mse: f64 = result
        .forecasts
        .iter()
        .zip(actual.iter())
        .map(|(pred, act)| (pred - act).powi(2))
        .sum::<f64>()
        / holdout as f64;

    // Guard against NaN/Inf
    if mse.is_nan() || mse.is_infinite() {
        return None;
    }

    Some(mse)
}

/// Check whether the data shows decelerating growth (S-curve behavior).
///
/// Compares average absolute growth in the first half vs second half.
/// Logistic/S-curve data has decreasing absolute growth over time, while
/// exponential data has increasing absolute growth. Returns true only
/// when growth in the second half is meaningfully slower than the first.
pub(crate) fn shows_deceleration(values: &[f64]) -> bool {
    let n = values.len();
    if n < 6 {
        return false;
    }

    let mid = n / 2;

    // Average absolute change per step in each half
    let first_half_growth: f64 = values[1..mid]
        .iter()
        .zip(values[..mid - 1].iter())
        .map(|(a, b)| a - b)
        .sum::<f64>()
        / (mid - 1) as f64;

    let second_half_growth: f64 = values[mid + 1..]
        .iter()
        .zip(values[mid..n - 1].iter())
        .map(|(a, b)| a - b)
        .sum::<f64>()
        / (n - mid - 1) as f64;

    // Only consider logistic if growth has clearly slowed (second half < 70% of first)
    let decelerating = second_half_growth < first_half_growth * 0.7;

    eprintln!(
        "chartml-forecast: auto: deceleration check: first_half_avg={:.2}, second_half_avg={:.2}, decelerating={}",
        first_half_growth, second_half_growth, decelerating
    );

    decelerating
}

/// Validate that a forecast result is reasonable relative to the input data.
///
/// For upward-trending data, the first forecast point should not drop
/// significantly below the last observation. Returns false if the forecast
/// looks nonsensical (indicating the wrong model was selected).
fn validate_forecast(series: &TimeSeries, result: &ForecastResult) -> bool {
    if result.forecasts.is_empty() {
        return false;
    }

    let n = series.len();
    if n < 3 {
        return true; // Can't assess trend with too few points
    }

    // Check if data is trending upward (last 3 points)
    let recent = &series.values[n - 3..];
    let trending_up = recent[2] > recent[0];

    if trending_up {
        let last_val = series.values[n - 1];
        let first_forecast = result.forecasts[0];

        // If trending up but forecast drops more than 10% below last observation, reject
        if first_forecast < last_val * 0.9 {
            eprintln!(
                "chartml-forecast: auto: forecast validation failed: last_val={:.2}, first_forecast={:.2} (dropped >10%)",
                last_val, first_forecast
            );
            return false;
        }
    }

    // Check for NaN/Inf in forecasts or bounds
    for i in 0..result.forecasts.len() {
        if result.forecasts[i].is_nan()
            || result.forecasts[i].is_infinite()
            || result.lower_bounds[i].is_nan()
            || result.lower_bounds[i].is_infinite()
            || result.upper_bounds[i].is_nan()
            || result.upper_bounds[i].is_infinite()
        {
            eprintln!(
                "chartml-forecast: auto: forecast validation failed: NaN/Inf at step {}",
                i
            );
            return false;
        }
    }

    true
}

/// Automatic model selection via cross-validation.
///
/// Holds out 20% of data (min MIN_DATA_POINTS, max n - MIN_DATA_POINTS),
/// evaluates candidate models on the holdout, and selects the one with
/// the lowest MSE. Refits the winner on the full dataset.
///
/// If there isn't enough data for CV (n < 2 * MIN_DATA_POINTS), falls back
/// to the original heuristic: seasonal ETS -> non-seasonal ETS -> linear.
///
/// Candidate models:
/// - Seasonal ETS (only if seasonality detected with strength > 0.3)
/// - Non-seasonal ETS
/// - Exponential (only if all values > 0)
/// - Logistic (only if data shows decelerating growth -- S-curve pattern)
/// - Linear (always included)
pub fn forecast_auto(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
) -> Result<ForecastResult, ForecastError> {
    let n = series.len();
    let season_length = detect_best_season_length(&series.values);

    // Not enough data for CV -- use original fallback heuristic
    if n < 2 * MIN_DATA_POINTS {
        eprintln!(
            "chartml-forecast: auto: insufficient data for CV (n={}), using fallback",
            n
        );
        let ets_result = if season_length > 1 {
            forecast_ets_seasonal(series, horizon, confidence_level, season_length)
        } else {
            forecast_ets(series, horizon, confidence_level)
        };
        return match ets_result {
            Ok(result) => Ok(result),
            Err(_) => forecast_linear(series, horizon, confidence_level),
        };
    }

    // Determine holdout size: 20% of data, bounded by MIN_DATA_POINTS on both sides
    let holdout = (n as f64 * 0.2).ceil() as usize;
    let holdout = holdout.max(MIN_DATA_POINTS).min(n - MIN_DATA_POINTS);

    // Build candidate list
    let all_positive = series.values.iter().all(|&v| v > 0.0);
    let deceleration = shows_deceleration(&series.values);
    let mut candidates: Vec<&str> = Vec::new();

    if season_length > 1 {
        candidates.push("seasonal_ets");
    }
    candidates.push("ets");
    if all_positive {
        candidates.push("exponential");
    }
    if deceleration {
        candidates.push("logistic");
    } else {
        eprintln!("chartml-forecast: auto: skipping logistic (no deceleration in data)");
    }
    candidates.push("linear");

    // Evaluate each candidate and rank by MSE
    let mut ranked: Vec<(&str, f64)> = Vec::new();

    for &model_name in &candidates {
        match evaluate_model_mse(series, holdout, model_name, season_length) {
            Some(mse) => {
                eprintln!(
                    "chartml-forecast: auto CV: model={} MSE={:.4}",
                    model_name, mse
                );
                ranked.push((model_name, mse));
            }
            None => {
                eprintln!(
                    "chartml-forecast: auto CV: model={} failed/skipped",
                    model_name
                );
            }
        }
    }

    // Sort by MSE (lowest first)
    ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    if ranked.is_empty() {
        eprintln!("chartml-forecast: auto: all candidates failed, falling back to linear");
        return forecast_linear(series, horizon, confidence_level);
    }

    // Try models in order of MSE, validating each refit
    for (model_name, mse) in &ranked {
        eprintln!(
            "chartml-forecast: auto: trying model={} (CV MSE={:.4}) on full data",
            model_name, mse
        );

        let result = match *model_name {
            "seasonal_ets" => {
                forecast_ets_seasonal(series, horizon, confidence_level, season_length)
            }
            "ets" => forecast_ets(series, horizon, confidence_level),
            "exponential" => forecast_exponential(series, horizon, confidence_level),
            "logistic" => forecast_logistic(series, horizon, confidence_level),
            "linear" => forecast_linear(series, horizon, confidence_level),
            _ => continue,
        };

        match result {
            Ok(forecast) => {
                if validate_forecast(series, &forecast) {
                    eprintln!("chartml-forecast: auto: selected model={}", model_name);
                    return Ok(forecast);
                }
                eprintln!(
                    "chartml-forecast: auto: model={} refit failed validation, trying next",
                    model_name
                );
            }
            Err(e) => {
                eprintln!(
                    "chartml-forecast: auto: model={} refit failed: {}, trying next",
                    model_name, e
                );
            }
        }
    }

    // All ranked models failed validation -- last resort
    eprintln!("chartml-forecast: auto: all models failed validation, falling back to linear");
    forecast_linear(series, horizon, confidence_level)
}
