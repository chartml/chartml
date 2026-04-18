//! `JsCallbackProvider` — bridges a JS async callback to the
//! [`DataSourceProvider`](chartml_core::resolver::DataSourceProvider) trait.
//!
//! Registered from JS via `ChartML::registerProvider(kind, callback)`. The JS
//! callback receives a [`FetchRequest`](chartml_core::resolver::FetchRequest)
//! shape and returns a `Promise` resolving to a `{ data, metadata? }` object,
//! where `data` is either an array of row objects or a `Uint8Array` of Arrow
//! IPC bytes.

use async_trait::async_trait;
use std::collections::HashMap;

use chartml_core::data::{DataTable, Row};
use chartml_core::resolver::{DataSourceProvider, FetchError, FetchRequest, FetchResult};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// Serializable DTO mirroring the `FetchRequest` fields that JS callbacks
/// care about. We don't expose the `cancel_token` (opaque) or the parsed
/// `cache` config (re-serialize from the spec at call time if needed).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchRequestDto {
    /// User-chosen name within the chart spec (`None` for unnamed flat data).
    source_name: Option<String>,
    /// Fully resolved flat-form spec — `datasource`, `query`, `url`, `rows`,
    /// etc. with `$param.name` references already substituted.
    spec: SpecDto,
    /// Per-request HTTP headers.
    headers: HashMap<String, String>,
    /// Tenant / workspace namespace (`None` for single-tenant deployments).
    namespace: Option<String>,
}

/// JSON-friendly mirror of `chartml_core::spec::InlineData` so JS sees plain
/// camelCase keys (`cacheTtl`) instead of the nested `cache: { ttl }` shape.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rows: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    datasource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
    /// Parsed cache TTL as a humantime string (`"5m"`, `"30s"`, ...) when
    /// the spec declared one. Forwarded so JS-side providers can choose to
    /// honor it (e.g. to set HTTP `Cache-Control` headers).
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_ttl: Option<String>,
    /// Component-layer `auto_refresh` flag — exposed verbatim so JS callbacks
    /// can mirror the same hint when proxying through their own caches.
    #[serde(default, skip_serializing_if = "is_false")]
    cache_auto_refresh: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl FetchRequestDto {
    fn from_request(req: &FetchRequest) -> Self {
        Self {
            source_name: req.source_name.clone(),
            spec: SpecDto {
                provider: req.spec.provider.clone(),
                rows: req.spec.rows.clone(),
                url: req.spec.url.clone(),
                endpoint: req.spec.endpoint.clone(),
                datasource: req.spec.datasource.clone(),
                query: req.spec.query.clone(),
                cache_ttl: req.spec.cache.as_ref().and_then(|c| c.ttl.clone()),
                cache_auto_refresh: req
                    .spec
                    .cache
                    .as_ref()
                    .and_then(|c| c.auto_refresh)
                    .unwrap_or(false),
            },
            headers: req.headers.clone(),
            namespace: req.namespace.clone(),
        }
    }
}

/// JS-backed `DataSourceProvider` adapter.
///
/// Wraps a JS callback:
/// `(request: FetchRequest) => Promise<{ data: object[] | Uint8Array, metadata?: object }>`
///
/// Errors from the JS callback (rejected promise OR thrown exception) become
/// [`FetchError::Other`]. The JS thread is single-threaded, so the trait is
/// satisfied with `?Send` on the wasm32 target (matches the `cfg_attr` on
/// `DataSourceProvider`).
pub struct JsCallbackProvider {
    /// User-chosen dispatch key (e.g. `"datasource"`, `"my-warehouse"`).
    /// Stored so error messages can name the failing provider.
    kind: String,
    callback: SendFunction,
}

