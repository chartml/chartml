use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use chartml_core::ChartML;
use chartml_core::element::ElementData;
use chartml_core::params::ParamValues;
use crate::element::render_element;
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};

/// Custom tooltip renderer type.
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

/// Extract the top-level `title:` field from a ChartML YAML string.
/// Returns None if no title is present or it is empty.
fn extract_yaml_title(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("title:") {
            let t = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
        // Stop scanning once we hit the visualize section
        if trimmed.starts_with("visualize:") || trimmed.starts_with("data:") {
            break;
        }
    }
    None
}

/// Main ChartML component for Leptos.
///
/// Renders a ChartML YAML spec reactively. Responds to:
/// - `spec` signal changes (YAML editing)
/// - Container resize (via ResizeObserver)
/// - `param_values` signal changes (interactive param controls)
///
/// # Param Integration
///
/// Pass a shared `RwSignal<ParamValues>` via the `param_values` prop.
/// When param UI controls update this signal, all charts re-render
/// with the new values resolved.
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
    /// Shared reactive param values — when updated by controls, charts re-render
    #[prop(optional)]
    param_values: Option<RwSignal<ParamValues>>,
) -> impl IntoView {
    let chartml = chartml.clone();
    let tooltip_state = provide_tooltip_context();

    // Track container width — updated by ResizeObserver
    let (container_width, set_container_width) = signal(0.0_f64);
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Set up ResizeObserver after mount
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            let width = el.client_width() as f64;
            if width > 0.0 {
                set_container_width.set(width);
            }

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
            // Chart content — re-renders when spec, width, OR param_values change
            {move || {
                let width = container_width.get();
                let yaml = spec.get();

                // Read param_values signal (if provided) to establish reactive dependency
                let params = param_values.map(|pv| pv.get());

                if yaml.trim().is_empty() {
                    return view! {
                        <div class="chartml-error">
                            <p style="color: #888; padding: 12px;">"Enter a ChartML YAML spec"</p>
                        </div>
                    }.into_any();
                }

                if width <= 0.0 {
                    return view! { <div /> }.into_any();
                }

                // Extract title from YAML before rendering (title is an HTML div, not SVG element)
                let chart_title = extract_yaml_title(&yaml);

                let result = chartml.render_from_yaml_with_params(
                    &yaml,
                    Some(width),
                    None,
                    params.as_ref(),
                );

                match result {
                    Ok(element) => view! {
                        <div>
                            {chart_title.map(|t| view! {
                                <div class="chart-title" style="font-size: 16px; font-weight: 600; color: #1a1a1a; margin-bottom: 8px;">
                                    {t}
                                </div>
                            })}
                            {render_element(&element)}
                        </div>
                    }.into_any(),
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
