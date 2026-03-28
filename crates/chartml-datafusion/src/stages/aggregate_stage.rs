//! Aggregate stage: compile AggregateSpec → SQL → DataFusion execution.

use chartml_core::error::ChartError;
use chartml_core::spec::AggregateSpec;
use datafusion::prelude::*;

use crate::sql_builder;

/// Execute the aggregate stage.
///
/// Generates SQL from the aggregate spec, executes via DataFusion,
/// and registers the result as a new table.
///
/// Returns the name of the output table.
pub async fn execute(
    ctx: &SessionContext,
    current_table: &str,
    spec: &AggregateSpec,
) -> Result<String, ChartError> {
    let sql = sql_builder::build_aggregate_sql(current_table, spec);
    let output_table = format!("__stage_agg_{}", current_table);

    let df = ctx
        .sql(&sql)
        .await
        .map_err(|e| ChartError::DataError(format!("Aggregate stage SQL error: {}", e)))?;

    let batches = df
        .collect()
        .await
        .map_err(|e| ChartError::DataError(format!("Aggregate stage collect error: {}", e)))?;

    let schema = if let Some(first) = batches.first() {
        first.schema()
    } else {
        // Empty result — create an empty table with no schema
        let empty_schema = arrow::datatypes::Schema::empty();
        let mem_table = datafusion::datasource::MemTable::try_new(
            std::sync::Arc::new(empty_schema),
            vec![vec![]],
        )
        .map_err(|e| ChartError::DataError(format!("Aggregate stage MemTable error: {}", e)))?;
        ctx.register_table(&output_table, std::sync::Arc::new(mem_table))
            .map_err(|e| {
                ChartError::DataError(format!("Aggregate stage register error: {}", e))
            })?;
        return Ok(output_table);
    };

    let mem_table = datafusion::datasource::MemTable::try_new(schema, vec![batches])
        .map_err(|e| ChartError::DataError(format!("Aggregate stage MemTable error: {}", e)))?;

    ctx.register_table(&output_table, std::sync::Arc::new(mem_table))
        .map_err(|e| ChartError::DataError(format!("Aggregate stage register error: {}", e)))?;

    Ok(output_table)
}
