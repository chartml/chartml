use async_trait::async_trait;
use std::collections::HashMap;
use crate::data::DataTable;
use crate::error::ChartError;
use crate::spec::TransformSpec;

/// Context available during transform execution.
#[derive(Debug, Clone, Default)]
pub struct TransformContext {
    /// Parameter values resolved from the spec.
    pub params: HashMap<String, serde_json::Value>,
}

/// Result of a transform operation.
#[derive(Debug, Clone)]
pub struct TransformResult {
    pub data: DataTable,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Transform middleware — processes data between fetch and render.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TransformMiddleware: Send + Sync {
    /// Transform input data according to the spec.
    async fn transform(
        &self,
        data: DataTable,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError>;
}
