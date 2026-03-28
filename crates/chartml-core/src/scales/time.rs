use chrono::NaiveDateTime;
use super::tick_step;

/// Maps a temporal domain (NaiveDateTime) to a continuous range via linear interpolation on timestamps.
/// Equivalent to D3's `scaleUtc()`.
pub struct ScaleTime {
    domain: (NaiveDateTime, NaiveDateTime),
    range: (f64, f64),
}

impl ScaleTime {
    /// Create a new time scale with the given domain and range.
    pub fn new(domain: (NaiveDateTime, NaiveDateTime), range: (f64, f64)) -> Self {
        Self { domain, range }
    }

    /// Map a datetime to a range value via linear interpolation on timestamps.
    pub fn map(&self, value: &NaiveDateTime) -> f64 {
        let t = self.to_timestamp(value);
        let (t0, t1) = self.domain_timestamps();
        let (r0, r1) = self.range;
        let domain_span = t1 - t0;
        if domain_span == 0.0 {
            return (r0 + r1) / 2.0;
        }
        r0 + (t - t0) / domain_span * (r1 - r0)
    }

    /// Inverse: range value back to NaiveDateTime.
    pub fn invert(&self, value: f64) -> NaiveDateTime {
        let (t0, t1) = self.domain_timestamps();
        let (r0, r1) = self.range;
        let range_span = r1 - r0;
        let t = if range_span == 0.0 {
            (t0 + t1) / 2.0
        } else {
            t0 + (value - r0) / range_span * (t1 - t0)
        };
        self.secs_to_datetime(t)
    }

    /// Generate nice time ticks.
    /// Strategy: convert to timestamps, use linear tick algorithm on the timestamp space,
    /// then convert back to NaiveDateTime.
    pub fn ticks(&self, count: usize) -> Vec<NaiveDateTime> {
        if count == 0 {
            return vec![];
        }
        let (t0, t1) = self.domain_timestamps();
        let min = t0.min(t1);
        let max = t0.max(t1);
        if min == max {
            return vec![self.secs_to_datetime(min)];
        }

        let step = tick_step(min, max, count);
        if step == 0.0 || !step.is_finite() {
            return vec![];
        }

        let mut ticks = Vec::new();
        let start = (min / step).ceil();
        let stop = (max / step).floor();

        let mut i = start;
        while i <= stop {
            let tick = i * step;
            ticks.push(self.secs_to_datetime(tick));
            i += 1.0;
        }

        if t0 > t1 {
            ticks.reverse();
        }

        ticks
    }

    /// Get the domain extent.
    pub fn domain(&self) -> (NaiveDateTime, NaiveDateTime) {
        self.domain
    }

    /// Get the range extent.
    pub fn range(&self) -> (f64, f64) {
        self.range
    }

    fn to_timestamp(&self, dt: &NaiveDateTime) -> f64 {
        dt.and_utc().timestamp() as f64
    }

    fn domain_timestamps(&self) -> (f64, f64) {
        (
            self.to_timestamp(&self.domain.0),
            self.to_timestamp(&self.domain.1),
        )
    }

    fn secs_to_datetime(&self, secs: f64) -> NaiveDateTime {
        // Clamp to valid chrono timestamp range to avoid panics
        let secs = (secs as i64).clamp(-8_334_632_851_200, 8_210_298_412_799);
        chrono::DateTime::from_timestamp(secs, 0)
            .unwrap_or_else(|| {
                // Fallback to epoch if somehow still out of range
                chrono::DateTime::from_timestamp(0, 0).unwrap()
            })
            .naive_utc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn make_dt(year: i32, month: u32, day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    #[test]
    fn time_scale_maps_midpoint() {
        let start = make_dt(2024, 1, 1);
        let end = make_dt(2024, 1, 31);
        let mid = make_dt(2024, 1, 16);
        let scale = ScaleTime::new((start, end), (0.0, 100.0));
        let result = scale.map(&mid);
        assert!(
            (result - 50.0).abs() < 2.0,
            "midpoint should map close to 50, got {}",
            result
        );
    }

    #[test]
    fn time_scale_maps_endpoints() {
        let start = make_dt(2024, 1, 1);
        let end = make_dt(2024, 1, 31);
        let scale = ScaleTime::new((start, end), (0.0, 100.0));
        assert!(
            (scale.map(&start) - 0.0).abs() < 1e-10,
            "start should map to 0"
        );
        assert!(
            (scale.map(&end) - 100.0).abs() < 1e-10,
            "end should map to 100"
        );
    }

    #[test]
    fn time_scale_inverts() {
        let start = make_dt(2024, 1, 1);
        let end = make_dt(2024, 1, 31);
        let scale = ScaleTime::new((start, end), (0.0, 100.0));
        let mid_value = 50.0;
        let inverted = scale.invert(mid_value);
        // The inverted value should be roughly the middle date
        let mid = make_dt(2024, 1, 16);
        let diff_secs = (inverted.and_utc().timestamp() - mid.and_utc().timestamp()).abs();
        assert!(
            diff_secs < 86400, // within 1 day
            "inverted date should be close to midpoint, got {:?}",
            inverted
        );
    }

    #[test]
    fn time_scale_ticks() {
        let start = make_dt(2024, 1, 1);
        let end = make_dt(2024, 12, 31);
        let scale = ScaleTime::new((start, end), (0.0, 100.0));
        let ticks = scale.ticks(5);
        assert!(
            !ticks.is_empty(),
            "should generate at least one tick"
        );
        assert!(
            ticks.len() <= 15,
            "should not generate too many ticks, got {}",
            ticks.len()
        );
        // All ticks should be within the domain
        for tick in &ticks {
            assert!(*tick >= start, "tick {:?} should be >= start", tick);
            assert!(*tick <= end, "tick {:?} should be <= end", tick);
        }
    }
}
