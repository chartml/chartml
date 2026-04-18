//! `JsCallbackCacheBackend` — bridges a JS object of async backend
//! handlers to the [`CacheBackend`](chartml_core::resolver::CacheBackend)
//! trait. Used when `ChartML::setCache(jsObject)` is called with a custom
//! JS-side backend (e.g. host-app-managed Redis-via-RPC, sessionStorage,
//! …).
//!
//! Required methods on the JS object:
//! - `get(keyHex: string) => Promise<CachedEntry | null>`
//! - `put(keyHex: string, entry: CachedEntry) => Promise<void>`
//! - `invalidate(keyHex: string) => Promise<void>`
//! - `invalidateByTag(tag: string) => Promise<void>`
//! - `clear() => Promise<void>`
//! - `shutdown?() => Promise<void>` (optional)
//!
//! `CachedEntry` shape on the JS side:
//! ```text
//! { rows: object[], fetchedAtMs: number, ttlMs: number,
//!   tags: string[], metadata: object }
//! ```
//!
//! Cache keys are passed as hexadecimal strings (`"0xdeadbeef..."`) so they
//! survive the JS `Number` 53-bit precision cap. Backends that want to
//! double-check can re-parse the hex back to a `BigInt` JS-side.

use async_trait::async_trait;
use std::collections::HashMap;
// `web_time` is the wasm32-compatible alias for `std::time` — must match the
// `SystemTime` flavor `CachedEntry::fetched_at` uses inside chartml-core.
use std::time::Duration;
use web_time::{SystemTime, UNIX_EPOCH};

use chartml_core::data::{DataTable, Row};
use chartml_core::resolver::{CacheBackend, CacheError, CachedEntry};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// Wraps a JS object that exposes async cache backend methods.
///
/// Methods missing from the JS object that the [`CacheBackend`] trait
/// requires produce a [`CacheError::Backend`] at call time (rather than at
/// `setCache` registration) so dynamic JS objects whose method set isn't
/// finalized at construction can still be inspected later.
pub struct JsCallbackCacheBackend {
    get: Option<SendFunction>,
    put: Option<SendFunction>,
    invalidate: Option<SendFunction>,
    invalidate_by_tag: Option<SendFunction>,
    clear: Option<SendFunction>,
    shutdown: Option<SendFunction>,
}

impl JsCallbackCacheBackend {
    /// Read the well-known method names off the supplied JS object. Returns
    /// `Err` only when the input is not an object — each individual method
    /// is allowed to be missing.
    pub fn from_js(handlers: JsValue) -> Result<Self, JsValue> {
        if !handlers.is_object() {
            return Err(JsValue::from_str(
                "setCache: backend must be an object exposing get / put / invalidate / invalidateByTag / clear",
            ));
        }
        // Each `read_callable` extraction takes a strong reference to the
        // method `js_sys::Function` itself. wasm-bindgen `JsValue`s (and the
        // `Function` newtype that derefs to one) hold their JS object alive
        // for as long as the Rust handle exists, so the individual method
        // refs are sufficient — we do NOT need to also pin the parent
        // `handlers` object. (Methods captured against the parent's `this`
        // are NOT supported by this bridge: each handler is invoked with
        // `JsValue::NULL` as the receiver, so a method that depends on
        // `this` would be broken in this code path regardless.)
        let this = Self {
            get: read_callable(&handlers, "get"),
            put: read_callable(&handlers, "put"),
            invalidate: read_callable(&handlers, "invalidate"),
            invalidate_by_tag: read_callable(&handlers, "invalidateByTag"),
            clear: read_callable(&handlers, "clear"),
            shutdown: read_callable(&handlers, "shutdown"),
        };
        // Best-effort sanity: at minimum get / put / invalidate / clear are
        // load-bearing for any non-trivial workload. A backend missing those
        // is almost certainly a typo, so flag it loudly at registration.
        let mut missing: Vec<&'static str> = Vec::new();
        if this.get.is_none() {
            missing.push("get");
        }
        if this.put.is_none() {
            missing.push("put");
        }
        if this.invalidate.is_none() {
            missing.push("invalidate");
        }
        if this.invalidate_by_tag.is_none() {
            missing.push("invalidateByTag");
        }
        if this.clear.is_none() {
            missing.push("clear");
        }
        if !missing.is_empty() {
            // Surface as Err on registration so the user sees the typo
            // immediately rather than on first cache hit.
            return Err(JsValue::from_str(&format!(
                "setCache: backend object is missing required method(s): {}",
                missing.join(", ")
            )));
        }
        Ok(this)
    }
}

