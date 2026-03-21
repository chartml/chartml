//! SQL stage: placeholder replacement + execution via DataFusion.

use chartml_core::error::ChartError;
use chartml_core::spec::SqlSpec;
use datafusion::prelude::*;

/// Replace `{sourceName}` placeholders in a SQL string with the actual table name.
fn replace_placeholders(sql: &str, source_name: &str, table_name: &str) -> String {
    // Replace {sourceName} with the quoted table name
    sql.replace(
        &format!("{{{}}}", source_name),
        &format!("\"{}\"", table_name),
    )
}

/// Execute the SQL stage.
///
/// For `SqlSpec::Single`: execute the single statement and register result.
/// For `SqlSpec::Multiple`: execute all but last as setup, last becomes result.
///
/// Returns the name of the output table registered in the session context.
pub async fn execute(
    ctx: &SessionContext,
    current_table: &str,
    spec: &SqlSpec,
) -> Result<String, ChartError> {
    let statements: Vec<String> = match spec {
        SqlSpec::Single(s) => vec![s.clone()],
        SqlSpec::Multiple(v) => v.clone(),
    };

    if statements.is_empty() {
        return Err(ChartError::DataError(
            "SQL stage: must contain at least one SQL statement".to_string(),
        ));
    }

    // Replace placeholders in all statements
    // We use "source" as the default placeholder name and also replace
    // the current_table name directly
    let resolved: Vec<String> = statements
        .iter()
        .map(|stmt| {
            let mut s = replace_placeholders(stmt, "source", current_table);
            s = replace_placeholders(&s, "sourceName", current_table);
            s
        })
        .collect();

    // Execute setup statements (all but the last)
    for sql in &resolved[..resolved.len() - 1] {
        ctx.sql(sql)
            .await
            .map_err(|e| ChartError::DataError(format!("SQL stage setup error: {}", e)))?
            .collect()
            .await
            .map_err(|e| ChartError::DataError(format!("SQL stage setup collect error: {}", e)))?;
    }

    // Execute the final statement and register as a new table
    let final_sql = &resolved[resolved.len() - 1];
    let output_table = format!("__stage_sql_{}", current_table);

    let df = ctx
        .sql(final_sql)
        .await
        .map_err(|e| ChartError::DataError(format!("SQL stage error: {}", e)))?;

    let batches = df
        .collect()
        .await
        .map_err(|e| ChartError::DataError(format!("SQL stage collect error: {}", e)))?;

    let schema = if let Some(first) = batches.first() {
        first.schema()
    } else {
        return Err(ChartError::DataError(
            "SQL stage returned no results".to_string(),
        ));
    };

    let mem_table = datafusion::datasource::MemTable::try_new(schema, vec![batches])
        .map_err(|e| ChartError::DataError(format!("SQL stage MemTable error: {}", e)))?;

    ctx.register_table(&output_table, std::sync::Arc::new(mem_table))
        .map_err(|e| ChartError::DataError(format!("SQL stage register error: {}", e)))?;

    Ok(output_table)
}
