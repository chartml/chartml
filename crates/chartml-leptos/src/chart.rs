use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use chartml_core::ChartML;
use chartml_core::element::ElementData;
use chartml_core::element::ChartElement;
use chartml_core::params::ParamValues;
use chartml_render::element_to_svg;
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};

/// Custom tooltip renderer type.
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

/// Extract the top-level `title:` field from a ChartML YAML string.
/// Returns None if no title is present or it is empty.
/// Extract width/height from a ChartElement for element_to_svg.
fn extract_svg_dimensions(element: &ChartElement) -> (f64, f64) {
    match element {
        ChartElement::Svg { width, height, viewbox, .. } => {
            (width.unwrap_or(viewbox.width), height.unwrap_or(viewbox.height))
        }
        _ => (800.0, 400.0),
    }
}

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

/// Check if a YAML spec contains a transform section.
fn has_transform_spec(yaml: &str) -> bool {
    yaml.contains("\ntransform:") || yaml.starts_with("transform:")
}

/// Main ChartML component for Leptos.
///
/// Renders a ChartML YAML spec reactively. Responds to:
/// - `spec` signal changes (YAML editing)
/// - Container resize (via ResizeObserver)
/// - `param_values` signal changes (interactive param controls)
///
/// Charts with `transform:` specs are rendered asynchronously via the
/// registered TransformMiddleware (DataFusion). Charts without transforms
/// render synchronously.
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

    // Set up ResizeObserver after mount; disconnect on disposal.
    type RoEntry = SendWrapper<Rc<RefCell<Option<(web_sys::ResizeObserver, Closure<dyn Fn(js_sys::Array)>)>>>>;
    let resize_observer: RoEntry = SendWrapper::new(Rc::new(RefCell::new(None)));

    let ro_clone = resize_observer.clone();
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
                *ro_clone.borrow_mut() = Some((observer, cb));
            }
        }
    });

    on_cleanup(move || {
        if let Some((observer, _cb)) = resize_observer.borrow_mut().take() {
            observer.disconnect();
        }
    });

    let container_class = if class.is_empty() {
        "chartml-container".to_string()
    } else {
        format!("chartml-container {}", class)
    };

    // Unified chart state: title, SVG HTML, error, loading.
    // All view closures read from this single signal — no split-brain from
    // multiple independent `spec.get()` subscriptions.
    #[derive(Clone)]
    struct ChartState {
        title: Option<String>,
        svg: String,
        error: Option<String>,
        loading: bool,
    }
    let (chart_state, set_chart_state) = signal(ChartState {
        title: None, svg: String::new(), error: None, loading: false,
    });

    // Generation counter to prevent stale async results from overwriting newer ones
    let render_gen: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0));

    // Unified render effect: reads spec/width/params, produces ChartState.
    // Sync charts are rendered inline; async charts spawn a task and set loading.
    let chartml_for_effect = chartml.clone();
    let render_gen_for_effect = render_gen.clone();
    Effect::new(move || {
        let yaml = spec.get();
        let width = container_width.get();
        let params = param_values.map(|pv| pv.get());

        if yaml.trim().is_empty() {
            set_chart_state.set(ChartState {
                title: None,
                svg: r#"<p style="color: #888; padding: 12px;">Enter a ChartML YAML spec</p>"#.to_string(),
                error: None, loading: false,
            });
            return;
        }
        if width <= 0.0 {
            return;
        }

        let title = extract_yaml_title(&yaml);

        if has_transform_spec(&yaml) {
            // Async path: set loading, bump generation, spawn task
            set_chart_state.set(ChartState {
                title: title.clone(), svg: String::new(), error: None, loading: true,
            });
            let my_gen = render_gen_for_effect.get() + 1;
            render_gen_for_effect.set(my_gen);
            let gen_ref = render_gen_for_effect.clone();

            let chartml_async = chartml_for_effect.clone();
            let yaml_owned = yaml.clone();
            let params_owned = params.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let result = chartml_async.render_from_yaml_with_params_async(
                    &yaml_owned, Some(width), None, params_owned.as_ref(),
                ).await;
                // Only apply if no newer render has been started
                if gen_ref.get() != my_gen { return; }
                match result {
                    Ok(el) => {
                        let (ew, eh) = extract_svg_dimensions(&el);
                        set_chart_state.set(ChartState {
                            title, svg: element_to_svg(&el, ew, eh), error: None, loading: false,
                        });
                    }
                    Err(e) => {
                        set_chart_state.set(ChartState {
                            title, svg: String::new(), error: Some(format!("{}", e)), loading: false,
                        });
                    }
                }
            });
        } else {
            // Sync path: render immediately
            match chartml_for_effect.render_from_yaml_with_params(&yaml, Some(width), None, params.as_ref()) {
                Ok(element) => {
                    let (ew, eh) = extract_svg_dimensions(&element);
                    set_chart_state.set(ChartState {
                        title, svg: element_to_svg(&element, ew, eh), error: None, loading: false,
                    });
                }
                Err(err) => {
                    set_chart_state.set(ChartState {
                        title, svg: String::new(), error: Some(format!("{}", err)), loading: false,
                    });
                }
            }
        }
    });

    view! {
        <div class=container_class style="position: relative;" node_ref=container_ref>
            // Chart title
            {move || {
                chart_state.get().title.map(|t| view! {
                    <div class="chart-title" style="font-size: 16px; font-weight: 600; color: #1a1a1a; margin-bottom: 8px;">
                        {t}
                    </div>
                })
            }}

            // Chart SVG — reactive inner_html so Leptos updates the DOM on signal change.
            // Safety: SVG is renderer-generated, not user input.
            <div inner_html=move || chart_state.get().svg />

            // Error display
            {move || {
                chart_state.get().error.map(|msg| view! {
                    <div class="chartml-error">
                        <p style="color: #dc3545; font-family: monospace; padding: 12px; background: #fff5f5; border: 1px solid #dc3545; border-radius: 4px;">
                            {msg}
                        </p>
                    </div>
                })
            }}

            // Loading state
            {move || {
                chart_state.get().loading.then(|| view! {
                    <div style="padding: 12px; color: #888;">"Loading..."</div>
                })
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
