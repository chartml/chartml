//! HTTP data source — fetches JSON from a URL using browser fetch() API.
//! This module is wasm32-only; it will not compile on native targets.
#![cfg(target_arch = "wasm32")]

use async_trait::async_trait;
use chartml_core::data::DataTable;
use chartml_core::error::ChartError;
use chartml_core::plugin::data_source::{DataSpec, FetchOptions};
use chartml_core::plugin::DataSource;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

pub struct HttpDataSource;

impl Default for HttpDataSource {
    fn default() -> Self {
        Self
    }
}

impl HttpDataSource {
    pub fn new() -> Self {
        Self
    }
}

// File is #![cfg(wasm32)] so only the ?Send variant is needed
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSource for HttpDataSource {
    async fn fetch(
        &self,
        spec: &DataSpec,
        _options: &FetchOptions,
    ) -> Result<DataTable, ChartError> {
        let url = spec
            .url
            .as_ref()
            .or(spec.endpoint.as_ref())
            .ok_or_else(|| {
                ChartError::DataError(
                    "HTTP data source requires 'url' or 'endpoint' field".into(),
                )
            })?;

        // Use browser fetch API
        let window = web_sys::window()
            .ok_or_else(|| ChartError::DataError("No window object available".into()))?;

        let opts = RequestInit::new();
        opts.set_method("GET");

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| ChartError::DataError(format!("Failed to create request: {e:?}")))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| ChartError::DataError(format!("Fetch failed: {e:?}")))?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| ChartError::DataError("Response is not a Response object".into()))?;

        if !resp.ok() {
            return Err(ChartError::DataError(format!(
                "HTTP {}: {}",
                resp.status(),
                resp.status_text()
            )));
        }

        let json = JsFuture::from(
            resp.json()
                .map_err(|e| ChartError::DataError(format!("json() failed: {e:?}")))?,
        )
        .await
        .map_err(|e| ChartError::DataError(format!("JSON parse failed: {e:?}")))?;

        let rows: Vec<std::collections::HashMap<String, serde_json::Value>> =
            serde_wasm_bindgen::from_value(json)
                .map_err(|e| ChartError::DataError(format!("Invalid JSON data: {e}")))?;

        DataTable::from_rows(&rows)
    }
}
