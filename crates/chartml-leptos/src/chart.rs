use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use chartml_core::ChartML;
use chartml_core::element::ElementData;
use crate::element::render_element;
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};

/// Custom tooltip renderer type.
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

/// Main ChartML component for Leptos.
///
/// Measures its container's width and renders the chart to fit.
/// Re-renders automatically when the container resizes (via ResizeObserver)
/// or when the spec changes.
///
/// # Tooltip Customization
///
/// 1. **CSS** — override `.chartml-tooltip` styles
/// 2. **Custom renderer** — `tooltip` prop: `Fn(&ElementData) -> AnyView`
/// 3. **Disable** — pass an empty renderer
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
    /// Optional custom tooltip renderer
    #[prop(optional)]
    tooltip: Option<TooltipRenderer>,
) -> impl IntoView {
    let chartml = chartml.clone();
    let tooltip_state = provide_tooltip_context();

    // Track container width — updated by ResizeObserver
    let (container_width, set_container_width) = signal(0.0_f64);

    // NodeRef to the container div for measuring
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Set up ResizeObserver after mount
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            // Initial measurement
            let width = el.client_width() as f64;
            if width > 0.0 {
                set_container_width.set(width);
            }

            // Observe resize
            let cb = Closure::<dyn Fn(js_sys::Array)>::new(move |entries: js_sys::Array| {
                if let Some(entry) = entries.get(0).dyn_ref::<web_sys::ResizeObserverEntry>() {
                    let rect = entry.content_rect();
                    let w = rect.width();
                    if w > 0.0 {
                        set_container_width.set(w);
                    }
                }
            });

            if let Ok(observer) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
                observer.observe(&el);
                // Leak the closure so it lives as long as the observer
                // (cleaned up when the component unmounts and the element is removed)
                cb.forget();
            }
        }
    });

    let container_class = if class.is_empty() {
        "chartml-container".to_string()
    } else {
        format!("chartml-container {}", class)
    };

    view! {
        <div class=container_class style="position: relative;" node_ref=container_ref>
            // Chart content — re-renders when spec OR container_width changes
            {move || {
                let width = container_width.get();
                let yaml = spec.get();

                if yaml.trim().is_empty() {
                    return view! {
                        <div class="chartml-error">
                            <p style="color: #888; padding: 12px;">"Enter a ChartML YAML spec"</p>
                        </div>
                    }.into_any();
                }

                // Don't render until we have a measured width
                if width <= 0.0 {
                    return view! { <div /> }.into_any();
                }

                match chartml.render_from_yaml_with_size(&yaml, Some(width), None) {
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

            // Tooltip overlay
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
