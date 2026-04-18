//! Pluggable persistent `CacheBackend` implementations (chartml 5.0 phase 3b).
//!
//! Each backend lives behind its own cargo feature so non-browser builds stay
//! lean. Today this hosts:
//!
//! - [`codec`] — pure-Rust binary codec for the on-disk blob format. Shared
//!   across every persistent backend so the framing has one source of truth.
//!   Compiled whenever a backend that uses it is on (today: `wasm-indexeddb`)
//!   OR whenever tests are running, so the framing is exercised on native via
//!   `cargo test --workspace` even with no feature flags enabled.
//! - [`indexeddb::IndexedDbBackend`] — browser-side persistent cache wired
//!   through the [`idb`] crate. Behind the `wasm-indexeddb` feature, only
//!   compiles on the `wasm32-unknown-unknown` target.
//!
//! Future tier-2 backends (file-system, SQLite, Redis, …) will land alongside
//! `indexeddb` here, each behind their own feature flag, and reuse the same
//! `codec` framing.

// `codec` is pure Rust (no `idb`/`js-sys`/`wasm-bindgen` deps). Compiled when
// any consuming backend is on, OR when tests are building — that way native
// `cargo test --workspace` exercises the byte framing without needing the
// browser feature on. Cfg-gating it to consumers + `test` keeps `cargo check`
// on native (no features, no tests) free of dead-code warnings, so the
// workspace-wide `dead_code = "deny"` policy stays honored without per-fn
// lint suppression attributes.
#[cfg(any(test, all(target_arch = "wasm32", feature = "wasm-indexeddb")))]
pub(crate) mod codec;

#[cfg(all(target_arch = "wasm32", feature = "wasm-indexeddb"))]
pub mod indexeddb;
