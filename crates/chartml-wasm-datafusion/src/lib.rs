use serde::Deserialize;
use wasm_bindgen::prelude::*;
use chartml_core::data::DataTable;
use chartml_core::plugin::transform::TransformContext;
use chartml_core::spec::TransformSpec;
use chartml_datafusion::DataFusionTransform;
use chartml_core::plugin::TransformMiddleware;

/// Deserializable wrapper matching the JS wire format for TransformContext.
/// JS sends `{ params: { key: value, ... } }` — this matches that shape.
#[derive(Deserialize, Default)]
struct ContextDto {
    #[serde(default)]
    params: std::collections::HashMap<String, serde_json::Value>,
}

/// Transform data using the DataFusion SQL engine.
///
/// Receives rows as JSON array, transform spec as JSON, context as JSON,
/// returns transformed rows + metadata.
/// This runs the full DataFusion pipeline: SQL → aggregate → forecast.
///
/// Note: For DataFusion, parameter substitution happens upstream in
/// chartml-core (params are resolved in the YAML string before parsing).
/// The context is passed through the trait interface for consistency with
/// custom JS transforms that may use it, but DataFusion itself does not
/// read context.params — the SQL already has concrete values by this point.
#[wasm_bindgen(js_name = "transform")]
pub async fn transform(rows_js: JsValue, spec_js: JsValue, context_js: JsValue) -> Result<JsValue, JsValue> {
    let rows: Vec<std::collections::HashMap<String, serde_json::Value>> =
        serde_wasm_bindgen::from_value(rows_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid rows: {}", e)))?;

    let data = DataTable::from_rows(&rows)
        .map_err(|e| JsValue::from_str(&format!("DataTable conversion failed: {}", e)))?;

    let spec: TransformSpec = serde_wasm_bindgen::from_value(spec_js)
        .map_err(|e| JsValue::from_str(&format!("Invalid transform spec: {}", e)))?;

    let context: TransformContext = if context_js.is_undefined() || context_js.is_null() {
        TransformContext::default()
    } else {
        let dto: ContextDto = serde_wasm_bindgen::from_value(context_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid transform context: {}", e)))?;
        TransformContext { params: dto.params }
    };

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
