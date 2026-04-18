use chartml_core::ChartML;

use crate::ChartMLRef;

/// Create a bare ChartML instance with no renderers registered.
/// Returns a `ChartMLRef` (`Arc<ChartML>` on native, `Rc<ChartML>` on WASM)
/// for sharing with components.
///
/// Users should register their own renderers after calling this,
/// or use `use_chartml_configured` to set up in one step.
///
/// Usage:
/// ```ignore
/// let chartml = use_chartml();
/// view! { <ChartMLChart spec=yaml chartml=chartml /> }
/// ```
pub fn use_chartml() -> ChartMLRef {
    ChartMLRef::new(ChartML::new())
}

/// Create a ChartML instance and configure it via the provided closure.
/// The closure receives a mutable reference to register renderers, data sources, etc.
///
/// Usage:
/// ```ignore
/// let chartml = use_chartml_configured(|c| {
///     c.register_renderer("bar", CartesianRenderer::new());
///     c.register_renderer("line", CartesianRenderer::new());
///     c.register_renderer("pie", PieRenderer::new());
/// });
/// view! { <ChartMLChart spec=yaml chartml=chartml /> }
/// ```
pub fn use_chartml_configured(configure: impl FnOnce(&mut ChartML)) -> ChartMLRef {
    let mut chartml = ChartML::new();
    configure(&mut chartml);
    ChartMLRef::new(chartml)
}
