//! Opaque handle wrappers for the chartml 5.0 three-stage pipeline.
//!
//! `FetchedChart` and `PreparedChart` are Rust-only types that hold a
//! `ChartSpec` plus internal `DataTable` storage. We can't expose them as
//! `wasm_bindgen` structs directly because their fields aren't all
//! wasm-bindgen-compatible, and we don't want to round-trip the whole spec
//! through JSON between every pipeline stage.
//!
//! Solution: keep both stage outputs in a thread-local slab indexed by an
//! integer handle. JS receives `{ __chartmlHandle: <number>, kind: <"fetched"|"prepared"> }`
//! and passes it back into the next stage method, which extracts the typed
//! value from the slab. Slots are consumed (not just read) so a handle can
//! only be used once — preventing double-consume bugs.

use std::cell::RefCell;
use std::collections::HashMap;

use chartml_core::pipeline::{FetchedChart as CoreFetchedChart, PreparedChart as CorePreparedChart};
use chartml_core::RenderOptions;
use wasm_bindgen::prelude::*;

/// Tagged opaque handle type carried across the JS↔Rust boundary.
const HANDLE_PROP: &str = "__chartmlHandle";
const KIND_PROP: &str = "__chartmlHandleKind";
const KIND_FETCHED: &str = "fetched";
const KIND_PREPARED: &str = "prepared";

thread_local! {
    static NEXT_ID: RefCell<u64> = const { RefCell::new(1) };
    static FETCHED_SLOTS: RefCell<HashMap<u64, FetchedSlot>> = RefCell::new(HashMap::new());
    static PREPARED_SLOTS: RefCell<HashMap<u64, PreparedSlot>> = RefCell::new(HashMap::new());
}

pub(crate) struct FetchedSlot {
    pub fetched: CoreFetchedChart,
    pub opts: RenderOptions,
}

pub(crate) struct PreparedSlot {
    pub prepared: CorePreparedChart,
    pub opts: RenderOptions,
}

fn next_id() -> u64 {
    NEXT_ID.with(|n| {
        let mut n = n.borrow_mut();
        let id = *n;
        *n = n.checked_add(1).unwrap_or(1);
        id
    })
}

/// Stage 1 wrapper. `into_js` produces the opaque handle JS holds; `unwrap`
/// consumes the handle and returns the stored slot.
pub(crate) struct FetchedChart;

impl FetchedChart {
    pub fn wrap(fetched: CoreFetchedChart, opts: RenderOptions) -> FetchedSlot {
        FetchedSlot { fetched, opts }
    }

    pub fn unwrap(handle: JsValue) -> Result<FetchedSlot, JsValue> {
        let id = read_handle(&handle, KIND_FETCHED)?;
        FETCHED_SLOTS
            .with(|s| s.borrow_mut().remove(&id))
            .ok_or_else(|| {
                JsValue::from_str(&format!(
                    "FetchedChart handle {id} is invalid (already consumed or unknown)"
                ))
            })
    }
}

impl FetchedSlot {
    pub fn into_js(self) -> JsValue {
        let id = next_id();
        FETCHED_SLOTS.with(|s| s.borrow_mut().insert(id, self));
        make_handle(id, KIND_FETCHED)
    }
}

/// Stage 2 wrapper. Same handle pattern as `FetchedChart`.
pub(crate) struct PreparedChart;

impl PreparedChart {
    pub fn wrap(prepared: CorePreparedChart, opts: RenderOptions) -> PreparedSlot {
        PreparedSlot { prepared, opts }
    }

    pub fn unwrap(handle: JsValue) -> Result<PreparedSlot, JsValue> {
        let id = read_handle(&handle, KIND_PREPARED)?;
        PREPARED_SLOTS
            .with(|s| s.borrow_mut().remove(&id))
            .ok_or_else(|| {
                JsValue::from_str(&format!(
                    "PreparedChart handle {id} is invalid (already consumed or unknown)"
                ))
            })
    }
}

impl PreparedSlot {
    pub fn into_js(self) -> JsValue {
        let id = next_id();
        PREPARED_SLOTS.with(|s| s.borrow_mut().insert(id, self));
        make_handle(id, KIND_PREPARED)
    }
}

fn make_handle(id: u64, kind: &str) -> JsValue {
    let obj = js_sys::Object::new();
    // Store the id as a plain `Number` — the slab id space is small (one per
    // outstanding stage call) so 53-bit precision is more than enough.
    //
    // `Reflect::set` on a freshly-constructed plain `js_sys::Object` cannot
    // fail: there is no proxy `set` trap, no frozen prototype, no read-only
    // property to clobber. `.expect()` documents that invariant — discarding
    // with `let _ =` would silently swallow a real environment bug.
    js_sys::Reflect::set(
        &obj,
        &JsValue::from_str(HANDLE_PROP),
        &JsValue::from_f64(id as f64),
    )
    .expect("Reflect::set on a plain js_sys::Object cannot fail");
    js_sys::Reflect::set(&obj, &JsValue::from_str(KIND_PROP), &JsValue::from_str(kind))
        .expect("Reflect::set on a plain js_sys::Object cannot fail");
    obj.into()
}

fn read_handle(value: &JsValue, expected_kind: &str) -> Result<u64, JsValue> {
    if !value.is_object() {
        return Err(JsValue::from_str(
            "expected an opaque pipeline handle (object with __chartmlHandle / __chartmlHandleKind)",
        ));
    }
    let kind = js_sys::Reflect::get(value, &JsValue::from_str(KIND_PROP))
        .map_err(|_| JsValue::from_str("handle missing __chartmlHandleKind"))?;
    let kind_str = kind.as_string().ok_or_else(|| {
        JsValue::from_str("handle.__chartmlHandleKind must be a string")
    })?;
    if kind_str != expected_kind {
        return Err(JsValue::from_str(&format!(
            "wrong pipeline handle kind: expected '{expected_kind}', got '{kind_str}'"
        )));
    }
    let id = js_sys::Reflect::get(value, &JsValue::from_str(HANDLE_PROP))
        .map_err(|_| JsValue::from_str("handle missing __chartmlHandle"))?;
    let id_f = id
        .as_f64()
        .ok_or_else(|| JsValue::from_str("handle.__chartmlHandle must be a number"))?;
    Ok(id_f as u64)
}
