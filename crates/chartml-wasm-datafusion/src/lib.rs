use wasm_bindgen::prelude::*;
use chartml_core::data::DataTable;
use chartml_core::plugin::transform::TransformContext;
use chartml_core::spec::TransformSpec;
use chartml_datafusion::DataFusionTransform;
use chartml_core::plugin::TransformMiddleware;

/// Transform data using the DataFusion SQL engine.
///
/// Receives rows as JSON array, transform spec as JSON, returns transformed rows + metadata.
/// This runs the full DataFusion pipeline: SQL -> aggregate -> forecast.
#[wasm_bindgen(js_name = "transform")]
pub async fn transform(rows_js: JsValue, spec_js: JsValue) -> Result<JsValue, JsValue> {
    let rows: Vec<std::collections::HashMap<String, serde_json::Value>> =
        serde_wasm_bindgen::from_value(rows_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid rows: {}", e)))?;

    let data = DataTable::from_rows(&rows)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let spec: TransformSpec = serde_wasm_bindgen::from_value(spec_js)
        .map_err(|e| JsValue::from_str(&format!("Invalid transform spec: {}", e)))?;

    let context = TransformContext::default();
    let transformer = DataFusionTransform;

    let result = transformer.transform(data, &spec, &context).await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Serialize result: { data: rows[], metadata: object }
    let result_rows = result.data.to_rows();
    let output = serde_json::json!({
        "data": result_rows,
        "metadata": result.metadata,
    });

    serde_wasm_bindgen::to_value(&output)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
