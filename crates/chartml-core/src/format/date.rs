/// Date formatter wrapping chrono's strftime.
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct DateFormatter {
    format_str: String,
}

impl DateFormatter {
    pub fn new(format_str: &str) -> Self {
        Self {
            format_str: format_str.to_string(),
        }
    }

    /// Format a chrono NaiveDateTime.
    pub fn format_datetime(&self, dt: &NaiveDateTime) -> String {
        dt.format(&self.format_str).to_string()
    }

    /// Format a date string (parse then format).
    /// Accepts ISO date strings like "2024-01-15" or "2024-01-15T10:30:00".
    pub fn format_date_str(&self, date_str: &str) -> Option<String> {
        // Try parsing as NaiveDateTime first, then as NaiveDate
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S") {
            return Some(self.format_datetime(&dt));
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.f") {
            return Some(self.format_datetime(&dt));
        }
        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            let dt = date.and_hms_opt(0, 0, 0)?;
            return Some(self.format_datetime(&dt));
        }
        None
    }
}

/// Special format marker returned by `detect_date_format` when all dates fall
/// on quarter boundaries (1st of Jan/Apr/Jul/Oct). Handled specially by
/// `reformat_date_label`.
pub(crate) const QUARTER_FORMAT: &str = "__QUARTER__";

/// Detect if labels look like dates and return a compact display format.
/// Checks a sample of labels for ISO date patterns (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS).
/// Returns a chrono strftime format string for compact display, or the special
/// `QUARTER_FORMAT` marker when all dates are quarter boundaries.
pub fn detect_date_format(labels: &[String]) -> Option<String> {
    if labels.is_empty() {
        return None;
    }

    // Sample up to 5 labels to check
    let sample_size = labels.len().min(5);
    let mut date_count = 0;
    let mut has_time = false;

    for label in labels.iter().take(sample_size) {
        let trimmed = label.trim();
        if chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() {
            date_count += 1;
        } else if chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S").is_ok()
            || chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        {
            date_count += 1;
            has_time = true;
        }
    }

    // If at least 80% of sampled labels are dates, treat as date axis
    if date_count * 5 >= sample_size * 4 {
        // Check for quarterly pattern: all dates fall on the 1st of Jan/Apr/Jul/Oct
        if !has_time && is_quarterly(labels) {
            return Some(QUARTER_FORMAT.to_string());
        }

        // Check if the data spans multiple calendar years
        let multi_year = spans_multiple_years(labels);

        if has_time {
            // If all timestamps share the same calendar date, use time-only format
            if all_same_date(labels) {
                Some("%H:%M".to_string())
            } else if multi_year {
                Some("%b %d '%y %H:%M".to_string())
            } else {
                Some("%b %d %H:%M".to_string())
            }
        } else if multi_year {
            Some("%b '%y".to_string())
        } else {
            Some("%b %d".to_string())
        }
    } else {
        None
    }
}

/// Check whether ALL parseable date labels fall on quarter boundaries
/// (day == 1 and month in {1, 4, 7, 10}).
fn is_quarterly(labels: &[String]) -> bool {
    use chrono::Datelike;

    const QUARTER_MONTHS: [u32; 4] = [1, 4, 7, 10];

    let mut found_any = false;
    for label in labels {
        let trimmed = label.trim();
        if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
            if d.day() != 1 || !QUARTER_MONTHS.contains(&d.month()) {
                return false;
            }
            found_any = true;
        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
            let d = dt.date();
            if d.day() != 1 || !QUARTER_MONTHS.contains(&d.month()) {
                return false;
            }
            found_any = true;
        }
        // Non-date labels are ignored (the caller already verified date-ness)
    }
    found_any
}

/// Check whether ALL timestamps share the same calendar date (YYYY-MM-DD).
/// When true, we can use a short time-only format like "%H:%M" instead of
/// including the date in every label.
fn all_same_date(labels: &[String]) -> bool {
    fn extract_date_prefix(s: &str) -> Option<String> {
        let t = s.trim();
        // Extract the YYYY-MM-DD portion from datetime strings
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S") {
            return Some(dt.date().to_string());
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f") {
            return Some(dt.date().to_string());
        }
        None
    }

    let mut first_date: Option<String> = None;
    for label in labels {
        if let Some(d) = extract_date_prefix(label) {
            match &first_date {
                None => first_date = Some(d),
                Some(fd) => {
                    if *fd != d {
                        return false;
                    }
                }
            }
        }
    }
    // Only return true if we actually found at least one date
    first_date.is_some()
}

