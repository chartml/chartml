pub mod chart;
pub mod element;
pub mod hooks;
pub mod params_ui;
pub mod tooltip;

pub use chart::{ChartMLChart, TooltipRenderer};
pub use element::render_element;

/// Shared-ownership wrapper around `ChartML` used by every Leptos hook,
/// component, and view in this crate. `Arc` on native targets so it can be
/// moved across `tokio::spawn` task boundaries; `Rc` on WASM where the
/// chartml resolver is inherently `?Send` (clippy's
/// `arc_with_non_send_sync` correctly rejects `Arc` for that case, and
/// `wasm32-unknown-unknown` is single-threaded so `Rc` is the right
/// primitive). Construct with `ChartMLRef::new(chartml)`.
#[cfg(not(target_arch = "wasm32"))]
pub type ChartMLRef = std::sync::Arc<chartml_core::ChartML>;
#[cfg(target_arch = "wasm32")]
pub type ChartMLRef = std::rc::Rc<chartml_core::ChartML>;

/// Shared-ownership wrapper around a [`chartml_core::DataSourceProvider`].
///
/// `Arc<dyn DataSourceProvider>` on every target — the trait itself is
/// `Send + Sync`, so a single `Arc` form works for native (where it can be
/// shared across `tokio::spawn` task boundaries) and for WASM (where
/// `wasm32-unknown-unknown` is single-threaded but the resolver's internal
/// provider registry is also `Arc<dyn ...>`-typed).
///
/// Use this alias when threading a provider into [`ChartMLChart`] via the
/// `provider` prop or via Leptos context.
pub type ProviderRef = std::sync::Arc<dyn chartml_core::DataSourceProvider>;

/// Shared-ownership wrapper around a [`chartml_core::CacheBackend`].
/// Re-exports [`chartml_core::CacheBackendRef`] (`Arc` on native, `Rc` on
/// WASM) — the trait drops its `Send + Sync` supertrait on wasm32 to
/// support backends like `IndexedDbBackend` whose `Rc<RefCell<Database>>`
/// internals are `!Send`, so an unconditional `std::sync::Arc<dyn
/// CacheBackend>` would trip `clippy::arc_with_non_send_sync` on wasm32.
pub use chartml_core::CacheBackendRef;

/// Shared-ownership wrapper around a [`chartml_core::ResolverHooks`] impl.
/// Mirrors [`chartml_core::HooksRef`] so consumers wiring hooks through
/// Leptos context have a single, Leptos-flavored re-export to import.
/// Cfg-gated `Arc`/`Rc` because hook impls don't need to satisfy `Send+Sync`
/// on WASM (the trait is `?Send` there to match async closures used by
/// callback-style host-app implementations).
pub use chartml_core::HooksRef;

/// Chart CSS for consumers who need it as a string (SSR, non-Leptos).
/// The `ChartMLChart` component injects this automatically on mount.
pub const CHARTML_CSS: &str = include_str!("../style/chartml.css");
pub use hooks::{use_chartml, use_chartml_configured};
pub use params_ui::ParamsControls;
pub use tooltip::{TooltipState, provide_tooltip_context, use_tooltip};
