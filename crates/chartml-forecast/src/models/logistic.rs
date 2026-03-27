use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{Dyn, OMatrix, OVector, Owned, Vector3, U3};
use statrs::distribution::{ContinuousCDF, StudentsT};

use crate::types::{detect_interval, ForecastResult, TimeSeries, MIN_DATA_POINTS};
use crate::ForecastError;

/// Logistic growth problem for Levenberg-Marquardt optimization.
///
/// Fits y = L / (1 + exp(-k * (x - x0))) with 3 parameters.
///
/// To prevent the capacity L from dropping below the observed maximum
/// (which causes forecasts to "dip" below the last data point), L is
/// reparameterized as:
///
///   L = y_max + exp(alpha)
///
/// The optimizer searches over [alpha, k, x0]. Since exp(alpha) > 0
/// for all alpha, L is guaranteed to exceed y_max.
struct LogisticProblem {
    /// Parameter vector [alpha, k, x0] where L = y_max + exp(alpha)
    params: Vector3<f64>,
    /// Maximum observed y value (floor for L)
    y_max: f64,
    /// x values (0-based indices)
    x: Vec<f64>,
    /// Observed y values
    y: Vec<f64>,
}

impl LogisticProblem {
    /// Compute L from the reparameterized alpha: L = y_max + exp(alpha)
    fn capacity(&self) -> f64 {
        self.y_max + self.params[0].exp()
    }
}

impl LeastSquaresProblem<f64, Dyn, U3> for LogisticProblem {
    type ParameterStorage = Owned<f64, U3>;
    type ResidualStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, U3>;

    fn set_params(&mut self, p: &Vector3<f64>) {
        self.params.copy_from(p);
    }

    fn params(&self) -> Vector3<f64> {
        self.params
    }

    fn residuals(&self) -> Option<OVector<f64, Dyn>> {
        let l = self.capacity();
        let k = self.params[1];
        let x0 = self.params[2];
        let n = self.x.len();

        let mut residuals = OVector::<f64, Dyn>::zeros(n);
        for i in 0..n {
            let exp_term = (-k * (self.x[i] - x0)).exp();
            let denom = 1.0 + exp_term;
            let predicted = l / denom;
            residuals[i] = self.y[i] - predicted;
        }
        Some(residuals)
    }

    fn jacobian(&self) -> Option<OMatrix<f64, Dyn, U3>> {
        let l = self.capacity();
        let exp_alpha = self.params[0].exp(); // dL/dalpha = exp(alpha)
        let k = self.params[1];
        let x0 = self.params[2];
        let n = self.x.len();

        let mut jac = OMatrix::<f64, Dyn, U3>::zeros(n);
        for i in 0..n {
            let exp_term = (-k * (self.x[i] - x0)).exp();
            let denom = 1.0 + exp_term;
            let denom_sq = denom * denom;

            // Jacobian of residual r = y - f(x) -> dr/dp = -df/dp
            // df/dalpha = (df/dL)(dL/dalpha) = (1/denom) * exp(alpha)
            jac[(i, 0)] = -exp_alpha / denom;
            // df/dk = L * (x - x0) * exp_term / denom^2
            jac[(i, 1)] = -l * (self.x[i] - x0) * exp_term / denom_sq;
            // df/dx0 = -L * k * exp_term / denom^2
            jac[(i, 2)] = l * k * exp_term / denom_sq;
        }
        Some(jac)
    }
}

/// Estimate initial logistic parameters from data.
///
/// Returns [alpha, k, x0] where L = y_max + exp(alpha).
/// Uses heuristics: L ~10% above observed max, k from the data range,
/// and x0 at the midpoint.
fn estimate_logistic_params(x: &[f64], _y: &[f64], y_max: f64) -> Vector3<f64> {
    // L_init: 10% above y_max -> alpha = ln(L_init - y_max) = ln(0.1 * y_max)
    let l_surplus = (y_max * 0.1).max(1.0);
    let alpha_init = l_surplus.ln();

    let x_min = x.first().copied().unwrap_or(0.0);
    let x_max = x.last().copied().unwrap_or(1.0);
    let x_range = (x_max - x_min).max(1.0);

    // k_init: steepness from data range
    let k_init = 4.0 / x_range;

    // x0_init: midpoint of range
    let x0_init = (x_max + x_min) / 2.0;

    Vector3::new(alpha_init, k_init, x0_init)
}