fn read_callable(handlers: &JsValue, name: &str) -> Option<SendFunction> {
    let value = js_sys::Reflect::get(handlers, &JsValue::from_str(name)).ok()?;
    if value.is_undefined() || value.is_null() {
        return None;
    }
    let func: js_sys::Function = value.dyn_into().ok()?;
    Some(SendFunction(func))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CachedEntryDto {
    rows: Vec<Row>,
    /// Epoch milliseconds — `Number`-safe well past the year 2100.
    fetched_at_ms: u64,
    ttl_ms: u64,
    tags: Vec<String>,
    metadata: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedEntryDtoIn {
    rows: Vec<Row>,
    fetched_at_ms: u64,
    ttl_ms: u64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,
}

impl CachedEntryDto {
    fn from_entry(entry: &CachedEntry) -> Self {
        let fetched_at_ms = entry
            .fetched_at
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        Self {
            rows: entry.data.to_rows(),
            fetched_at_ms,
            ttl_ms: entry.ttl.as_millis().min(u64::MAX as u128) as u64,
            tags: entry.tags.clone(),
            metadata: entry.metadata.clone(),
        }
    }
}

impl TryFrom<CachedEntryDtoIn> for CachedEntry {
    type Error = CacheError;
    fn try_from(dto: CachedEntryDtoIn) -> Result<Self, Self::Error> {
        let data = DataTable::from_rows(&dto.rows)
            .map_err(|e| CacheError::Backend(format!("from_rows failed: {e}")))?;
        let fetched_at = UNIX_EPOCH
            .checked_add(Duration::from_millis(dto.fetched_at_ms))
            .unwrap_or_else(SystemTime::now);
        Ok(CachedEntry {
            data,
            fetched_at,
            ttl: Duration::from_millis(dto.ttl_ms),
            tags: dto.tags,
            metadata: dto.metadata,
        })
    }
}

/// Common helper: invoke a JS method, await the returned Promise (if any),
/// and surface failures as [`CacheError::Backend`].
async fn call_method_promise(
    method_name: &str,
    func: &js_sys::Function,
    args: &js_sys::Array,
) -> Result<JsValue, CacheError> {
    let result = func
        .apply(&JsValue::NULL, args)
        .map_err(|e| CacheError::Backend(format!("backend.{method_name} threw: {e:?}")))?;
    // Tolerate sync handlers returning undefined / a non-thenable.
    if let Ok(promise) = result.clone().dyn_into::<js_sys::Promise>() {
        JsFuture::from(promise)
            .await
            .map_err(|e| CacheError::Backend(format!("backend.{method_name} rejected: {e:?}")))
    } else {
        Ok(result)
    }
}

fn key_to_hex(key: u64) -> JsValue {
    JsValue::from_str(&format!("{key:#x}"))
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl CacheBackend for JsCallbackCacheBackend {
    async fn get(&self, key: u64) -> Option<CachedEntry> {
        let cb = self.get.as_ref()?;
        let args = js_sys::Array::of1(&key_to_hex(key));
        let resolved = match call_method_promise("get", &cb.0, &args).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(target: "chartml::wasm::cache", "{e}");
                return None;
            }
        };
        if resolved.is_null() || resolved.is_undefined() {
            return None;
        }
        let dto: CachedEntryDtoIn = match serde_wasm_bindgen::from_value(resolved) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    target: "chartml::wasm::cache",
                    "backend.get returned a value that did not match CachedEntry: {e}"
                );
                return None;
            }
        };
        match CachedEntry::try_from(dto) {
            Ok(entry) => Some(entry),
            Err(e) => {
                tracing::warn!(target: "chartml::wasm::cache", "{e}");
                None
            }
        }
    }

    async fn put(&self, key: u64, entry: CachedEntry) -> Result<(), CacheError> {
        let cb = self
            .put
            .as_ref()
            .ok_or_else(|| CacheError::Backend("put handler missing".into()))?;
        let dto = CachedEntryDto::from_entry(&entry);
        let entry_js = serde_wasm_bindgen::to_value(&dto)
            .map_err(|e| CacheError::Backend(format!("entry serialize failed: {e}")))?;
        let args = js_sys::Array::of2(&key_to_hex(key), &entry_js);
        call_method_promise("put", &cb.0, &args).await?;
        Ok(())
    }

    async fn invalidate(&self, key: u64) -> Result<(), CacheError> {
        let cb = self
            .invalidate
            .as_ref()
            .ok_or_else(|| CacheError::Backend("invalidate handler missing".into()))?;
        let args = js_sys::Array::of1(&key_to_hex(key));
        call_method_promise("invalidate", &cb.0, &args).await?;
        Ok(())
    }

    async fn invalidate_by_tag(&self, tag: &str) -> Result<(), CacheError> {
        let cb = self
            .invalidate_by_tag
            .as_ref()
            .ok_or_else(|| CacheError::Backend("invalidateByTag handler missing".into()))?;
        let args = js_sys::Array::of1(&JsValue::from_str(tag));
        call_method_promise("invalidateByTag", &cb.0, &args).await?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let cb = self
            .clear
            .as_ref()
            .ok_or_else(|| CacheError::Backend("clear handler missing".into()))?;
        let args = js_sys::Array::new();
        call_method_promise("clear", &cb.0, &args).await?;
        Ok(())
    }

    async fn shutdown(&self) {
        if let Some(cb) = &self.shutdown {
            let args = js_sys::Array::new();
            if let Err(e) = call_method_promise("shutdown", &cb.0, &args).await {
                tracing::warn!(target: "chartml::wasm::cache", "{e}");
            }
        }
    }
}
