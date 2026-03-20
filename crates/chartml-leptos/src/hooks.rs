use std::sync::Arc;
use chartml_core::ChartML;

/// Create a bare ChartML instance with no renderers registered.
/// Returns an Arc for sharing with components.
///
/// Users should register their own renderers after calling this,
/// or use `use_chartml_configured` to set up in one step.
///
/// Usage:
/// ```
/// let chartml = use_chartml();
/// view! { <ChartMLChart spec=yaml chartml=chartml /> }
/// ```
pub fn use_chartml() -> Arc<ChartML> {
    Arc::new(ChartML::new())
}

/// Create a ChartML instance and configure it via the provided closure.
/// The closure receives a mutable reference to register renderers, data sources, etc.
///
/// Usage:
/// ```
/// let chartml = use_chartml_configured(|c| {
///     c.register_renderer("bar", CartesianRenderer::new());
///     c.register_renderer("line", CartesianRenderer::new());
///     c.register_renderer("pie", PieRenderer::new());
/// });
/// view! { <ChartMLChart spec=yaml chartml=chartml /> }
/// ```
pub fn use_chartml_configured(configure: impl FnOnce(&mut ChartML)) -> Arc<ChartML> {
    let mut chartml = ChartML::new();
    configure(&mut chartml);
    Arc::new(chartml)
}