/// Check whether the labels span multiple calendar years by comparing the
/// year of the first parseable date with the year of the last parseable date.
fn spans_multiple_years(labels: &[String]) -> bool {
    fn extract_year(s: &str) -> Option<i32> {
        use chrono::Datelike;
        let t = s.trim();
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
            return Some(d.year());
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S") {
            return Some(dt.date().year());
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f") {
            return Some(dt.date().year());
        }
        None
    }

    let first_year = labels.iter().find_map(|l| extract_year(l));
    let last_year = labels.iter().rev().find_map(|l| extract_year(l));

    match (first_year, last_year) {
        (Some(f), Some(l)) => f != l,
        _ => false,
    }
}

/// Reformat a date label using the given strftime format string.
/// If `format_str` is the special `QUARTER_FORMAT` marker, formats as "Q1 '23" etc.
/// If the label cannot be parsed as a date, returns it unchanged.
pub fn reformat_date_label(label: &str, format_str: &str) -> String {
    if format_str == QUARTER_FORMAT {
        return format_as_quarter(label).unwrap_or_else(|| label.to_string());
    }
    let formatter = DateFormatter::new(format_str);
    formatter.format_date_str(label).unwrap_or_else(|| label.to_string())
}

/// Format a date string as a quarter label like "Q1 '23".
fn format_as_quarter(label: &str) -> Option<String> {
    use chrono::Datelike;

    let trimmed = label.trim();
    let date = if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        d
    } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        dt.date()
    } else {
        return None;
    };

    let quarter = match date.month() {
        1 => 1,
        4 => 2,
        7 => 3,
        10 => 4,
        _ => return None,
    };

    let year_short = date.year() % 100;
    Some(format!("Q{} '{:02}", quarter, year_short))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_format_date_identity() {
        assert_eq!(
            DateFormatter::new("%Y-%m-%d").format_date_str("2024-01-15"),
            Some("2024-01-15".to_string())
        );
    }

    #[test]
    fn test_format_date_month_day_year() {
        assert_eq!(
            DateFormatter::new("%b %d, %Y").format_date_str("2024-01-15"),
            Some("Jan 15, 2024".to_string())
        );
    }

    #[test]
    fn test_format_date_month_year() {
        assert_eq!(
            DateFormatter::new("%b %Y").format_date_str("2024-03-01"),
            Some("Mar 2024".to_string())
        );
    }

    #[test]
    fn test_format_datetime() {
        let dt = chrono::NaiveDate::from_ymd_opt(2024, 6, 15)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        assert_eq!(
            DateFormatter::new("%Y-%m-%d %H:%M").format_datetime(&dt),
            "2024-06-15 14:30"
        );
    }

    #[test]
    fn test_format_date_str_with_time() {
        assert_eq!(
            DateFormatter::new("%Y-%m-%d %H:%M:%S").format_date_str("2024-01-15T10:30:00"),
            Some("2024-01-15 10:30:00".to_string())
        );
    }

    #[test]
    fn test_format_date_str_invalid() {
        assert_eq!(
            DateFormatter::new("%Y-%m-%d").format_date_str("not-a-date"),
            None
        );
    }

    #[test]
    fn test_detect_quarterly_format() {
        let labels: Vec<String> = vec![
            "2023-01-01".into(), "2023-04-01".into(),
            "2023-07-01".into(), "2023-10-01".into(),
        ];
        let fmt = detect_date_format(&labels).unwrap();
        assert_eq!(fmt, QUARTER_FORMAT);
    }

    #[test]
    fn test_reformat_quarterly_labels() {
        assert_eq!(reformat_date_label("2023-01-01", QUARTER_FORMAT), "Q1 '23");
        assert_eq!(reformat_date_label("2023-04-01", QUARTER_FORMAT), "Q2 '23");
        assert_eq!(reformat_date_label("2023-07-01", QUARTER_FORMAT), "Q3 '23");
        assert_eq!(reformat_date_label("2023-10-01", QUARTER_FORMAT), "Q4 '23");
        assert_eq!(reformat_date_label("2025-01-01", QUARTER_FORMAT), "Q1 '25");
    }

    #[test]
    fn test_non_quarterly_dates_not_detected_as_quarterly() {
        // Monthly dates should NOT trigger quarterly detection
        let labels: Vec<String> = vec![
            "2023-01-01".into(), "2023-02-01".into(),
            "2023-03-01".into(), "2023-04-01".into(),
        ];
        let fmt = detect_date_format(&labels).unwrap();
        assert_ne!(fmt, QUARTER_FORMAT);
    }
}
