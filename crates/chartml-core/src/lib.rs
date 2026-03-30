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
pub mod params;
pub mod theme;

pub use error::ChartError;
pub use spec::{parse, ChartMLSpec, Component};
pub use element::ChartElement;
pub use plugin::{ChartConfig, ChartRenderer, DataSource, TransformMiddleware, DatasourceResolver};
pub use registry::ChartMLRegistry;

use std::collections::HashMap;
use crate::data::{Row, DataTable};
use crate::spec::{ChartSpec, DataRef};

/// Main ChartML instance. Orchestrates parsing, data fetching, and rendering.
/// Maintains source and parameter registries that persist across render calls,
/// matching the JS ChartML class behavior.
pub struct ChartML {
    registry: ChartMLRegistry,
    /// Named source data, registered via register_component() or
    /// automatically collected from multi-document YAML specs.
    sources: HashMap<String, DataTable>,
    /// Parameter default values, collected from type: params components.
    param_values: params::ParamValues,
    /// Default color palette — used when the spec doesn't specify `style.colors`.
    /// Mirrors the JS ChartML `setDefaultPalette()` API.
    default_palette: Option<Vec<String>>,
}

impl ChartML {
    /// Create a new empty ChartML instance.
    pub fn new() -> Self {
        Self {
            registry: ChartMLRegistry::new(),
            sources: HashMap::new(),
            param_values: params::ParamValues::new(),
            default_palette: None,
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

    /// Set the default color palette for charts that don't specify `style.colors`.
    /// Matches the JS ChartML `setDefaultPalette()` API.
    pub fn set_default_palette(&mut self, colors: Vec<String>) {
        self.default_palette = Some(colors);
    }

    // --- Component registration (matches JS chartml.registerComponent()) ---

    /// Register a non-chart component (source, style, config, params) from a YAML string.
    /// Sources are stored in the instance and available to all subsequent render calls.
    /// This matches the JS `chartml.registerComponent(spec)` API.
    pub fn register_component(&mut self, yaml: &str) -> Result<(), ChartError> {
        let parsed = spec::parse(yaml)?;
        match parsed {
            ChartMLSpec::Single(component) => self.register_single_component(*component),
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
                    let json_rows = self.convert_json_rows(rows)?;
                    let data = DataTable::from_rows(&json_rows)?;
                    self.sources.insert(source_spec.name.clone(), data);
                }
                Ok(())
            }
            spec::Component::Params(params_spec) => {
                let defaults = params::collect_param_defaults(&[&params_spec]);
                self.param_values.extend(defaults);
                Ok(())
            }
            spec::Component::Style(_) | spec::Component::Config(_) => {
                // Style/config registration — stored for future use
                Ok(())
            }
            spec::Component::Chart(..) => {
                Err(ChartError::InvalidSpec(
                    "Cannot register chart components. Use render_from_yaml() instead.".into()
                ))
            }
        }
    }

    /// Register a named source directly from a DataTable.
    pub fn register_source(&mut self, name: &str, data: DataTable) {
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
        self.render_from_yaml_with_params(yaml, container_width, container_height, None)
    }

