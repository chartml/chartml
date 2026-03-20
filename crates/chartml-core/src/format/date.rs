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

/// Detect if labels look like dates and return a compact display format.
/// Checks a sample of labels for ISO date patterns (YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS).
/// Returns a chrono strftime format string for compact display.
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
        if has_time {
            Some("%b %d %H:%M".to_string())
        } else {
            Some("%b %d".to_string())
        }
    } else {
        None
    }
}

/// Reformat a date label using the given strftime format string.
/// If the label cannot be parsed as a date, returns it unchanged.
pub fn reformat_date_label(label: &str, format_str: &str) -> String {
    let formatter = DateFormatter::new(format_str);
    formatter.format_date_str(label).unwrap_or_else(|| label.to_string())
}

#[cfg(test)]
mod tests {
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
}
