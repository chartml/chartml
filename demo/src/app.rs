use leptos::prelude::*;
use send_wrapper::SendWrapper;
use chartml_core::ChartML;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_datafusion::DataFusionTransform;
use chartml_leptos::{ChartMLChart, ChartMLRef};

use crate::editor::YamlEditor;
use crate::gallery::{Gallery, DEFAULT_SPEC};
use crate::examples::{ExamplesPage, register_page_sources};
// `provider_example` is wasm-only — see its module declaration in main.rs
// for why. The "Providers" tab below silently no-ops on native builds.
#[cfg(target_arch = "wasm32")]
use crate::provider_example::ProviderExamplesPage;

fn setup_chartml() -> ChartMLRef {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_renderer("pie", PieRenderer::new());
    c.register_renderer("doughnut", PieRenderer::new());
    c.register_renderer("scatter", ScatterRenderer::new());
    c.register_renderer("metric", MetricRenderer::new());
    // Register DataFusion transform middleware
    c.register_transform(DataFusionTransform);
    // Register named sources from the examples page
    register_page_sources(&mut c);
    ChartMLRef::new(c)
}

#[component]
pub fn App() -> impl IntoView {
    // `ChartMLRef` is `Rc<ChartML>` on WASM (where the chartml resolver is
    // `?Send` because its inflight `Shared<LocalBoxFuture<...>>` map is
    // single-threaded). Leptos's reactive function bound is `Send + 'static`,
    // so we wrap the handle in `SendWrapper` before pushing it through the
    // `view!` closure. wasm32-unknown-unknown is single-threaded, so the
    // wrapper's "drop on the wrong thread = panic" rule cannot trigger.
    let chartml = SendWrapper::new(setup_chartml());
    let (tab, set_tab) = signal("examples".to_string());
    let (spec, set_spec) = signal(DEFAULT_SPEC.to_string());

    view! {
        <div class="app">
            <header class="app-header">
                <h1 class="app-title">"chartml-rs"</h1>
                <span class="app-subtitle">"ChartML rendered natively in Rust/WASM"</span>
                <nav class="app-nav">
                    <button
                        class=move || if tab.get() == "examples" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set("examples".to_string())
                    >"Examples"</button>
                    <button
                        class=move || if tab.get() == "editor" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set("editor".to_string())
                    >"Editor"</button>
                    <button
                        class=move || if tab.get() == "providers" { "nav-btn active" } else { "nav-btn" }
                        on:click=move |_| set_tab.set("providers".to_string())
                    >"Providers"</button>
                </nav>
            </header>

            {move || {
                let chartml: ChartMLRef = (*chartml).clone();
                match tab.get().as_str() {
                    "examples" => view! {
                        <ExamplesPage chartml=chartml />
                    }.into_any(),
                    "providers" => provider_examples_view(),
                    _ => {
                        let chartml_editor = chartml.clone();
                        view! {
                            <main class="app-main">
                                <div class="editor-panel">
                                    <div class="panel-header">"YAML Editor"</div>
                                    <YamlEditor
                                        value=spec
                                        on_change=move |new_val: String| set_spec.set(new_val)
                                    />
                                </div>
                                <div class="preview-panel">
                                    <div class="panel-header">"Live Preview"</div>
                                    <div class="preview-content">
                                        <ChartMLChart spec=spec chartml=chartml_editor />
                                    </div>
                                </div>
                            </main>
                            <section class="gallery-section">
                                <div class="panel-header">"Gallery \u{2014} Click to load"</div>
                                <Gallery on_select=move |yaml: String| set_spec.set(yaml) />
                            </section>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

/// Build the "Providers" tab content. Wasm builds mount the real
/// `ProviderExamplesPage`; native builds (workspace `cargo check`) emit a
/// stub explaining the page is browser-only. The match arm in `App()` calls
/// this so neither branch references a wasm-only symbol unconditionally.
#[cfg(target_arch = "wasm32")]
fn provider_examples_view() -> AnyView {
    view! { <ProviderExamplesPage /> }.into_any()
}

#[cfg(not(target_arch = "wasm32"))]
fn provider_examples_view() -> AnyView {
    view! {
        <div style="padding: 24px; color: #666; font-family: monospace;">
            "Provider examples are browser-only \u{2014} build this demo for \
             wasm32-unknown-unknown via `trunk serve` to see them."
        </div>
    }
    .into_any()
}
