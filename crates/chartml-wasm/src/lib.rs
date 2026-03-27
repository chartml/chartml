use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_core::ChartML;
use chartml_render::element_to_svg;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmChartML {
    inner: ChartML,
}

impl Default for WasmChartML {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmChartML {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmChartML {
        let mut chartml = ChartML::new();
        // Register all built-in renderers
        chartml.register_renderer("bar", CartesianRenderer::new());
        chartml.register_renderer("line", CartesianRenderer::new());
        chartml.register_renderer("area", CartesianRenderer::new());
        chartml.register_renderer("pie", PieRenderer::new());
        chartml.register_renderer("doughnut", PieRenderer::new());
        chartml.register_renderer("scatter", ScatterRenderer::new());
        chartml.register_renderer("bubble", ScatterRenderer::new());
        chartml.register_renderer("metric", MetricRenderer::new());
        WasmChartML { inner: chartml }
    }

    /// Render a ChartML YAML spec to an SVG string.
    #[wasm_bindgen(js_name = "renderToSvg")]
    pub fn render_to_svg(&self, yaml: &str, options: JsValue) -> Result<String, JsValue> {
        let (width, height) = parse_render_options(&options);
        let element = self
            .inner
            .render_from_yaml_with_size(yaml, width, height)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let w = width.unwrap_or(800.0);
        let h = height.unwrap_or(400.0);
        Ok(element_to_svg(&element, w, h))
    }

    /// Render a ChartML YAML spec to a ChartElement JSON object.
    #[wasm_bindgen(js_name = "renderToElement")]
    pub fn render_to_element(&self, yaml: &str, options: JsValue) -> Result<JsValue, JsValue> {
        let (width, height) = parse_render_options(&options);
        let element = self
            .inner
            .render_from_yaml_with_size(yaml, width, height)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&element).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Register a named YAML component (source, style, config, params).
    #[wasm_bindgen(js_name = "registerComponent")]
    pub fn register_component(&mut self, yaml: &str) -> Result<(), JsValue> {
        self.inner
            .register_component(yaml)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

fn parse_render_options(options: &JsValue) -> (Option<f64>, Option<f64>) {
    if options.is_undefined() || options.is_null() {
        return (None, None);
    }
    let width = js_sys::Reflect::get(options, &"width".into())
        .ok()
        .and_then(|v| v.as_f64());
    let height = js_sys::Reflect::get(options, &"height".into())
        .ok()
        .and_then(|v| v.as_f64());
    (width, height)
}
