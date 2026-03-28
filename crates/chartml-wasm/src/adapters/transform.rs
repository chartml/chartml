use async_trait::async_trait;
use std::collections::HashMap;

use chartml_core::data::DataTable;
use chartml_core::error::ChartError;
use chartml_core::plugin::transform::{TransformContext, TransformResult};
use chartml_core::plugin::TransformMiddleware;
use chartml_core::spec::TransformSpec;
use serde::{Deserialize, Serialize};
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
/// `(rows: object[], spec: object, context: object) => Promise<{data: object[], metadata: object}>`
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
        data: DataTable,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError> {
        // Convert DataTable to JSON rows for JS
        let rows = data.to_rows();
        let rows_js = serde_wasm_bindgen::to_value(&rows)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // TransformSpec already implements Serialize
        let spec_js = serde_wasm_bindgen::to_value(spec)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // TransformContext via our DTO
        let ctx_dto = TransformContextDto::from_context(context);
        let ctx_js = serde_wasm_bindgen::to_value(&ctx_dto)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // Call the JS function with 3 args
        let promise = self
            .transform_fn
            .0
            .call3(&JsValue::NULL, &rows_js, &spec_js, &ctx_js)
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
