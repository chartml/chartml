use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSpec {
    pub version: u32,
    pub name: String,
    pub provider: String,
    pub rows: Option<Vec<serde_json::Value>>,
    pub url: Option<String>,
    pub endpoint: Option<String>,
    pub cache: Option<CacheConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfig {
    pub ttl: String,
}