    /// Render with explicit param value overrides.
    /// `param_overrides` are current interactive values that take priority over defaults.
    pub fn render_from_yaml_with_params(
        &self,
        yaml: &str,
        container_width: Option<f64>,
        container_height: Option<f64>,
        param_overrides: Option<&params::ParamValues>,
    ) -> Result<ChartElement, ChartError> {
        // Step 1: Collect ALL param values — defaults + overrides.
        // Priority: overrides > persistent defaults > inline defaults
        let mut all_params = self.param_values.clone();

        // Extract inline (chart-level) param defaults from the raw YAML
        let inline_defaults = params::extract_inline_param_defaults(yaml);
        all_params.extend(inline_defaults);

        // Apply overrides (interactive values from UI controls)
        if let Some(overrides) = param_overrides {
            all_params.extend(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        // Resolve parameter references in the YAML string
        let resolved_yaml = if !all_params.is_empty() {
            params::resolve_param_references(yaml, &all_params)
        } else {
            yaml.to_string()
        };

        let parsed = spec::parse(&resolved_yaml)?;

        // Step 2: Collect document-local params and re-resolve if needed.
        let mut local_params = self.param_values.clone();
        let mut has_local_params = false;
        if let ChartMLSpec::Array(ref components) = parsed {
            for component in components {
                if let Component::Params(params_spec) = component {
                    let defaults = params::collect_param_defaults(&[params_spec]);
                    local_params.extend(defaults);
                    has_local_params = true;
                }
            }
        }

        // If we found local params, re-resolve and re-parse
        let parsed = if has_local_params && local_params.len() > self.param_values.len() {
            let re_resolved = params::resolve_param_references(yaml, &local_params);
            spec::parse(&re_resolved)?
        } else {
            parsed
        };

        // Step 3: Collect sources (persistent + document-local).
        let mut sources: HashMap<String, DataTable> = self.sources.clone();

        if let ChartMLSpec::Array(ref components) = parsed {
            for component in components {
                if let Component::Source(source_spec) = component {
                    if let Some(ref rows) = source_spec.rows {
                        let json_rows = self.convert_json_rows(rows)?;
                        let data = DataTable::from_rows(&json_rows)?;
                        sources.insert(source_spec.name.clone(), data);
                    }
                }
            }
        }

        // Collect all chart components
        let chart_specs: Vec<&ChartSpec> = match &parsed {
            ChartMLSpec::Single(component) => match component.as_ref() {
                Component::Chart(chart) => vec![chart.as_ref()],
                _ => vec![],
            },
            ChartMLSpec::Array(components) => {
                components.iter()
                    .filter_map(|c| match c {
                        Component::Chart(chart) => Some(chart.as_ref()),
                        _ => None,
                    })
                    .collect()
            }
        };

        // If no charts, check for params components to render as UI controls
        if chart_specs.is_empty() {
            let params_specs: Vec<&spec::ParamsSpec> = match &parsed {
                ChartMLSpec::Single(component) => match component.as_ref() {
                    Component::Params(p) => vec![p],
                    _ => vec![],
                },
                ChartMLSpec::Array(components) => {
                    components.iter()
                        .filter_map(|c| match c {
                            Component::Params(p) => Some(p),
                            _ => None,
                        })
                        .collect()
                }
            };

            if !params_specs.is_empty() {
                return Ok(self.render_params_ui(&params_specs));
            }

            return Err(ChartError::InvalidSpec("No chart or params component found".into()));
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
        sources: &HashMap<String, DataTable>,
    ) -> Result<ChartElement, ChartError> {
        let chart_type = &chart_spec.visualize.chart_type;

        // Look up renderer
        let renderer = self.registry.get_renderer(chart_type)
            .ok_or_else(|| ChartError::UnknownChartType(chart_type.clone()))?;

        // Extract data (inline or from named source)
        let mut data = self.extract_data(chart_spec, sources)?;

        // Apply transforms if specified (sync fallback: DataTable → Vec<Row> → transform → DataTable)
        if let Some(ref transform_spec) = chart_spec.transform {
            let rows = data.to_rows();
            let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
            data = DataTable::from_rows(&transformed_rows)?;
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
            .or_else(|| self.default_palette.clone())
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
    fn extract_data(&self, chart_spec: &ChartSpec, sources: &HashMap<String, DataTable>) -> Result<DataTable, ChartError> {
        match &chart_spec.data {
            DataRef::Inline(inline) => {
                let rows = inline.rows.as_ref()
                    .ok_or_else(|| ChartError::DataError("Inline data source has no rows".into()))?;
                let json_rows = self.convert_json_rows(rows)?;
                DataTable::from_rows(&json_rows)
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

    /// Render params components as UI controls (Div/Span elements).
    /// Matches the JS paramsUI.js visual output with proper CSS classes.
    fn render_params_ui(&self, params_specs: &[&spec::ParamsSpec]) -> ChartElement {
        let mut param_groups = Vec::new();

        for params_spec in params_specs {
            for param in &params_spec.params {
                let control = self.render_param_control(param);
                param_groups.push(ChartElement::Div {
                    class: "chartml-param-group".to_string(),
                    style: HashMap::new(),
                    children: vec![control],
                });
            }
        }

        ChartElement::Div {
            class: "chartml-params".to_string(),
            style: HashMap::from([
                ("display".to_string(), "flex".to_string()),
                ("flex-wrap".to_string(), "wrap".to_string()),
                ("gap".to_string(), "12px".to_string()),
                ("padding".to_string(), "12px 0".to_string()),
            ]),
            children: param_groups,
        }
    }

    /// Render a single parameter control based on its type.
    fn render_param_control(&self, param: &spec::ParamDef) -> ChartElement {
        let label = ChartElement::Span {
            class: "chartml-param-label".to_string(),
            style: HashMap::from([
                ("font-size".to_string(), "12px".to_string()),
                ("font-weight".to_string(), "600".to_string()),
                ("color".to_string(), "#555".to_string()),
                ("display".to_string(), "block".to_string()),
                ("margin-bottom".to_string(), "4px".to_string()),
            ]),
            content: param.label.clone(),
        };

        let control = match param.param_type.as_str() {
            "multiselect" => {
                let _options_text = param.options.as_ref()
                    .map(|opts| opts.join(", "))
                    .unwrap_or_default();
                let default_text = param.default.as_ref()
                    .map(|d| match d {
                        serde_json::Value::Array(arr) => arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        _ => d.to_string(),
                    })
                    .unwrap_or_default();
                ChartElement::Div {
                    class: "chartml-param-control chartml-param-multiselect".to_string(),
                    style: HashMap::from([
                        ("background".to_string(), "#f5f5f5".to_string()),
                        ("border".to_string(), "1px solid #ddd".to_string()),
                        ("border-radius".to_string(), "4px".to_string()),
                        ("padding".to_string(), "6px 10px".to_string()),
                        ("font-size".to_string(), "13px".to_string()),
                        ("color".to_string(), "var(--chartml-text, #374151)".to_string()),
                        ("min-width".to_string(), "140px".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "".to_string(),
                        style: HashMap::new(),
                        content: default_text,
                    }],
                }
            }
            "select" => {
                let default_text = param.default.as_ref()
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();
                ChartElement::Div {
                    class: "chartml-param-control chartml-param-select".to_string(),
                    style: HashMap::from([
                        ("background".to_string(), "#f5f5f5".to_string()),
                        ("border".to_string(), "1px solid #ddd".to_string()),
                        ("border-radius".to_string(), "4px".to_string()),
                        ("padding".to_string(), "6px 10px".to_string()),
                        ("font-size".to_string(), "13px".to_string()),
                        ("color".to_string(), "var(--chartml-text, #374151)".to_string()),
                        ("min-width".to_string(), "120px".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "".to_string(),
                        style: HashMap::new(),
                        content: format!("{} ▾", default_text),
                    }],
                }
            }
            "daterange" => {
                let default_text = param.default.as_ref()
                    .map(|d| {
                        let start = d.get("start").and_then(|v| v.as_str()).unwrap_or("");
                        let end = d.get("end").and_then(|v| v.as_str()).unwrap_or("");
                        format!("{} → {}", start, end)
                    })
                    .unwrap_or_default();
                ChartElement::Div {
                    class: "chartml-param-control chartml-param-daterange".to_string(),
                    style: HashMap::from([
                        ("background".to_string(), "#f5f5f5".to_string()),
                        ("border".to_string(), "1px solid #ddd".to_string()),
                        ("border-radius".to_string(), "4px".to_string()),
                        ("padding".to_string(), "6px 10px".to_string()),
                        ("font-size".to_string(), "13px".to_string()),
                        ("color".to_string(), "var(--chartml-text, #374151)".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "".to_string(),
                        style: HashMap::new(),
                        content: default_text,
                    }],
                }
            }
            "number" => {
                let default_text = param.default.as_ref()
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                ChartElement::Div {
                    class: "chartml-param-control chartml-param-number".to_string(),
                    style: HashMap::from([
                        ("background".to_string(), "#f5f5f5".to_string()),
                        ("border".to_string(), "1px solid #ddd".to_string()),
                        ("border-radius".to_string(), "4px".to_string()),
                        ("padding".to_string(), "6px 10px".to_string()),
                        ("font-size".to_string(), "13px".to_string()),
                        ("color".to_string(), "var(--chartml-text, #374151)".to_string()),
                        ("min-width".to_string(), "80px".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "".to_string(),
                        style: HashMap::new(),
                        content: default_text,
                    }],
                }
            }
            _ => {
                let default_text = param.default.as_ref()
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                ChartElement::Div {
                    class: "chartml-param-control chartml-param-text".to_string(),
                    style: HashMap::from([
                        ("background".to_string(), "#f5f5f5".to_string()),
                        ("border".to_string(), "1px solid #ddd".to_string()),
                        ("border-radius".to_string(), "4px".to_string()),
                        ("padding".to_string(), "6px 10px".to_string()),
                        ("font-size".to_string(), "13px".to_string()),
                        ("color".to_string(), "var(--chartml-text, #374151)".to_string()),
                    ]),
                    children: vec![ChartElement::Span {
                        class: "".to_string(),
                        style: HashMap::new(),
                        content: param.placeholder.clone().unwrap_or(default_text),
                    }],
                }
            }
        };

        ChartElement::Div {
            class: "chartml-param-item".to_string(),
            style: HashMap::from([
                ("display".to_string(), "flex".to_string()),
                ("flex-direction".to_string(), "column".to_string()),
            ]),
            children: vec![label, control],
        }
    }

    // --- Async rendering (for use with TransformMiddleware, e.g. DataFusion) ---

    /// Async render with full parameter support — mirrors `render_from_yaml_with_params`
    /// but uses the registered TransformMiddleware for ALL transforms (sql, aggregate, forecast).
    /// Falls back to built-in sync transform only if no middleware is registered.
    pub async fn render_from_yaml_with_params_async(
        &self,
        yaml: &str,
        container_width: Option<f64>,
        container_height: Option<f64>,
        param_overrides: Option<&params::ParamValues>,
    ) -> Result<ChartElement, ChartError> {
        // Step 1: Resolve params (same as sync path)
        let mut all_params = self.param_values.clone();
        let inline_defaults = params::extract_inline_param_defaults(yaml);
        all_params.extend(inline_defaults);
        if let Some(overrides) = param_overrides {
            all_params.extend(overrides.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        let resolved_yaml = if !all_params.is_empty() {
            params::resolve_param_references(yaml, &all_params)
        } else {
            yaml.to_string()
        };

        let parsed = spec::parse(&resolved_yaml)?;

        // Step 2: Collect sources
        let mut sources: HashMap<String, DataTable> = self.sources.clone();
        if let ChartMLSpec::Array(ref components) = parsed {
            for component in components {
                if let Component::Source(source_spec) = component {
                    if let Some(ref rows) = source_spec.rows {
                        let json_rows = self.convert_json_rows(rows)?;
                        let data = DataTable::from_rows(&json_rows)?;
                        sources.insert(source_spec.name.clone(), data);
                    }
                }
            }
        }

        // Step 3: Extract chart spec
        let chart_spec: &ChartSpec = match &parsed {
            ChartMLSpec::Single(component) => match component.as_ref() {
                Component::Chart(chart) => chart.as_ref(),
                _ => return Err(ChartError::InvalidSpec("No chart component found".into())),
            },
            ChartMLSpec::Array(components) => {
                components.iter()
                    .find_map(|c| match c {
                        Component::Chart(chart) => Some(chart.as_ref()),
                        _ => None,
                    })
                    .ok_or_else(|| ChartError::InvalidSpec("No chart component found".into()))?
            }
        };

        // Step 4: Resolve data
        let chart_data = self.resolve_chart_data(chart_spec, &sources)?;

        // Step 5: Transform — use middleware for ALL transforms when registered
        let transformed_data = if let Some(ref transform_spec) = chart_spec.transform {
            if let Some(middleware) = self.registry.get_transform() {
                let context = plugin::TransformContext::default();
                let result = middleware.transform(chart_data, transform_spec, &context).await?;
                result.data
            } else {
                // No middleware — fall back to built-in sync transform (aggregate only)
                // DataTable → Vec<Row> → apply_transforms → DataTable
                let rows = chart_data.to_rows();
                let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
                DataTable::from_rows(&transformed_rows)?
            }
        } else {
            chart_data
        };

        // Step 6: Render
        self.build_and_render(chart_spec, &transformed_data, container_width, container_height)
    }

    /// Async render with external data — for integration tests and programmatic use.
    /// Data is used as fallback when spec has empty inline rows.
    pub async fn render_from_yaml_with_data_async(
        &self,
        yaml: &str,
        data: DataTable,
    ) -> Result<ChartElement, ChartError> {
        // Register data as "source", then delegate to full async render
        let parsed = spec::parse(yaml)?;
        let chart_spec: &ChartSpec = match &parsed {
            ChartMLSpec::Single(component) => match component.as_ref() {
                Component::Chart(chart) => chart.as_ref(),
                _ => return Err(ChartError::InvalidSpec("No chart component found".into())),
            },
            ChartMLSpec::Array(components) => {
                components.iter()
                    .find_map(|c| match c { Component::Chart(chart) => Some(chart.as_ref()), _ => None })
                    .ok_or_else(|| ChartError::InvalidSpec("No chart component found".into()))?
            }
        };

        let chart_data = match &chart_spec.data {
            DataRef::Inline(inline) => {
                let inline_rows = inline.rows.as_ref()
                    .map(|r| self.convert_json_rows(r))
                    .transpose()?
                    .unwrap_or_default();
                let inline_table = DataTable::from_rows(&inline_rows)?;
                if inline_table.is_empty() && !data.is_empty() { data } else { inline_table }
            }
            DataRef::Named(name) => {
                self.sources.get(name).cloned()
                    .ok_or_else(|| ChartError::DataError(format!("Source '{}' not found", name)))?
            }
        };

        let transformed_data = if let Some(ref transform_spec) = chart_spec.transform {
            if let Some(middleware) = self.registry.get_transform() {
                let context = plugin::TransformContext::default();
                let result = middleware.transform(chart_data, transform_spec, &context).await?;
                result.data
            } else if transform_spec.sql.is_some() || transform_spec.forecast.is_some() {
                return Err(ChartError::InvalidSpec(
                    "Spec uses sql or forecast transforms but no TransformMiddleware is registered".into()
                ));
            } else {
                // Sync fallback: DataTable → Vec<Row> → apply_transforms → DataTable
                let rows = chart_data.to_rows();
                let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
                DataTable::from_rows(&transformed_rows)?
            }
        } else {
            chart_data
        };

        self.build_and_render(chart_spec, &transformed_data, None, None)
    }

    /// Resolve chart data from inline rows or named sources.
    fn resolve_chart_data(&self, chart_spec: &ChartSpec, sources: &HashMap<String, DataTable>) -> Result<DataTable, ChartError> {
        match &chart_spec.data {
            DataRef::Inline(inline) => {
                let json_rows = inline.rows.as_ref()
                    .map(|r| self.convert_json_rows(r))
                    .transpose()?
                    .unwrap_or_default();
                DataTable::from_rows(&json_rows)
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

    /// Build chart config and render — shared by sync and async paths.
    fn build_and_render(
        &self,
        chart_spec: &ChartSpec,
        data: &DataTable,
        container_width: Option<f64>,
        container_height: Option<f64>,
    ) -> Result<ChartElement, ChartError> {
        let chart_type = &chart_spec.visualize.chart_type;
        let renderer = self.registry.get_renderer(chart_type)
            .ok_or_else(|| ChartError::UnknownChartType(chart_type.clone()))?;

        let default_height = renderer.default_dimensions(&chart_spec.visualize)
            .map(|d| d.height)
            .unwrap_or(400.0);

        let height = chart_spec.visualize.style.as_ref()
            .and_then(|s| s.height)
            .unwrap_or(container_height.unwrap_or(default_height));

        let width = chart_spec.visualize.style.as_ref()
            .and_then(|s| s.width)
            .unwrap_or(container_width.unwrap_or(800.0));

        let colors = chart_spec.visualize.style.as_ref()
            .and_then(|s| s.colors.clone())
            .or_else(|| self.default_palette.clone())
            .unwrap_or_else(|| {
                color::get_chart_colors(12, color::palettes::get_palette("autumn_forest"))
            });

        let config = plugin::ChartConfig {
            visualize: chart_spec.visualize.clone(),
            title: chart_spec.title.clone(),
            width,
            height,
            colors,
        };

        renderer.render(data, &config)
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
        fn render(&self, _data: &DataTable, _config: &ChartConfig) -> Result<ChartElement, ChartError> {
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
