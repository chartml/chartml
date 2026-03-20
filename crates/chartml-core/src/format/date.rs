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
