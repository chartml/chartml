//! Conversion between `Vec<Row>` (HashMap<String, serde_json::Value>) and Arrow RecordBatch.

use arrow::array::{
    ArrayRef, BooleanArray, Float64Array, RecordBatch, StringBuilder, StringArray,
};
use arrow::datatypes::{DataType, Field, Schema};
use chartml_core::data::Row;
use chartml_core::error::ChartError;
use std::sync::Arc;

/// Inferred column type from JSON values.
#[derive(Debug, Clone, Copy, PartialEq)]
enum InferredType {
    Float64,
    Boolean,
    Utf8,
    Null,
}

/// Convert `Vec<Row>` into an Arrow `RecordBatch`.
///
/// Type inference strategy:
/// - Numbers → Float64
/// - Booleans → Boolean
/// - Strings → Utf8
/// - Null → nullable (skipped during inference)
/// - Mixed types → coerced to Utf8
pub fn rows_to_record_batch(rows: &[Row]) -> Result<RecordBatch, ChartError> {
    if rows.is_empty() {
        // Return an empty RecordBatch with no columns
        let schema = Arc::new(Schema::new(Vec::<Field>::new()));
        return Ok(RecordBatch::new_empty(schema));
    }

    // 1. Collect unique column names preserving insertion order
    let mut column_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        for key in row.keys() {
            if seen.insert(key.clone()) {
                column_names.push(key.clone());
            }
        }
    }
    // Sort for deterministic column order
    column_names.sort();

    // 2. Infer types for each column
    let mut col_types: Vec<InferredType> = vec![InferredType::Null; column_names.len()];
    for row in rows {
        for (i, name) in column_names.iter().enumerate() {
            if let Some(val) = row.get(name) {
                let val_type = match val {
                    serde_json::Value::Number(_) => InferredType::Float64,
                    serde_json::Value::Bool(_) => InferredType::Boolean,
                    serde_json::Value::String(_) => InferredType::Utf8,
                    serde_json::Value::Null => InferredType::Null,
                    _ => InferredType::Utf8, // arrays/objects → string
                };

                col_types[i] = merge_types(col_types[i], val_type);
            }
        }
    }

    // Convert Null columns to Utf8 (no data → string)
    for t in &mut col_types {
        if *t == InferredType::Null {
            *t = InferredType::Utf8;
        }
    }

    // 3. Build schema
    let fields: Vec<Field> = column_names
        .iter()
        .zip(col_types.iter())
        .map(|(name, typ)| {
            let dt = match typ {
                InferredType::Float64 => DataType::Float64,
                InferredType::Boolean => DataType::Boolean,
                InferredType::Utf8 | InferredType::Null => DataType::Utf8,
            };
            Field::new(name, dt, true) // all columns nullable
        })
        .collect();
    let schema = Arc::new(Schema::new(fields));

    // 4. Build arrays column by column
    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(column_names.len());
    for (i, name) in column_names.iter().enumerate() {
        let arr: ArrayRef = match col_types[i] {
            InferredType::Float64 => {
                let values: Vec<Option<f64>> = rows
                    .iter()
                    .map(|row| {
                        row.get(name).and_then(|v| match v {
                            serde_json::Value::Number(n) => n.as_f64(),
                            serde_json::Value::String(s) => s.parse::<f64>().ok(),
                            serde_json::Value::Null => None,
                            _ => None,
                        })
                    })
                    .collect();
                Arc::new(Float64Array::from(values))
            }
            InferredType::Boolean => {
                let values: Vec<Option<bool>> = rows
                    .iter()
                    .map(|row| {
                        row.get(name).and_then(|v| match v {
                            serde_json::Value::Bool(b) => Some(*b),
                            serde_json::Value::Null => None,
                            _ => None,
                        })
                    })
                    .collect();
                Arc::new(BooleanArray::from(values))
            }
            InferredType::Utf8 | InferredType::Null => {
                let mut builder = StringBuilder::new();
                for row in rows {
                    match row.get(name) {
                        Some(serde_json::Value::String(s)) => builder.append_value(s),
                        Some(serde_json::Value::Number(n)) => {
                            builder.append_value(n.to_string())
                        }
                        Some(serde_json::Value::Bool(b)) => {
                            builder.append_value(b.to_string())
                        }
                        Some(serde_json::Value::Null) | None => builder.append_null(),
                        Some(other) => builder.append_value(other.to_string()),
                    }
                }
                Arc::new(builder.finish())
            }
        };
        arrays.push(arr);
    }

    RecordBatch::try_new(schema, arrays)
        .map_err(|e| ChartError::DataError(format!("Failed to create RecordBatch: {}", e)))
}

