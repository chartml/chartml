use linregress::{FormulaRegressionBuilder, RegressionDataBuilder};
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::types::{detect_interval, ForecastResult, TimeSeries, MIN_DATA_POINTS};
use crate::ForecastError;

/// Run linear regression (OLS) forecasting on a time series.
///
/// Fits y = intercept + slope * x where x is the 0-based index of each data point.
/// Generates prediction intervals using the Student's t-distribution at the
/// given confidence level.
pub fn forecast_linear(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
) -> Result<ForecastResult, ForecastError> {
    let n = series.len();
    if n < MIN_DATA_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_DATA_POINTS,
            actual: n,
            context: "linear forecasting".to_string(),
        });
    }

    // Build regression data: x = [0, 1, 2, ...], y = values
    let x_vals: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let data = vec![
        ("Y".to_string(), series.values.clone()),
        ("X".to_string(), x_vals.clone()),
    ];

    let regression_data = RegressionDataBuilder::new()
        .build_from(data)
        .map_err(|e| ForecastError::ModelFit(format!("Failed to build regression data: {}", e)))?;

    let model = FormulaRegressionBuilder::new()
        .data(&regression_data)
        .formula("Y ~ X")
        .fit()
        .map_err(|e| ForecastError::ModelFit(format!("Linear regression fitting failed: {}", e)))?;

    // parameters() returns [intercept, slope]
    let params = model.parameters();
    let intercept = params[0];
    let slope = params[1];

    // Compute standard error of the regression (residual standard error)
    // scale() returns the variance of residuals: SSR / (n - 2)
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

    // Handle constant or near-perfect-fit data: residual SE ~ 0 means the model
    // fits perfectly, so prediction intervals collapse to zero width.
    if residual_se < 1e-10 || sum_sq_dev < 1e-10 {
        for i in 1..=horizon {
            let x_pred = (n - 1 + i) as f64;
            let y_hat = intercept + slope * x_pred;
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

    // Student's t-distribution with n-2 degrees of freedom (standard form)
    let df = (n - 2) as f64;
    let t_dist = StudentsT::new(0.0, 1.0, df)
        .map_err(|e| ForecastError::Prediction(format!("Failed to create t-distribution: {}", e)))?;
    let alpha = 1.0 - confidence_level;
    let t_value = t_dist.inverse_cdf(1.0 - alpha / 2.0);

    for i in 1..=horizon {
        let x_pred = (n - 1 + i) as f64; // Continue from the last index
        let y_hat = intercept + slope * x_pred;

        // Prediction interval for a new observation:
        // PI = y_hat +/- t * se * sqrt(1 + 1/n + (x_pred - x_mean)^2 / sum_sq_dev)
        let pi_width = t_value
            * residual_se
            * (1.0_f64 + 1.0 / n as f64 + (x_pred - x_mean).powi(2) / sum_sq_dev).sqrt();

        if y_hat.is_nan() || pi_width.is_nan() || pi_width.is_infinite() {
            return Err(ForecastError::Prediction(
                "Prediction interval computation produced invalid values".to_string(),
            ));
        }

        timestamps.push(last_ts + i as i32 * interval_days);
        forecasts.push(y_hat);
        lower_bounds.push(y_hat - pi_width);
        upper_bounds.push(y_hat + pi_width);
    }

    Ok(ForecastResult {
        timestamps,
        forecasts,
        lower_bounds,
        upper_bounds,
    })
}
