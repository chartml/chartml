use augurs_seasons::PeriodogramDetector;
use serde::{Deserialize, Serialize};

use crate::ForecastError;

/// A single detected seasonal period with its strength.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityResult {
    /// The detected period length (e.g., 7 for weekly in daily data).
    pub period: u32,
    /// Strength of the seasonal component, normalized to 0.0 - 1.0.
    pub strength: f64,
}

/// Minimum number of data points required for seasonality detection.
/// Needs enough data to observe at least two full cycles of a potential period.
pub const MIN_SEASONALITY_POINTS: usize = 8;

/// Minimum spectral power (as fraction of max) to consider a period as candidate.
const PERIODOGRAM_POWER_THRESHOLD: f64 = 0.01;

/// Maximum number of candidate periods to validate with autocorrelation.
const MAX_CANDIDATES: usize = 20;

/// Minimum autocorrelation strength to report a detected seasonal period.
const MIN_SEASONALITY_STRENGTH: f64 = 0.1;

/// Detect seasonal periods in a time series.
///
/// Uses a two-stage approach:
/// 1. The `augurs-seasons` periodogram to identify candidate periods from the
///    frequency domain (sorted by spectral power).
/// 2. Autocorrelation at each candidate period to compute a strength measure
///    that is interpretable as 0.0 (no seasonality) to 1.0 (perfect periodicity).
///
/// Only periods with autocorrelation strength > 0.1 are returned.
///
/// # Arguments
/// * `values` - The time series values (evenly spaced assumed).
///
/// # Returns
/// A vector of `SeasonalityResult` sorted by descending strength.
/// Returns an empty vector if no seasonality is detected.
pub fn detect_seasonality(values: &[f64]) -> Result<Vec<SeasonalityResult>, ForecastError> {
    if values.len() < MIN_SEASONALITY_POINTS {
        return Err(ForecastError::InsufficientData {
            required: MIN_SEASONALITY_POINTS,
            actual: values.len(),
            context: "seasonality detection".to_string(),
        });
    }

    // Check for constant data (zero variance) - no seasonality possible
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance =
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    if variance < 1e-10 {
        return Ok(Vec::new());
    }

    // Stage 1: Get candidate periods from the periodogram.
    // Use a low threshold to capture more candidates, then filter by autocorrelation.
    let max_period = (values.len() / 2) as u32;
    let detector = PeriodogramDetector::builder()
        .min_period(2)
        .max_period(max_period)
        .build();

    let periodogram = detector.periodogram(values);

    let max_power = periodogram
        .powers
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    if max_power <= 0.0 || !max_power.is_finite() {
        return Ok(Vec::new());
    }

    // Collect all periods with at least 1% of max power as candidates.
    // The periodogram gives us frequency-domain candidates; we validate
    // them with autocorrelation below.
    let mut candidates: Vec<(u32, f64)> = periodogram
        .periods
        .iter()
        .zip(periodogram.powers.iter())
        .filter(|(_, &power)| power > max_power * PERIODOGRAM_POWER_THRESHOLD)
        .map(|(&period, &power)| (period, power))
        .collect();

    // Sort candidates by power descending to process strongest first
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Limit to top candidates to avoid excessive computation
    candidates.truncate(MAX_CANDIDATES);

    // Stage 2: Validate each candidate with autocorrelation and compute strength.
    let mut results: Vec<SeasonalityResult> = candidates
        .into_iter()
        .filter_map(|(period, _power)| {
            let strength =
                autocorrelation_at_lag(values, period as usize, mean, variance);
            // Only include if autocorrelation indicates meaningful periodicity
            if strength > MIN_SEASONALITY_STRENGTH {
                Some(SeasonalityResult {
                    period,
                    strength: strength.clamp(0.0, 1.0),
                })
            } else {
                None
            }
        })
        .collect();

    // Sort by descending strength
    results.sort_by(|a, b| {
        b.strength
            .partial_cmp(&a.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

/// Compute the autocorrelation of the series at a specific lag.
///
/// Returns a value between -1.0 and 1.0, where values close to 1.0 indicate
/// strong positive correlation at that lag (i.e., strong seasonality).
fn autocorrelation_at_lag(values: &[f64], lag: usize, mean: f64, variance: f64) -> f64 {
    if lag >= values.len() || variance < 1e-10 {
        return 0.0;
    }

    let valid_pairs = (values.len() - lag) as f64;
    let covariance: f64 = values[..values.len() - lag]
        .iter()
        .zip(values[lag..].iter())
        .map(|(a, b)| (a - mean) * (b - mean))
        .sum::<f64>()
        / valid_pairs;

    covariance / variance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekly_pattern_detected() {
        // Generate a year of daily data with a clear weekly pattern
        let values: Vec<f64> = (0..365)
            .map(|i| {
                let weekly =
                    10.0 * (2.0 * std::f64::consts::PI * i as f64 / 7.0).sin();
                50.0 + weekly
            })
            .collect();

        let results = detect_seasonality(&values).unwrap();
        assert!(
            !results.is_empty(),
            "Should detect at least one seasonal period"
        );
        // Period 7 should be among the detected results with high strength
        let has_weekly = results.iter().any(|r| r.period == 7 && r.strength > 0.5);
        assert!(
            has_weekly,
            "Should detect period=7 with high strength, got: {:?}",
            results
        );
    }

    #[test]
    fn test_no_seasonality_constant() {
        let values: Vec<f64> = vec![42.0; 100];
        let results = detect_seasonality(&values).unwrap();
        assert!(
            results.is_empty(),
            "Constant data should have no seasonality"
        );
    }

    #[test]
    fn test_no_seasonality_linear_trend() {
        // Pure linear trend - no periodic component
        let values: Vec<f64> = (0..200).map(|i| 10.0 + 0.5 * i as f64).collect();
        let results = detect_seasonality(&values).unwrap();
        // Linear trend may produce some weak artifacts, but nothing strong
        assert!(
            results.is_empty() || results[0].strength < 0.5,
            "Linear trend should have no strong seasonality, got: {:?}",
            results
        );
    }

    #[test]
    fn test_too_few_points() {
        let values = vec![1.0, 2.0, 3.0];
        let result = detect_seasonality(&values);
        assert!(result.is_err());
    }

    #[test]
    fn test_strength_is_bounded() {
        let values: Vec<f64> = (0..365)
            .map(|i| {
                let weekly =
                    10.0 * (2.0 * std::f64::consts::PI * i as f64 / 7.0).sin();
                50.0 + weekly
            })
            .collect();

        let results = detect_seasonality(&values).unwrap();
        for r in &results {
            assert!(r.strength >= 0.0, "Strength should be >= 0.0");
            assert!(r.strength <= 1.0, "Strength should be <= 1.0");
        }
    }
}
