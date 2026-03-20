use std::sync::Arc;

use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_core::error::ChartError;
use crate::element::render_element;

/// Main ChartML component for Leptos.
/// Parses a YAML spec and renders the chart reactively.
///
/// Usage:
/// ```
/// let chartml = Arc::new(ChartML::new());
/// view! { <ChartMLChart spec=yaml_string chartml=chartml /> }
/// ```
#[component]
pub fn ChartMLChart(
    /// ChartML YAML specification string
    #[prop(into)]
    spec: Signal<String>,
    /// Pre-configured ChartML instance wrapped in Arc (since ChartML contains trait objects)
    chartml: Arc<ChartML>,
    /// Optional CSS class for the container div
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    let chartml = chartml.clone();

    // Use a derived signal (closure) instead of Memo to avoid PartialEq requirement
    // on ChartElement and ChartError
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
        <div class=container_class>
            {move || {
                match chart_result() {
                    Ok(element) => {
                        render_element(&element).into_any()
                    }
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
        </div>
    }
}
