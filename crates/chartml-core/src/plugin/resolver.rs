use async_trait::async_trait;
use crate::error::ChartError;

/// Connection configuration resolved from a datasource slug.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub provider: String,
    pub connection_string: Option<String>,
    pub config: std::collections::HashMap<String, serde_json::Value>,
}

/// Datasource resolver — resolves a slug to connection config.
#[async_trait]
pub trait DatasourceResolver: Send + Sync {
    async fn resolve(&self, slug: &str) -> Result<ConnectionConfig, ChartError>;
}
