//! Aggregate stage: compile AggregateSpec → SQL → DataFusion execution.
//!
//! When no explicit `sort` is specified, the stage preserves the insertion
//! order of dimension values from the source data.  It does this by adding
//! a row-number column to the source, including `MIN("__row_num__")` in the
//! aggregate query, ordering the result by that column, and then stripping
//! it from the final output.

use chartml_core::error::ChartError;
use chartml_core::spec::AggregateSpec;
use datafusion::prelude::*;

use crate::sql_builder;

/// Name of the synthetic row-number column injected for insertion-order
/// preservation.
const ROW_NUM_COL: &str = "__row_num__";

/// Returns `true` when the spec carries an explicit, non-empty sort clause.
fn has_explicit_sort(spec: &AggregateSpec) -> bool {
    spec.sort.as_ref().is_some_and(|s| !s.is_empty())
}

/// Returns `true` when the spec has dimensions and at least one aggregated
/// measure — i.e. a GROUP BY will actually be emitted.
fn needs_group_by(spec: &AggregateSpec) -> bool {
    !spec.dimensions.is_empty()
        && spec
            .measures
            .iter()
            .any(|m| m.aggregation.is_some() || m.expression.is_some())
}

/// Register a row-numbered view of the source table.
///
/// Creates a new table named `{source}__numbered` with an additional
/// `__row_num__` column that reflects the original row order.
async fn register_numbered_source(
    ctx: &SessionContext,
    source: &str,
) -> Result<String, ChartError> {
    let numbered_name = format!("{}__numbered", source);
    let sql = format!(
        "SELECT *, ROW_NUMBER() OVER () AS \"{}\" FROM {}",
        ROW_NUM_COL, source
    );
    let df = ctx
        .sql(&sql)
        .await
        .map_err(|e| ChartError::DataError(format!("Row-number SQL error: {}", e)))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| ChartError::DataError(format!("Row-number collect error: {}", e)))?;

    let schema = match batches.first() {
        Some(b) => b.schema(),
        None => {
            // Source is empty — register an empty table and return.
            let empty = arrow::datatypes::Schema::empty();
            let mem = datafusion::datasource::MemTable::try_new(
                std::sync::Arc::new(empty),
                vec![vec![]],
            )
            .map_err(|e| ChartError::DataError(format!("MemTable error: {}", e)))?;
            ctx.register_table(&numbered_name, std::sync::Arc::new(mem))
                .map_err(|e| ChartError::DataError(format!("Register error: {}", e)))?;
            return Ok(numbered_name);
        }
    };

    let mem = datafusion::datasource::MemTable::try_new(schema, vec![batches])
        .map_err(|e| ChartError::DataError(format!("MemTable error: {}", e)))?;
    ctx.register_table(&numbered_name, std::sync::Arc::new(mem))
        .map_err(|e| ChartError::DataError(format!("Register error: {}", e)))?;
    Ok(numbered_name)
}

/// Execute the aggregate stage.
///
/// Generates SQL from the aggregate spec, executes via DataFusion,
/// and registers the result as a new table.
///
/// When the spec contains dimensions, aggregated measures, and no explicit
/// sort, the stage automatically preserves the insertion order of dimension
/// values by injecting a row-number column, aggregating its MIN, sorting
/// by it, and removing it from the final output.
///
/// Returns the name of the output table.
pub async fn execute(
    ctx: &SessionContext,
    current_table: &str,
    spec: &AggregateSpec,
) -> Result<String, ChartError> {
    let preserve_order = !has_explicit_sort(spec) && needs_group_by(spec);
    let output_table = format!("__stage_agg_{}", current_table);

    if preserve_order {
        execute_with_insertion_order(ctx, current_table, spec, &output_table).await
    } else {
        execute_plain(ctx, current_table, spec, &output_table).await
    }
}

/// Standard path: run the aggregate SQL as-is and register the output.
async fn execute_plain(
    ctx: &SessionContext,
    current_table: &str,
    spec: &AggregateSpec,
    output_table: &str,
) -> Result<String, ChartError> {
    let sql = sql_builder::build_aggregate_sql(current_table, spec);
    let batches = run_sql(ctx, &sql).await?;
    register_batches(ctx, output_table, batches).await
}

