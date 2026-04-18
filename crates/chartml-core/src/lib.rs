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
pub mod svg;
pub mod pipeline;

pub use error::ChartError;
pub use spec::{parse, ChartMLSpec, Component};
pub use element::ChartElement;
pub use plugin::{ChartConfig, ChartRenderer, DataSource, TransformMiddleware, DatasourceResolver};
pub use registry::ChartMLRegistry;
pub use theme::Theme;
pub use pipeline::{FetchedChart, PreparedChart, FetchMetadata, PreparedMetadata, RenderOptions};

use std::collections::HashMap;
use std::time::SystemTime;
use indexmap::IndexMap;
use crate::data::{Row, DataTable};
use crate::spec::{ChartSpec, DataRef, InlineData};

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
    /// Theme colors for chart chrome (axes, grid, text).
    /// Defaults to light mode. Set via `set_theme()` to match your app's appearance.
    theme: theme::Theme,
}

impl ChartML {
    /// Create a new empty ChartML instance.
    pub fn new() -> Self {
        Self {
            registry: ChartMLRegistry::new(),
            sources: HashMap::new(),
            param_values: params::ParamValues::new(),
            default_palette: None,
            theme: theme::Theme::default(),
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

    /// Set the theme for chart chrome colors (axes, grid, text, background).
    /// Use `Theme::default()` for light mode, `Theme::dark()` for dark mode,
    /// or construct a custom `Theme` to match your application's appearance.
    pub fn set_theme(&mut self, theme: theme::Theme) {
        self.theme = theme;
    }

    /// Get a reference to the current theme. Consumers (e.g. chartml-leptos)
    /// use this to thread typography into HTML chrome rendered outside the SVG.
    pub fn theme(&self) -> &theme::Theme {
        &self.theme
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
    ///
    /// On native targets, when a `TransformMiddleware` is registered the sync
    /// path dispatches through it via `pollster::block_on`, so multi-source
    /// `NamedMap` + SQL joins work identically to the async path. On WASM the
    /// async middleware can't be polled synchronously — multi-source maps and
    /// `sql` / `forecast` transforms surface a clear error pointing the caller
    /// to `render_from_yaml_with_params_async`.
    fn render_chart_internal(
        &self,
        chart_spec: &ChartSpec,
        container_width: Option<f64>,
        container_height: Option<f64>,
        sources: &HashMap<String, DataTable>,
    ) -> Result<ChartElement, ChartError> {
        // Resolve every declared source into an ordered map. Inline / Named /
        // single-entry NamedMap collapse to a 1-entry map; multi-entry NamedMap
        // produces one entry per declared source.
        let chart_sources = self.resolve_chart_data(chart_spec, sources)?;

        // Apply transforms — preferring the registered middleware so the sync
        // and async paths share semantics (DataFusion SQL, multi-source joins,
        // etc.). Falls back to the built-in aggregate-only sync transform when
        // no middleware is registered AND the spec only uses an aggregate.
        let data = self.run_sync_transform_pipeline(chart_spec, &chart_sources)?;

        let (element, _, _) =
            self.build_and_render(chart_spec, &data, container_width, container_height)?;
        Ok(element)
    }

    /// Run the transform stage on the sync render path, sharing dispatch logic
    /// with the async path. When a `TransformMiddleware` is registered, the
    /// async `transform` call is driven to completion with `pollster::block_on`
    /// on native targets. WASM has no synchronous executor available, so the
    /// sync path keeps the legacy aggregate-only fallback there and surfaces a
    /// clear error if the spec needs middleware features (sql / forecast /
    /// multi-source joins).
    fn run_sync_transform_pipeline(
        &self,
        chart_spec: &ChartSpec,
        chart_sources: &IndexMap<String, DataTable>,
    ) -> Result<DataTable, ChartError> {
        let Some(transform_spec) = chart_spec.transform.as_ref() else {
            return single_source_or_err_no_transform(chart_sources);
        };

        if let Some(_middleware) = self.registry.get_transform() {
            // Native: drive the async middleware to completion synchronously so
            // multi-source NamedMap + SQL joins work on both sync and async
            // entry points. WASM has no sync executor — surface a clear error
            // so callers move to the async API instead of hanging.
            #[cfg(not(target_arch = "wasm32"))]
            {
                let context = plugin::TransformContext::default();
                let result = pollster::block_on(
                    _middleware.transform(chart_sources, transform_spec, &context),
                )?;
                return Ok(result.data);
            }
            #[cfg(target_arch = "wasm32")]
            {
                return Err(ChartError::InvalidSpec(
                    "Sync render cannot drive the registered TransformMiddleware on WASM. Call `render_from_yaml_with_params_async` instead.".into(),
                ));
            }
        }

        // No middleware registered — fall back to the built-in aggregate
        // transform. Multi-source maps and sql / forecast transforms require
        // middleware; surface a clear error for those.
        if transform_spec.sql.is_some() || transform_spec.forecast.is_some() {
            return Err(ChartError::InvalidSpec(format!(
                "Spec uses `{}` transform but no TransformMiddleware is registered. Call `register_transform(DataFusionTransform)` (or another middleware) before rendering.",
                describe_transform(transform_spec),
            )));
        }
        let single = single_source_or_err(chart_sources, transform_spec)?;
        let rows = single.to_rows();
        let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
        DataTable::from_rows(&transformed_rows)
    }

    /// Resolve a single named-map entry: look up `name` in pre-registered
    /// sources first; if the entry carried inline `rows`, materialize those.
    /// Returns an error if neither path produces a table.
    fn materialize_named_entry(
        &self,
        name: &str,
        inline: &InlineData,
        sources: &HashMap<String, DataTable>,
    ) -> Result<DataTable, ChartError> {
        if let Some(table) = sources.get(name) {
            return Ok(table.clone());
        }
        if let Some(rows) = &inline.rows {
            let json_rows = self.convert_json_rows(rows)?;
            return DataTable::from_rows(&json_rows);
        }
        Err(ChartError::DataError(format!(
            "Named data source '{}' is not pre-registered (call `register_source(\"{}\", ...)` before rendering) and the spec did not provide inline `rows`.",
            name, name,
        )))
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
                        ("color".to_string(), self.theme.text.clone()),
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
                        ("color".to_string(), self.theme.text.clone()),
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
                        ("color".to_string(), self.theme.text.clone()),
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
                        ("color".to_string(), self.theme.text.clone()),
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
                        ("color".to_string(), self.theme.text.clone()),
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

    // --- Three-stage pipeline (chartml 5.0 phase 2) ---

    /// Stage 1 of the chartml 5.0 pipeline: parse YAML, resolve params,
    /// and produce a `FetchedChart` whose `sources` map contains every
    /// named source the chart needs.
    ///
    /// Phase 2 reads sources from pre-registered `self.sources` only —
    /// inline `rows:` are materialized at parse time, named refs are looked
    /// up in the persistent registry, and named maps materialize each entry.
    /// Phase 3 will replace the body with a provider/resolver dispatch
    /// while keeping this exact public signature.
    pub async fn fetch(
        &self,
        yaml: &str,
        opts: &RenderOptions,
    ) -> Result<FetchedChart, ChartError> {
        // Param resolution + chart extraction is shared between fetch and the
        // legacy async path; no I/O happens here in phase 2.
        let (chart_spec, sources) =
            self.parse_and_collect_sources(yaml, opts.params_ref())?;
        let chart_sources = self.resolve_chart_data(&chart_spec, &sources)?;

        Ok(FetchedChart {
            spec: chart_spec,
            sources: chart_sources,
            metadata: FetchMetadata::empty_now(),
        })
    }

    /// Stage 2: collapse the fetched sources into a single `DataTable` ready
    /// for the renderer. Runs the registered `TransformMiddleware` when a
    /// `transform:` block is present, falls back to the built-in
    /// aggregate-only transform when no middleware is registered, or
    /// passes the lone source through unchanged when no transform is
    /// declared.
    ///
    /// Validation rules (error text begins with the React/JS-matching wording,
    /// then appends extra source-count context for debuggability):
    /// - 0 sources → internal invariant violation (`fetch` always produces ≥1 entry).
    /// - 1 source, no transform → passthrough.
    /// - >1 sources, no transform → error beginning with `"Named data sources require a transform block when multiple sources are defined"` followed by `(got N sources: …)` detail.
    /// - Otherwise → middleware (or built-in fallback for aggregate-only).
    pub async fn transform(
        &self,
        fetched: FetchedChart,
        _opts: &RenderOptions,
    ) -> Result<PreparedChart, ChartError> {
        // `_opts` is reserved — phase 3 will thread params through TransformContext.
        let FetchedChart { spec, sources, metadata: _ } = fetched;

        if sources.is_empty() {
            // Internal invariant: phase 2 fetch always produces ≥1 entry.
            return Err(ChartError::InvalidSpec(
                "Internal invariant violation: ChartML::fetch produced zero sources. \
                 Every spec must resolve to at least one named source before transform.".into(),
            ));
        }

        let sources_used: Vec<String> = sources.keys().cloned().collect();

        let (data, transform_applied) = match spec.transform.as_ref() {
            None => {
                // No transform → passthrough requires exactly one source;
                // multi-source maps without a transform have no defined
                // merge semantics. Error text begins with the React-matching
                // wording, then appends source-count context for debuggability.
                let single = single_source_or_err_no_transform(&sources)?;
                (single, false)
            }
            Some(transform_spec) => {
                if let Some(middleware) = self.registry.get_transform() {
                    let context = plugin::TransformContext::default();
                    let result = middleware
                        .transform(&sources, transform_spec, &context)
                        .await?;
                    (result.data, true)
                } else {
                    // No middleware — built-in fallback handles aggregate-only on a single table.
                    let single_ref = single_source_or_err(&sources, transform_spec)?;
                    let rows = single_ref.to_rows();
                    let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
                    (DataTable::from_rows(&transformed_rows)?, true)
                }
            }
        };

        Ok(PreparedChart {
            spec,
            data,
            metadata: PreparedMetadata {
                refreshed_at: SystemTime::now(),
                transform_applied,
                sources_used,
            },
        })
    }

    /// Stage 3: render an already-prepared chart to an SVG string. Sync and
    /// pure — no I/O, no async — so consumers can resize-render from the
    /// same `PreparedChart` repeatedly without re-fetching or re-transforming.
    pub fn render_prepared_to_svg(
        &self,
        prepared: &PreparedChart,
        opts: &RenderOptions,
    ) -> Result<String, ChartError> {
        let (element, svg_width, svg_height) = self.build_and_render(
            &prepared.spec,
            &prepared.data,
            opts.width,
            opts.height,
        )?;
        Ok(svg::element_to_svg(&element, svg_width, svg_height))
    }

    /// Convenience: run the full async pipeline (fetch + transform +
    /// render_prepared_to_svg) in one call. Equivalent to chaining the
    /// three stages explicitly; use the explicit form when you need to
    /// cache the intermediate `FetchedChart` / `PreparedChart`.
    pub async fn render_to_svg_async(
        &self,
        yaml: &str,
        opts: &RenderOptions,
    ) -> Result<String, ChartError> {
        let fetched = self.fetch(yaml, opts).await?;
        let prepared = self.transform(fetched, opts).await?;
        self.render_prepared_to_svg(&prepared, opts)
    }

    // --- Async rendering (for use with TransformMiddleware, e.g. DataFusion) ---

    /// Async render with full parameter support — mirrors `render_from_yaml_with_params`
    /// but uses the registered TransformMiddleware for ALL transforms (sql, aggregate, forecast).
    /// Falls back to built-in sync transform only if no middleware is registered.
    ///
    /// Back-compat shim over the chartml 5.0 three-stage pipeline. Returns
    /// `ChartElement` (not `String`) so existing internal callers
    /// (`chartml-leptos`, `chartml-render`, npm wrappers) keep compiling
    /// unchanged. Will be deprecated in phase 7 once every caller has
    /// migrated to `render_to_svg_async`.
    pub async fn render_from_yaml_with_params_async(
        &self,
        yaml: &str,
        container_width: Option<f64>,
        container_height: Option<f64>,
        param_overrides: Option<&params::ParamValues>,
    ) -> Result<ChartElement, ChartError> {
        let opts = RenderOptions {
            width: container_width,
            height: container_height,
            params: param_overrides.cloned(),
        };
        let fetched = self.fetch(yaml, &opts).await?;
        let prepared = self.transform(fetched, &opts).await?;
        let (element, _, _) = self.build_and_render(
            &prepared.spec,
            &prepared.data,
            opts.width,
            opts.height,
        )?;
        Ok(element)
    }

    /// Shared step 1+2 for `fetch` and the legacy async path: resolve params
    /// (including local `params:` blocks), parse the YAML, and collect every
    /// inline-source component into a working `HashMap`. Returns the FIRST
    /// chart spec found (matching the legacy single-chart contract; multi-
    /// chart specs continue to flow through the sync `render_from_yaml`
    /// path which already handles them).
    fn parse_and_collect_sources(
        &self,
        yaml: &str,
        param_overrides: Option<&params::ParamValues>,
    ) -> Result<(ChartSpec, HashMap<String, DataTable>), ChartError> {
        // Param resolution mirrors the sync path: defaults < inline defaults < overrides.
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

        // Collect persistent + document-local inline sources.
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

        // Extract the chart spec — first chart wins, matching the legacy
        // single-chart contract of `render_from_yaml_with_params_async`.
        // Cloning is cheap (ChartSpec is mostly small fields + a few Vec/Option).
        let chart_spec: ChartSpec = match &parsed {
            ChartMLSpec::Single(component) => match component.as_ref() {
                Component::Chart(chart) => chart.as_ref().clone(),
                _ => return Err(ChartError::InvalidSpec("No chart component found".into())),
            },
            ChartMLSpec::Array(components) => components
                .iter()
                .find_map(|c| match c {
                    Component::Chart(chart) => Some(chart.as_ref().clone()),
                    _ => None,
                })
                .ok_or_else(|| ChartError::InvalidSpec("No chart component found".into()))?,
        };

        Ok((chart_spec, sources))
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

        // Build the named-source map. Single-source shapes (Inline / Named)
        // produce a 1-entry map; NamedMap produces one entry per declared
        // source. Pre-registered sources fill in entries that don't carry
        // inline rows.
        let chart_sources: IndexMap<String, DataTable> = match &chart_spec.data {
            DataRef::Inline(inline) => {
                // `unwrap_or_default()` collapses "no `rows:` key" and "rows: []" to
                // the same empty `Vec<Row>` — the `is_empty()` check below then
                // falls through to the caller-supplied `data`, which is the
                // explicit contract of `render_from_yaml_with_data_async`.
                let inline_rows = inline.rows.as_ref()
                    .map(|r| self.convert_json_rows(r))
                    .transpose()?
                    .unwrap_or_default();
                let inline_table = DataTable::from_rows(&inline_rows)?;
                let chosen = if inline_table.is_empty() && !data.is_empty() {
                    data
                } else {
                    inline_table
                };
                let mut map = IndexMap::new();
                map.insert("source".to_string(), chosen);
                map
            }
            DataRef::Named(name) => {
                let table = self.sources.get(name).cloned().ok_or_else(|| {
                    ChartError::DataError(format!("Source '{}' not found", name))
                })?;
                let mut map = IndexMap::new();
                map.insert(name.clone(), table);
                map
            }
            DataRef::NamedMap(map) => {
                let mut out = IndexMap::new();
                for (name, inline) in map {
                    let table = self.materialize_named_entry(name, inline, &self.sources)?;
                    out.insert(name.clone(), table);
                }
                out
            }
        };

        let transformed_data = if let Some(ref transform_spec) = chart_spec.transform {
            if let Some(middleware) = self.registry.get_transform() {
                let context = plugin::TransformContext::default();
                let result = middleware.transform(&chart_sources, transform_spec, &context).await?;
                result.data
            } else if transform_spec.sql.is_some() || transform_spec.forecast.is_some() {
                return Err(ChartError::InvalidSpec(
                    "Spec uses sql or forecast transforms but no TransformMiddleware is registered".into()
                ));
            } else {
                // Sync fallback: DataTable → Vec<Row> → apply_transforms → DataTable.
                // The sync path only handles a single table — multi-source maps
                // require a registered TransformMiddleware to join.
                let single = single_source_or_err(&chart_sources, transform_spec)?;
                let rows = single.to_rows();
                let transformed_rows = transform::apply_transforms(rows, transform_spec)?;
                DataTable::from_rows(&transformed_rows)?
            }
        } else {
            single_source_or_err_no_transform(&chart_sources)?
        };

        let (element, _, _) =
            self.build_and_render(chart_spec, &transformed_data, None, None)?;
        Ok(element)
    }

    /// Resolve a chart spec's `data:` reference into a map of named source
    /// tables. The map is `IndexMap`-typed so insertion order from the YAML is
    /// preserved when the spec uses a multi-source `data:` map.
    ///
    /// - `DataRef::Inline(flat)` → 1-entry map keyed `"source"` (the canonical
    ///   default name; transform middleware aliases this so legacy SQL keeps
    ///   working).
    /// - `DataRef::Named(name)` → 1-entry map keyed `name`, looked up in
    ///   pre-registered sources.
    /// - `DataRef::NamedMap(map)` → one entry per declared source. Each entry
    ///   is resolved via pre-registered sources first, falling back to inline
    ///   `rows` carried directly on the entry. All entries must resolve to a
    ///   table; missing sources produce a clear error message.
    fn resolve_chart_data(
        &self,
        chart_spec: &ChartSpec,
        sources: &HashMap<String, DataTable>,
    ) -> Result<IndexMap<String, DataTable>, ChartError> {
        let mut out = IndexMap::new();
        match &chart_spec.data {
            DataRef::Inline(inline) => {
                let json_rows = inline
                    .rows
                    .as_ref()
                    .map(|r| self.convert_json_rows(r))
                    .transpose()?
                    .unwrap_or_default();
                let table = DataTable::from_rows(&json_rows)?;
                out.insert("source".to_string(), table);
            }
            DataRef::Named(name) => {
                let table = sources.get(name).cloned().ok_or_else(|| {
                    ChartError::DataError(format!("Named data source '{}' not found", name))
                })?;
                out.insert(name.clone(), table);
            }
            DataRef::NamedMap(map) => {
                for (name, inline) in map {
                    let table = self.materialize_named_entry(name, inline, sources)?;
                    out.insert(name.clone(), table);
                }
            }
        }
        Ok(out)
    }

    /// Build chart config and render — shared by sync and async paths.
    ///
    /// Returns `(element, width, height)` so callers that need the resolved
    /// SVG envelope (e.g. `render_prepared_to_svg`) can use the *same*
    /// dimensions that were baked into the layout. This avoids a dual
    /// source-of-truth — the renderer's `default_dimensions()` is consulted
    /// exactly once, here.
    fn build_and_render(
        &self,
        chart_spec: &ChartSpec,
        data: &DataTable,
        container_width: Option<f64>,
        container_height: Option<f64>,
    ) -> Result<(ChartElement, f64, f64), ChartError> {
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
            theme: self.theme.clone(),
        };

        let element = renderer.render(data, &config)?;
        Ok((element, width, height))
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

/// Helper: when no `TransformMiddleware` is registered, the sync fallback can
/// only operate on a single source table. Multi-source maps with a transform
/// require the user to register a middleware (e.g. `DataFusionTransform`) that
/// can join the sources.
fn single_source_or_err<'a>(
    sources: &'a IndexMap<String, DataTable>,
    transform_spec: &spec::TransformSpec,
) -> Result<&'a DataTable, ChartError> {
    if sources.len() == 1 {
        return Ok(sources
            .values()
            .next()
            .expect("sources has 1 entry"));
    }
    Err(ChartError::InvalidSpec(format!(
        "Multi-source `data:` map (got {} sources: {}) with transform `{}` requires a registered TransformMiddleware to join the sources. Call `register_transform(DataFusionTransform)` (or another middleware) before rendering.",
        sources.len(),
        sources.keys().cloned().collect::<Vec<_>>().join(", "),
        describe_transform(transform_spec),
    )))
}

/// Helper: when no transform is declared, the renderer needs exactly one
/// source table. Multi-source maps without a transform have no defined merge
/// semantics — surface a clear error so the user adds a transform block.
fn single_source_or_err_no_transform(
    sources: &IndexMap<String, DataTable>,
) -> Result<DataTable, ChartError> {
    if sources.len() == 1 {
        return Ok(sources
            .values()
            .next()
            .expect("sources has 1 entry")
            .clone());
    }
    Err(ChartError::InvalidSpec(format!(
        "Named data sources require a transform block when multiple sources are defined (got {} sources: {}).",
        sources.len(),
        sources.keys().cloned().collect::<Vec<_>>().join(", "),
    )))
}

fn describe_transform(spec: &spec::TransformSpec) -> &'static str {
    if spec.sql.is_some() {
        "sql"
    } else if spec.aggregate.is_some() {
        "aggregate"
    } else if spec.forecast.is_some() {
        "forecast"
    } else {
        "transform"
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
