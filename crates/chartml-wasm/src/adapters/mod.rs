pub mod data_source;
pub mod renderer;
pub mod resolver;
pub mod transform;

/// Wrapper to satisfy Send+Sync bounds on wasm32 where js_sys::Function is !Send.
/// Only compiled on wasm32 — on native targets, these adapters are not usable
/// (there is no JS runtime), so the type is gated.
pub(crate) struct SendFunction(pub(crate) js_sys::Function);

// Sound because wasm32-unknown-unknown is single-threaded.
unsafe impl Send for SendFunction {}
unsafe impl Sync for SendFunction {}
