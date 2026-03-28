use async_trait::async_trait;

use chartml_core::error::ChartError;
use chartml_core::plugin::resolver::ConnectionConfig;
use chartml_core::plugin::DatasourceResolver;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// JS-backed datasource resolver adapter.
///
/// Wraps a JS callback:
/// `(slug: string) => Promise<{provider: string, connectionString?: string, config: object}>`
pub struct JsDatasourceResolver {
    resolver_fn: SendFunction,
}

impl JsDatasourceResolver {
    pub fn new(resolver_fn: js_sys::Function) -> Self {
        Self {
            resolver_fn: SendFunction(resolver_fn),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DatasourceResolver for JsDatasourceResolver {
    async fn resolve(&self, slug: &str) -> Result<ConnectionConfig, ChartError> {
        let slug_js = JsValue::from_str(slug);

        // Call the JS function — it returns a Promise
        let promise = self
            .resolver_fn
            .0
            .call1(&JsValue::NULL, &slug_js)
            .map_err(|e| ChartError::DataError(format!("JS resolver error: {e:?}")))?;

        let promise: js_sys::Promise = promise
            .dyn_into()
            .map_err(|_| ChartError::DataError("Resolver did not return a Promise".into()))?;
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| ChartError::DataError(format!("Resolver Promise rejected: {e:?}")))?;

        // ConnectionConfig already derives Deserialize
        let config: ConnectionConfig = serde_wasm_bindgen::from_value(result)
            .map_err(|e| ChartError::DataError(format!("Invalid resolver result: {e}")))?;

        Ok(config)
    }
}
