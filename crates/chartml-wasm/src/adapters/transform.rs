use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;

use chartml_core::data::DataTable;
use chartml_core::error::ChartError;
use chartml_core::plugin::transform::{TransformContext, TransformResult};
use chartml_core::plugin::TransformMiddleware;
use chartml_core::spec::TransformSpec;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// Serializable DTO for `TransformContext`.
#[derive(Serialize)]
struct TransformContextDto {
    params: HashMap<String, serde_json::Value>,
}

impl TransformContextDto {
    fn from_context(ctx: &TransformContext) -> Self {
        Self {
            params: ctx.params.clone(),
        }
    }
}

/// Shape returned by the JS transform callback.
#[derive(Deserialize)]
struct JsTransformResultDto {
    data: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,
}

/// JS-backed transform middleware adapter.
///
/// Wraps a JS callback:
/// `(sources: Record<string, object[]>, spec: object, context: object) => Promise<{data: object[], metadata: object}>`
///
/// `sources` is an object keyed by source name. For single-source pipelines the
/// object has one entry; for multi-source pipelines (named-map `data:`), one
/// entry per declared name, in YAML insertion order.
pub struct JsTransformMiddleware {
    transform_fn: SendFunction,
}

impl JsTransformMiddleware {
    pub fn new(transform_fn: js_sys::Function) -> Self {
        Self {
            transform_fn: SendFunction(transform_fn),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TransformMiddleware for JsTransformMiddleware {
    async fn transform(
        &self,
        sources: &IndexMap<String, DataTable>,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError> {
        // Convert each source's DataTable into a JSON-row array, preserving
        // insertion order so JS callbacks see source names in the same order
        // they appear in the YAML `data:` map.
        //
        // Use the json-compatible serializer so the IndexMap and per-row
        // HashMaps materialize as plain JS objects, matching the documented
        // `Record<string, Array<Record<string, unknown>>>` contract. The
        // default `to_value` would emit ES `Map`s, which third-party JS
        // callbacks (and the chartml-wasm-datafusion bridge) cannot read as
        // structured records.
        let serializer = Serializer::json_compatible();
        let sources_js_map: IndexMap<String, Vec<chartml_core::data::Row>> = sources
            .iter()
            .map(|(name, table)| (name.clone(), table.to_rows()))
            .collect();
        let sources_js = sources_js_map
            .serialize(&serializer)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // TransformSpec already implements Serialize
        let spec_js = spec
            .serialize(&serializer)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // TransformContext via our DTO
        let ctx_dto = TransformContextDto::from_context(context);
        let ctx_js = ctx_dto
            .serialize(&serializer)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // Call the JS function with 3 args
        let promise = self
            .transform_fn
            .0
            .call3(&JsValue::NULL, &sources_js, &spec_js, &ctx_js)
            .map_err(|e| ChartError::DataError(format!("JS transform error: {e:?}")))?;

        let promise: js_sys::Promise = promise.dyn_into().map_err(|_| {
            ChartError::DataError("Transform did not return a Promise".into())
        })?;
        let result = JsFuture::from(promise).await.map_err(|e| {
            ChartError::DataError(format!("Transform Promise rejected: {e:?}"))
        })?;

        // Deserialize the {data, metadata} result
        let dto: JsTransformResultDto = serde_wasm_bindgen::from_value(result)
            .map_err(|e| ChartError::DataError(format!("Invalid transform result: {e}")))?;

        let result_data = DataTable::from_rows(&dto.data)?;

        Ok(TransformResult {
            data: result_data,
            metadata: dto.metadata,
        })
    }
}
