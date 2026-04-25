use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformSpec {
    pub sql: Option<SqlSpec>,
    pub aggregate: Option<AggregateSpec>,
    pub forecast: Option<ForecastSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SqlSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForecastSpec {
    pub timestamp: String,
    pub value: String,
    pub horizon: Option<u64>,
    pub confidence_level: Option<f64>,
    pub model: Option<String>,
    #[serde(default)]
    pub group_by: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateSpec {
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub measures: Vec<Measure>,
    pub filters: Option<FilterGroup>,
    pub sort: Option<Vec<SortSpec>>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Dimension {
    Simple(String),
    Detailed(DimensionSpec),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionSpec {
    pub column: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub dim_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measure {
    pub column: Option<String>,
    pub aggregation: Option<String>,
    pub name: String,
    pub expression: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterGroup {
    pub combinator: Option<String>,
    pub rules: Vec<FilterRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub field: String,
    pub operator: String,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub field: String,
    pub direction: Option<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_backward_compat_aggregate_only() {
        let yaml = r#"
aggregate:
  dimensions:
    - region
  measures:
    - column: amount
      aggregation: sum
      name: totalAmount
  limit: 10
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.aggregate.is_some());
        assert!(spec.sql.is_none());
        assert!(spec.forecast.is_none());
        let agg = spec.aggregate.unwrap();
        assert_eq!(agg.dimensions.len(), 1);
        assert_eq!(agg.measures.len(), 1);
        assert_eq!(agg.limit, Some(10));
    }

    #[test]
    fn test_sql_single_string() {
        let yaml = r#"
sql: "SELECT * FROM sales WHERE year = 2024"
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.aggregate.is_none());
        assert!(spec.forecast.is_none());
        match spec.sql.unwrap() {
            SqlSpec::Single(s) => assert_eq!(s, "SELECT * FROM sales WHERE year = 2024"),
            other => panic!("Expected SqlSpec::Single, got {:?}", other),
        }
    }

    #[test]
    fn test_sql_multiple_strings() {
        let yaml = r#"
sql:
  - "SELECT * FROM sales"
  - "WHERE year = 2024"
  - "ORDER BY revenue DESC"
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        match spec.sql.unwrap() {
            SqlSpec::Multiple(v) => {
                assert_eq!(v.len(), 3);
                assert_eq!(v[0], "SELECT * FROM sales");
                assert_eq!(v[1], "WHERE year = 2024");
                assert_eq!(v[2], "ORDER BY revenue DESC");
            }
            other => panic!("Expected SqlSpec::Multiple, got {:?}", other),
        }
    }

    #[test]
    fn test_forecast_all_fields() {
        let yaml = r#"
forecast:
  timestamp: date
  value: revenue
  horizon: 60
  confidenceLevel: 0.99
  model: arima
  groupBy:
    - region
    - category
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.sql.is_none());
        assert!(spec.aggregate.is_none());
        let forecast = spec.forecast.unwrap();
        assert_eq!(forecast.timestamp, "date");
        assert_eq!(forecast.value, "revenue");
        assert_eq!(forecast.horizon, Some(60));
        assert_eq!(forecast.confidence_level, Some(0.99));
        assert_eq!(forecast.model, Some("arima".to_string()));
        let groups = forecast.group_by.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], "region");
        assert_eq!(groups[1], "category");
    }

    #[test]
    fn test_forecast_required_fields_only() {
        let yaml = r#"
forecast:
  timestamp: date
  value: revenue
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        let forecast = spec.forecast.unwrap();
        assert_eq!(forecast.timestamp, "date");
        assert_eq!(forecast.value, "revenue");
        assert!(forecast.horizon.is_none());
        assert!(forecast.confidence_level.is_none());
        assert!(forecast.model.is_none());
        assert!(forecast.group_by.is_none());
    }

    #[test]
    fn test_all_three_fields() {
        let yaml = r#"
sql: "SELECT * FROM sales"
aggregate:
  dimensions:
    - region
  measures:
    - column: amount
      aggregation: sum
      name: totalAmount
forecast:
  timestamp: date
  value: totalAmount
  horizon: 30
"#;
        let spec: TransformSpec = serde_yaml::from_str(yaml).unwrap();
        match spec.sql.unwrap() {
            SqlSpec::Single(s) => assert_eq!(s, "SELECT * FROM sales"),
            other => panic!("Expected SqlSpec::Single, got {:?}", other),
        }
        let agg = spec.aggregate.unwrap();
        assert_eq!(agg.dimensions.len(), 1);
        assert_eq!(agg.measures.len(), 1);
        let forecast = spec.forecast.unwrap();
        assert_eq!(forecast.timestamp, "date");
        assert_eq!(forecast.value, "totalAmount");
        assert_eq!(forecast.horizon, Some(30));
    }
}
