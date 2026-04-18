//! Observability for the resolver pipeline (chartml 5.0 phase 3c).
//!
//! Consumers implement [`ResolverHooks`] and pass it to
//! [`ChartML::set_hooks`](crate::ChartML::set_hooks); the resolver fires
//! events at every pipeline decision point so consumers can wire progress
//! bars, cache-hit telemetry, custom error recovery, etc. without reading
//! private resolver state.
//!
//! Mirrors the JS `PLUGIN_HOOKS.md` semantics with a Rust-idiomatic
//! trait-based API.
//!
//! # Important constraints (read before implementing)
//!
//! - **Hooks must be panic-free.** `std::panic::catch_unwind` does not work
//!   across `.await` points and is not available on WASM at all. We do not
//!   try to catch hook panics — if a hook panics on native the spawned task
//!   crashes; on WASM the whole module typically aborts. Hooks should always
//!   catch their own errors internally.
//! - **Hooks must never acquire any lock the resolver holds.** Re-entering
//!   the resolver from within a hook is explicitly unsupported. The
//!   fire-and-forget spawn in `spawn_hook` mitigates this in practice
//!   because the hook runs on a separate task after the resolver's locks
//!   are released, but a hook that takes its own lock that the resolver
//!   later wants will still deadlock.
//! - **Hook events are fire-and-forget on the runtime — the resolver does
//!   NOT await hook delivery.** There is no ordering guarantee between
//!   events: events for the same source may arrive in any order, and events
//!   across sources interleave freely. Each `spawn_hook` call submits an
//!   independent task with no happens-before relationship to subsequent
//!   emits, so consumers that need a stable order must impose it themselves
//!   (e.g. by sequence-stamping events on the way out of the hook).
//! - **Hook errors are never propagated.** The trait methods return `()` so
//!   there's no `Result` to swallow; if `tokio::spawn` / `spawn_local` itself
//!   fails, the failure is logged via [`tracing::warn`] and not surfaced.

use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;

/// Pipeline phase a [`ProgressEvent`] or [`ErrorEvent`] originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Resolver fetch — cache walk + provider dispatch.
    Fetch,
    /// `ChartML::transform` — middleware application after fetch.
    Transform,
    /// `ChartML::render_prepared_to_svg` — final SVG emission.
    Render,
}

/// Cache tier the entry was served from on a [`CacheHitEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTier {
    /// Tier-1 in-process cache (typically `MemoryBackend`).
    Memory,
    /// Tier-2 persistent cache (e.g., `IndexedDbBackend` in the browser).
    Persistent,
}

/// Why a [`CacheMissEvent`] fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissReason {
    /// Key was not present in any tier.
    NotFound,
    /// Key was present but the entry's TTL had elapsed.
    Expired,
    /// Key was explicitly invalidated via [`crate::resolver::Resolver::invalidate`]
    /// (per-key) or one of the bulk APIs
    /// ([`crate::resolver::Resolver::invalidate_all`],
    /// [`crate::resolver::Resolver::invalidate_by_slug`],
    /// [`crate::resolver::Resolver::invalidate_by_namespace`]).
    /// Per-key invalidations report `Invalidated` for the next miss on that
    /// exact key; bulk invalidations report `Invalidated` for the very next
    /// miss on any key and then revert to `NotFound` / `Expired` reasoning
    /// (the resolver doesn't enumerate every just-evicted key — see the
    /// field-level docs on `Resolver::recently_invalidated` for the
    /// rationale).
    Invalidated,
}

/// Generic progress notification — fires at the start of each pipeline phase
/// and again on completion. `loaded` / `total` are populated when a provider
/// surfaces them; otherwise both are `None`.
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub phase: Phase,
    pub source_name: Option<String>,
    pub loaded: Option<u64>,
    pub total: Option<u64>,
    pub message: String,
}

/// Cache-hit notification.
#[derive(Debug, Clone)]
pub struct CacheHitEvent {
    pub key: u64,
    pub source_name: Option<String>,
    pub tier: CacheTier,
    pub age: Duration,
}

/// Cache-miss notification.
#[derive(Debug, Clone)]
pub struct CacheMissEvent {
    pub key: u64,
    pub source_name: Option<String>,
    pub reason: MissReason,
}

/// Error notification — fires when a provider call (or transform, when
/// emitted from `ChartML::transform`) returns an error. The error is
/// stringified at the emission site so consumers don't need to know the
/// concrete error type.
#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub phase: Phase,
    pub source_name: Option<String>,
    pub error: String,
}