/// Run logistic growth forecasting on a time series.
///
/// Fits y = L / (1 + exp(-k * (x - x0))) using Levenberg-Marquardt
/// nonlinear least squares. Generates prediction intervals via the
/// delta method (first-order Taylor expansion of parameter uncertainty).
pub fn forecast_logistic(
    series: &TimeSeries,
    horizon: usize,
    confidence_level: f64,
) -> Result<ForecastResult, ForecastError> {
    let n = series.len();
    if n < MIN_DATA_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_DATA_POINTS,
            actual: n,
            context: "logistic forecasting".to_string(),
        });
    }

    let x_vals: Vec<f64> = (0..n).map(|i| i as f64).collect();

    // y_max is the floor for capacity L (reparameterization ensures L > y_max)
    let y_max = series
        .values
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    // Estimate initial parameters [alpha, k, x0]
    let initial_params = estimate_logistic_params(&x_vals, &series.values, y_max);

    // Set up and solve the optimization problem
    let problem = LogisticProblem {
        params: initial_params,
        y_max,
        x: x_vals.clone(),
        y: series.values.clone(),
    };

    let (result, report) = LevenbergMarquardt::new().minimize(problem);

    if !report.termination.was_successful() {
        let l_init = y_max + initial_params[0].exp();
        return Err(ForecastError::Convergence(format!(
            "Logistic model failed to converge (initial params: L={:.2}, k={:.4}, x0={:.2}). \
             Try a different model or ensure data follows an S-curve pattern.",
            l_init, initial_params[1], initial_params[2]
        )));
    }

    // Recover L from reparameterized alpha
    let mut l = result.capacity();
    let k = result.params[1];
    let x0 = result.params[2];

    // Post-hoc capacity adjustment: ensure the model doesn't predict below the
    // last observation. The logistic function asymptotically approaches L but
    // never reaches it, so if L ~ y_max the forecast can visually "drop" below
    // the last data point. Nudge L upward so f(x_last) >= y_last.
    let x_last = (n - 1) as f64;
    let exp_last = (-k * (x_last - x0)).exp();
    let f_last = l / (1.0 + exp_last);
    let y_last = series.values[n - 1];
    if f_last < y_last {
        // Solve: y_last = L_new / (1 + exp_last) -> L_new = y_last * (1 + exp_last)
        // Add tiny margin to avoid floating-point edge case
        l = y_last * (1.0 + exp_last) * 1.001;
    }

    eprintln!(
        "chartml-forecast: logistic fit: L={:.4}, k={:.4}, x0={:.4} (y_max={:.4})",
        l, k, x0, y_max
    );

    // Compute residual variance using the final (adjusted) L
    let df = n as f64 - 3.0;
    if df <= 0.0 {
        return Err(ForecastError::InsufficientData {
            required: 4,
            actual: n,
            context: "logistic prediction intervals".to_string(),
        });
    }

    let ss_res: f64 = x_vals
        .iter()
        .zip(series.values.iter())
        .map(|(&xi, &yi)| {
            let pred = l / (1.0 + (-k * (xi - x0)).exp());
            (yi - pred).powi(2)
        })
        .sum();
    let s_sq = ss_res / df;

    // Build the data Jacobian w.r.t. [L, k, x0] using the final (adjusted) L.
    // We switch from the reparameterized [alpha, k, x0] to direct [L, k, x0]
    // because the post-hoc L adjustment invalidates the reparameterized covariance.
    let mut data_jac = OMatrix::<f64, Dyn, U3>::zeros(n);
    for i in 0..n {
        let exp_term = (-k * (x_vals[i] - x0)).exp();
        let denom = 1.0 + exp_term;
        let denom_sq = denom * denom;
        data_jac[(i, 0)] = -1.0 / denom; // dr/dL
        data_jac[(i, 1)] = -l * (x_vals[i] - x0) * exp_term / denom_sq; // dr/dk
        data_jac[(i, 2)] = l * k * exp_term / denom_sq; // dr/dx0
    }

    let jtj = data_jac.transpose() * &data_jac;

    // If J^T*J is singular (e.g. near-perfect fit or degenerate data), fall back
    // to residual-only prediction intervals (no parameter uncertainty component).
    let param_cov = jtj.try_inverse().map(|inv| inv * s_sq);

    if param_cov.is_none() {
        eprintln!("chartml-forecast: logistic: J^T*J singular, using residual-only intervals");
    }

    // Generate forecasts with prediction intervals
    let interval_days = detect_interval(&series.timestamps);
    let last_ts = series
        .last_timestamp()
        .ok_or(ForecastError::EmptyTimeSeries)?;

    let t_dist = StudentsT::new(0.0, 1.0, df)
        .map_err(|e| ForecastError::Prediction(format!("Failed to create t-distribution: {}", e)))?;
    let ci_alpha = 1.0 - confidence_level;
    let t_value = t_dist.inverse_cdf(1.0 - ci_alpha / 2.0);

    let mut timestamps = Vec::with_capacity(horizon);
    let mut forecasts = Vec::with_capacity(horizon);
    let mut lower_bounds = Vec::with_capacity(horizon);
    let mut upper_bounds = Vec::with_capacity(horizon);

    for i in 1..=horizon {
        let x_pred = (n - 1 + i) as f64;
        let exp_term = (-k * (x_pred - x0)).exp();
        let denom = 1.0 + exp_term;
        let denom_sq = denom * denom;
        let y_hat = l / denom;

        // Prediction variance: delta method if covariance available, else residual-only
        let pred_var = if let Some(ref cov) = param_cov {
            // Jacobian of f w.r.t. [L, k, x0] at this forecast point
            let j_pred = Vector3::new(
                1.0 / denom,                               // df/dL
                l * (x_pred - x0) * exp_term / denom_sq,   // df/dk
                -l * k * exp_term / denom_sq,               // df/dx0
            );
            (j_pred.transpose() * cov * j_pred)[(0, 0)] + s_sq
        } else {
            // Residual-only: no parameter uncertainty, just observation noise
            s_sq
        };

        let pred_se = pred_var.max(0.0).sqrt();
        let pi_width = t_value * pred_se;

        if y_hat.is_nan() || pi_width.is_nan() || pi_width.is_infinite() {
            return Err(ForecastError::Prediction(
                "Logistic prediction interval computation produced invalid values".to_string(),
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
