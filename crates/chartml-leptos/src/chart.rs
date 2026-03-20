use std::sync::Arc;

use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_core::element::ElementData;
use chartml_core::error::ChartError;
use crate::element::render_element;
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};

/// Custom tooltip renderer type.
/// Receives the ElementData for the hovered element and returns a view.
///
/// Example:
/// ```rust
/// let my_tooltip: TooltipRenderer = Arc::new(|data: &ElementData| {
///     view! {
///         <div class="my-custom-tooltip">
///             <strong>{data.label.clone()}</strong>
///             <span>{data.value.clone()}</span>
///         </div>
///     }.into_any()
/// });
/// ```
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

/// Main ChartML component for Leptos.
///
/// Parses a YAML spec and renders the chart reactively. Provides a shared
/// tooltip context — interactive chart elements set tooltip data on hover,
/// and this component renders the tooltip as an HTML overlay.
///
/// # Tooltip Customization
///
/// Three levels of customization:
///
/// 1. **CSS only** — override `.chartml-tooltip` styles (background, font, padding, etc.)
/// 2. **Custom renderer** — provide a `tooltip` prop with your own `Fn(&ElementData) -> AnyView`
/// 3. **Disable** — set `tooltip=Arc::new(|_| view! {}.into_any())` to suppress tooltips
///
/// # Usage
/// ```rust
/// let chartml = Arc::new(ChartML::new());
///
/// // Default tooltip (CSS-customizable):
/// view! { <ChartMLChart spec=yaml chartml=chartml /> }
///
/// // Custom tooltip:
/// let custom: TooltipRenderer = Arc::new(|data| {
///     view! { <div class="my-tip">{data.label.clone()}</div> }.into_any()
/// });
/// view! { <ChartMLChart spec=yaml chartml=chartml tooltip=custom /> }
/// ```
#[component]
pub fn ChartMLChart(
    /// ChartML YAML specification string
    #[prop(into)]
    spec: Signal<String>,
    /// Pre-configured ChartML instance
    chartml: Arc<ChartML>,
    /// Optional CSS class for the container
    #[prop(optional)]
    class: &'static str,
    /// Optional custom tooltip renderer. If not provided, uses the default.
    #[prop(optional)]
    tooltip: Option<TooltipRenderer>,
) -> impl IntoView {
    let chartml = chartml.clone();

    // Provide shared tooltip context for all interactive elements in this chart
    let tooltip_state = provide_tooltip_context();

    let chart_result = move || {
        let yaml = spec.get();
        if yaml.trim().is_empty() {
            return Err(ChartError::InvalidSpec("Empty spec".into()));
        }
        chartml.render_from_yaml(&yaml)
    };

    let container_class = if class.is_empty() {
        "chartml-container".to_string()
    } else {
        format!("chartml-container {}", class)
    };

    view! {
        <div class=container_class style="position: relative;">
            // Chart content
            {move || {
                match chart_result() {
                    Ok(element) => render_element(&element).into_any(),
                    Err(err) => {
                        view! {
                            <div class="chartml-error">
                                <p style="color: #dc3545; font-family: monospace; padding: 12px; background: #fff5f5; border: 1px solid #dc3545; border-radius: 4px;">
                                    {format!("Chart error: {}", err)}
                                </p>
                            </div>
                        }.into_any()
                    }
                }
            }}

            // Tooltip overlay — single HTML div, positioned fixed near cursor
            {
                let tooltip = tooltip.clone();
                move || {
                    let state = tooltip_state.get();
                    if !state.visible() {
                        return view! { <div style="display:none;" /> }.into_any();
                    }

                    let style = format!(
                        "position: fixed; left: {}px; top: {}px; pointer-events: none; z-index: 1000;",
                        state.x + 12.0,
                        state.y - 12.0,
                    );

                    let content = if let Some(ref renderer) = tooltip {
                        renderer(state.data.as_ref().unwrap())
                    } else {
                        view! { <DefaultTooltip state=state.clone() /> }.into_any()
                    };

                    view! {
                        <div class="chartml-tooltip" style=style>
                            {content}
                        </div>
                    }.into_any()
                }
            }
        </div>
    }
}
