use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    params: HashMap<String, serde_json::Value>,
}

/// Transform data using the DataFusion SQL engine.
///
/// Receives sources as a JS object keyed by source name (`Record<string, object[]>`),
/// transform spec as JSON, context as JSON, returns transformed rows + metadata.
/// This runs the full DataFusion pipeline: SQL → aggregate → forecast.
///
/// Wire contract (matches `chartml_wasm::adapters::transform::JsTransformMiddleware`):
/// - `sources_js`: `Record<string, Array<Record<string, unknown>>>` — one entry per
///   declared source name, in YAML insertion order. Single-source pipelines pass
///   a one-entry object; multi-source pipelines (named-map `data:`) pass one entry
///   per declared source.
/// - `spec_js`: serialized [`TransformSpec`].
/// - `context_js`: `{ params: object }` matching the [`TransformContext`] shape.
///
/// The adapter feeds the resulting `IndexMap<String, DataTable>` directly to
/// [`DataFusionTransform::transform`], which handles the single-source-aliases-to-
/// `"source"` back-compat path itself — no aliasing logic lives here.
///
/// Note: For DataFusion, parameter substitution happens upstream in
/// chartml-core (params are resolved in the YAML string before parsing).
/// The context is passed through the trait interface for consistency with
/// custom JS transforms that may use it, but DataFusion itself does not
/// read context.params — the SQL already has concrete values by this point.
#[wasm_bindgen(js_name = "transform")]
pub async fn transform(
    sources_js: JsValue,
    spec_js: JsValue,
    context_js: JsValue,
) -> Result<JsValue, JsValue> {
    // Deserialize into IndexMap to preserve YAML insertion order — critical for
    // the `current_table` default in DataFusionTransform when SQL is omitted on
    // a single-source pipeline.
    let sources_raw: IndexMap<String, Vec<HashMap<String, serde_json::Value>>> =
        serde_wasm_bindgen::from_value(sources_js)
            .map_err(|e| JsValue::from_str(&format!("Invalid sources: {}", e)))?;

    if sources_raw.is_empty() {
        return Err(JsValue::from_str(
            "Invalid sources: at least one source table is required",
        ));
    }

    let mut sources: IndexMap<String, DataTable> = IndexMap::with_capacity(sources_raw.len());
    for (name, rows) in sources_raw {
        let table = DataTable::from_rows(&rows).map_err(|e| {
            JsValue::from_str(&format!(
                "DataTable conversion failed for source '{}': {}",
                name, e
            ))
        })?;
        sources.insert(name, table);
    }

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
    let result = transformer
        .transform(&sources, &spec, &context)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Serialize as a plain JS object (not the default ES `Map`) so the calling
    // adapter — `chartml_wasm::adapters::transform::JsTransformMiddleware`,
    // which deserializes via a struct DTO — can read the `data` / `metadata`
    // fields. The default `serde_wasm_bindgen::to_value` would emit a `Map`
    // and the round-trip would fail with "missing field `data`".
    let output = TransformResultDto {
        data: result.data.to_rows(),
        metadata: result.metadata,
    };
    output
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Wire shape returned to the JS adapter. Owns its data so we can drive
/// `serde_wasm_bindgen::Serializer::json_compatible()` directly (the
/// `serde_json::json!` macro does not support custom serializers).
#[derive(Serialize)]
struct TransformResultDto {
    data: Vec<chartml_core::data::Row>,
    metadata: HashMap<String, serde_json::Value>,
}
