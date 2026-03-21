//! Forecast stage: extract series from DataFusion, run chartml-forecast, merge results.

use chartml_core::data::{Row, get_f64};
use chartml_core::error::ChartError;
use chartml_core::spec::ForecastSpec;
use chartml_forecast::{ForecastConfig, ForecastModel, TimeSeries};
use datafusion::prelude::*;
use std::collections::HashMap;

use crate::conversion;

/// Map a model name string to the ForecastModel enum.
/// Returns an error for unrecognized model names.
fn parse_model(model_str: &str) -> Result<ForecastModel, ChartError> {
    match model_str.to_lowercase().as_str() {
        "ets" => Ok(ForecastModel::ETS),
        "linear" => Ok(ForecastModel::Linear),
        "exponential" => Ok(ForecastModel::Exponential),
        "logistic" => Ok(ForecastModel::Logistic),
        "auto" => Ok(ForecastModel::Auto),
        other => Err(ChartError::DataError(format!(
            "Unknown forecast model '{}'. Valid models: ets, linear, exponential, logistic, auto",
            other
        ))),
    }
}

/// Execute the forecast stage.
///
/// 1. Read current table data from DataFusion
/// 2. Extract timestamp + value columns into TimeSeries
/// 3. If group_by: partition, forecast each group independently
/// 4. Call chartml_forecast::forecast() with appropriate config
/// 5. Merge historical rows with forecast rows (adding is_forecast, lower_bound, upper_bound)
/// 6. Register merged result and return table name
pub async fn execute(
    ctx: &SessionContext,
    current_table: &str,
    spec: &ForecastSpec,
) -> Result<String, ChartError> {
    // 1. Read current table into rows
    let df = ctx
        .table(current_table)
        .await
        .map_err(|e| ChartError::DataError(format!("Forecast stage table error: {}", e)))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| ChartError::DataError(format!("Forecast stage collect error: {}", e)))?;
    let rows = conversion::record_batch_to_rows(&batches);

    if rows.is_empty() {
        return Err(ChartError::DataError(
            "Forecast stage: no data to forecast".to_string(),
        ));
    }

    // 2. Build forecast config
    let horizon = spec.horizon.unwrap_or(30) as usize;
    let confidence_level = spec.confidence_level.unwrap_or(0.95);
    let model = parse_model(spec.model.as_deref().unwrap_or("auto"))?;
    let config = ForecastConfig {
        model,
        horizon,
        confidence_level,
    };

    // 3. Group data if needed
    let groups: Vec<(Vec<(String, String)>, Vec<&Row>)> =
        if let Some(ref group_by) = spec.group_by {
            if group_by.is_empty() {
                vec![(vec![], rows.iter().collect())]
            } else {
                partition_by_groups(&rows, group_by)
            }
        } else {
            vec![(vec![], rows.iter().collect())]
        };

    // 4. Forecast each group and merge
    let mut result_rows: Vec<Row> = Vec::new();

    for (group_key, group_rows) in &groups {
        // Extract time series
        let (series, extra_cols) = extract_series(group_rows, &spec.timestamp, &spec.value)?;

        // Run forecast
        let forecast_result = chartml_forecast::forecast(&series, &config).map_err(|e| {
            ChartError::DataError(format!("Forecast error: {}", e))
        })?;

        // Add historical rows with is_forecast=false
        let last_historical_value = group_rows.last()
            .and_then(|r| get_f64(r, &spec.value));

        for row in group_rows.iter() {
            let mut out_row = (*row).clone();
            out_row.insert("is_forecast".to_string(), serde_json::json!(false));
            out_row.insert("forecast".to_string(), serde_json::Value::Null);
            out_row.insert("lower_bound".to_string(), serde_json::Value::Null);
            out_row.insert("upper_bound".to_string(), serde_json::Value::Null);
            result_rows.push(out_row);
        }

        // Set forecast on last historical row for seamless dashed-line connection
        if let Some(last_val) = last_historical_value {
            if let Some(last_row) = result_rows.last_mut() {
                last_row.insert("forecast".to_string(), serde_json::json!(last_val));
            }
        }

        // Add forecast rows
        for i in 0..forecast_result.forecasts.len() {
            let mut out_row = Row::new();

            // Set timestamp
            out_row.insert(
                spec.timestamp.clone(),
                serde_json::json!(forecast_result.timestamps[i]),
            );

            // Original value is null for forecast rows (separate series)
            out_row.insert(spec.value.clone(), serde_json::Value::Null);

            // Set group-by columns
            for (key, val) in group_key {
                out_row.insert(key.clone(), serde_json::json!(val));
            }

            // Set any extra columns from the first row to null
            for col_name in &extra_cols {
                if !out_row.contains_key(col_name) {
                    out_row.insert(col_name.clone(), serde_json::Value::Null);
                }
            }

            // Set forecast fields
            out_row.insert("is_forecast".to_string(), serde_json::json!(true));
            out_row.insert(
                "forecast".to_string(),
                serde_json::json!(forecast_result.forecasts[i]),
            );
            out_row.insert(
                "lower_bound".to_string(),
                serde_json::json!(forecast_result.lower_bounds[i]),
            );
            out_row.insert(
                "upper_bound".to_string(),
                serde_json::json!(forecast_result.upper_bounds[i]),
            );

            result_rows.push(out_row);
        }
    }

    // 5. Register the result
    let output_table = format!("__stage_fcast_{}", current_table);

    if result_rows.is_empty() {
        return Err(ChartError::DataError(
            "Forecast stage produced no results".to_string(),
        ));
    }

    let batch = conversion::rows_to_record_batch(&result_rows)?;
    let schema = batch.schema();

    let mem_table =
        datafusion::datasource::MemTable::try_new(schema, vec![vec![batch]])
            .map_err(|e| {
                ChartError::DataError(format!("Forecast stage MemTable error: {}", e))
            })?;

    ctx.register_table(&output_table, std::sync::Arc::new(mem_table))
        .map_err(|e| {
            ChartError::DataError(format!("Forecast stage register error: {}", e))
        })?;

    Ok(output_table)
}

