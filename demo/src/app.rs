use std::sync::Arc;
use leptos::prelude::*;
use chartml_core::ChartML;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_leptos::ChartMLChart;

use crate::editor::YamlEditor;
use crate::gallery::{Gallery, DEFAULT_SPEC};
use crate::examples::{ExamplesPage, register_page_sources};

fn setup_chartml() -> Arc<ChartML> {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_renderer("pie", PieRenderer::new());
    c.register_renderer("doughnut", PieRenderer::new());
    c.register_renderer("scatter", ScatterRenderer::new());
    c.register_renderer("metric", MetricRenderer::new());
    // Register named sources from the examples page
    register_page_sources(&mut c);
    Arc::new(c)
}

#[component]
pub fn App() -> impl IntoView {
    let chartml = setup_chartml();
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
                </nav>
            </header>

            {move || {
                let chartml = chartml.clone();
                if tab.get() == "examples" {
                    view! {
                        <ExamplesPage chartml=chartml />
                    }.into_any()
                } else {
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
            }}
        </div>
    }
}
