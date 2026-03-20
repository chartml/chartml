use async_trait::async_trait;
use std::collections::HashMap;
use crate::data::Row;
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
    pub data: Vec<Row>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Transform middleware — processes data between fetch and render.
#[async_trait]
pub trait TransformMiddleware: Send + Sync {
    /// Transform input data according to the spec.
    async fn transform(
        &self,
        data: Vec<Row>,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError>;
}
