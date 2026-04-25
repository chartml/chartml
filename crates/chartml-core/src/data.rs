use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Date32Array, Date64Array, Decimal128Array, Float32Array,
    Float64Array, Int8Array, Int16Array, Int32Array, Int64Array, RecordBatch, StringBuilder,
    StringArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

use crate::error::ChartError;

/// A row of data — a map of field names to values.
/// Used for inline YAML data and backward compatibility.
pub type Row = HashMap<String, serde_json::Value>;

// ── Legacy free functions (kept for backward compat / built-in transforms) ──

/// Extract an f64 value from a Row by field name.
/// Handles both Number and String (parsed) values.
pub fn get_f64(row: &Row, field: &str) -> Option<f64> {
    match row.get(field)? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Extract a string value from a Row by field name.
pub fn get_string(row: &Row, field: &str) -> Option<String> {
    match row.get(field)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// Compute the extent (min, max) of a numeric field across rows.
pub fn extent_rows(data: &[Row], field: &str) -> Option<(f64, f64)> {
    let values: Vec<f64> = data.iter().filter_map(|row| get_f64(row, field)).collect();
    if values.is_empty() {
        return None;
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some((min, max))
}

/// Sum a numeric field across rows.
pub fn sum_rows(data: &[Row], field: &str) -> f64 {
    data.iter().filter_map(|row| get_f64(row, field)).sum()
}

/// Group rows by a field value.
pub fn group_by_rows<'a>(data: &'a [Row], field: &str) -> HashMap<String, Vec<&'a Row>> {
    let mut groups: HashMap<String, Vec<&'a Row>> = HashMap::new();
    for row in data {
        if let Some(key) = get_string(row, field) {
            groups.entry(key).or_default().push(row);
        }
    }
    groups
}

/// Get unique values for a field, in order of first appearance.
pub fn unique_values_rows(data: &[Row], field: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for row in data {
        if let Some(val) = get_string(row, field) {
            if seen.insert(val.clone()) {
                result.push(val);
            }
        }
    }
    result
}

// Keep old names as aliases for backward compatibility
pub use extent_rows as extent;
pub use sum_rows as sum;
pub use group_by_rows as group_by;
pub use unique_values_rows as unique_values;

// ── DataTable: Arrow-backed columnar data ──

/// Type-preserving columnar data backed by Arrow RecordBatch.
///
/// Provides row-oriented accessors for renderer compatibility while
/// maintaining full Arrow type fidelity (timestamps, dates, decimals, etc.)
/// for the transform pipeline.
#[derive(Debug, Clone)]
pub struct DataTable {
    batch: RecordBatch,
    /// Column name → column index for fast lookup.
    field_index: HashMap<String, usize>,
}

impl DataTable {
    /// Wrap an existing Arrow RecordBatch.
    pub fn from_record_batch(batch: RecordBatch) -> Self {
        let field_index = batch
            .schema()
            .fields()
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name().clone(), i))
            .collect();
        Self { batch, field_index }
    }

    /// Convert JSON rows (from inline YAML data) into a DataTable.
    /// Type inference: Numbers → Float64, Booleans → Boolean, Strings → Utf8.
    pub fn from_rows(rows: &[Row]) -> Result<Self, ChartError> {
        if rows.is_empty() {
            let schema = Arc::new(Schema::new(Vec::<Field>::new()));
            let batch = RecordBatch::new_empty(schema);
            return Ok(Self::from_record_batch(batch));
        }

        // Collect column names preserving first-appearance order, then sort for determinism
        let mut column_names: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            for key in row.keys() {
                if seen.insert(key.clone()) {
                    column_names.push(key.clone());
                }
            }
        }
        column_names.sort();

        // Infer types
        let mut col_types: Vec<InferredType> = vec![InferredType::Null; column_names.len()];
        for row in rows {
            for (i, name) in column_names.iter().enumerate() {
                if let Some(val) = row.get(name) {
                    let val_type = match val {
                        serde_json::Value::Number(_) => InferredType::Float64,
                        serde_json::Value::Bool(_) => InferredType::Boolean,
                        serde_json::Value::String(_) => InferredType::Utf8,
                        serde_json::Value::Null => InferredType::Null,
                        _ => InferredType::Utf8,
                    };
                    col_types[i] = merge_inferred(col_types[i], val_type);
                }
            }
        }

        // Null → Utf8
        for t in &mut col_types {
            if *t == InferredType::Null {
                *t = InferredType::Utf8;
            }
        }

        // Build schema
        let fields: Vec<Field> = column_names
            .iter()
            .zip(col_types.iter())
            .map(|(name, typ)| {
                let dt = match typ {
                    InferredType::Float64 => DataType::Float64,
                    InferredType::Boolean => DataType::Boolean,
                    InferredType::Utf8 | InferredType::Null => DataType::Utf8,
                };
                Field::new(name, dt, true)
            })
            .collect();
        let schema = Arc::new(Schema::new(fields));

        // Build arrays
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

        let batch = RecordBatch::try_new(schema, arrays)
            .map_err(|e| ChartError::DataError(format!("Failed to create RecordBatch: {}", e)))?;
        Ok(Self::from_record_batch(batch))
    }

    /// Deserialize from Arrow IPC bytes.
    pub fn from_ipc_bytes(bytes: &[u8]) -> Result<Self, ChartError> {
        use arrow::ipc::reader::StreamReader;
        use std::io::Cursor;

        let cursor = Cursor::new(bytes);
        let reader = StreamReader::try_new(cursor, None)
            .map_err(|e| ChartError::DataError(format!("Failed to read Arrow IPC: {}", e)))?;

        let schema = reader.schema();
        let mut batches = Vec::new();
        for batch_result in reader {
            let batch = batch_result.map_err(|e| {
                ChartError::DataError(format!("Failed to read Arrow batch: {}", e))
            })?;
            batches.push(batch);
        }

        if batches.is_empty() {
            return Ok(Self::from_record_batch(RecordBatch::new_empty(schema)));
        }

        if batches.len() == 1 {
            return Ok(Self::from_record_batch(batches.remove(0)));
        }

        // Concatenate multiple batches
        let batch = arrow::compute::concat_batches(&schema, &batches)
            .map_err(|e| ChartError::DataError(format!("Failed to concat batches: {}", e)))?;
        Ok(Self::from_record_batch(batch))
    }

    /// Serialize to Arrow IPC bytes.
    pub fn to_ipc_bytes(&self) -> Result<Vec<u8>, ChartError> {
        use arrow::ipc::writer::StreamWriter;

        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &self.batch.schema())
                .map_err(|e| ChartError::DataError(format!("Failed to create IPC writer: {}", e)))?;
            writer.write(&self.batch).map_err(|e| {
                ChartError::DataError(format!("Failed to write Arrow batch: {}", e))
            })?;
            writer.finish().map_err(|e| {
                ChartError::DataError(format!("Failed to finish IPC stream: {}", e))
            })?;
        }
        Ok(buf)
    }

    // ── Accessors ──

    /// Number of rows.
    pub fn num_rows(&self) -> usize {
        self.batch.num_rows()
    }

    /// Number of columns.
    pub fn num_columns(&self) -> usize {
        self.batch.num_columns()
    }

    /// Whether the table has no rows.
    pub fn is_empty(&self) -> bool {
        self.batch.num_rows() == 0
    }

    /// Get the underlying RecordBatch.
    pub fn record_batch(&self) -> &RecordBatch {
        &self.batch
    }

    /// Get the Arrow schema.
    pub fn schema(&self) -> Arc<Schema> {
        self.batch.schema()
    }

    /// Get a column by name.
    fn column(&self, field: &str) -> Option<&ArrayRef> {
        self.field_index.get(field).map(|&i| self.batch.column(i))
    }

    /// Extract an f64 value from a specific row and field.
    /// Handles all numeric Arrow types, Date32 (days since epoch), and Timestamps (epoch millis).
    pub fn get_f64(&self, row: usize, field: &str) -> Option<f64> {
        let col = self.column(field)?;
        if col.is_null(row) {
            return None;
        }
        arrow_to_f64(col, row)
    }

    /// Extract a string value from a specific row and field.
    /// Formats temporal types as ISO strings, numbers as decimal strings.
    pub fn get_string(&self, row: usize, field: &str) -> Option<String> {
        let col = self.column(field)?;
        if col.is_null(row) {
            return None;
        }
        arrow_to_string(col, row)
    }

    /// Get unique string values for a field, preserving first-appearance order.
    pub fn unique_values(&self, field: &str) -> Vec<String> {
        let col = match self.column(field) {
            Some(c) => c,
            None => return Vec::new(),
        };
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for i in 0..self.batch.num_rows() {
            if col.is_null(i) {
                continue;
            }
            if let Some(val) = arrow_to_string(col, i) {
                if seen.insert(val.clone()) {
                    result.push(val);
                }
            }
        }
        result
    }

    /// Get all string values for a field, preserving row order (including duplicates).
    pub fn all_values(&self, field: &str) -> Vec<String> {
        let col = match self.column(field) {
            Some(c) => c,
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        for i in 0..self.batch.num_rows() {
            if col.is_null(i) {
                continue;
            }
            if let Some(val) = arrow_to_string(col, i) {
                result.push(val);
            }
        }
        result
    }

    /// Compute the extent (min, max) of a numeric field.
    pub fn extent(&self, field: &str) -> Option<(f64, f64)> {
        let col = self.column(field)?;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut found = false;
        for i in 0..self.batch.num_rows() {
            if col.is_null(i) {
                continue;
            }
            if let Some(v) = arrow_to_f64(col, i) {
                found = true;
                if v < min {
                    min = v;
                }
                if v > max {
                    max = v;
                }
            }
        }
        if found {
            Some((min, max))
        } else {
            None
        }
    }

    /// Sum a numeric field across all rows.
    pub fn sum(&self, field: &str) -> f64 {
        let col = match self.column(field) {
            Some(c) => c,
            None => return 0.0,
        };
        let mut total = 0.0;
        for i in 0..self.batch.num_rows() {
            if !col.is_null(i) {
                if let Some(v) = arrow_to_f64(col, i) {
                    total += v;
                }
            }
        }
        total
    }

    /// Group rows by a field value, returning a map of group key → DataTable.
    pub fn group_by(&self, field: &str) -> HashMap<String, DataTable> {
        let col = match self.column(field) {
            Some(c) => c,
            None => return HashMap::new(),
        };

        // Collect indices per group
        let mut group_indices: HashMap<String, Vec<u32>> = HashMap::new();
        let mut key_order: Vec<String> = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        for i in 0..self.batch.num_rows() {
            if col.is_null(i) {
                continue;
            }
            if let Some(key) = arrow_to_string(col, i) {
                if seen_keys.insert(key.clone()) {
                    key_order.push(key.clone());
                }
                group_indices.entry(key).or_default().push(i as u32);
            }
        }

        // Build sub-tables using arrow::compute::take
        let mut result = HashMap::new();
        for key in key_order {
            if let Some(indices) = group_indices.get(&key) {
                let indices_arr = UInt32Array::from(indices.clone());
                let take_result: Result<Vec<ArrayRef>, _> = self
                    .batch
                    .columns()
                    .iter()
                    .map(|col| arrow::compute::take(col.as_ref(), &indices_arr, None))
                    .collect();
                if let Ok(columns) = take_result {
                    if let Ok(sub_batch) = RecordBatch::try_new(self.batch.schema(), columns) {
                        result.insert(key, DataTable::from_record_batch(sub_batch));
                    }
                }
            }
        }
        result
    }

    /// Check if a field exists in the schema.
    pub fn has_field(&self, field: &str) -> bool {
        self.field_index.contains_key(field)
    }

    /// Get all field names.
    pub fn field_names(&self) -> Vec<String> {
        self.batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    /// Convert the Arrow RecordBatch back to JSON rows.
    /// Used for the built-in sync transform fallback path which operates on `Vec<Row>`.
    pub fn to_rows(&self) -> Vec<Row> {
        let num_rows = self.batch.num_rows();
        let schema = self.batch.schema();
        let fields = schema.fields();

        let mut rows = Vec::with_capacity(num_rows);
        for row_idx in 0..num_rows {
            let mut row = Row::new();
            for (col_idx, field) in fields.iter().enumerate() {
                let col = self.batch.column(col_idx);
                if col.is_null(row_idx) {
                    row.insert(field.name().clone(), serde_json::Value::Null);
                    continue;
                }
                let value = match col.data_type() {
                    DataType::Boolean => {
                        let v = col.as_any().downcast_ref::<BooleanArray>()
                            .expect("DataType::Boolean arm guarantees BooleanArray")
                            .value(row_idx);
                        serde_json::json!(v)
                    }
                    DataType::Float64 | DataType::Float32 |
                    DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 |
                    DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 |
                    DataType::Decimal128(_, _) => {
                        if let Some(v) = arrow_to_f64(col, row_idx) {
                            serde_json::json!(v)
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    // Dates and timestamps → preserve as ISO strings, not raw f64
                    DataType::Date32 | DataType::Date64 |
                    DataType::Timestamp(_, _) => {
                        if let Some(s) = arrow_to_string(col, row_idx) {
                            serde_json::Value::String(s)
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    _ => {
                        if let Some(s) = arrow_to_string(col, row_idx) {
                            serde_json::Value::String(s)
                        } else {
                            serde_json::Value::Null
                        }
                    }
                };
                row.insert(field.name().clone(), value);
            }
            rows.push(row);
        }
        rows
    }
}

// ── Arrow value extraction helpers ──

/// Extract an f64 from any Arrow array at the given index.
fn arrow_to_f64(col: &ArrayRef, idx: usize) -> Option<f64> {
    match col.data_type() {
        DataType::Float64 => {
            Some(col.as_any().downcast_ref::<Float64Array>()
                .expect("DataType::Float64 arm guarantees Float64Array")
                .value(idx))
        }
        DataType::Float32 => {
            Some(col.as_any().downcast_ref::<Float32Array>()
                .expect("DataType::Float32 arm guarantees Float32Array")
                .value(idx) as f64)
        }
        DataType::Int64 => {
            Some(col.as_any().downcast_ref::<Int64Array>()
                .expect("DataType::Int64 arm guarantees Int64Array")
                .value(idx) as f64)
        }
        DataType::Int32 => {
            Some(col.as_any().downcast_ref::<Int32Array>()
                .expect("DataType::Int32 arm guarantees Int32Array")
                .value(idx) as f64)
        }
        DataType::Int16 => {
            Some(col.as_any().downcast_ref::<Int16Array>()
                .expect("DataType::Int16 arm guarantees Int16Array")
                .value(idx) as f64)
        }
        DataType::Int8 => {
            Some(col.as_any().downcast_ref::<Int8Array>()
                .expect("DataType::Int8 arm guarantees Int8Array")
                .value(idx) as f64)
        }
        DataType::UInt64 => {
            Some(col.as_any().downcast_ref::<UInt64Array>()
                .expect("DataType::UInt64 arm guarantees UInt64Array")
                .value(idx) as f64)
        }
        DataType::UInt32 => {
            Some(col.as_any().downcast_ref::<UInt32Array>()
                .expect("DataType::UInt32 arm guarantees UInt32Array")
                .value(idx) as f64)
        }
        DataType::UInt16 => {
            Some(col.as_any().downcast_ref::<UInt16Array>()
                .expect("DataType::UInt16 arm guarantees UInt16Array")
                .value(idx) as f64)
        }
        DataType::UInt8 => {
            Some(col.as_any().downcast_ref::<UInt8Array>()
                .expect("DataType::UInt8 arm guarantees UInt8Array")
                .value(idx) as f64)
        }
        DataType::Boolean => {
            let v = col.as_any().downcast_ref::<BooleanArray>()
                .expect("DataType::Boolean arm guarantees BooleanArray")
                .value(idx);
            Some(if v { 1.0 } else { 0.0 })
        }
        DataType::Date32 => {
            // Days since epoch
            Some(col.as_any().downcast_ref::<Date32Array>()
                .expect("DataType::Date32 arm guarantees Date32Array")
                .value(idx) as f64)
        }
        DataType::Date64 => {
            // Milliseconds since epoch
            Some(col.as_any().downcast_ref::<Date64Array>()
                .expect("DataType::Date64 arm guarantees Date64Array")
                .value(idx) as f64)
        }
        DataType::Timestamp(unit, _) => {
            let raw = match unit {
                TimeUnit::Second => col
                    .as_any()
                    .downcast_ref::<TimestampSecondArray>()
                    .expect("Timestamp(Second) arm guarantees TimestampSecondArray")
                    .value(idx),
                TimeUnit::Millisecond => col
                    .as_any()
                    .downcast_ref::<TimestampMillisecondArray>()
                    .expect("Timestamp(Millisecond) arm guarantees TimestampMillisecondArray")
                    .value(idx),
                TimeUnit::Microsecond => col
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .expect("Timestamp(Microsecond) arm guarantees TimestampMicrosecondArray")
                    .value(idx),
                TimeUnit::Nanosecond => col
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .expect("Timestamp(Nanosecond) arm guarantees TimestampNanosecondArray")
                    .value(idx),
            };
            // Convert to epoch milliseconds for consistent f64 representation
            let millis = match unit {
                TimeUnit::Second => raw * 1000,
                TimeUnit::Millisecond => raw,
                TimeUnit::Microsecond => raw / 1000,
                TimeUnit::Nanosecond => raw / 1_000_000,
            };
            Some(millis as f64)
        }
        DataType::Decimal128(_, scale) => {
            let raw = col
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .expect("DataType::Decimal128 arm guarantees Decimal128Array")
                .value(idx);
            let divisor = 10_f64.powi(*scale as i32);
            Some(raw as f64 / divisor)
        }
        DataType::Utf8 => {
            // Try parsing string as number
            let s = col.as_any().downcast_ref::<StringArray>()
                .expect("DataType::Utf8 arm guarantees StringArray")
                .value(idx);
            s.parse::<f64>().ok()
        }
        _ => None,
    }
}

/// Extract a string from any Arrow array at the given index.
fn arrow_to_string(col: &ArrayRef, idx: usize) -> Option<String> {
    match col.data_type() {
        DataType::Utf8 => {
            Some(
                col.as_any()
                    .downcast_ref::<StringArray>()
                    .expect("DataType::Utf8 arm guarantees StringArray")
                    .value(idx)
                    .to_string(),
            )
        }
        DataType::LargeUtf8 => {
            Some(
                col.as_any()
                    .downcast_ref::<arrow::array::LargeStringArray>()
                    .expect("DataType::LargeUtf8 arm guarantees LargeStringArray")
                    .value(idx)
                    .to_string(),
            )
        }
        DataType::Float64 => {
            let v = col.as_any().downcast_ref::<Float64Array>()
                .expect("DataType::Float64 arm guarantees Float64Array")
                .value(idx);
            Some(format_f64(v))
        }
        DataType::Float32 => {
            let v = col.as_any().downcast_ref::<Float32Array>()
                .expect("DataType::Float32 arm guarantees Float32Array")
                .value(idx) as f64;
            Some(format_f64(v))
        }
        DataType::Int64 => {
            Some(col.as_any().downcast_ref::<Int64Array>()
                .expect("DataType::Int64 arm guarantees Int64Array")
                .value(idx).to_string())
        }
        DataType::Int32 => {
            Some(col.as_any().downcast_ref::<Int32Array>()
                .expect("DataType::Int32 arm guarantees Int32Array")
                .value(idx).to_string())
        }
        DataType::Int16 => {
            Some(col.as_any().downcast_ref::<Int16Array>()
                .expect("DataType::Int16 arm guarantees Int16Array")
                .value(idx).to_string())
        }
        DataType::Int8 => {
            Some(col.as_any().downcast_ref::<Int8Array>()
                .expect("DataType::Int8 arm guarantees Int8Array")
                .value(idx).to_string())
        }
        DataType::UInt64 => {
            Some(col.as_any().downcast_ref::<UInt64Array>()
                .expect("DataType::UInt64 arm guarantees UInt64Array")
                .value(idx).to_string())
        }
        DataType::UInt32 => {
            Some(col.as_any().downcast_ref::<UInt32Array>()
                .expect("DataType::UInt32 arm guarantees UInt32Array")
                .value(idx).to_string())
        }
        DataType::UInt16 => {
            Some(col.as_any().downcast_ref::<UInt16Array>()
                .expect("DataType::UInt16 arm guarantees UInt16Array")
                .value(idx).to_string())
        }
        DataType::UInt8 => {
            Some(col.as_any().downcast_ref::<UInt8Array>()
                .expect("DataType::UInt8 arm guarantees UInt8Array")
                .value(idx).to_string())
        }
        DataType::Boolean => {
            Some(col.as_any().downcast_ref::<BooleanArray>()
                .expect("DataType::Boolean arm guarantees BooleanArray")
                .value(idx).to_string())
        }
        DataType::Date32 => {
            let days = col.as_any().downcast_ref::<Date32Array>()
                .expect("DataType::Date32 arm guarantees Date32Array")
                .value(idx);
            Some(days_to_iso(days as i64))
        }
        DataType::Date64 => {
            let millis = col.as_any().downcast_ref::<Date64Array>()
                .expect("DataType::Date64 arm guarantees Date64Array")
                .value(idx);
            // Convert millis to days, then to ISO
            let days = millis / 86_400_000;
            Some(days_to_iso(days))
        }
        DataType::Timestamp(unit, tz) => {
            let raw = match unit {
                TimeUnit::Second => col
                    .as_any()
                    .downcast_ref::<TimestampSecondArray>()
                    .expect("Timestamp(Second) arm guarantees TimestampSecondArray")
                    .value(idx),
                TimeUnit::Millisecond => col
                    .as_any()
                    .downcast_ref::<TimestampMillisecondArray>()
                    .expect("Timestamp(Millisecond) arm guarantees TimestampMillisecondArray")
                    .value(idx),
                TimeUnit::Microsecond => col
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .expect("Timestamp(Microsecond) arm guarantees TimestampMicrosecondArray")
                    .value(idx),
                TimeUnit::Nanosecond => col
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .expect("Timestamp(Nanosecond) arm guarantees TimestampNanosecondArray")
                    .value(idx),
            };
            // Convert to seconds + subseconds using Euclidean division
            // so the remainder is always non-negative (safe for pre-epoch timestamps).
            let (secs, nanos_u32) = match unit {
                TimeUnit::Second => (raw, 0u32),
                TimeUnit::Millisecond => {
                    let (s, r) = (raw.div_euclid(1000), raw.rem_euclid(1000));
                    (s, (r * 1_000_000) as u32)
                }
                TimeUnit::Microsecond => {
                    let (s, r) = (raw.div_euclid(1_000_000), raw.rem_euclid(1_000_000));
                    (s, (r * 1000) as u32)
                }
                TimeUnit::Nanosecond => {
                    let (s, r) = (raw.div_euclid(1_000_000_000), raw.rem_euclid(1_000_000_000));
                    (s, r as u32)
                }
            };
            let iso = epoch_to_iso(secs, nanos_u32);
            if tz.is_some() {
                Some(format!("{}Z", iso))
            } else {
                Some(iso)
            }
        }
        DataType::Decimal128(_, scale) => {
            let raw = col
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .expect("DataType::Decimal128 arm guarantees Decimal128Array")
                .value(idx);
            Some(format_decimal128(raw, *scale))
        }
        _ => None,
    }
}

/// Format an f64 nicely — remove trailing zeros for integers.
fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}

/// Convert days-since-epoch to ISO date string (YYYY-MM-DD).
fn days_to_iso(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Convert epoch seconds + nanos to ISO datetime string.
fn epoch_to_iso(secs: i64, nanos: u32) -> String {
    let days = if secs >= 0 {
        secs / 86400
    } else {
        (secs - 86399) / 86400
    };
    let day_secs = secs - days * 86400;
    let (year, month, day) = civil_from_days(days);
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    if nanos == 0 {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year, month, day, hours, minutes, seconds
        )
    } else {
        let millis = nanos / 1_000_000;
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}",
            year, month, day, hours, minutes, seconds, millis
        )
    }
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

/// Format a Decimal128 value with the given scale.
fn format_decimal128(raw: i128, scale: i8) -> String {
    if scale <= 0 {
        return raw.to_string();
    }
    let divisor = 10_i128.pow(scale as u32);
    let whole = raw / divisor;
    let frac = (raw % divisor).abs();
    // For negative values between -1 and 0 (e.g., -0.05), whole truncates to 0
    // and loses the sign. Restore it explicitly.
    let sign = if raw < 0 && whole == 0 { "-" } else { "" };
    format!("{}{}.{:0>width$}", sign, whole, frac, width = scale as usize)
}

// ── Type inference helpers ──

#[derive(Debug, Clone, Copy, PartialEq)]
enum InferredType {
    Float64,
    Boolean,
    Utf8,
    Null,
}

fn merge_inferred(existing: InferredType, new: InferredType) -> InferredType {
    if new == InferredType::Null {
        return existing;
    }
    if existing == InferredType::Null {
        return new;
    }
    if existing == new {
        return existing;
    }
    // Mixed types → string
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

    // ── Legacy Row tests ──

    #[test]
    fn get_f64_from_number() {
        let row = make_row(vec![("value", json!(42.5))]);
        assert_eq!(get_f64(&row, "value"), Some(42.5));
    }

    #[test]
    fn get_f64_from_string() {
        let row = make_row(vec![("value", json!("123.45"))]);
        assert_eq!(get_f64(&row, "value"), Some(123.45));
    }

    #[test]
    fn get_f64_missing_field() {
        let row = make_row(vec![("other", json!(1.0))]);
        assert_eq!(get_f64(&row, "value"), None);
    }

    #[test]
    fn get_string_from_various() {
        let row_num = make_row(vec![("x", json!(42))]);
        assert_eq!(get_string(&row_num, "x"), Some("42".to_string()));

        let row_str = make_row(vec![("x", json!("hello"))]);
        assert_eq!(get_string(&row_str, "x"), Some("hello".to_string()));

        let row_bool = make_row(vec![("x", json!(true))]);
        assert_eq!(get_string(&row_bool, "x"), Some("true".to_string()));

        let row_null = make_row(vec![("x", json!(null))]);
        assert_eq!(get_string(&row_null, "x"), None);
    }

    #[test]
    fn extent_basic() {
        let data = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(30.0))]),
            make_row(vec![("v", json!(20.0))]),
        ];
        assert_eq!(extent(&data, "v"), Some((10.0, 30.0)));
    }

    #[test]
    fn extent_empty() {
        let data: Vec<Row> = vec![];
        assert_eq!(extent(&data, "v"), None);

        let data = vec![make_row(vec![("other", json!(1.0))])];
        assert_eq!(extent(&data, "v"), None);
    }

    #[test]
    fn sum_basic() {
        let data = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(20.0))]),
            make_row(vec![("v", json!(30.0))]),
        ];
        assert_eq!(sum(&data, "v"), 60.0);
    }

    #[test]
    fn group_by_basic() {
        let data = vec![
            make_row(vec![("cat", json!("A")), ("v", json!(1))]),
            make_row(vec![("cat", json!("B")), ("v", json!(2))]),
            make_row(vec![("cat", json!("A")), ("v", json!(3))]),
        ];
        let groups = group_by(&data, "cat");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["A"].len(), 2);
        assert_eq!(groups["B"].len(), 1);
    }

    #[test]
    fn unique_values_preserves_order() {
        let data = vec![
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("apple"))]),
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("cherry"))]),
            make_row(vec![("x", json!("apple"))]),
        ];
        let uniq = unique_values(&data, "x");
        assert_eq!(uniq, vec!["banana", "apple", "cherry"]);
    }

    // ── DataTable tests ──

    #[test]
    fn datatable_from_rows_roundtrip() {
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
        ];

        let dt = DataTable::from_rows(&rows).unwrap();
        assert_eq!(dt.num_rows(), 2);
        assert_eq!(dt.num_columns(), 3);

        assert_eq!(dt.get_string(0, "name"), Some("Alice".to_string()));
        assert_eq!(dt.get_f64(0, "age"), Some(30.0));
        assert_eq!(dt.get_string(1, "name"), Some("Bob".to_string()));
        assert_eq!(dt.get_f64(1, "age"), Some(25.0));
    }

    #[test]
    fn datatable_empty() {
        let dt = DataTable::from_rows(&[]).unwrap();
        assert_eq!(dt.num_rows(), 0);
        assert!(dt.is_empty());
    }

    #[test]
    fn datatable_unique_values() {
        let rows = vec![
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("apple"))]),
            make_row(vec![("x", json!("banana"))]),
            make_row(vec![("x", json!("cherry"))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        // Values preserve first-appearance order from the rows
        assert_eq!(dt.unique_values("x"), vec!["banana", "apple", "cherry"]);
    }

    #[test]
    fn datatable_extent() {
        let rows = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(30.0))]),
            make_row(vec![("v", json!(20.0))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        assert_eq!(dt.extent("v"), Some((10.0, 30.0)));
    }

    #[test]
    fn datatable_group_by() {
        let rows = vec![
            make_row(vec![("cat", json!("A")), ("v", json!(1))]),
            make_row(vec![("cat", json!("B")), ("v", json!(2))]),
            make_row(vec![("cat", json!("A")), ("v", json!(3))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        let groups = dt.group_by("cat");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups["A"].num_rows(), 2);
        assert_eq!(groups["B"].num_rows(), 1);
    }

    #[test]
    fn datatable_sum() {
        let rows = vec![
            make_row(vec![("v", json!(10.0))]),
            make_row(vec![("v", json!(20.0))]),
            make_row(vec![("v", json!(30.0))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        assert_eq!(dt.sum("v"), 60.0);
    }

    #[test]
    fn datatable_ipc_roundtrip() {
        let rows = vec![
            make_row(vec![("name", json!("Alice")), ("score", json!(95.5))]),
            make_row(vec![("name", json!("Bob")), ("score", json!(87.0))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        let bytes = dt.to_ipc_bytes().unwrap();
        let dt2 = DataTable::from_ipc_bytes(&bytes).unwrap();
        assert_eq!(dt2.num_rows(), 2);
        assert_eq!(dt2.get_string(0, "name"), Some("Alice".to_string()));
        assert_eq!(dt2.get_f64(1, "score"), Some(87.0));
    }

    #[test]
    fn datatable_record_batch_with_timestamps() {
        use arrow::array::TimestampMicrosecondArray;
        use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

        // Build a RecordBatch with a proper Timestamp column
        let schema = Arc::new(Schema::new(vec![
            Field::new("ts", DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())), true),
            Field::new("value", DataType::Float64, true),
        ]));
        // 2026-01-15T10:30:00Z = 1768474200 seconds = 1768474200000000 microseconds
        let ts_array = TimestampMicrosecondArray::from(vec![Some(1768474200000000i64)])
            .with_timezone("UTC");
        let val_array = Float64Array::from(vec![Some(42.0)]);
        let batch = RecordBatch::try_new(
            schema,
            vec![Arc::new(ts_array) as ArrayRef, Arc::new(val_array) as ArrayRef],
        )
        .unwrap();

        let dt = DataTable::from_record_batch(batch);
        // get_f64 returns epoch millis
        assert_eq!(dt.get_f64(0, "ts"), Some(1768474200000.0));
        // get_string returns ISO 8601 with Z suffix (has timezone)
        let ts_str = dt.get_string(0, "ts").unwrap();
        assert!(ts_str.ends_with('Z'), "Expected Z suffix, got: {}", ts_str);
        assert!(ts_str.starts_with("2026-01-15T"), "Expected 2026-01-15T, got: {}", ts_str);
    }

    #[test]
    fn datatable_record_batch_with_dates() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("d", DataType::Date32, true),
        ]));
        // 2026-01-15 = 20468 days since epoch
        let date_array = Date32Array::from(vec![Some(20468)]);
        let batch = RecordBatch::try_new(schema, vec![Arc::new(date_array) as ArrayRef]).unwrap();

        let dt = DataTable::from_record_batch(batch);
        assert_eq!(dt.get_string(0, "d"), Some("2026-01-15".to_string()));
        assert_eq!(dt.get_f64(0, "d"), Some(20468.0));
    }

    #[test]
    fn datatable_has_field() {
        let rows = vec![make_row(vec![("x", json!(1))])];
        let dt = DataTable::from_rows(&rows).unwrap();
        assert!(dt.has_field("x"));
        assert!(!dt.has_field("y"));
    }

    #[test]
    fn datatable_null_values() {
        let rows = vec![
            make_row(vec![("x", json!(1.0)), ("y", json!(null))]),
            make_row(vec![("x", json!(null)), ("y", json!("hello"))]),
        ];
        let dt = DataTable::from_rows(&rows).unwrap();
        assert_eq!(dt.get_f64(0, "x"), Some(1.0));
        assert_eq!(dt.get_f64(0, "y"), None);
        assert_eq!(dt.get_f64(1, "x"), None);
        assert_eq!(dt.get_string(1, "y"), Some("hello".to_string()));
    }
}
