use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use indexmap::IndexMap;

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
///
/// Receives a map of named source tables (insertion-ordered, matching the YAML
/// `data:` map order) and produces a single result table that the renderer
/// consumes. Implementations are expected to:
///
/// - Register every entry in `sources` so user-authored SQL can reference
///   any source by its declared name.
/// - For single-entry maps with a non-`"source"` key, additionally register
///   the sole table under the alias `"source"` so legacy SQL referencing
///   `FROM source` keeps working. Multi-entry maps are NOT aliased — the
///   caller's SQL must use the explicit source names.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TransformMiddleware: Send + Sync {
    /// Transform input data according to the spec.
    async fn transform(
        &self,
        sources: &IndexMap<String, DataTable>,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError>;

    /// Batch-oriented transform. Receives multiple RecordBatches per source,
    /// allowing implementations to register them into MemTable without
    /// concatenation.
    ///
    /// Default concatenates batches into DataTables and delegates to
    /// `transform()`. Override in `DataFusionTransform` to avoid the concat.
    async fn transform_batches(
        &self,
        sources: &IndexMap<String, (SchemaRef, Vec<RecordBatch>)>,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError> {
        let mut data_tables = IndexMap::with_capacity(sources.len());
        for (name, (schema, batches)) in sources {
            let batch = if batches.is_empty() {
                RecordBatch::new_empty(Arc::clone(schema))
            } else {
                arrow::compute::concat_batches(schema, batches).map_err(|e| {
                    ChartError::DataError(format!(
                        "Failed to concat batches for source '{}': {}",
                        name, e
                    ))
                })?
            };
            data_tables.insert(name.clone(), DataTable::from_record_batch(batch));
        }
        self.transform(&data_tables, spec, context).await
    }
}
