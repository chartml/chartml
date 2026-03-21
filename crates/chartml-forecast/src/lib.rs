pub mod grouping;
pub mod models;
pub mod seasonality;
pub mod types;

use serde::{Deserialize, Serialize};

pub use seasonality::{detect_seasonality, SeasonalityResult};
pub use types::{ForecastResult, TimeSeries};

/// Error type for forecasting operations.
#[derive(Debug, thiserror::Error)]
pub enum ForecastError {
    #[error("Insufficient data: need at least {required} data points for {context}, got {actual}")]
    InsufficientData {
        required: usize,
        actual: usize,
        context: String,
    },

    #[error("Empty time series: no timestamps available")]
    EmptyTimeSeries,

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Model fitting failed: {0}")]
    ModelFit(String),

    #[error("Prediction failed: {0}")]
    Prediction(String),

    #[error("Model convergence failed: {0}")]
    Convergence(String),
}

/// Which forecasting model to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForecastModel {
    /// Exponential smoothing (ETS) with automatic component selection.
    ETS,
    /// Ordinary least-squares linear regression.
    Linear,
    /// Exponential growth (log-linear regression). Requires all values > 0.
    Exponential,
    /// Logistic / S-curve growth via Levenberg-Marquardt nonlinear least squares.
    Logistic,
    /// Automatic model selection via cross-validation.
    Auto,
}

/// Configuration for a forecasting operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastConfig {
    /// Which model to use.
    pub model: ForecastModel,
    /// Number of future time steps to forecast.
    pub horizon: usize,
    /// Confidence level for prediction intervals (e.g. 0.95 for 95%).
    pub confidence_level: f64,
}

