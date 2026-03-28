pub mod renderer;

// Async adapters use JsFuture which is !Send — only compile on wasm32
#[cfg(target_arch = "wasm32")]
pub mod data_source;
#[cfg(target_arch = "wasm32")]
pub mod resolver;
#[cfg(target_arch = "wasm32")]
pub mod transform;

/// Wrapper to satisfy Send+Sync bounds on wasm32 where js_sys::Function is !Send.
/// Only compiled on wasm32 — on native targets, these adapters are not usable
/// (there is no JS runtime), so the type is gated.
pub(crate) struct SendFunction(pub(crate) js_sys::Function);

// Sound because wasm32-unknown-unknown is single-threaded.
unsafe impl Send for SendFunction {}
unsafe impl Sync for SendFunction {}
