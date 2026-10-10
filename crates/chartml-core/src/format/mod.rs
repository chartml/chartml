mod number;
mod date;

pub use number::NumberFormatter;
pub use date::{DateFormatter, detect_date_format, reformat_date_label};

/// Format a numeric value using a d3-format string, or a sensible default.
///
/// When `format_str` is `Some`, delegates to [`NumberFormatter`]. When `None`,
/// uses [`default_format_value`] which applies comma separators for integers
/// and minimal-precision formatting for floats.
pub fn format_value(value: f64, format_str: Option<&str>) -> String {
    match format_str {
        Some(fmt) => NumberFormatter::new(fmt).format(value),
        None => default_format_value(value),
    }
}

/// Default numeric formatting: integers get comma separators (e.g. `847,293`),
/// floats get minimal precision with trailing-zero trimming.
pub fn default_format_value(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        // Use comma separator for large integers
        let abs = value.abs() as u64;
        let formatted = number::insert_commas(&abs.to_string());
        if value < 0.0 {
            format!("-{}", formatted)
        } else {
            formatted
        }
    } else {
        // Use enough decimal places to show significant digits.
        // For values like 0.007, we need 3 decimals; for 1.5, 1 decimal suffices.
        let abs_val = value.abs();
        let precision = if abs_val < 1e-15 {
            1usize
        } else if abs_val >= 1.0 {
            // For values >= 1, one decimal is fine (e.g. 3.5 -> "3.5")
            1usize
        } else {
            // For values < 1, compute digits needed: -floor(log10(abs)) gives the
            // position of the first significant digit. Add 1 to show at least two
            // significant fractional digits (e.g. 0.007 -> precision 3 -> "0.007").
            let digits = -(abs_val.log10().floor()) as usize;
            digits.max(1)
        };
        // Format and strip unnecessary trailing zeros after the decimal point,
        // but keep at least one decimal digit.
        let formatted = format!("{:.prec$}", value, prec = precision);
        let trimmed = formatted.trim_end_matches('0');
        // Ensure we don't end with just a decimal point (e.g. "3." -> "3.0")
        if trimmed.ends_with('.') {
            format!("{}0", trimmed)
        } else {
            trimmed.to_string()
        }
    }
}