/// Run a forecast on the given time series using the specified configuration.
///
/// Dispatches to the appropriate model based on `config.model`.
/// Wraps the call in `catch_unwind` to prevent panics from propagating
/// (important for WASM where panics abort the process).
pub fn forecast(
    series: &TimeSeries,
    config: &ForecastConfig,
) -> Result<ForecastResult, ForecastError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match config.model {
            ForecastModel::ETS => {
                models::ets::forecast_ets(series, config.horizon, config.confidence_level)
            }
            ForecastModel::Linear => {
                models::linear::forecast_linear(series, config.horizon, config.confidence_level)
            }
            ForecastModel::Exponential => {
                models::exponential::forecast_exponential(series, config.horizon, config.confidence_level)
            }
            ForecastModel::Logistic => {
                models::logistic::forecast_logistic(series, config.horizon, config.confidence_level)
            }
            ForecastModel::Auto => {
                models::auto::forecast_auto(series, config.horizon, config.confidence_level)
            }
        }
    }));

    match result {
        Ok(inner) => inner,
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            Err(ForecastError::ModelFit(format!("model panicked: {}", msg)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a TimeSeries with daily timestamps starting from day 1000.
    fn make_series(values: Vec<f64>) -> TimeSeries {
        let n = values.len();
        let timestamps: Vec<i32> = (0..n).map(|i| 1000 + i as i32).collect();
        TimeSeries { timestamps, values }
    }

    // ==================== Exponential model tests ====================

    #[test]
    fn test_exponential_pure_growth() {
        // y = 100 * exp(0.03 * x) for 30 points
        let values: Vec<f64> = (0..30).map(|i| 100.0 * (0.03 * i as f64).exp()).collect();
        let series = make_series(values.clone());
        let result =
            models::exponential::forecast_exponential(&series, 5, 0.95).unwrap();

        assert_eq!(result.forecasts.len(), 5);
        assert_eq!(result.timestamps.len(), 5);

        // Forecasts should continue upward beyond the last observed value
        let last_observed = values.last().unwrap();
        for fc in &result.forecasts {
            assert!(
                *fc > *last_observed,
                "Forecast {:.2} should exceed last observed {:.2}",
                fc,
                last_observed
            );
        }

        // Each forecast step should be larger than the previous
        for w in result.forecasts.windows(2) {
            assert!(w[1] > w[0], "Forecasts should be monotonically increasing");
        }
    }

    #[test]
    fn test_exponential_rejects_non_positive() {
        let values = vec![1.0, 2.0, 0.0, 4.0, 5.0]; // contains zero
        let series = make_series(values);
        let err = models::exponential::forecast_exponential(&series, 3, 0.95).unwrap_err();
        assert!(
            err.to_string().contains("strictly positive"),
            "Error: {}",
            err
        );

        let values = vec![1.0, -2.0, 3.0, 4.0, 5.0]; // contains negative
        let series = make_series(values);
        let err = models::exponential::forecast_exponential(&series, 3, 0.95).unwrap_err();
        assert!(
            err.to_string().contains("strictly positive"),
            "Error: {}",
            err
        );
    }

    #[test]
    fn test_exponential_asymmetric_intervals() {
        // With real exponential data + some noise, intervals should be asymmetric
        let values: Vec<f64> = (0..20)
            .map(|i| {
                let base = 50.0 * (0.05 * i as f64).exp();
                // Add small noise to prevent perfect fit (which collapses intervals)
                base + (i as f64 * 0.7).sin() * 2.0
            })
            .collect();
        let series = make_series(values);
        let result =
            models::exponential::forecast_exponential(&series, 5, 0.95).unwrap();

        for i in 0..result.forecasts.len() {
            let upper_width = result.upper_bounds[i] - result.forecasts[i];
            let lower_width = result.forecasts[i] - result.lower_bounds[i];

            // In exp space, upper should be wider than lower (log-normal property)
            assert!(
                upper_width > lower_width,
                "Step {}: upper_width={:.2} should exceed lower_width={:.2}",
                i,
                upper_width,
                lower_width
            );
        }
    }

    // ==================== Logistic model tests ====================

    #[test]
    fn test_logistic_s_curve() {
        // y = 500 / (1 + exp(-0.15 * (x - 25))) for 50 points
        let values: Vec<f64> = (0..50)
            .map(|i| 500.0 / (1.0 + (-0.15 * (i as f64 - 25.0)).exp()))
            .collect();
        let series = make_series(values);
        let result =
            models::logistic::forecast_logistic(&series, 10, 0.95).unwrap();

        assert_eq!(result.forecasts.len(), 10);

        // Forecasts should plateau near 500 (the capacity)
        for fc in &result.forecasts {
            assert!(
                *fc < 520.0,
                "Forecast {:.2} should not significantly exceed capacity 500",
                fc
            );
            assert!(
                *fc > 400.0,
                "Forecast {:.2} should be near the capacity, not far below",
                fc
            );
        }
    }

    #[test]
    fn test_logistic_dashboard_data_no_drop() {
        // Exact data from the dashboard Test F chart -- S-curve approaching ~1000
        let values = vec![
            15.0, 28.0, 55.0, 105.0, 195.0, 340.0, 490.0, 620.0, 720.0, 790.0,
            840.0, 875.0, 900.0, 918.0, 930.0,
        ];
        let last_actual = *values.last().unwrap();
        let series = make_series(values);
        let result =
            models::logistic::forecast_logistic(&series, 6, 0.95).unwrap();

        eprintln!("Last actual value: {:.2}", last_actual);
        for (i, fc) in result.forecasts.iter().enumerate() {
            eprintln!(
                "  forecast[{}] = {:.4}, lower = {:.4}, upper = {:.4}",
                i, fc, result.lower_bounds[i], result.upper_bounds[i]
            );
        }

        // The first forecast should NOT drop below the last actual value.
        assert!(
            result.forecasts[0] >= last_actual - 1.0,
            "First forecast {:.2} drops below last actual {:.2} -- capacity L likely too low",
            result.forecasts[0],
            last_actual
        );

        // Forecasts should be monotonically non-decreasing (logistic with k>0)
        for w in result.forecasts.windows(2) {
            assert!(
                w[1] >= w[0] - 0.01,
                "Forecasts should be non-decreasing: {:.4} -> {:.4}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn test_logistic_bounded_by_capacity() {
        // Generate clear S-curve data approaching 1000
        let values: Vec<f64> = (0..60)
            .map(|i| 1000.0 / (1.0 + (-0.2 * (i as f64 - 30.0)).exp()))
            .collect();
        let series = make_series(values);
        let result =
            models::logistic::forecast_logistic(&series, 5, 0.95).unwrap();

        // Point forecasts should stay near or below capacity
        for fc in &result.forecasts {
            assert!(
                *fc < 1100.0,
                "Forecast {:.2} exceeds capacity 1000 by too much",
                fc
            );
        }
    }

    #[test]
    fn test_logistic_insufficient_data() {
        let values = vec![1.0, 2.0, 3.0]; // only 3 points, need MIN_DATA_POINTS
        let series = make_series(values);
        let err = models::logistic::forecast_logistic(&series, 3, 0.95).unwrap_err();
        assert!(
            err.to_string().contains("data points"),
            "Error: {}",
            err
        );
    }

    // ==================== Auto mode tests ====================

    #[test]
    fn test_auto_exponential_data_selects_exponential() {
        // Pure exponential growth -- auto should pick exponential over linear
        let values: Vec<f64> = (0..30)
            .map(|i| {
                let base = 10.0 * (0.08 * i as f64).exp();
                base + (i as f64 * 1.3).sin() * 0.5 // tiny noise
            })
            .collect();
        let series = make_series(values);

        // This should succeed regardless of which model is selected
        let result = models::auto::forecast_auto(&series, 5, 0.95);
        assert!(result.is_ok(), "Auto should succeed on exponential data");
        let result = result.unwrap();
        assert_eq!(result.forecasts.len(), 5);

        // The forecasts should be increasing (exponential growth)
        for w in result.forecasts.windows(2) {
            assert!(w[1] > w[0], "Forecasts should increase for exponential data");
        }
    }

    #[test]
    fn test_auto_scurve_data() {
        // S-curve data -- auto should handle gracefully (likely picks logistic)
        let values: Vec<f64> = (0..40)
            .map(|i| 200.0 / (1.0 + (-0.2 * (i as f64 - 20.0)).exp()))
            .collect();
        let series = make_series(values);

        let result = models::auto::forecast_auto(&series, 5, 0.95);
        assert!(result.is_ok(), "Auto should succeed on S-curve data");
        let result = result.unwrap();
        assert_eq!(result.forecasts.len(), 5);

        // Forecasts should be near the capacity, not wildly above
        for fc in &result.forecasts {
            assert!(
                *fc < 250.0,
                "S-curve forecast {:.2} should not overshoot capacity 200 significantly",
                fc
            );
        }
    }

    #[test]
    fn test_auto_linear_data() {
        // Linear data -- auto should work (picks linear or ETS)
        let values: Vec<f64> = (0..20)
            .map(|i| 10.0 + 3.0 * i as f64 + (i as f64 * 0.5).sin())
            .collect();
        let series = make_series(values);

        let result = models::auto::forecast_auto(&series, 5, 0.95);
        assert!(result.is_ok(), "Auto should succeed on linear data");
        assert_eq!(result.unwrap().forecasts.len(), 5);
    }

    #[test]
    fn test_auto_with_negatives_skips_exponential() {
        // Data with negatives -- exponential should be skipped, but auto should still work
        let values: Vec<f64> = (0..20).map(|i| -10.0 + 2.0 * i as f64).collect();
        let series = make_series(values);

        let result = models::auto::forecast_auto(&series, 5, 0.95);
        assert!(
            result.is_ok(),
            "Auto should succeed even with negative values"
        );
        assert_eq!(result.unwrap().forecasts.len(), 5);
    }

    #[test]
    fn test_deceleration_exponential_data() {
        // Exponential growth data -- should NOT show deceleration
        let values: Vec<f64> = (0..12)
            .map(|i| 120.0 * (0.12 * i as f64).exp())
            .collect();
        assert!(
            !models::auto::shows_deceleration(&values),
            "Exponential data should not show deceleration"
        );
    }

    #[test]
    fn test_deceleration_logistic_data() {
        // S-curve data past the inflection point -- SHOULD show deceleration
        let values = vec![
            15.0, 28.0, 55.0, 105.0, 195.0, 340.0, 490.0, 620.0, 720.0, 790.0, 840.0, 875.0,
            900.0, 918.0, 930.0,
        ];
        assert!(
            models::auto::shows_deceleration(&values),
            "S-curve data past inflection should show deceleration"
        );
    }

    #[test]
    fn test_auto_fallback_small_data() {
        // With exactly MIN_DATA_POINTS, should use fallback (not CV)
        let values: Vec<f64> = (0..types::MIN_DATA_POINTS)
            .map(|i| i as f64 * 2.0 + 1.0)
            .collect();
        let series = make_series(values);

        let result = models::auto::forecast_auto(&series, 3, 0.95);
        assert!(
            result.is_ok(),
            "Auto should succeed with minimal data via fallback"
        );
    }

    // ==================== Public API tests ====================

    #[test]
    fn test_forecast_api_linear() {
        let values: Vec<f64> = (0..20).map(|i| 10.0 + 2.0 * i as f64).collect();
        let series = make_series(values);
        let config = ForecastConfig {
            model: ForecastModel::Linear,
            horizon: 5,
            confidence_level: 0.95,
        };
        let result = forecast(&series, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().forecasts.len(), 5);
    }

    #[test]
    fn test_forecast_api_auto() {
        let values: Vec<f64> = (0..20).map(|i| 10.0 + 2.0 * i as f64).collect();
        let series = make_series(values);
        let config = ForecastConfig {
            model: ForecastModel::Auto,
            horizon: 5,
            confidence_level: 0.95,
        };
        let result = forecast(&series, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().forecasts.len(), 5);
    }
}
