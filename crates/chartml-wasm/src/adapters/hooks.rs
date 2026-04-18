//! `JsCallbackHooks` — bridges a JS object of optional callbacks to the
//! [`ResolverHooks`](chartml_core::resolver::ResolverHooks) trait.
//!
//! Registered from JS via `ChartML::setHooks({ onProgress, onCacheHit, ... })`.
//! Every handler is optional; missing ones are no-ops.
//!
//! Each handler receives a plain JS object that mirrors the matching Rust
//! event struct in camelCase (`{ phase, sourceName, loaded, total, message }`,
//! `{ key, sourceName, tier, age }`, etc.). Handlers may be `async` (return a
//! Promise) — the resolver fires events fire-and-forget so a slow JS sink
//! cannot stall the pipeline.

use async_trait::async_trait;
use serde::Serialize;

use chartml_core::resolver::{
    CacheHitEvent, CacheMissEvent, CacheTier, ErrorEvent, MissReason, Phase, ProgressEvent,
    ResolverHooks,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::SendFunction;

/// Set of JS callbacks for the four observability events.
///
/// All handlers are optional — when a slot is `None`, the corresponding
/// trait method is a no-op. Constructed by reading the four well-known
/// property names off a JS object inside `ChartML::setHooks`.
pub struct JsCallbackHooks {
    on_progress: Option<SendFunction>,
    on_cache_hit: Option<SendFunction>,
    on_cache_miss: Option<SendFunction>,
    on_error: Option<SendFunction>,
}

impl JsCallbackHooks {
    /// Read each of `onProgress` / `onCacheHit` / `onCacheMiss` / `onError`
    /// off the supplied JS object. Properties that are missing OR not
    /// callable are simply omitted (no error) so partial subscribers work.
    pub fn from_js(handlers: &JsValue) -> Self {
        Self {
            on_progress: read_callable(handlers, "onProgress"),
            on_cache_hit: read_callable(handlers, "onCacheHit"),
            on_cache_miss: read_callable(handlers, "onCacheMiss"),
            on_error: read_callable(handlers, "onError"),
        }
    }
}

/// Pull a property by name off a JS object and return it wrapped in a
/// `SendFunction` if and only if it's callable. Missing or non-function
/// values simply yield `None`.
fn read_callable(handlers: &JsValue, name: &str) -> Option<SendFunction> {
    if !handlers.is_object() {
        return None;
    }
    let value = js_sys::Reflect::get(handlers, &JsValue::from_str(name)).ok()?;
    if value.is_undefined() || value.is_null() {
        return None;
    }
    let func: js_sys::Function = value.dyn_into().ok()?;
    Some(SendFunction(func))
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ResolverHooks for JsCallbackHooks {
    async fn on_progress(&self, event: ProgressEvent) {
        if let Some(cb) = &self.on_progress {
            invoke_handler(&cb.0, &ProgressEventDto::from(event)).await;
        }
    }

    async fn on_cache_hit(&self, event: CacheHitEvent) {
        if let Some(cb) = &self.on_cache_hit {
            invoke_handler(&cb.0, &CacheHitEventDto::from(event)).await;
        }
    }

    async fn on_cache_miss(&self, event: CacheMissEvent) {
        if let Some(cb) = &self.on_cache_miss {
            invoke_handler(&cb.0, &CacheMissEventDto::from(event)).await;
        }
    }

    async fn on_error(&self, event: ErrorEvent) {
        if let Some(cb) = &self.on_error {
            invoke_handler(&cb.0, &ErrorEventDto::from(event)).await;
        }
    }
}

/// Serialize the DTO, invoke the JS callback with it, and await the result
/// if the callback returned a Promise. Any error is logged via `tracing` and
/// dropped — hooks are documented as fire-and-forget, so we never propagate
/// failures back to the resolver.
async fn invoke_handler<T: Serialize>(handler: &js_sys::Function, event: &T) {
    let event_js = match serde_wasm_bindgen::to_value(event) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                target: "chartml::wasm::hooks",
                "failed to serialize hook event: {e}"
            );
            return;
        }
    };
    let result = match handler.call1(&JsValue::NULL, &event_js) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                target: "chartml::wasm::hooks",
                "JS hook threw: {e:?}"
            );
            return;
        }
    };
    // If the handler returned a Promise, await it so async sinks complete.
    // Sync-only handlers return undefined / a non-thenable — those are no-ops.
    if let Ok(promise) = result.dyn_into::<js_sys::Promise>() {
        if let Err(e) = JsFuture::from(promise).await {
            tracing::warn!(
                target: "chartml::wasm::hooks",
                "JS hook Promise rejected: {e:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// DTOs — flatten the Rust event structs into JS-friendly camelCase objects.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEventDto {
    phase: PhaseDto,
    source_name: Option<String>,
    loaded: Option<u64>,
    total: Option<u64>,
    message: String,
}

impl From<ProgressEvent> for ProgressEventDto {
    fn from(e: ProgressEvent) -> Self {
        Self {
            phase: e.phase.into(),
            source_name: e.source_name,
            loaded: e.loaded,
            total: e.total,
            message: e.message,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheHitEventDto {
    /// `u64` cache key serialized as a JS string to avoid the `BigInt`/`Number`
    /// precision-loss footgun. Consumers can `BigInt(event.key)` if they need
    /// the numeric form.
    key: String,
    source_name: Option<String>,
    tier: CacheTierDto,
    /// Entry age in milliseconds — `Number`-safe (well under 2^53 for any
    /// realistic TTL).
    age_ms: u64,
}

impl From<CacheHitEvent> for CacheHitEventDto {
    fn from(e: CacheHitEvent) -> Self {
        Self {
            key: format!("{:#x}", e.key),
            source_name: e.source_name,
            tier: e.tier.into(),
            age_ms: e.age.as_millis().min(u64::MAX as u128) as u64,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheMissEventDto {
    key: String,
    source_name: Option<String>,
    reason: MissReasonDto,
}

impl From<CacheMissEvent> for CacheMissEventDto {
    fn from(e: CacheMissEvent) -> Self {
        Self {
            key: format!("{:#x}", e.key),
            source_name: e.source_name,
            reason: e.reason.into(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEventDto {
    phase: PhaseDto,
    source_name: Option<String>,
    error: String,
}

impl From<ErrorEvent> for ErrorEventDto {
    fn from(e: ErrorEvent) -> Self {
        Self {
            phase: e.phase.into(),
            source_name: e.source_name,
            error: e.error,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum PhaseDto {
    Fetch,
    Transform,
    Render,
}

impl From<Phase> for PhaseDto {
    fn from(p: Phase) -> Self {
        match p {
            Phase::Fetch => PhaseDto::Fetch,
            Phase::Transform => PhaseDto::Transform,
            Phase::Render => PhaseDto::Render,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum CacheTierDto {
    Memory,
    Persistent,
}

impl From<CacheTier> for CacheTierDto {
    fn from(t: CacheTier) -> Self {
        match t {
            CacheTier::Memory => CacheTierDto::Memory,
            CacheTier::Persistent => CacheTierDto::Persistent,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum MissReasonDto {
    NotFound,
    Expired,
    Invalidated,
}

impl From<MissReason> for MissReasonDto {
    fn from(r: MissReason) -> Self {
        match r {
            MissReason::NotFound => MissReasonDto::NotFound,
            MissReason::Expired => MissReasonDto::Expired,
            MissReason::Invalidated => MissReasonDto::Invalidated,
        }
    }
}
