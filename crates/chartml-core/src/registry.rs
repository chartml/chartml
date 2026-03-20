use std::collections::HashMap;
use crate::plugin::{DataSource, TransformMiddleware, ChartRenderer, DatasourceResolver};

/// Central plugin registry. Holds all registered plugins and provides lookup.
pub struct ChartMLRegistry {
    data_sources: HashMap<String, Box<dyn DataSource>>,
    transforms: Vec<Box<dyn TransformMiddleware>>,
    renderers: HashMap<String, Box<dyn ChartRenderer>>,
    datasource_resolver: Option<Box<dyn DatasourceResolver>>,
}

impl ChartMLRegistry {
    pub fn new() -> Self {
        Self {
            data_sources: HashMap::new(),
            transforms: Vec::new(),
            renderers: HashMap::new(),
            datasource_resolver: None,
        }
    }

    pub fn register_data_source(&mut self, name: &str, source: impl DataSource + 'static) {
        self.data_sources.insert(name.to_string(), Box::new(source));
    }

    pub fn register_transform(&mut self, middleware: impl TransformMiddleware + 'static) {
        self.transforms.push(Box::new(middleware));
    }

    pub fn set_transform(&mut self, middleware: impl TransformMiddleware + 'static) {
        self.transforms.clear();
        self.transforms.push(Box::new(middleware));
    }

    pub fn register_renderer(&mut self, chart_type: &str, renderer: impl ChartRenderer + 'static) {
        self.renderers.insert(chart_type.to_string(), Box::new(renderer));
    }

    pub fn set_datasource_resolver(&mut self, resolver: impl DatasourceResolver + 'static) {
        self.datasource_resolver = Some(Box::new(resolver));
    }

    pub fn get_renderer(&self, chart_type: &str) -> Option<&dyn ChartRenderer> {
        self.renderers.get(chart_type).map(|r| r.as_ref())
    }

    pub fn get_data_source(&self, name: &str) -> Option<&dyn DataSource> {
        self.data_sources.get(name).map(|s| s.as_ref())
    }

    pub fn has_renderer(&self, chart_type: &str) -> bool {
        self.renderers.contains_key(chart_type)
    }

    pub fn renderer_types(&self) -> Vec<String> {
        self.renderers.keys().cloned().collect()
    }
}

impl Default for ChartMLRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Row;
    use crate::element::{ChartElement, ViewBox};
    use crate::error::ChartError;
    use crate::plugin::ChartConfig;

    // Create a mock renderer for testing
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
    fn registry_register_and_lookup_renderer() {
        let mut registry = ChartMLRegistry::new();
        registry.register_renderer("bar", MockRenderer);
        assert!(registry.has_renderer("bar"));
        assert!(!registry.has_renderer("pie"));
    }

    #[test]
    fn registry_renderer_types() {
        let mut registry = ChartMLRegistry::new();
        registry.register_renderer("bar", MockRenderer);
        registry.register_renderer("line", MockRenderer);
        let types = registry.renderer_types();
        assert!(types.contains(&"bar".to_string()));
        assert!(types.contains(&"line".to_string()));
    }

    #[test]
    fn registry_default() {
        let registry = ChartMLRegistry::default();
        assert!(registry.renderer_types().is_empty());
    }
}
