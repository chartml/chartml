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

#[component]
pub fn App() -> impl IntoView {
    // Set up ChartML engine with all renderers
    let chartml = {
        let mut c = ChartML::new();
        c.register_renderer("bar", CartesianRenderer::new());
        c.register_renderer("line", CartesianRenderer::new());
        c.register_renderer("area", CartesianRenderer::new());
        c.register_renderer("pie", PieRenderer::new());
        c.register_renderer("doughnut", PieRenderer::new());
        c.register_renderer("scatter", ScatterRenderer::new());
        c.register_renderer("metric", MetricRenderer::new());
        Arc::new(c)
    };

    // YAML spec state -- starts with default bar chart
    let (spec, set_spec) = signal(DEFAULT_SPEC.to_string());

    view! {
        <div class="app">
            <header class="app-header">
                <h1 class="app-title">"chartml-rs"</h1>
                <span class="app-subtitle">"ChartML rendered natively in Rust/WASM"</span>
            </header>

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
                        <ChartMLChart spec=spec chartml=chartml.clone() />
                    </div>
                </div>
            </main>

            <section class="gallery-section">
                <div class="panel-header">"Gallery \u{2014} Click to load"</div>
                <Gallery on_select=move |yaml: String| set_spec.set(yaml) />
            </section>
        </div>
    }
}
