use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error::ChartError;

/// Connection configuration resolved from a datasource slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub provider: String,
    pub connection_string: Option<String>,
    pub config: std::collections::HashMap<String, serde_json::Value>,
}

/// Datasource resolver — resolves a slug to connection config.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait DatasourceResolver: Send + Sync {
    async fn resolve(&self, slug: &str) -> Result<ConnectionConfig, ChartError>;
}
