use async_trait::async_trait;
use crate::data::DataTable;
use crate::error::ChartError;

/// Options for data fetching.
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    /// Optional cache TTL hint.
    pub cache_ttl: Option<String>,
}

/// Specification for fetching data.
#[derive(Debug, Clone)]
pub struct DataSpec {
    /// Provider type: "inline", "http", "api", etc.
    pub provider: String,
    /// Inline rows (if provider is "inline").
    pub rows: Option<Vec<serde_json::Value>>,
    /// URL for HTTP/API providers.
    pub url: Option<String>,
    /// API endpoint.
    pub endpoint: Option<String>,
}

/// Data source plugin — fetches raw data from a provider.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait DataSource: Send + Sync {
    /// Fetch data as an Arrow-backed DataTable.
    async fn fetch(&self, spec: &DataSpec, options: &FetchOptions) -> Result<DataTable, ChartError>;
}