impl JsCallbackProvider {
    pub fn new(kind: String, callback: js_sys::Function) -> Self {
        Self {
            kind,
            callback: SendFunction(callback),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSourceProvider for JsCallbackProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError> {
        let dto = FetchRequestDto::from_request(&request);
        let request_js = serde_wasm_bindgen::to_value(&dto).map_err(|e| {
            FetchError::Other(format!(
                "provider '{kind}': failed to serialize FetchRequest: {e}",
                kind = self.kind,
            ))
        })?;

        // Invoke the JS callback. A throw lands here as `Err(JsValue)`.
        let result = self
            .callback
            .0
            .call1(&JsValue::NULL, &request_js)
            .map_err(|e| {
                FetchError::Other(format!(
                    "provider '{kind}' threw on call: {msg}",
                    kind = self.kind,
                    msg = describe_js_error(&e),
                ))
            })?;

        // Awaitable contract: the callback MUST return a Promise.
        let promise: js_sys::Promise = result.dyn_into().map_err(|_| {
            FetchError::Other(format!(
                "provider '{kind}' did not return a Promise (returned a non-thenable JS value)",
                kind = self.kind,
            ))
        })?;
        let resolved = JsFuture::from(promise).await.map_err(|e| {
            FetchError::Other(format!(
                "provider '{kind}' Promise rejected: {msg}",
                kind = self.kind,
                msg = describe_js_error(&e),
            ))
        })?;

        decode_fetch_result(&self.kind, &resolved)
    }
}

/// Pull a human-readable string out of an arbitrary JS value (typically a
/// thrown `Error` or the rejection value from a `Promise`). Falls back to
/// `format!("{:?}", value)` when the value isn't an object with a `.message`
/// property.
fn describe_js_error(value: &JsValue) -> String {
    if let Some(s) = value.as_string() {
        return s;
    }
    if value.is_object() {
        if let Ok(message) = js_sys::Reflect::get(value, &JsValue::from_str("message")) {
            if let Some(s) = message.as_string() {
                return s;
            }
        }
    }
    format!("{value:?}")
}

/// Decode the JS return value from a provider callback into a [`FetchResult`].
///
/// Accepted `data` shapes:
/// - **Array of row objects** — each entry is an object whose own enumerable
///   string-keyed properties become row columns (the canonical chartml shape).
/// - **`Uint8Array` of Arrow IPC bytes** — decoded via
///   [`DataTable::from_ipc_bytes`].
///
/// `metadata` is optional; when absent the result carries an empty
/// `HashMap`. Anything else (`null`, scalar, missing `data`, …) surfaces as
/// `FetchError::DecodeFailed` so the host app sees the JS-side mistake
/// rather than a silently empty table.
fn decode_fetch_result(kind: &str, resolved: &JsValue) -> Result<FetchResult, FetchError> {
    if !resolved.is_object() {
        return Err(FetchError::DecodeFailed(format!(
            "provider '{kind}' Promise resolved to a non-object value (expected {{ data, metadata? }})"
        )));
    }

    let data_js = js_sys::Reflect::get(resolved, &JsValue::from_str("data")).map_err(|e| {
        FetchError::DecodeFailed(format!(
            "provider '{kind}' result missing `data` property: {msg}",
            msg = describe_js_error(&e),
        ))
    })?;
    if data_js.is_undefined() || data_js.is_null() {
        return Err(FetchError::DecodeFailed(format!(
            "provider '{kind}' result missing `data` property"
        )));
    }

    let data = if data_js.is_instance_of::<js_sys::Uint8Array>() {
        let bytes = js_sys::Uint8Array::from(data_js.clone()).to_vec();
        DataTable::from_ipc_bytes(&bytes).map_err(|e| {
            FetchError::DecodeFailed(format!(
                "provider '{kind}' Arrow IPC decode failed: {e}"
            ))
        })?
    } else if js_sys::Array::is_array(&data_js) {
        let rows: Vec<Row> = serde_wasm_bindgen::from_value(data_js).map_err(|e| {
            FetchError::DecodeFailed(format!(
                "provider '{kind}' rows decode failed (expected array of objects): {e}"
            ))
        })?;
        DataTable::from_rows(&rows).map_err(|e| {
            FetchError::DecodeFailed(format!("provider '{kind}' from_rows failed: {e}"))
        })?
    } else {
        return Err(FetchError::DecodeFailed(format!(
            "provider '{kind}' `data` must be an array of objects or a Uint8Array (Arrow IPC)"
        )));
    };

    // `metadata` is optional and defaults to an empty map.
    let metadata = match js_sys::Reflect::get(resolved, &JsValue::from_str("metadata")) {
        Ok(meta_js) if !meta_js.is_undefined() && !meta_js.is_null() => {
            serde_wasm_bindgen::from_value(meta_js).map_err(|e| {
                FetchError::DecodeFailed(format!(
                    "provider '{kind}' metadata decode failed: {e}"
                ))
            })?
        }
        _ => HashMap::new(),
    };

    Ok(FetchResult { data, metadata })
}