/// Convert Arrow `RecordBatch` slices back into `Vec<Row>`.
pub fn record_batch_to_rows(batches: &[RecordBatch]) -> Vec<Row> {
    let mut rows = Vec::new();

    for batch in batches {
        let schema = batch.schema();
        for row_idx in 0..batch.num_rows() {
            let mut row = Row::new();
            for (col_idx, field) in schema.fields().iter().enumerate() {
                let col = batch.column(col_idx);
                let value = arrow_value_to_json(col, row_idx);
                row.insert(field.name().clone(), value);
            }
            rows.push(row);
        }
    }

    rows
}

/// Extract a single cell from an Arrow array as serde_json::Value.
fn arrow_value_to_json(array: &dyn arrow::array::Array, idx: usize) -> serde_json::Value {
    if array.is_null(idx) {
        return serde_json::Value::Null;
    }

    match array.data_type() {
        DataType::Float64 => {
            let arr = array
                .as_any()
                .downcast_ref::<Float64Array>()
                .expect("DataType::Float64 arm guarantees Float64Array");
            let v = arr.value(idx);
            serde_json::json!(v)
        }
        DataType::Float32 => {
            let arr = array
                .as_any()
                .downcast_ref::<arrow::array::Float32Array>()
                .expect("DataType::Float32 arm guarantees Float32Array");
            serde_json::json!(arr.value(idx) as f64)
        }
        DataType::Int8 => {
            let arr = array.as_any().downcast_ref::<arrow::array::Int8Array>()
                .expect("DataType::Int8 arm guarantees Int8Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::Int16 => {
            let arr = array.as_any().downcast_ref::<arrow::array::Int16Array>()
                .expect("DataType::Int16 arm guarantees Int16Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::Int32 => {
            let arr = array.as_any().downcast_ref::<arrow::array::Int32Array>()
                .expect("DataType::Int32 arm guarantees Int32Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::Int64 => {
            let arr = array.as_any().downcast_ref::<arrow::array::Int64Array>()
                .expect("DataType::Int64 arm guarantees Int64Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt8 => {
            let arr = array.as_any().downcast_ref::<arrow::array::UInt8Array>()
                .expect("DataType::UInt8 arm guarantees UInt8Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt16 => {
            let arr = array.as_any().downcast_ref::<arrow::array::UInt16Array>()
                .expect("DataType::UInt16 arm guarantees UInt16Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt32 => {
            let arr = array.as_any().downcast_ref::<arrow::array::UInt32Array>()
                .expect("DataType::UInt32 arm guarantees UInt32Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::UInt64 => {
            let arr = array.as_any().downcast_ref::<arrow::array::UInt64Array>()
                .expect("DataType::UInt64 arm guarantees UInt64Array");
            serde_json::json!(arr.value(idx))
        }
        DataType::Boolean => {
            let arr = array
                .as_any()
                .downcast_ref::<BooleanArray>()
                .expect("DataType::Boolean arm guarantees BooleanArray");
            serde_json::json!(arr.value(idx))
        }
        DataType::Utf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("DataType::Utf8 arm guarantees StringArray");
            serde_json::json!(arr.value(idx))
        }
        DataType::LargeUtf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<arrow::array::LargeStringArray>()
                .expect("DataType::LargeUtf8 arm guarantees LargeStringArray");
            serde_json::json!(arr.value(idx))
        }
        DataType::Date32 => {
            // Date32 stores days since epoch — convert to ISO date string
            let arr = array
                .as_any()
                .downcast_ref::<arrow::array::Date32Array>()
                .expect("DataType::Date32 arm guarantees Date32Array");
            let days = arr.value(idx);
            // Convert days since epoch to YYYY-MM-DD
            let naive = days_to_iso(days as i64);
            serde_json::json!(naive)
        }
        _ => {
            // Fallback: use debug representation
            serde_json::Value::String(format!("{:?}", array.data_type()))
        }
    }
}

/// Convert days-since-epoch to ISO date string (YYYY-MM-DD).
fn days_to_iso(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Convert days since Unix epoch to (year, month, day).
/// Algorithm from Howard Hinnant's date algorithms.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Merge two inferred types, handling conflicts by coercing to Utf8.
fn merge_types(existing: InferredType, new: InferredType) -> InferredType {
    if new == InferredType::Null {
        return existing;
    }
    if existing == InferredType::Null {
        return new;
    }
    if existing == new {
        return existing;
    }
    // Types conflict → coerce to string
    InferredType::Utf8
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_row(pairs: Vec<(&str, serde_json::Value)>) -> Row {
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn test_rows_to_batch_roundtrip() {
        let rows = vec![
            make_row(vec![
                ("name", json!("Alice")),
                ("age", json!(30)),
                ("active", json!(true)),
            ]),
            make_row(vec![
                ("name", json!("Bob")),
                ("age", json!(25)),
                ("active", json!(false)),
            ]),
            make_row(vec![
                ("name", json!("Charlie")),
                ("age", json!(35)),
                ("active", json!(true)),
            ]),
        ];

        let batch = rows_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 3);
        assert_eq!(batch.num_columns(), 3);

        let result = record_batch_to_rows(&[batch]);
        assert_eq!(result.len(), 3);

        // Verify values roundtripped correctly
        for (orig, converted) in rows.iter().zip(result.iter()) {
            assert_eq!(
                orig.get("name").and_then(|v| v.as_str()),
                converted.get("name").and_then(|v| v.as_str()),
            );
            assert_eq!(
                orig.get("age").and_then(|v| v.as_f64()),
                converted.get("age").and_then(|v| v.as_f64()),
            );
            assert_eq!(
                orig.get("active").and_then(|v| v.as_bool()),
                converted.get("active").and_then(|v| v.as_bool()),
            );
        }
    }

    #[test]
    fn test_empty_rows() {
        let rows: Vec<Row> = vec![];
        let batch = rows_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 0);
        let result = record_batch_to_rows(&[batch]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_null_values() {
        let rows = vec![
            make_row(vec![("x", json!(1.0)), ("y", json!(null))]),
            make_row(vec![("x", json!(null)), ("y", json!("hello"))]),
        ];

        let batch = rows_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_rows(), 2);

        let result = record_batch_to_rows(&[batch]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_mixed_types_coerce_to_string() {
        // First row has number, second has string for same column
        let rows = vec![
            make_row(vec![("val", json!(42))]),
            make_row(vec![("val", json!("hello"))]),
        ];

        let batch = rows_to_record_batch(&rows).unwrap();
        // Should coerce to Utf8
        assert_eq!(batch.schema().field(0).data_type(), &DataType::Utf8);
    }

    #[test]
    fn test_missing_fields() {
        // Rows with different keys
        let rows = vec![
            make_row(vec![("a", json!(1.0))]),
            make_row(vec![("b", json!("x"))]),
        ];

        let batch = rows_to_record_batch(&rows).unwrap();
        assert_eq!(batch.num_columns(), 2);
        assert_eq!(batch.num_rows(), 2);
    }
}
