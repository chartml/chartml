use chartml_core::data::DataTable;
use chartml_core::element::{ChartElement, Dimensions};
use chartml_core::error::ChartError;
use chartml_core::plugin::renderer::{ChartConfig, ChartRenderer};
use chartml_core::spec::VisualizeSpec;

use super::SendFunction;

/// A `ChartRenderer` adapter that delegates to a JavaScript function.
///
/// The JS function receives `(rows: object[], config: object)` and must
/// return a `ChartElement`-shaped JSON object.
pub struct JsChartRenderer {
    render_fn: SendFunction,
}

impl JsChartRenderer {
    pub fn new(render_fn: js_sys::Function) -> Self {
        Self {
            render_fn: SendFunction(render_fn),
        }
    }
}

impl ChartRenderer for JsChartRenderer {
    fn render(&self, data: &DataTable, config: &ChartConfig) -> Result<ChartElement, ChartError> {
        // Convert data to JSON rows
        let rows = data.to_rows();
        let rows_js = serde_wasm_bindgen::to_value(&rows)
            .map_err(|e| ChartError::RenderError(e.to_string()))?;

        // Convert config to JSON
        let config_js = serde_wasm_bindgen::to_value(config)
            .map_err(|e| ChartError::RenderError(e.to_string()))?;

        // Call the JS function
        let result = self
            .render_fn
            .0
            .call2(&wasm_bindgen::JsValue::NULL, &rows_js, &config_js)
            .map_err(|e| ChartError::RenderError(format!("JS renderer error: {e:?}")))?;

        // Deserialize the returned ChartElement
        let element: ChartElement = serde_wasm_bindgen::from_value(result)
            .map_err(|e| ChartError::RenderError(format!("Invalid ChartElement from JS: {e}")))?;

        Ok(element)
    }

    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        None
    }
}