/// Extract a TimeSeries from rows, using the given timestamp and value column names.
/// Returns the series and a list of "extra" column names (columns besides timestamp and value).
fn extract_series(
    rows: &[&Row],
    timestamp_col: &str,
    value_col: &str,
) -> Result<(TimeSeries, Vec<String>), ChartError> {
    let mut timestamps = Vec::with_capacity(rows.len());
    let mut values = Vec::with_capacity(rows.len());

    for row in rows {
        let ts = row
            .get(timestamp_col)
            .and_then(|v| match v {
                serde_json::Value::Number(n) => {
                    // Try as_i64 first (integer values), then as_f64 (float values from Arrow roundtrip)
                    n.as_i64()
                        .map(|n| n as i32)
                        .or_else(|| n.as_f64().map(|f| f as i32))
                }
                serde_json::Value::String(s) => s.parse::<i32>().ok(),
                _ => None,
            })
            .ok_or_else(|| {
                ChartError::DataError(format!(
                    "Forecast: missing or non-numeric timestamp column '{}'",
                    timestamp_col
                ))
            })?;

        let val = row
            .get(value_col)
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) => s.parse::<f64>().ok(),
                _ => None,
            })
            .ok_or_else(|| {
                ChartError::DataError(format!(
                    "Forecast: missing or non-numeric value column '{}'",
                    value_col
                ))
            })?;

        timestamps.push(ts);
        values.push(val);
    }

    // Collect extra column names
    let extra_cols: Vec<String> = if let Some(first) = rows.first() {
        first
            .keys()
            .filter(|k| k.as_str() != timestamp_col && k.as_str() != value_col)
            .cloned()
            .collect()
    } else {
        vec![]
    };

    Ok((TimeSeries { timestamps, values }, extra_cols))
}

/// Partition rows by group-by column values.
/// Returns a list of (group_key, rows) where group_key is a vec of (column_name, value) pairs.
fn partition_by_groups<'a>(
    rows: &'a [Row],
    group_by: &[String],
) -> Vec<(Vec<(String, String)>, Vec<&'a Row>)> {
    let mut groups: HashMap<Vec<String>, Vec<&'a Row>> = HashMap::new();
    let mut key_order: Vec<Vec<String>> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for row in rows {
        let key: Vec<String> = group_by
            .iter()
            .map(|col| {
                row.get(col)
                    .and_then(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Number(n) => Some(n.to_string()),
                        _ => Some(v.to_string()),
                    })
                    .unwrap_or_default()
            })
            .collect();

        if seen.insert(key.clone()) {
            key_order.push(key.clone());
        }
        groups.entry(key).or_default().push(row);
    }

    key_order
        .into_iter()
        .map(|key| {
            let group_key: Vec<(String, String)> = group_by
                .iter()
                .zip(key.iter())
                .map(|(col, val)| (col.clone(), val.clone()))
                .collect();
            let group_rows = groups.remove(&key).unwrap_or_default();
            (group_key, group_rows)
        })
        .collect()
}
