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
pub mod transform;

pub use error::ChartError;
pub use spec::{parse, ChartMLSpec, Component};
pub use element::ChartElement;
pub use plugin::{ChartConfig, ChartRenderer, DataSource, TransformMiddleware, DatasourceResolver};
pub use registry::ChartMLRegistry;

use std::collections::HashMap;
use crate::data::Row;
use crate::spec::{ChartSpec, DataRef};

/// Main ChartML instance. Orchestrates parsing, data fetching, and rendering.
/// Maintains a source registry that persists across render calls,
/// matching the JS ChartML class behavior.
pub struct ChartML {
    registry: ChartMLRegistry,
    /// Named source data, registered via register_component() or
    /// automatically collected from multi-document YAML specs.
    sources: HashMap<String, Vec<Row>>,
}

impl ChartML {
    /// Create a new empty ChartML instance.
    pub fn new() -> Self {
        Self {
            registry: ChartMLRegistry::new(),
            sources: HashMap::new(),
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

    // --- Component registration (matches JS chartml.registerComponent()) ---

    /// Register a non-chart component (source, style, config, params) from a YAML string.
    /// Sources are stored in the instance and available to all subsequent render calls.
    /// This matches the JS `chartml.registerComponent(spec)` API.
    pub fn register_component(&mut self, yaml: &str) -> Result<(), ChartError> {
        let parsed = spec::parse(yaml)?;
        match parsed {
            ChartMLSpec::Single(component) => self.register_single_component(component),
            ChartMLSpec::Array(components) => {
                for component in components {
                    self.register_single_component(component)?;
                }
                Ok(())
            }
        }
    }

    fn register_single_component(&mut self, component: spec::Component) -> Result<(), ChartError> {
        match component {
            spec::Component::Source(source_spec) => {
                if let Some(ref rows) = source_spec.rows {
                    let data = self.convert_json_rows(rows)?;
                    self.sources.insert(source_spec.name.clone(), data);
                }
                Ok(())
            }
            spec::Component::Style(_) | spec::Component::Config(_) | spec::Component::Params(_) => {
                // Style/config/params registration — stored for future use
                Ok(())
            }
            spec::Component::Chart(_) => {
                Err(ChartError::InvalidSpec(
                    "Cannot register chart components. Use render_from_yaml() instead.".into()
                ))
            }
        }
    }

    /// Register a named source directly from data rows.
    pub fn register_source(&mut self, name: &str, data: Vec<Row>) {
        self.sources.insert(name.to_string(), data);
    }

    // --- Rendering ---

    /// Parse a YAML string and render the chart component(s).
    /// Returns the ChartElement tree.
    /// Uses default dimensions (800x400) unless the spec overrides them.
    pub fn render_from_yaml(&self, yaml: &str) -> Result<ChartElement, ChartError> {
        self.render_from_yaml_with_size(yaml, None, None)
    }

    /// Parse a YAML string and render with an explicit container size.
    /// `container_width` overrides the default width (used when the spec doesn't specify one).
    /// `container_height` overrides the default height.
    pub fn render_from_yaml_with_size(
        &self,
        yaml: &str,
        container_width: Option<f64>,
        container_height: Option<f64>,
    ) -> Result<ChartElement, ChartError> {
        let parsed = spec::parse(yaml)?;

        // Start with the persistent source registry, then overlay
        // any document-local sources (from multi-document YAML).
        let mut sources: HashMap<String, Vec<Row>> = self.sources.clone();

        if let ChartMLSpec::Array(ref components) = parsed {
            for component in components {
                if let Component::Source(source_spec) = component {
                    if let Some(ref rows) = source_spec.rows {
                        let data = self.convert_json_rows(rows)?;
                        sources.insert(source_spec.name.clone(), data);
                    }
                }
            }
        }

        // Collect all chart components
        let chart_specs: Vec<&ChartSpec> = match &parsed {
            ChartMLSpec::Single(Component::Chart(chart)) => vec![chart],
            ChartMLSpec::Array(components) => {
                components.iter()
                    .filter_map(|c| match c {
                        Component::Chart(chart) => Some(chart),
                        _ => None,
                    })
                    .collect()
            }
            _ => return Err(ChartError::InvalidSpec("No chart component found".into())),
        };

        if chart_specs.is_empty() {
            return Err(ChartError::InvalidSpec("No chart component found".into()));
        }

        if chart_specs.len() == 1 {
            self.render_chart_internal(chart_specs[0], container_width, container_height, &sources)
        } else {
            // Multiple charts — render each and wrap in a grid container
            let mut children = Vec::new();
            for spec in chart_specs {
                match self.render_chart_internal(spec, container_width, container_height, &sources) {
                    Ok(element) => children.push(element),
                    Err(e) => {
                        // Continue rendering other charts even if one fails
                        children.push(ChartElement::Div {
                            class: "chartml-error".to_string(),
                            style: HashMap::new(),
                            children: vec![ChartElement::Span {
                                class: "".to_string(),
                                style: HashMap::new(),
                                content: format!("Chart error: {}", e),
                            }],
                        });
                    }
                }
            }
            Ok(ChartElement::Div {
                class: "chartml-multi-chart".to_string(),
                style: HashMap::from([
                    ("display".to_string(), "grid".to_string()),
                    ("grid-template-columns".to_string(), format!("repeat({}, 1fr)", children.len().min(4))),
                    ("gap".to_string(), "16px".to_string()),
                ]),
                children,
            })
        }
    }

    /// Render a parsed ChartSpec into a ChartElement tree.
    pub fn render_chart(&self, chart_spec: &ChartSpec) -> Result<ChartElement, ChartError> {
        self.render_chart_with_size(chart_spec, None, None)
    }

    /// Render a parsed ChartSpec with explicit container dimensions.
    /// Spec-level width/height take priority; container size is the fallback.
    pub fn render_chart_with_size(
        &self,
        chart_spec: &ChartSpec,
        container_width: Option<f64>,
        container_height: Option<f64>,
    ) -> Result<ChartElement, ChartError> {
        let sources = HashMap::new();
        self.render_chart_internal(chart_spec, container_width, container_height, &sources)
    }

    /// Internal render method that accepts named sources for resolution.
    fn render_chart_internal(
        &self,
        chart_spec: &ChartSpec,
        container_width: Option<f64>,
        container_height: Option<f64>,
        sources: &HashMap<String, Vec<Row>>,
    ) -> Result<ChartElement, ChartError> {
        let chart_type = &chart_spec.visualize.chart_type;

        // Look up renderer
        let renderer = self.registry.get_renderer(chart_type)
            .ok_or_else(|| ChartError::UnknownChartType(chart_type.clone()))?;

        // Extract data (inline or from named source)
        let mut data = self.extract_data(chart_spec, sources)?;

        // Apply transforms if specified
        if let Some(ref transform_spec) = chart_spec.transform {
            data = transform::apply_transforms(data, transform_spec)?;
        }

        // Build chart config — spec dimensions override container dimensions
        let default_height = renderer.default_dimensions(&chart_spec.visualize)
            .map(|d| d.height)
            .unwrap_or(400.0);

        let height = chart_spec.visualize.style
            .as_ref()
            .and_then(|s| s.height)
            .or(container_height)
            .unwrap_or(default_height);

        let width = chart_spec.visualize.style
            .as_ref()
            .and_then(|s| s.width)
            .or(container_width)
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

    /// Extract data from a chart spec, resolving both inline and named sources.
    fn extract_data(&self, chart_spec: &ChartSpec, sources: &HashMap<String, Vec<Row>>) -> Result<Vec<Row>, ChartError> {
        match &chart_spec.data {
            DataRef::Inline(inline) => {
                let rows = inline.rows.as_ref()
                    .ok_or_else(|| ChartError::DataError("Inline data source has no rows".into()))?;
                self.convert_json_rows(rows)
            }
            DataRef::Named(name) => {
                sources.get(name)
                    .cloned()
                    .ok_or_else(|| ChartError::DataError(
                        format!("Named data source '{}' not found", name)
                    ))
            }
        }
    }

    /// Convert JSON value rows into typed Row objects.
    fn convert_json_rows(&self, rows: &[serde_json::Value]) -> Result<Vec<Row>, ChartError> {
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

    #[test]
    fn chartml_named_source_resolution() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);

        let yaml = r#"---
type: source
version: 1
name: q1_sales
provider: inline
rows:
  - { month: "Jan", revenue: 100 }
  - { month: "Feb", revenue: 200 }
---
type: chart
version: 1
title: Revenue by Month
data: q1_sales
visualize:
  type: bar
  columns: month
  rows: revenue
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_ok(), "named source render failed: {:?}", result.err());
    }

    #[test]
    fn chartml_named_source_not_found() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);

