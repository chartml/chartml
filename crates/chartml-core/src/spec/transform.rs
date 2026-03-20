use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformSpec {
    pub aggregate: Option<AggregateSpec>,
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
