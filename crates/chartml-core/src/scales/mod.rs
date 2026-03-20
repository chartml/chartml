mod linear;
mod band;

pub use linear::ScaleLinear;
pub use band::ScaleBand;

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
