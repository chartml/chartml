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

/// Chart CSS for consumers who need it as a string (SSR, non-Leptos).
/// The `ChartMLChart` component injects this automatically on mount.
pub const CHARTML_CSS: &str = include_str!("../style/chartml.css");
pub use hooks::{use_chartml, use_chartml_configured};
pub use params_ui::ParamsControls;
pub use tooltip::{TooltipState, provide_tooltip_context, use_tooltip};
