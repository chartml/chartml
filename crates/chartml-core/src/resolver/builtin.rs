//! Built-in providers: `InlineProvider` and `HttpProvider`.
//!
//! Both are pre-registered on `ChartML::new()` under their respective dispatch
//! keys (`"inline"`, `"http"`). Hosts can override either by calling
//! `register_provider("inline", ...)` / `register_provider("http", ...)` with
//! their own implementation — e.g., an HTTP provider that adds OAuth refresh.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::data::{DataTable, Row};

use super::{DataSourceProvider, FetchError, FetchRequest, FetchResult};

/// Provider for `data: { rows: [...] }` shapes. Materializes the inline rows
/// into a `DataTable` via `DataTable::from_rows`. Empty rows produce an
/// empty table — the same as the legacy inline path.
pub struct InlineProvider;

impl InlineProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InlineProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSourceProvider for InlineProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError> {
        let rows_value = request.spec.rows.unwrap_or_default();
        let rows: Vec<Row> = rows_value
            .into_iter()
            .map(|value| match value {
                serde_json::Value::Object(map) => Ok(map.into_iter().collect::<Row>()),
                other => Err(FetchError::DecodeFailed(format!(
                    "InlineProvider expects each row to be a JSON object, got: {other}"
                ))),
            })
            .collect::<Result<Vec<Row>, FetchError>>()?;

        let data = DataTable::from_rows(&rows)
            .map_err(|e| FetchError::DecodeFailed(format!("from_rows failed: {e}")))?;
        Ok(FetchResult {
            data,
            metadata: HashMap::new(),
        })
    }
}

/// Provider for `data: { url: "..." }` shapes. Issues a GET via `reqwest`
/// (works on both native and WASM with no feature-flag branching).
///
/// Header handling:
/// - `with_default_headers(...)` sets defaults that apply to every request.
/// - `FetchRequest.headers` overrides any default with the same name on a
///   per-request basis (matches HTTP intuition: per-call wins over default).
///
/// Decode rule:
/// - `Content-Type: application/vnd.apache.arrow.*` → decode as Arrow IPC
///   bytes via `DataTable::from_ipc_bytes`.
/// - anything else → parse as JSON. JSON arrays of objects flow through
///   `DataTable::from_rows`; JSON objects with a top-level array key (`rows`,
///   `data`, or `results`) are unwrapped automatically to match the most
///   common API conventions.
pub struct HttpProvider {
    client: reqwest::Client,
    default_headers: HashMap<String, String>,
}

impl HttpProvider {
    /// New provider with no default headers. Convenience for the default
    /// `register_provider("http", HttpProvider::new())` registration.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            default_headers: HashMap::new(),
        }
    }

    /// Builder: attach default headers (e.g. `Authorization`, `User-Agent`)
    /// applied to every request unless overridden by `FetchRequest.headers`.
    pub fn with_default_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.default_headers = headers;
        self
    }
}

impl Default for HttpProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl DataSourceProvider for HttpProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResult, FetchError> {
        let url = request
            .spec
            .url
            .as_deref()
            .ok_or_else(|| FetchError::Other(
                "HttpProvider requires `url` in the data spec".to_string(),
            ))?;

        // Merge headers: defaults first, then per-request overrides.
        let mut merged: HashMap<String, String> = self.default_headers.clone();
        for (k, v) in &request.headers {
            merged.insert(k.clone(), v.clone());
        }

        let mut req = self.client.get(url);
        for (name, value) in &merged {
            req = req.header(name, value);
        }

        let response = req
            .send()
            .await
            .map_err(|e| FetchError::QueryFailed(format!("HTTP GET {url} failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::QueryFailed(format!(
                "HTTP GET {url} returned status {status}"
            )));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FetchError::DecodeFailed(format!("body read failed: {e}")))?;

        let data = if content_type.starts_with("application/vnd.apache.arrow") {
            DataTable::from_ipc_bytes(&bytes)
                .map_err(|e| FetchError::DecodeFailed(format!("Arrow IPC decode failed: {e}")))?
        } else {
            decode_json_to_table(&bytes)?
        };

        Ok(FetchResult {
            data,
            metadata: HashMap::new(),
        })
    }
}