        let yaml = r#"
type: chart
version: 1
data: nonexistent_source
visualize:
  type: bar
  columns: x
  rows: y
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "Expected 'not found' error, got: {}", err);
    }

    #[test]
    fn chartml_multi_chart_rendering() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);

        let yaml = r#"
- type: chart
  version: 1
  title: Chart A
  data:
    provider: inline
    rows:
      - { x: "A", y: 10 }
  visualize:
    type: bar
    columns: x
    rows: y
- type: chart
  version: 1
  title: Chart B
  data:
    provider: inline
    rows:
      - { x: "B", y: 20 }
  visualize:
    type: bar
    columns: x
    rows: y
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_ok(), "multi-chart render failed: {:?}", result.err());
        match result.unwrap() {
            ChartElement::Div { class, children, .. } => {
                assert_eq!(class, "chartml-multi-chart");
                assert_eq!(children.len(), 2);
            }
            other => panic!("Expected Div wrapper, got {:?}", other),
        }
    }

    #[test]
    fn chartml_named_source_with_transform() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);

        let yaml = r#"---
type: source
version: 1
name: raw_sales
provider: inline
rows:
  - { region: "North", revenue: 100 }
  - { region: "North", revenue: 200 }
  - { region: "South", revenue: 150 }
---
type: chart
version: 1
title: Revenue by Region
data: raw_sales
transform:
  aggregate:
    dimensions:
      - region
    measures:
      - column: revenue
        aggregation: sum
        name: total_revenue
    sort:
      - field: total_revenue
        direction: desc
visualize:
  type: bar
  columns: region
  rows: total_revenue
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_ok(), "transform pipeline render failed: {:?}", result.err());
    }

    #[test]
    fn chartml_multi_chart_with_shared_source() {
        let mut chartml = ChartML::new();
        chartml.register_renderer("bar", MockRenderer);
        chartml.register_renderer("metric", MockRenderer);

        let yaml = r#"---
type: source
version: 1
name: kpis
provider: inline
rows:
  - { totalRevenue: 1500000, previousRevenue: 1200000 }
---
- type: chart
  version: 1
  title: Revenue
  data: kpis
  visualize:
    type: metric
    value: totalRevenue
- type: chart
  version: 1
  title: Prev Revenue
  data: kpis
  visualize:
    type: metric
    value: previousRevenue
"#;

        let result = chartml.render_from_yaml(yaml);
        assert!(result.is_ok(), "multi-chart shared source failed: {:?}", result.err());
    }
}