/// Insertion-order-preserving path.
///
/// 1. Register a numbered copy of the source table.
/// 2. Build the normal aggregate SQL against the numbered source.
/// 3. Inject `MIN("__row_num__")` into the SELECT and append
///    `ORDER BY MIN("__row_num__")`.
/// 4. Strip the ordering column from the final output.
async fn execute_with_insertion_order(
    ctx: &SessionContext,
    current_table: &str,
    spec: &AggregateSpec,
    output_table: &str,
) -> Result<String, ChartError> {
    // Step 1 — numbered source
    let numbered = register_numbered_source(ctx, current_table).await?;

    // Step 2 — build aggregate SQL against the numbered source
    let base_sql = sql_builder::build_aggregate_sql(&numbered, spec);

    // Step 3 — inject the ordering column and ORDER BY clause.
    // Use the alias in ORDER BY to avoid DataFusion creating a duplicate
    // expression entry for the same MIN(...) aggregate.
    let select_expr = format!("MIN(\"{}\") AS \"{}\"", ROW_NUM_COL, ROW_NUM_COL);
    let order_expr = format!("\"{}\"", ROW_NUM_COL);
    let augmented_sql = inject_insertion_order(&base_sql, &select_expr, &order_expr);

    let batches = run_sql(ctx, &augmented_sql).await?;

    // Step 4 — strip the __row_num__ column
    let batches = strip_column(&batches, ROW_NUM_COL)?;

    register_batches(ctx, output_table, batches).await
}

/// Inject an insertion-order column into an aggregate SQL statement.
///
/// Adds `order_alias` (e.g. `MIN("__row_num__") AS "__row_num__"`) to the
/// SELECT list and inserts `ORDER BY order_expr` (e.g. `MIN("__row_num__")`)
/// before any LIMIT clause (or at the end if there is none).
fn inject_insertion_order(sql: &str, order_alias: &str, order_expr: &str) -> String {
    // The SQL from build_aggregate_sql always starts with "SELECT\n  ".
    // Insert the order alias as an additional column at the end of the
    // SELECT list, right before the FROM clause.
    //
    // Structure: SELECT\n  col1,\n  col2\nFROM ...[\nGROUP BY ...][\nHAVING ...][\nLIMIT ...]
    let from_pos = match sql.find("\nFROM ") {
        Some(pos) => pos,
        None => return sql.to_string(),
    };

    let mut result = String::with_capacity(sql.len() + order_alias.len() + 64);
    result.push_str(&sql[..from_pos]);
    result.push_str(",\n  ");
    result.push_str(order_alias);

    let rest = &sql[from_pos..];

    // Insert ORDER BY before LIMIT if present, otherwise append at end.
    let order_clause = format!("\nORDER BY {}", order_expr);
    if let Some(limit_pos) = rest.find("\nLIMIT ") {
        result.push_str(&rest[..limit_pos]);
        result.push_str(&order_clause);
        result.push_str(&rest[limit_pos..]);
    } else {
        result.push_str(rest);
        result.push_str(&order_clause);
    }

    result
}

/// Execute a SQL statement and return the collected record batches.
async fn run_sql(
    ctx: &SessionContext,
    sql: &str,
) -> Result<Vec<arrow::array::RecordBatch>, ChartError> {
    let df = ctx
        .sql(sql)
        .await
        .map_err(|e| ChartError::DataError(format!("Aggregate stage SQL error: {}", e)))?;
    df.collect()
        .await
        .map_err(|e| ChartError::DataError(format!("Aggregate stage collect error: {}", e)))
}

/// Remove a column by name from every batch.
fn strip_column(
    batches: &[arrow::array::RecordBatch],
    col_name: &str,
) -> Result<Vec<arrow::array::RecordBatch>, ChartError> {
    batches
        .iter()
        .map(|batch| {
            let schema = batch.schema();
            let idx = schema.index_of(col_name).map_err(|e| {
                ChartError::DataError(format!(
                    "Cannot find column '{}' to strip: {}",
                    col_name, e
                ))
            })?;
            let mut batch = batch.clone();
            batch.remove_column(idx);
            Ok(batch)
        })
        .collect()
}

/// Register record batches as a named in-memory table.
async fn register_batches(
    ctx: &SessionContext,
    table_name: &str,
    batches: Vec<arrow::array::RecordBatch>,
) -> Result<String, ChartError> {
    let schema = if let Some(first) = batches.first() {
        first.schema()
    } else {
        let empty_schema = arrow::datatypes::Schema::empty();
        let mem_table = datafusion::datasource::MemTable::try_new(
            std::sync::Arc::new(empty_schema),
            vec![vec![]],
        )
        .map_err(|e| ChartError::DataError(format!("Aggregate stage MemTable error: {}", e)))?;
        ctx.register_table(table_name, std::sync::Arc::new(mem_table))
            .map_err(|e| {
                ChartError::DataError(format!("Aggregate stage register error: {}", e))
            })?;
        return Ok(table_name.to_string());
    };

    let mem_table = datafusion::datasource::MemTable::try_new(schema, vec![batches])
        .map_err(|e| ChartError::DataError(format!("Aggregate stage MemTable error: {}", e)))?;

    ctx.register_table(table_name, std::sync::Arc::new(mem_table))
        .map_err(|e| ChartError::DataError(format!("Aggregate stage register error: {}", e)))?;

    Ok(table_name.to_string())
}
