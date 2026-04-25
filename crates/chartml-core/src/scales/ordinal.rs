/// Maps discrete domain values to discrete range values (typically colors).
/// Equivalent to D3's `scaleOrdinal()`.
pub struct ScaleOrdinal {
    domain: Vec<String>,
    range: Vec<String>,
}

impl ScaleOrdinal {
    /// Create a new ordinal scale with the given domain and range.
    pub fn new(domain: Vec<String>, range: Vec<String>) -> Self {
        Self { domain, range }
    }

    /// Map a domain value to a range value.
    /// Uses index-based lookup: domain[i] -> range[i % range.len()]
    /// Returns None if the value is not in the domain or if the range is empty.
    pub fn map(&self, value: &str) -> Option<&str> {
        if self.range.is_empty() {
            return None;
        }
        let index = self.domain.iter().position(|d| d == value)?;
        Some(&self.range[index % self.range.len()])
    }

    /// Get the range value at index i (cycling if i >= range.len()).
    /// Returns None if the range is empty.
    pub fn map_index(&self, index: usize) -> Option<&str> {
        if self.range.is_empty() {
            return None;
        }
        Some(&self.range[index % self.range.len()])
    }

    /// Get the domain values.
    pub fn domain(&self) -> &[String] {
        &self.domain
    }

    /// Get the range values.
    pub fn range(&self) -> &[String] {
        &self.range
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn ordinal_basic_mapping() {
        let scale = ScaleOrdinal::new(
            vec!["a".into(), "b".into(), "c".into()],
            vec!["red".into(), "green".into(), "blue".into()],
        );
        assert_eq!(scale.map("a"), Some("red"));
        assert_eq!(scale.map("b"), Some("green"));
        assert_eq!(scale.map("c"), Some("blue"));
    }

    #[test]
    fn ordinal_cycling() {
        let scale = ScaleOrdinal::new(
            vec!["a".into(), "b".into(), "c".into(), "d".into()],
            vec!["red".into(), "green".into()],
        );
        assert_eq!(scale.map("a"), Some("red"));
        assert_eq!(scale.map("b"), Some("green"));
        assert_eq!(scale.map("c"), Some("red"));   // cycles back
        assert_eq!(scale.map("d"), Some("green"));  // cycles back
    }

    #[test]
    fn ordinal_unknown_returns_none() {
        let scale = ScaleOrdinal::new(
            vec!["a".into(), "b".into()],
            vec!["red".into(), "green".into()],
        );
        assert_eq!(scale.map("unknown"), None);
    }

    #[test]
    fn ordinal_empty_range() {
        let scale = ScaleOrdinal::new(
            vec!["a".into(), "b".into()],
            vec![],
        );
        assert_eq!(scale.map("a"), None);
    }

    #[test]
    fn ordinal_map_index() {
        let scale = ScaleOrdinal::new(
            vec!["a".into(), "b".into()],
            vec!["red".into(), "green".into(), "blue".into()],
        );
        assert_eq!(scale.map_index(0), Some("red"));
        assert_eq!(scale.map_index(1), Some("green"));
        assert_eq!(scale.map_index(2), Some("blue"));
        assert_eq!(scale.map_index(3), Some("red")); // cycles
    }
}
