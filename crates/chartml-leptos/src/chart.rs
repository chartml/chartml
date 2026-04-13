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
use chartml_core::theme::Theme;
use crate::element::render_element;
use crate::tooltip::{provide_tooltip_context, DefaultTooltip};

/// Custom tooltip renderer type.
pub type TooltipRenderer = Arc<dyn Fn(&ElementData) -> AnyView + Send + Sync>;

/// Inject chart CSS into the document head (idempotent — checks for existing style tag).
/// CSS is embedded at compile time from style/chartml.css.
fn inject_chartml_css() {
    #[cfg(target_arch = "wasm32")]
    {
        const CSS: &str = include_str!("../style/chartml.css");
        const CSS_ID: &str = "chartml-injected-styles";
        let document = web_sys::window().unwrap().document().unwrap();
        if document.get_element_by_id(CSS_ID).is_some() {
            return; // Already injected
        }
        let style = document.create_element("style").unwrap();
        style.set_attribute("id", CSS_ID).unwrap();
        style.set_text_content(Some(CSS));
        document.head().unwrap().append_child(&style).unwrap();
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

/// Build the inline `style` attribute for the HTML `<div class="chart-title">`
/// from a `Theme`. Threads title typography (family, size, weight, style) out
/// of the theme so users can override the chart title font without touching
/// the Leptos component.
///
/// Sentinel discipline: when the theme's title typography fields equal
/// `Theme::default()`, we emit the *legacy* hardcoded values (16px / 600 /
/// no font-family / no font-style) so the default-rendered DOM is byte-
/// identical to pre-Phase-4 output. Phase 2's `Theme::default()` values for
/// title typography (14px / 700) do not match the pre-Phase-4 legacy, so we
/// must bridge via the sentinel rather than pass the defaults through.
/// Anything else is emitted verbatim.
///
/// The `color` property is intentionally left as the CSS-var fallback
/// `var(--chartml-text-strong, #1f2937)` so browser-side dark mode still
/// works via CSS custom properties.
pub(crate) fn build_title_style(theme: &Theme) -> String {
    let default = Theme::default();

    // Legacy sentinel values (match pre-Phase-4 hardcoded HTML).
    const LEGACY_TITLE_FONT_SIZE: &str = "16px";
    const LEGACY_TITLE_FONT_WEIGHT: &str = "600";

    let font_size = if theme.title_font_size == default.title_font_size {
        LEGACY_TITLE_FONT_SIZE.to_string()
    } else if theme.title_font_size.fract() == 0.0 {
        format!("{}px", theme.title_font_size as i64)
    } else {
        format!("{}px", theme.title_font_size)
    };

    let font_weight = if theme.title_font_weight == default.title_font_weight {
        LEGACY_TITLE_FONT_WEIGHT.to_string()
    } else {
        theme.title_font_weight.to_string()
    };

    let mut style = format!(
        "font-size: {}; font-weight: {}; color: var(--chartml-text-strong, #1f2937); margin-bottom: 8px;",
        font_size, font_weight
    );

    // Legacy HTML had no `font-family` — only emit when the theme overrides it.
    if theme.title_font_family != default.title_font_family {
        style.push_str(&format!(" font-family: {};", theme.title_font_family));
    }

    // Same story for `font-style`.
    if theme.title_font_style != default.title_font_style {
        style.push_str(&format!(" font-style: {};", theme.title_font_style));
    }

    style
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

    // Inject chart CSS into document head on first mount (idempotent)
    inject_chartml_css();

    // Track container width — updated by ResizeObserver
    let (container_width, set_container_width) = signal(0.0_f64);
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Set up ResizeObserver after mount; disconnect on disposal.
    // Debounce: only update container_width after resize activity stops for 200ms.
    // Matches the markdown-react plugin pattern (250ms debounce + initial grace).
    type RoEntry = SendWrapper<Rc<RefCell<Option<(web_sys::ResizeObserver, Closure<dyn Fn(js_sys::Array)>)>>>>;
    let resize_observer: RoEntry = SendWrapper::new(Rc::new(RefCell::new(None)));
    let debounce_handle: SendWrapper<Rc<RefCell<Option<i32>>>> = SendWrapper::new(Rc::new(RefCell::new(None)));

    let ro_clone = resize_observer.clone();
    let debounce_clone = debounce_handle.clone();
    Effect::new(move || {
        if let Some(el) = container_ref.get() {
            // Set initial width immediately (no debounce for first measurement)
            let width = el.client_width() as f64;
            if width > 0.0 {
                set_container_width.set(width);
            }

            let dh = debounce_clone.clone();
            let cb = Closure::<dyn Fn(js_sys::Array)>::new(move |entries: js_sys::Array| {
                if let Some(entry) = entries.get(0).dyn_ref::<web_sys::ResizeObserverEntry>() {
                    let rect = entry.content_rect();
                    let w = rect.width();
                    if w > 0.0 {
                        // Cancel any pending debounce
                        if let Some(tid) = dh.borrow_mut().take() {
                            web_sys::window().unwrap().clear_timeout_with_handle(tid);
                        }
                        // Set new debounced timeout
                        let cb_js = Closure::once_into_js(move || {
                            set_container_width.set(w);
                        });
                        let tid = web_sys::window().unwrap()
                            .set_timeout_with_callback_and_timeout_and_arguments_0(
                                cb_js.unchecked_ref(), 200,
                            ).unwrap_or(0);
                        *dh.borrow_mut() = Some(tid);
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
        if let Some(tid) = debounce_handle.borrow_mut().take() {
            web_sys::window().unwrap().clear_timeout_with_handle(tid);
        }
    });

    let container_class = if class.is_empty() {
        "chartml-container".to_string()
    } else {
        format!("chartml-container {}", class)
    };

    // Unified chart state: title, element tree, error, loading.
    // All view closures read from this single signal — no split-brain from
    // multiple independent `spec.get()` subscriptions.
    #[derive(Clone)]
    struct ChartState {
        title: Option<String>,
        element: Option<ChartElement>,
        error: Option<String>,
        loading: bool,
    }
    let (chart_state, set_chart_state) = signal(ChartState {
        title: None, element: None, error: None, loading: false,
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
                title: None, element: None,
                error: Some("Enter a ChartML YAML spec".to_string()), loading: false,
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
                title: title.clone(), element: None, error: None, loading: true,
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
                        set_chart_state.set(ChartState {
                            title, element: Some(el), error: None, loading: false,
                        });
                    }
                    Err(e) => {
                        set_chart_state.set(ChartState {
                            title, element: None, error: Some(format!("{}", e)), loading: false,
                        });
                    }
                }
            });
        } else {
            // Sync path: render immediately
            match chartml_for_effect.render_from_yaml_with_params(&yaml, Some(width), None, params.as_ref()) {
                Ok(element) => {
                    set_chart_state.set(ChartState {
                        title, element: Some(element), error: None, loading: false,
                    });
                }
                Err(err) => {
                    set_chart_state.set(ChartState {
                        title, element: None, error: Some(format!("{}", err)), loading: false,
                    });
                }
            }
        }
    });

    // Build the title style string once from the theme. The theme is set on
    // the ChartML instance before it's handed to this component and does not
    // change reactively, so a one-shot computation is correct here.
    let title_style = build_title_style(chartml.theme());

    view! {
        <div class=container_class style="position: relative;" node_ref=container_ref>
            // Chart title
            {
                let title_style = title_style.clone();
                move || {
                    let title_style = title_style.clone();
                    chart_state.get().title.map(|t| view! {
                        <div class="chart-title" style=title_style>
                            {t}
                        </div>
                    })
                }
            }

            // Chart content — render_element() produces Leptos views with
            // interactive tooltip wrappers, preserving ElementData mouse handlers.
            {move || {
                chart_state.get().element.map(|el| render_element(&el))
            }}

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

#[cfg(test)]
mod title_style_tests {
    use super::*;
    use chartml_core::theme::Theme;

    #[test]
    fn default_theme_emits_legacy_hardcoded_values() {
        // Default theme must produce the exact pre-Phase-4 style string so
        // existing browser DOM is unchanged.
        let style = build_title_style(&Theme::default());
        assert!(style.contains("font-size: 16px"), "style: {}", style);
        assert!(style.contains("font-weight: 600"), "style: {}", style);
        assert!(style.contains("color: var(--chartml-text-strong, #1f2937)"));
        assert!(style.contains("margin-bottom: 8px"));
        // Legacy had no font-family / font-style on the title.
        assert!(!style.contains("font-family:"), "style: {}", style);
        assert!(!style.contains("font-style:"), "style: {}", style);
    }

    #[test]
    fn custom_title_typography_is_threaded_through() {
        let theme = Theme {
            title_font_family: "Georgia, serif".into(),
            title_font_size: 22.0,
            title_font_weight: 800,
            title_font_style: "italic".into(),
            ..Theme::default()
        };
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 22px"), "style: {}", style);
        assert!(style.contains("font-weight: 800"), "style: {}", style);
        assert!(style.contains("font-family: Georgia, serif"), "style: {}", style);
        assert!(style.contains("font-style: italic"), "style: {}", style);
    }

    #[test]
    fn fractional_font_size_preserves_decimal() {
        let theme = Theme { title_font_size: 18.5, ..Theme::default() };
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 18.5px"), "style: {}", style);
    }

    /// Phase 10 — Kyomi sanity check #1 (serif chart title). The full Kyomi
    /// theme targets an `Instrument Serif, Georgia, serif` title face. The
    /// integration-level `phase10_kyomi_sanity.rs` test cannot reach this
    /// helper (it's `pub(crate)`), so this unit test pins the serif family
    /// in the emitted title style string.
    #[test]
    fn phase10_kyomi_title_uses_serif_family() {
        let theme = Theme {
            title_font_family: "Instrument Serif, Georgia, serif".into(),
            title_font_size: 22.0,
            title_font_weight: 400,
            title_font_style: "normal".into(),
            ..Theme::default()
        };
        let style = build_title_style(&theme);
        assert!(
            style.contains("font-family: Instrument Serif, Georgia, serif"),
            "kyomi title must use Instrument Serif family: {}",
            style,
        );
        assert!(style.contains("font-size: 22px"), "style: {}", style);
        assert!(style.contains("font-weight: 400"), "style: {}", style);
    }

    #[test]
    fn only_size_override_keeps_legacy_weight_and_omits_family() {
        let theme = Theme { title_font_size: 20.0, ..Theme::default() };
        let style = build_title_style(&theme);
        assert!(style.contains("font-size: 20px"));
        // Weight still matches default sentinel -> legacy 600.
        assert!(style.contains("font-weight: 600"));
        assert!(!style.contains("font-family:"));
        assert!(!style.contains("font-style:"));
    }
}
