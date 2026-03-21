use linregress::{FormulaRegressionBuilder, RegressionDataBuilder};
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::types::{detect_interval, ForecastResult, TimeSeries, MIN_DATA_POINTS};
use crate::ForecastError;

/// Run exponential growth forecasting on a time series.
///
/// Fits y = a * exp(b * x) by log-transforming values and applying OLS:
/// ln(y) = ln(a) + b*x. All values must be strictly positive (> 0).
/// Prediction intervals are computed in log space and back-transformed,
/// producing naturally asymmetric intervals in the original space.
pub fn forecast_exponential(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
) -> Result<ForecastResult, ForecastError> {
    let n = series.len();
    if n < MIN_DATA_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_DATA_POINTS,
            actual: n,
            context: "exponential forecasting".to_string(),
        });
    }

    // All values must be strictly positive for log transform
    if series.values.iter().any(|&v| v <= 0.0) {
        return Err(ForecastError::InvalidData(
            "Exponential model requires all values to be strictly positive (> 0)".to_string(),
        ));
    }

    // Log-transform the values
    let log_values: Vec<f64> = series.values.iter().map(|v| v.ln()).collect();
    let x_vals: Vec<f64> = (0..n).map(|i| i as f64).collect();

    // Fit OLS on (x, ln(y))
    let data = vec![
        ("Y".to_string(), log_values),
        ("X".to_string(), x_vals.clone()),
    ];

    let regression_data = RegressionDataBuilder::new()
        .build_from(data)
        .map_err(|e| ForecastError::ModelFit(format!("Failed to build regression data: {}", e)))?;

    let model = FormulaRegressionBuilder::new()
        .data(&regression_data)
        .formula("Y ~ X")
        .fit()
        .map_err(|e| ForecastError::ModelFit(format!("Exponential regression fitting failed: {}", e)))?;

    let params = model.parameters();
    let log_intercept = params[0]; // ln(a)
    let slope = params[1]; // b

    let residual_se = model.scale().sqrt();

    // Compute x_mean and sum of squared deviations for prediction intervals
    let x_mean: f64 = x_vals.iter().sum::<f64>() / n as f64;
    let sum_sq_dev: f64 = x_vals.iter().map(|&x| (x - x_mean).powi(2)).sum();

    // Detect the time interval and generate future timestamps
    let interval_days = detect_interval(&series.timestamps);
    let last_ts = series
        .last_timestamp()
        .ok_or(ForecastError::EmptyTimeSeries)?;

    let mut timestamps = Vec::with_capacity(horizon);
    let mut forecasts = Vec::with_capacity(horizon);
    let mut lower_bounds = Vec::with_capacity(horizon);
    let mut upper_bounds = Vec::with_capacity(horizon);

    // Handle near-perfect fit
    if residual_se < 1e-10 || sum_sq_dev < 1e-10 {
        for i in 1..=horizon {
            let x_pred = (n - 1 + i) as f64;
            let log_y_hat = log_intercept + slope * x_pred;
            let y_hat = log_y_hat.exp();
            timestamps.push(last_ts + i as i32 * interval_days);
            forecasts.push(y_hat);
            lower_bounds.push(y_hat);
            upper_bounds.push(y_hat);
        }
        return Ok(ForecastResult {
            timestamps,
            forecasts,
            lower_bounds,
            upper_bounds,
        });
    }

    // Student's t-distribution with n-2 degrees of freedom
    let df = (n - 2) as f64;
    let t_dist = StudentsT::new(0.0, 1.0, df)
        .map_err(|e| ForecastError::Prediction(format!("Failed to create t-distribution: {}", e)))?;
    let alpha = 1.0 - confidence_level;
    let t_value = t_dist.inverse_cdf(1.0 - alpha / 2.0);

    for i in 1..=horizon {
        let x_pred = (n - 1 + i) as f64;
        let log_y_hat = log_intercept + slope * x_pred;

        // Prediction interval in log space (same formula as linear)
        let pi_width = t_value
            * residual_se
            * (1.0_f64 + 1.0 / n as f64 + (x_pred - x_mean).powi(2) / sum_sq_dev).sqrt();

        if log_y_hat.is_nan() || pi_width.is_nan() || pi_width.is_infinite() {
            return Err(ForecastError::Prediction(
                "Exponential prediction interval computation produced invalid values".to_string(),
            ));
        }

        // Back-transform from log space: intervals become asymmetric
        let y_hat = log_y_hat.exp();
        let lower = (log_y_hat - pi_width).exp();
        let upper = (log_y_hat + pi_width).exp();

        timestamps.push(last_ts + i as i32 * interval_days);
        forecasts.push(y_hat);
        lower_bounds.push(lower);
        upper_bounds.push(upper);
    }

    Ok(ForecastResult {
        timestamps,
        forecasts,
        lower_bounds,
        upper_bounds,
    })
}
