mod linear;
mod band;
mod time;
mod ordinal;
mod sqrt;

pub use linear::ScaleLinear;
pub use band::ScaleBand;
pub use time::ScaleTime;
pub use ordinal::ScaleOrdinal;
pub use sqrt::ScaleSqrt;

/// Common interface for continuous scales.
/// Note: Each scale type also has its own specific methods beyond this trait.
pub trait ContinuousScale {
    /// Map a domain value to a range value.
    fn map(&self, value: f64) -> f64;
    /// Get the domain extent as (min, max).
    fn domain(&self) -> (f64, f64);
    /// Get the range extent as (min, max).
    fn range(&self) -> (f64, f64);
    /// Generate approximately `count` nice tick values within the domain.
    fn ticks(&self, count: usize) -> Vec<f64>;
    /// Clamp a value to domain bounds.
    fn clamp(&self, value: f64) -> f64;
}

/// Calculate a nice step size for the given range and approximate tick count.
/// Uses D3's tick step algorithm with sqrt(50)/sqrt(10)/sqrt(2) thresholds.
pub(crate) fn tick_step(min: f64, max: f64, count: usize) -> f64 {
    let raw_step = (max - min) / count as f64;
    let magnitude = 10_f64.powf(raw_step.log10().floor());
    let error = raw_step / magnitude;

    if error >= 50_f64.sqrt() {
        10.0 * magnitude
    } else if error >= 10_f64.sqrt() {
        5.0 * magnitude
    } else if error >= 2_f64.sqrt() {
        2.0 * magnitude
    } else {
        magnitude
    }
}

/// Round a value to remove floating point artifacts based on step precision.
pub(crate) fn round_to_precision(value: f64, step: f64) -> f64 {
    if step == 0.0 {
        return value;
    }
    let decimals = (-step.log10().floor()).max(0.0) as i32;
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}
