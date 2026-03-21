//! DataFusion-backed transform middleware for ChartML.
//!
//! Implements the 3-stage pipeline: SQL → Aggregate → Forecast.
//! Compatible with both native (server) and WASM (browser) targets.

pub mod conversion;
pub mod sql_builder;
pub mod stages;

use async_trait::async_trait;
use chartml_core::data::Row;
use chartml_core::error::ChartError;
use chartml_core::plugin::transform::{TransformContext, TransformMiddleware, TransformResult};
use chartml_core::spec::TransformSpec;
use datafusion::prelude::*;
use std::collections::HashMap;

/// DataFusion-backed transform middleware.
///
/// Processes data through a 3-stage pipeline:
/// 1. **SQL stage** — execute raw SQL with placeholder replacement
/// 2. **Aggregate stage** — declarative GROUP BY / measures / filters
/// 3. **Forecast stage** — time series forecasting via chartml-forecast
pub struct DataFusionTransform;

#[async_trait]
impl TransformMiddleware for DataFusionTransform {
    async fn transform(
        &self,
        data: Vec<Row>,
        spec: &TransformSpec,
        _context: &TransformContext,
    ) -> Result<TransformResult, ChartError> {
        let ctx = SessionContext::new();

        // Register input data as "source" table
        let batch = conversion::rows_to_record_batch(&data)?;
        let schema = batch.schema();
        let mem_table =
            datafusion::datasource::MemTable::try_new(schema, vec![vec![batch]]).map_err(|e| {
                ChartError::DataError(format!("Failed to create source MemTable: {}", e))
            })?;
        ctx.register_table("source", std::sync::Arc::new(mem_table))
            .map_err(|e| {
                ChartError::DataError(format!("Failed to register source table: {}", e))
            })?;

        let mut current_table = "source".to_string();

        // Stage 1: SQL
        if let Some(ref sql_spec) = spec.sql {
            current_table =
                stages::sql_stage::execute(&ctx, &current_table, sql_spec).await?;
        }

        // Stage 2: Aggregate
        if let Some(ref agg_spec) = spec.aggregate {
            current_table =
                stages::aggregate_stage::execute(&ctx, &current_table, agg_spec).await?;
        }

        // Stage 3: Forecast
        if let Some(ref forecast_spec) = spec.forecast {
            current_table =
                stages::forecast_stage::execute(&ctx, &current_table, forecast_spec).await?;
        }

        // Collect final result
        let df = ctx
            .table(&current_table)
            .await
            .map_err(|e| ChartError::DataError(format!("Failed to read result table: {}", e)))?;
        let batches = df
            .collect()
            .await
            .map_err(|e| ChartError::DataError(format!("Failed to collect results: {}", e)))?;
        let rows = conversion::record_batch_to_rows(&batches);

        Ok(TransformResult {
            data: rows,
            metadata: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chartml_core::spec::*;
    use serde_json::json;

    fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    fn sales_data() -> Vec<Row> {
        vec![
            make_row(vec![
                ("region", json!("North")),
                ("product", json!("Widget")),
                ("revenue", json!(100.0)),
                ("units", json!(10.0)),
            ]),
            make_row(vec![
                ("region", json!("North")),
                ("product", json!("Gadget")),
                ("revenue", json!(200.0)),
                ("units", json!(15.0)),
            ]),
            make_row(vec![
                ("region", json!("South")),
                ("product", json!("Widget")),
                ("revenue", json!(150.0)),
                ("units", json!(12.0)),
            ]),
            make_row(vec![
                ("region", json!("South")),
                ("product", json!("Widget")),
                ("revenue", json!(50.0)),
                ("units", json!(5.0)),
            ]),
            make_row(vec![
                ("region", json!("East")),
                ("product", json!("Gadget")),
                ("revenue", json!(300.0)),
                ("units", json!(20.0)),
            ]),
        ]
    }

    #[tokio::test]
    async fn test_full_pipeline_aggregate() {
        let data = sales_data();
        let spec = TransformSpec {
            sql: None,
            forecast: None,
            aggregate: Some(AggregateSpec {
                dimensions: vec![Dimension::Simple("region".to_string())],
                measures: vec![Measure {
                    column: Some("revenue".to_string()),
                    aggregation: Some("sum".to_string()),
                    name: "total_revenue".to_string(),
                    expression: None,
                }],
                filters: None,
                sort: Some(vec![SortSpec {
                    field: "total_revenue".to_string(),
                    direction: Some("desc".to_string()),
                }]),
                limit: None,
            }),
        };

        let transform = DataFusionTransform;
        let context = TransformContext::default();
        let result = transform.transform(data, &spec, &context).await.unwrap();

        assert_eq!(result.data.len(), 3, "Should have 3 regions");

        // Results should be sorted descending by total_revenue
        let revenues: Vec<f64> = result
            .data
            .iter()
            .map(|r| r.get("total_revenue").unwrap().as_f64().unwrap())
            .collect();

        // North=300, East=300, South=200
        assert!(
            revenues[0] >= revenues[1],
            "First should be >= second: {:?}",
            revenues
        );
        assert!(
            revenues[1] >= revenues[2],
            "Second should be >= third: {:?}",
            revenues
        );
        assert_eq!(revenues[2], 200.0, "South total should be 200");
    }

    #[tokio::test]
    async fn test_full_pipeline_forecast() {
        // Create time series data (linear: y = 10 + 2x)
        let data: Vec<Row> = (0..20)
            .map(|i| {
                make_row(vec![
                    ("timestamp", json!(1000 + i)),
                    ("value", json!(10.0 + 2.0 * i as f64)),
                ])
            })
            .collect();

        let spec = TransformSpec {
            sql: None,
            aggregate: None,
            forecast: Some(ForecastSpec {
                timestamp: "timestamp".to_string(),
                value: "value".to_string(),
                horizon: Some(5),
                confidence_level: Some(0.95),
                model: Some("linear".to_string()),
                group_by: None,
            }),
        };

        let transform = DataFusionTransform;
        let context = TransformContext::default();
        let result = transform.transform(data, &spec, &context).await.unwrap();

        // Should have 20 historical + 5 forecast rows
        assert_eq!(
            result.data.len(),
            25,
            "Should have 25 rows (20 historical + 5 forecast)"
        );

        // Check that forecast rows have is_forecast=true
        let forecast_rows: Vec<&Row> = result
            .data
            .iter()
            .filter(|r| r.get("is_forecast").and_then(|v| v.as_bool()) == Some(true))
            .collect();
        assert_eq!(forecast_rows.len(), 5, "Should have 5 forecast rows");

        // Forecast values should have forecast, lower_bound, upper_bound
        for row in &forecast_rows {
            assert!(
                row.get("forecast").is_some(),
                "Forecast row should have 'forecast' field"
            );
            assert!(
                row.get("lower_bound").is_some(),
                "Forecast row should have 'lower_bound' field"
            );
            assert!(
                row.get("upper_bound").is_some(),
                "Forecast row should have 'upper_bound' field"
            );
        }

        // Historical rows should have is_forecast=false
        let historical_rows: Vec<&Row> = result
            .data
            .iter()
            .filter(|r| r.get("is_forecast").and_then(|v| v.as_bool()) == Some(false))
            .collect();
        assert_eq!(historical_rows.len(), 20, "Should have 20 historical rows");
    }

    #[tokio::test]
    async fn test_full_pipeline_sql() {
        let data = sales_data();
        let spec = TransformSpec {
            sql: Some(SqlSpec::Single(
                "SELECT * FROM \"source\" WHERE \"revenue\" > 100".to_string(),
            )),
            aggregate: None,
            forecast: None,
        };

        let transform = DataFusionTransform;
        let context = TransformContext::default();
        let result = transform.transform(data, &spec, &context).await.unwrap();

        // Only rows with revenue > 100 should remain
        assert!(
            result.data.len() < 5,
            "Should filter out some rows, got {}",
            result.data.len()
        );
        for row in &result.data {
            let rev = row.get("revenue").unwrap().as_f64().unwrap();
            assert!(rev > 100.0, "Revenue should be > 100, got {}", rev);
        }
    }
}
