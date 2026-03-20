pub mod error;
pub mod spec;
pub mod scales;
pub mod shapes;
pub mod layout;
pub mod format;
pub mod color;
pub mod plugin;
pub mod registry;
pub mod element;
pub mod data;

pub use error::ChartError;
pub use spec::{parse, ChartMLSpec, Component};
pub use element::ChartElement;
pub use plugin::{ChartConfig, ChartRenderer, DataSource, TransformMiddleware, DatasourceResolver};
pub use registry::ChartMLRegistry;

use crate::data::Row;
use crate::spec::{ChartSpec, DataRef};

/// Main ChartML instance. Orchestrates parsing, data fetching, and rendering.
pub struct ChartML {
    registry: ChartMLRegistry,
}

impl ChartML {
    /// Create a new empty ChartML instance.
    pub fn new() -> Self {
        Self {
            registry: ChartMLRegistry::new(),
        }
    }

    /// Create with default built-in plugins.
    /// (No built-in renderers — those come from chartml-chart-* crates)
    pub fn with_defaults() -> Self {
        Self::new()
    }

    // --- Registration methods (delegate to registry) ---

    pub fn register_renderer(&mut self, chart_type: &str, renderer: impl ChartRenderer + 'static) {
        self.registry.register_renderer(chart_type, renderer);
    }

    pub fn register_data_source(&mut self, name: &str, source: impl DataSource + 'static) {
        self.registry.register_data_source(name, source);
    }

    pub fn register_transform(&mut self, middleware: impl TransformMiddleware + 'static) {
        self.registry.register_transform(middleware);
    }

    pub fn set_datasource_resolver(&mut self, resolver: impl DatasourceResolver + 'static) {
        self.registry.set_datasource_resolver(resolver);
    }

    // --- Rendering ---

    /// Parse a YAML string and render the first chart component.
    /// Returns the ChartElement tree for the first chart found.
    pub fn render_from_yaml(&self, yaml: &str) -> Result<ChartElement, ChartError> {
        let parsed = spec::parse(yaml)?;

        // Find the first chart component
        let chart_spec = match &parsed {
            ChartMLSpec::Single(Component::Chart(chart)) => chart,
            ChartMLSpec::Array(components) => {
                components.iter()
                    .find_map(|c| match c {
                        Component::Chart(chart) => Some(chart),
                        _ => None,
                    })
                    .ok_or_else(|| ChartError::InvalidSpec("No chart component found".into()))?
            }
            _ => return Err(ChartError::InvalidSpec("Expected a chart component".into())),
        };

        self.render_chart(chart_spec)
    }

    /// Render a parsed ChartSpec into a ChartElement tree.
    pub fn render_chart(&self, chart_spec: &ChartSpec) -> Result<ChartElement, ChartError> {
        let chart_type = &chart_spec.visualize.chart_type;

        // Look up renderer
        let renderer = self.registry.get_renderer(chart_type)
            .ok_or_else(|| ChartError::UnknownChartType(chart_type.clone()))?;

        // Extract inline data (for v0.1, only inline data is supported)
        let data = self.extract_inline_data(chart_spec)?;

        // Build chart config
        let height = chart_spec.visualize.style
            .as_ref()
            .and_then(|s| s.height)
            .or_else(|| renderer.default_dimensions(&chart_spec.visualize).map(|d| d.height))
            .unwrap_or(400.0);

        let width = chart_spec.visualize.style
            .as_ref()
            .and_then(|s| s.width)
            .unwrap_or(800.0);

        let colors = chart_spec.visualize.style
            .as_ref()
            .and_then(|s| s.colors.clone())
            .unwrap_or_else(|| {
                color::get_chart_colors(12, color::palettes::get_palette("autumn_forest"))
            });

        let config = ChartConfig {
            visualize: chart_spec.visualize.clone(),
            title: chart_spec.title.clone(),
            width,
            height,
            colors,
        };

        renderer.render(&data, &config)
    }

    /// Extract inline data from a chart spec.
    fn extract_inline_data(&self, chart_spec: &ChartSpec) -> Result<Vec<Row>, ChartError> {
        match &chart_spec.data {
            DataRef::Inline(inline) => {
                let rows = inline.rows.as_ref()
                    .ok_or_else(|| ChartError::DataError("Inline data source has no rows".into()))?;

                // Convert serde_json::Value objects to Row (HashMap<String, Value>)
                let mut result = Vec::with_capacity(rows.len());
                for value in rows {
                    match value {
                        serde_json::Value::Object(map) => {
                            let row: Row = map.iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            result.push(row);
                        }
                        _ => return Err(ChartError::DataError(
                            "Data rows must be objects".into()
                        )),
                    }
                }
                Ok(result)
            }
            DataRef::Named(name) => {
                // For v0.1, named sources aren't resolved
                Err(ChartError::DataError(
                    format!("Named data source '{}' not yet supported in v0.1 (use inline data)", name)
                ))
            }
        }
    }

    /// Get a reference to the internal registry.
    pub fn registry(&self) -> &ChartMLRegistry {
        &self.registry
    }

    /// Get a mutable reference to the internal registry.
    pub fn registry_mut(&mut self) -> &mut ChartMLRegistry {
        &mut self.registry
    }
}

impl Default for ChartML {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ViewBox;

    struct MockRenderer;

    impl ChartRenderer for MockRenderer {
        fn render(&self, _data: &[Row], _config: &ChartConfig) -> Result<ChartElement, ChartError> {
            Ok(ChartElement::Svg {
                viewbox: ViewBox::new(0.0, 0.0, 800.0, 400.0),
                width: Some(800.0),
                height: Some(400.0),
                class: "mock".to_string(),
                children: vec![],
            })
        }
    }

    #[test]
    fn chartml_render_from_yaml_with_mock() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);

        let yaml = r#"
type: chart
version: 1
title: Test
data:
  provider: inline
  rows:
    - { x: "A", y: 10 }
    - { x: "B", y: 20 }
visualize:
  type: bar
  columns: x
  rows: y
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_ok(), "render failed: {:?}", result.err());
    }

    #[test]
    fn chartml_unknown_chart_type() {
        let chartml = ChartML::new();
        let yaml = r#"
type: chart
version: 1
data:
  provider: inline
  rows: []
visualize:
  type: unknown_type
  columns: x
  rows: y
"#;
        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_err());
    }
}
