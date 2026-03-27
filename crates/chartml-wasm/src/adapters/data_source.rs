use async_trait::async_trait;
use chartml_core::data::DataTable;
use chartml_core::error::ChartError;
use chartml_core::plugin::data_source::{DataSpec, FetchOptions};
use chartml_core::plugin::DataSource;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// A serializable DTO mirroring `DataSpec` + `FetchOptions` for passing to JS.
/// We do not modify chartml-core; instead we convert manually.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DataSpecDto {
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rows: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_ttl: Option<String>,
}

impl DataSpecDto {
    fn from_spec(spec: &DataSpec, options: &FetchOptions) -> Self {
        Self {
            provider: spec.provider.clone(),
            rows: spec.rows.clone(),
            url: spec.url.clone(),
            endpoint: spec.endpoint.clone(),
            cache_ttl: options.cache_ttl.clone(),
        }
    }
}

/// JS-backed data source adapter.
///
/// Wraps a JS callback `(spec: object) => Promise<object[]>` and implements
/// the `DataSource` trait so it can be registered with `ChartML`.
pub struct JsDataSource {
    fetch_fn: SendFunction,
}

impl JsDataSource {
    pub fn new(fetch_fn: js_sys::Function) -> Self {
        Self {
            fetch_fn: SendFunction(fetch_fn),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSource for JsDataSource {
    async fn fetch(
        &self,
        spec: &DataSpec,
        options: &FetchOptions,
    ) -> Result<DataTable, ChartError> {
        // Serialize spec + options to a JS value via our DTO
        let dto = DataSpecDto::from_spec(spec, options);
        let spec_js = serde_wasm_bindgen::to_value(&dto)
            .map_err(|e| ChartError::DataError(e.to_string()))?;

        // Call the JS function — it returns a Promise
        let promise = self
            .fetch_fn
            .0
            .call1(&JsValue::NULL, &spec_js)
            .map_err(|e| ChartError::DataError(format!("JS data source error: {e:?}")))?;

        // Convert to Promise and await
        let promise: js_sys::Promise = promise
            .dyn_into()
            .map_err(|_| ChartError::DataError("Data source did not return a Promise".into()))?;
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| ChartError::DataError(format!("Data source Promise rejected: {e:?}")))?;

        // Deserialize the returned array-of-objects into rows
        let rows: Vec<std::collections::HashMap<String, serde_json::Value>> =
            serde_wasm_bindgen::from_value(result)
                .map_err(|e| ChartError::DataError(format!("Invalid data from JS source: {e}")))?;

        DataTable::from_rows(&rows)
    }
}