/// Decode an HTTP response body as JSON and convert to a `DataTable`.
///
/// Accepts three top-level shapes for compatibility with common API
/// conventions:
/// - `[ {...}, {...} ]` → array of objects (the canonical chartml shape).
/// - `{ "rows": [ ... ] }`, `{ "data": [ ... ] }`, `{ "results": [ ... ] }` →
///   unwrap the array key and treat as the canonical shape.
/// - anything else → `DecodeFailed` with the discovered shape in the error.
fn decode_json_to_table(bytes: &[u8]) -> Result<DataTable, FetchError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| FetchError::DecodeFailed(format!("JSON parse failed: {e}")))?;

    let array = match value {
        serde_json::Value::Array(arr) => arr,
        serde_json::Value::Object(mut obj) => {
            // Common top-level wrapper conventions, in order of preference.
            const ARRAY_KEYS: [&str; 3] = ["rows", "data", "results"];
            let mut found: Option<Vec<serde_json::Value>> = None;
            for key in ARRAY_KEYS {
                if let Some(serde_json::Value::Array(arr)) = obj.remove(key) {
                    found = Some(arr);
                    break;
                }
            }
            found.ok_or_else(|| {
                FetchError::DecodeFailed(
                    "JSON object response must have a top-level `rows`, `data`, or `results` array key"
                        .to_string(),
                )
            })?
        }
        other => {
            return Err(FetchError::DecodeFailed(format!(
                "JSON response must be an array of objects or an object with a `rows`/`data`/`results` array; got: {}",
                discriminant_name(&other),
            )));
        }
    };

    let rows: Vec<Row> = array
        .into_iter()
        .map(|v| match v {
            serde_json::Value::Object(map) => Ok(map.into_iter().collect::<Row>()),
            other => Err(FetchError::DecodeFailed(format!(
                "JSON array entries must be objects, got: {}",
                discriminant_name(&other),
            ))),
        })
        .collect::<Result<Vec<Row>, FetchError>>()?;

    DataTable::from_rows(&rows)
        .map_err(|e| FetchError::DecodeFailed(format!("from_rows failed: {e}")))
}

/// Pretty-print a JSON value's variant for error messages.
fn discriminant_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::resolver::FetchRequest;
    use crate::spec::InlineData;
    use serde_json::json;

    fn empty_request(spec: InlineData) -> FetchRequest {
        FetchRequest {
            source_name: None,
            spec,
            cache: None,
            headers: HashMap::new(),
            namespace: None,
            cancel_token: None,
        }
    }

    #[tokio::test]
    async fn inline_provider_basic_rows() {
        let provider = InlineProvider::new();
        let spec = InlineData {
            provider: Some("inline".into()),
            rows: Some(vec![
                json!({"x": "A", "y": 1}),
                json!({"x": "B", "y": 2}),
            ]),
            url: None,
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        };
        let result = provider.fetch(empty_request(spec)).await.unwrap();
        assert_eq!(result.data.num_rows(), 2);
    }

    #[tokio::test]
    async fn inline_provider_empty_rows() {
        let provider = InlineProvider::new();
        let spec = InlineData {
            provider: Some("inline".into()),
            rows: Some(vec![]),
            url: None,
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        };
        let result = provider.fetch(empty_request(spec)).await.unwrap();
        assert_eq!(result.data.num_rows(), 0);
    }

    #[tokio::test]
    async fn inline_provider_rejects_non_object_rows() {
        let provider = InlineProvider::new();
        let spec = InlineData {
            provider: Some("inline".into()),
            rows: Some(vec![json!(42)]),
            url: None,
            endpoint: None,
            cache: None,
            datasource: None,
            query: None,
        };
        let err = provider.fetch(empty_request(spec)).await.unwrap_err();
        assert!(matches!(err, FetchError::DecodeFailed(_)));
    }

    #[test]
    fn json_decode_array_of_objects() {
        let body = b"[{\"x\":1},{\"x\":2}]";
        let table = decode_json_to_table(body).unwrap();
        assert_eq!(table.num_rows(), 2);
    }

    #[test]
    fn json_decode_rows_wrapper() {
        let body = b"{\"rows\":[{\"x\":1}]}";
        let table = decode_json_to_table(body).unwrap();
        assert_eq!(table.num_rows(), 1);
    }

    #[test]
    fn json_decode_data_wrapper() {
        let body = b"{\"data\":[{\"x\":1},{\"x\":2}]}";
        let table = decode_json_to_table(body).unwrap();
        assert_eq!(table.num_rows(), 2);
    }

    #[test]
    fn json_decode_rejects_bare_object() {
        let body = b"{\"foo\":\"bar\"}";
        let err = decode_json_to_table(body).unwrap_err();
        assert!(matches!(err, FetchError::DecodeFailed(_)));
    }

    #[test]
    fn json_decode_rejects_scalar() {
        let body = b"42";
        let err = decode_json_to_table(body).unwrap_err();
        assert!(matches!(err, FetchError::DecodeFailed(_)));
    }
}
