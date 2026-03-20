use std::collections::HashMap;

/// A row of data — a map of field names to values.
/// Matches the JS behavior where data is an array of objects.
pub type Row = HashMap<String, serde_json::Value>;

/// Extract an f64 value from a Row by field name.
/// Handles both Number and String (parsed) values.
pub fn get_f64(row: &Row, field: &str) -> Option<f64> {
    match row.get(field)? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Extract a string value from a Row by field name.
pub fn get_string(row: &Row, field: &str) -> Option<String> {
    match row.get(field)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// Compute the extent (min, max) of a numeric field across rows.
/// Equivalent to D3's d3.extent().
pub fn extent(data: &[Row], field: &str) -> Option<(f64, f64)> {
    let values: Vec<f64> = data.iter().filter_map(|row| get_f64(row, field)).collect();
    if values.is_empty() {
        return None;
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some((min, max))
}

/// Sum a numeric field across rows.
pub fn sum(data: &[Row], field: &str) -> f64 {
    data.iter().filter_map(|row| get_f64(row, field)).sum()
}

/// Group rows by a field value.
pub fn group_by<'a>(data: &'a [Row], field: &str) -> HashMap<String, Vec<&'a Row>> {
    let mut groups: HashMap<String, Vec<&'a Row>> = HashMap::new();
    for row in data {
        if let Some(key) = get_string(row, field) {
            groups.entry(key).or_default().push(row);
        }
    }
    groups
}

/// Get unique values for a field, in order of first appearance.
pub fn unique_values(data: &[Row], field: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for row in data {
        if let Some(val) = get_string(row, field) {
            if seen.insert(val.clone()) {
                result.push(val);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn get_f64_from_number() {
        let row = make_row(vec![("value", json!(42.5))]);
        assert_eq!(get_f64(&row, "value"), Some(42.5));
    }

    #[test]
    fn get_f64_from_string() {
        let row = make_row(vec![("value", json!("123.45"))]);
        assert_eq!(get_f64(&row, "value"), Some(123.45));
    }

    #[test]
    fn get_f64_missing_field() {
        let row = make_row(vec![("other", json!(1.0))]);
        assert_eq!(get_f64(&row, "value"), None);
    }

    #[test]
    fn get_string_from_various() {
        let row_num = make_row(vec![("x", json!(42))]);
        assert_eq!(get_string(&row_num, "x"), Some("42".to_string()));

        let row_str = make_row(vec![("x", json!("hello"))]);
        assert_eq!(get_string(&row_str, "x"), Some("hello".to_string()));

        let row_bool = make_row(vec![("x", json!(true))]);
        assert_eq!(get_string(&row_bool, "x"), Some("true".to_string()));

        let row_null = make_row(vec![("x", json!(null))]);
        assert_eq!(get_string(&row_null, "x"), None);
    }

    #[test]
    fn extent_basic() {
        let data = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(30.0))]),
            make_row(vec![("v", json!(20.0))]),
        ];
        assert_eq!(extent(&data, "v"), Some((10.0, 30.0)));
    }

    #[test]
    fn extent_empty() {
        let data: Vec<Row> = vec![];
        assert_eq!(extent(&data, "v"), None);

        // Also test with rows that don't have the field
        let data = vec![make_row(vec![("other", json!(1.0))])];
        assert_eq!(extent(&data, "v"), None);
    }

    #[test]
    fn sum_basic() {
        let data = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(20.0))]),
            make_row(vec![("v", json!(30.0))]),
        ];
        assert_eq!(sum(&data, "v"), 60.0);
    }

    #[test]
    fn group_by_basic() {
        let data = vec![
            make_row(vec![("cat", json!("A")), ("v", json!(1))]),
            make_row(vec![("cat", json!("B")), ("v", json!(2))]),
            make_row(vec![("cat", json!("A")), ("v", json!(3))]),
        ];
        let groups = group_by(&data, "cat");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["A"].len(), 2);
        assert_eq!(groups["B"].len(), 1);
    }

    #[test]
    fn unique_values_preserves_order() {
        let data = vec![
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("apple"))]),
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("cherry"))]),
            make_row(vec![("x", json!("apple"))]),
        ];
        let uniq = unique_values(&data, "x");
        assert_eq!(uniq, vec!["banana", "apple", "cherry"]);
    }
}
