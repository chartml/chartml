use crate::format::NumberFormatter;

/// Position of the axis relative to the chart area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisPosition {
    Bottom, // X-axis
    Left,   // Primary Y-axis
    Right,  // Secondary Y-axis
    Top,    // Rare, but supported
}

/// A single tick mark with its position and label.
#[derive(Debug, Clone)]
pub struct TickMark {
    pub position: f64,    // Pixel position along the axis
    pub value: f64,       // The actual data value
    pub label: String,    // Formatted label text
}

/// A single tick mark for band/category axes.
#[derive(Debug, Clone)]
pub struct CategoryTickMark {
    pub position: f64,    // Pixel position (center of band)
    pub label: String,    // Category label text
    pub bandwidth: f64,   // Width of the band
}

/// Calculate the optimal number of ticks based on available axis length in pixels.
/// Formula: floor(axis_length / 50), clamped to 4..=10
/// Matches JS chartml: maxFittableTicks = floor(chartWidth / 50), clamped 4-10.
pub fn adaptive_tick_count(axis_length_px: f64) -> usize {
    ((axis_length_px / 50.0).floor() as usize).clamp(4, 10)
}

/// Generates tick positions and formatted labels for a continuous axis.
pub struct AxisLayout {
    position: AxisPosition,
    tick_count: usize,
    formatter: Option<NumberFormatter>,
}

impl AxisLayout {
    pub fn new(position: AxisPosition) -> Self {
        Self {
            position,
            tick_count: 5, // reasonable default
            formatter: None,
        }
    }

    pub fn bottom() -> Self { Self::new(AxisPosition::Bottom) }
    pub fn left() -> Self { Self::new(AxisPosition::Left) }
    pub fn right() -> Self { Self::new(AxisPosition::Right) }

    pub fn tick_count(mut self, count: usize) -> Self {
        self.tick_count = count;
        self
    }

    pub fn formatter(mut self, fmt: NumberFormatter) -> Self {
        self.formatter = Some(fmt);
        self
    }

    /// Generate tick marks for a continuous scale.
    /// Takes the scale's domain and range, generates ticks, maps them to positions.
    pub fn generate_continuous_ticks(
        &self,
        domain: (f64, f64),
        range: (f64, f64),
    ) -> Vec<TickMark> {
        use crate::scales::ScaleLinear;
        let scale = ScaleLinear::new(domain, range);
        let tick_values = scale.ticks(self.tick_count);

        tick_values.iter().map(|&value| {
            let position = scale.map(value);
            let label = match &self.formatter {
                Some(fmt) => fmt.format(value),
                None => default_format(value),
            };
            TickMark { position, value, label }
        }).collect()
    }

    /// Generate tick marks for a band/category scale.
    pub fn generate_band_ticks(
        &self,
        labels: &[String],
        range: (f64, f64),
    ) -> Vec<CategoryTickMark> {
        use crate::scales::ScaleBand;
        let scale = ScaleBand::new(labels.to_vec(), range);
        let bandwidth = scale.bandwidth();

        labels.iter().map(|label| {
            let position = scale.map(label).unwrap_or(0.0) + bandwidth / 2.0; // center of band
            CategoryTickMark {
                position,
                label: label.clone(),
                bandwidth,
            }
        }).collect()
    }

    pub fn position(&self) -> AxisPosition { self.position }
}

/// Default number format: no trailing zeros, reasonable precision.
fn default_format(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        // Trim trailing zeros
        let s = format!("{:.6}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_ticks_count() {
        let axis = AxisLayout::bottom().tick_count(5);
        let ticks = axis.generate_continuous_ticks((0.0, 100.0), (0.0, 500.0));
        assert!(ticks.len() >= 3 && ticks.len() <= 10,
            "Expected 3-10 ticks, got {}", ticks.len());
    }

    #[test]
    fn continuous_ticks_positions() {
        let axis = AxisLayout::bottom().tick_count(5);
        let ticks = axis.generate_continuous_ticks((0.0, 100.0), (0.0, 500.0));
        for tick in &ticks {
            assert!(tick.position >= 0.0 && tick.position <= 500.0,
                "Tick position {} out of range [0, 500]", tick.position);
        }
    }

    #[test]
    fn continuous_ticks_with_formatter() {
        let fmt = NumberFormatter::new("$,.0f");
        let axis = AxisLayout::bottom().tick_count(5).formatter(fmt);
        let ticks = axis.generate_continuous_ticks((0.0, 100.0), (0.0, 500.0));
        for tick in &ticks {
            assert!(tick.label.starts_with('$') || tick.label.starts_with("-$"),
                "Expected label starting with '$', got '{}'", tick.label);
        }
    }

    #[test]
    fn band_ticks_centered() {
        let labels: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        let axis = AxisLayout::bottom();
        let ticks = axis.generate_band_ticks(&labels, (0.0, 300.0));
        for tick in &ticks {
            // Center position should be within the range
            assert!(tick.position >= 0.0 && tick.position <= 300.0,
                "Band tick position {} out of range", tick.position);
        }
    }

    #[test]
    fn band_ticks_count_matches_labels() {
        let labels: Vec<String> = vec!["A".into(), "B".into(), "C".into(), "D".into()];
        let axis = AxisLayout::bottom();
        let ticks = axis.generate_band_ticks(&labels, (0.0, 400.0));
        assert_eq!(ticks.len(), labels.len(),
            "Expected {} ticks, got {}", labels.len(), ticks.len());
    }

    #[test]
    fn default_format_integer() {
        assert_eq!(default_format(100.0), "100");
    }

    #[test]
    fn default_format_decimal() {
        assert_eq!(default_format(3.14), "3.14");
    }

    #[test]
    fn adaptive_tick_count_small() {
        assert_eq!(adaptive_tick_count(150.0), 4); // floor(150/50)=3, clamped to 4
    }

    #[test]
    fn adaptive_tick_count_medium() {
        assert_eq!(adaptive_tick_count(350.0), 7);
    }

    #[test]
    fn adaptive_tick_count_large() {
        assert_eq!(adaptive_tick_count(600.0), 10); // floor(600/50)=12, clamped to 10
    }
}