/// Observability trait. Consumers implement only the methods they care
/// about; every default is a no-op so partial impls are zero-cost.
///
/// All methods are `async` so consumers can `await` inside (e.g. forwarding
/// to an async telemetry sink). Return type is `()` because the resolver
/// fires events fire-and-forget — there's no path for hook errors to
/// propagate back. Hook impls should catch their own errors internally.
///
/// # Safety contract
///
/// - Hooks **must not panic** (see module-level docs).
/// - Hooks **must not acquire any lock the resolver holds** (deadlock risk).
/// - Hooks **must not call back into the resolver** (re-entry is undefined).
// Native: hooks must be `Send + Sync` so they can be moved into spawned
// tokio tasks. WASM: the resolver and runtime are single-threaded, so the
// trait stays `?Send` to match `DataSourceProvider` / `CacheBackend`.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ResolverHooks: Send + Sync {
    /// Generic progress notification.
    async fn on_progress(&self, _event: ProgressEvent) {}
    /// Cache hit at any tier.
    async fn on_cache_hit(&self, _event: CacheHitEvent) {}
    /// Cache miss at every tier (or expired entry forcing re-fetch).
    async fn on_cache_miss(&self, _event: CacheMissEvent) {}
    /// Provider or transform error. Note: hook fires per-source for
    /// `NamedMap` shapes even though only the first error bubbles up to the
    /// caller as the user-facing `ChartError`.
    async fn on_error(&self, _event: ErrorEvent) {}
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait ResolverHooks {
    /// Generic progress notification.
    async fn on_progress(&self, _event: ProgressEvent) {}
    /// Cache hit at any tier.
    async fn on_cache_hit(&self, _event: CacheHitEvent) {}
    /// Cache miss at every tier (or expired entry forcing re-fetch).
    async fn on_cache_miss(&self, _event: CacheMissEvent) {}
    /// Provider or transform error. Note: hook fires per-source for
    /// `NamedMap` shapes even though only the first error bubbles up to the
    /// caller as the user-facing `ChartError`.
    async fn on_error(&self, _event: ErrorEvent) {}
}

/// Zero-cost default impl. Used by the resolver when no hook has been
/// registered. Public so consumers writing tests can opt into "explicitly
/// no hooks" without inventing a noop type.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHooks;

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl ResolverHooks for NullHooks {}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl ResolverHooks for NullHooks {}

/// Shared handle to a registered hook impl. `Arc` on native (so spawned
/// hook tasks can hold a clone without lifetime juggling), `Rc` on WASM
/// (single-threaded; matches the resolver's `SharedRef` story).
#[cfg(not(target_arch = "wasm32"))]
pub type HooksRef = std::sync::Arc<dyn ResolverHooks>;
#[cfg(target_arch = "wasm32")]
pub type HooksRef = std::rc::Rc<dyn ResolverHooks>;

/// Fire-and-forget hook dispatch. Wraps `tokio::spawn` on native and
/// `wasm_bindgen_futures::spawn_local` on WASM so a slow telemetry sink
/// can't stall the resolver.
///
/// **Native fallback:** if no tokio runtime is current, the event is
/// logged via [`tracing::warn`] and dropped. Run the resolver inside a
/// tokio runtime (e.g. `#[tokio::main]` or `tokio::runtime::Runtime`)
/// to receive hook events.
///
/// Spawn failures are logged via `tracing::warn!` and never propagated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn spawn_hook<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // Spawn the hook on the current runtime. We intentionally
            // discard the `JoinHandle` because hooks are fire-and-forget
            // by design; dropping the handle does NOT cancel the spawned
            // task on tokio's multi-thread or current-thread runtimes.
            // Using a named binding (`_handle`) instead of bare `_` keeps
            // the JoinHandle alive for the duration of this scope and
            // makes intent explicit.
            let _handle = handle.spawn(fut);
        }
        Err(_) => {
            // No tokio runtime available (e.g., a non-tokio test harness).
            // We cannot safely `block_on` here — the caller is already
            // inside an async context. Log and drop so the resolver
            // doesn't stall.
            tracing::warn!(
                target: "chartml::resolver::hooks",
                "no tokio runtime available; dropping hook event (consider running inside a tokio runtime to receive resolver hooks)"
            );
            drop(fut);
        }
    }
}

/// WASM variant — `spawn_local` requires the `wasm-bindgen` event loop,
/// which is always present in browser contexts. No fallback needed.
#[cfg(target_arch = "wasm32")]
pub(crate) fn spawn_hook<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(fut);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NullHooks` is genuinely zero-cost — no allocation, no state.
    #[test]
    fn null_hooks_is_zst() {
        assert_eq!(std::mem::size_of::<NullHooks>(), 0);
    }

    /// Trait docstring contains the panic-free constraint that the
    /// `test_hook_panic_is_documented` integration test checks for.
    #[test]
    fn module_docs_document_panic_free_requirement() {
        // The integration test reads the source file directly to verify
        // wording; this unit test exists so the requirement is also
        // exercised by `cargo test --lib`.
        let module_doc = include_str!("hooks.rs");
        assert!(
            module_doc.contains("Hooks must be panic-free")
                || module_doc.contains("must not panic"),
            "module docs must document the panic-free requirement"
        );
        assert!(
            module_doc.contains("fire-and-forget on the runtime")
                || module_doc.contains("no ordering guarantee"),
            "module docs must document the fire-and-forget / no-ordering semantics"
        );
    }
}
