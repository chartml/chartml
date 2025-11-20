/**
 * @chartml/core - A declarative markup language for creating beautiful, interactive data visualizations
 *
 * This is the core ChartML library with built-in support for inline and HTTP data sources.
 * Visualization rendering is pure D3 with no external dependencies beyond the data layer.
 *
 * @example
 * import { renderChart } from '@chartml/core';
 *
 * const spec = `
 * data:
 *   - month: Jan
 *     revenue: 45000
 *   - month: Feb
 *     revenue: 52000
 *
 * visualize:
 *   type: bar
 *   columns: month
 *   rows: revenue
 *   style:
 *     title: "Monthly Revenue"
 * `;
 *
 * await renderChart(spec, document.getElementById('chart'));
 */

// Import core CSS styles (tooltips, etc.)
import './chartml.css';

import * as yaml from 'js-yaml';
import { renderD3CartesianChart } from './d3CartesianChart.js';
import { mapChartMLToD3Config } from './d3ChartMapper.js';
import { getChartColors } from './colorUtils.js';
import { getMergedConfig } from './config.js';
import { parseComponent, COMPONENT_TYPES, extractReferences } from './componentParser.js';
import { createRegistry } from './registry.js';
import { globalRegistry } from './pluginRegistry.js';
import { d3Aggregate } from './aggregate.js';
import { renderParams } from './paramsUI.js';
import { resolveParamReferences, extractParamReferences } from './parameterResolver.js';
import { SourceRefreshRegistry } from './sourceRefreshRegistry.js';
import { ParamChangeRegistry } from './paramChangeRegistry.js';

/**
 * Chart Instance Class
 *
 * Returned by ChartML.render() to provide programmatic control over a rendered chart.
 * Allows refreshing data and accessing metadata without parsing YAML.
 */
class Chart {
  constructor(chartml, spec, container, options, middlewareMetadata = {}) {
    this.chartml = chartml;
    this.spec = spec;  // Store IMMUTABLE spec reference
    this.container = container;
    this.options = options;
    this.onRefreshStateChange = null;  // Callback for refresh state changes

    // Use dimensions from middleware metadata if available (includes plugin defaults)
    // Otherwise use instance method to get expected dimensions (supports plugins)
    let dimensions = middlewareMetadata.dimensions;
    if (!dimensions) {
      dimensions = chartml.getExpectedDimensions(spec);
    }

    // CRITICAL: sourceName comes from metadata (extracted during render from IMMUTABLE spec)
    // This ensures we always have the correct source name even if spec was mutated elsewhere
    this.sourceName = middlewareMetadata.sourceName || null;

    this.metadata = {
      // Use refreshedAt from middleware metadata if available, otherwise current time
      last_updated: middlewareMetadata.refreshedAt || Date.now(),
      dimensions: dimensions,  // { width, height }
      // Include any other metadata from middleware
      ...middlewareMetadata
    };

    // Subscribe to source refresh notifications if chart uses a named source
    if (this.sourceName) {
      chartml.sourceRefreshRegistry.subscribe(this.sourceName, this);
      console.log('[Chart constructor] Subscribed to source refresh notifications:', this.sourceName);
    }

    // Subscribe to parameter change notifications for any param scopes this chart depends on
    // Extract param references from spec (e.g., ['dashboard_filters.region', 'top_n'])
    const paramRefs = extractParamReferences(spec);
    this.paramScopes = new Set();  // Track scopes for cleanup in destroy()

    for (const ref of paramRefs) {
      // Only subscribe to named param scopes (those with dots)
      // Chart-level params (no dot) don't need subscription
      if (ref.includes('.')) {
        const scopeName = ref.split('.')[0];  // Extract 'dashboard_filters' from 'dashboard_filters.region'

        // Avoid duplicate subscriptions if chart uses multiple params from same scope
        if (!this.paramScopes.has(scopeName)) {
          this.paramScopes.add(scopeName);
          chartml.paramChangeRegistry.subscribe(scopeName, this);
          console.log('[Chart constructor] Subscribed to param scope:', scopeName);
        }
      }
    }
  }

  /**
   * Set callback for refresh state changes (for animating refresh button, etc.)
   * @param {Function} callback - Called with (isRefreshing: boolean)
   */
  setRefreshStateCallback(callback) {
    this.onRefreshStateChange = callback;
  }

  /**
   * Refresh the chart by re-fetching data from source (bypassing cache) and re-rendering
   * @returns {Promise<void>}
   *
   * If chart uses a named data source, notifies all other charts using the same source
   * that a refresh is happening (they show spinners). Middleware deduplicates the actual fetch.
   */
  async refresh() {
    // If chart uses a named source, coordinate refresh notifications through registry
    if (this.sourceName) {
      console.log('[Chart.refresh] Coordinating refresh for source:', this.sourceName);
      await this.chartml.sourceRefreshRegistry.refreshSource(this.sourceName, async () => {
        // This callback does the actual refresh for THIS chart
        // Registry will notify all other charts using this source
        // CRITICAL: Pass this Chart's spec, not the shared ChartML instance's currentSpec
        await this.chartml._renderChartWithParams(this.container, {
          ...this.options,
          bypassCache: true,
          spec: this.spec  // Use THIS chart's spec
        });
        this.metadata.last_updated = Date.now();
      }, this);  // Pass THIS chart so registry knows which one initiated
      return;
    }

    // No named source - refresh independently
    try {
      if (this.onRefreshStateChange) {
        this.onRefreshStateChange(true);
      }

      console.log('[Chart.refresh] Refreshing with bypassCache (no source coordination)');

      await this.chartml._renderChartWithParams(this.container, {
        ...this.options,
        bypassCache: true,
        spec: this.spec  // Use THIS chart's spec
      });

      this.metadata.last_updated = Date.now();
    } finally {
      if (this.onRefreshStateChange) {
        this.onRefreshStateChange(false);
      }
    }
  }

  /**
   * Re-render the chart using cached data (no fetch)
   * Used when another chart refreshed the shared data source
   * @returns {Promise<void>}
   */
  async rerender() {
    console.log('[Chart.rerender] Re-rendering with parameter resolution');
    // Call full render() with original spec to re-resolve parameters
    // render() will handle parameter resolution and return a new Chart instance
    await this.chartml.render(this.spec, this.container, {
      ...this.options,
      bypassCache: false  // Use cached data
    });
  }

  /**
   * Destroy the chart and clean up
   */
  destroy() {
    // Unsubscribe from source refresh notifications
    if (this.sourceName) {
      this.chartml.sourceRefreshRegistry.unsubscribe(this.sourceName, this);
      console.log('[Chart.destroy] Unsubscribed from source refresh notifications:', this.sourceName);
    }

    // Unsubscribe from parameter change notifications
    if (this.paramScopes) {
      for (const scopeName of this.paramScopes) {
        this.chartml.paramChangeRegistry.unsubscribe(scopeName, this);
        console.log('[Chart.destroy] Unsubscribed from param scope:', scopeName);
      }
    }

    // Clear container
    if (this.container) {
      this.container.innerHTML = '';
    }
  }

  /**
   * Get chart metadata (e.g., last refresh timestamp)
   * @returns {Object} Metadata object with last_updated timestamp
   */
  getMetadata() {
    return { ...this.metadata };
  }
}

/**
 * ChartML Renderer Class
 *
 * Main class for rendering ChartML specifications into interactive D3 visualizations.
 * Supports plugin system for extensible data sources and aggregate middleware.
 *
 * ChartML is palette-agnostic - it doesn't define any built-in palettes.
 * The parent application provides the default palette as an array of colors.
 */
export class ChartML {
  constructor(options = {}) {
    this.dataSources = new Map();
    this.aggregateMiddleware = [];
    this.chartRenderers = new Map();  // Chart renderer plugins
    this.defaultPalette = options.defaultPalette || null;  // Array of color strings from parent app
    this.loadingIndicator = options.loadingIndicator || null;  // Optional custom loading indicator function

    // Create param change registry for coordinating parameter updates across charts
    this.paramChangeRegistry = new ParamChangeRegistry();

    // Create component registry for sources, styles, configs (pass paramChangeRegistry for notifications)
    this.registry = options.registry || createRegistry(this.paramChangeRegistry);

    // Create source refresh registry for coordinating refreshes across charts
    this.sourceRefreshRegistry = new SourceRefreshRegistry();

    // Plugin hooks for advanced features
    this.hooks = {
      onProgress: options.onProgress || null,        // Progress callback for streaming
      onCacheHit: options.onCacheHit || null,        // Cache hit notification
      onCacheMiss: options.onCacheMiss || null,      // Cache miss notification
      onError: options.onError || null,              // Error callback
      onLoadingChange: options.onLoadingChange || null  // Loading state change callback (isLoading: boolean)
    };

    // Parameter/filter state management
    this.paramValues = {};        // Current parameter values { field: value }
    this.paramsDefinition = null; // Params definition (from registry or inline)
    this.filterContainer = null;  // Reference to filter UI container
    this.chartContainer = null;   // Reference to chart container

    // Register built-in data sources
    this._registerBuiltInDataSources();

    // Register built-in aggregation middleware
    this._registerBuiltInAggregation();

    // Register built-in chart renderers (backward compatibility)
    // These will be moved to plugins in the future
    this._registerBuiltInChartRenderers();
  }

  /**
   * Set the default palette to use for charts
   * @param {Array} palette - Array of color strings (e.g., ['#ff0000', '#00ff00', '#0000ff'])
   */
  setDefaultPalette(palette) {
    if (!Array.isArray(palette)) {
      throw new Error('Invalid palette: must be an array of color strings');
    }
    this.defaultPalette = palette;
  }

  /**
   * Register built-in aggregation middleware (d3-array)
   */
  _registerBuiltInAggregation() {
    this.registerAggregateMiddleware(d3Aggregate);
  }

  /**
   * Register built-in data sources (inline and HTTP)
   */
  _registerBuiltInDataSources() {
    // Inline data source
    this.registerDataSource('inline', async (spec) => {
      if (Array.isArray(spec.rows)) {
        return spec.rows;
      }
      throw new Error('Inline data source requires rows to be an array');
    });

    // HTTP data source
    this.registerDataSource('http', async (spec) => {
      if (typeof spec.data === 'string' && (spec.data.startsWith('http://') || spec.data.startsWith('https://'))) {
        const response = await fetch(spec.data);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error('HTTP data source must return a JSON array');
        }
        return data;
      }
      throw new Error('HTTP data source requires data to be a URL string');
    });
  }

  /**
   * Register a custom data source plugin
   *
   * @param {string} name - Data source name (e.g., 'bigquery', 'postgres')
   * @param {Function} handler - Async function that returns data array
   *
   * @example
   * chartml.registerDataSource('bigquery', async (spec) => {
   *   // Execute BigQuery and return rows
   *   return rows;
   * });
   */
  registerDataSource(name, handler) {
    this.dataSources.set(name, handler);
  }

  /**
   * Emit a hook event to registered callbacks
   * @private
   */
  _emitHook(hookName, ...args) {
    if (this.hooks[hookName] && typeof this.hooks[hookName] === 'function') {
      try {
        this.hooks[hookName](...args);
      } catch (error) {
        console.error(`[ChartML] Hook ${hookName} error:`, error);
      }
    }
  }

  /**
   * Register aggregate middleware plugin
   *
   * @param {Function} middleware - Async function that transforms data
   *
   * @example
   * chartml.registerAggregateMiddleware(async (data, aggregateSpec) => {
   *   // Transform data using DuckDB or other engine
   *   return transformedData;
   * });
   */
  registerAggregateMiddleware(middleware) {
    this.aggregateMiddleware.push(middleware);
  }

  /**
   * Set aggregate middleware, replacing any existing middleware (including defaults)
   *
   * This is the preferred method when you want to replace the default d3Aggregate
   * middleware with a custom implementation like DuckDB.
   *
   * @param {Function} middleware - Async function that transforms data
   *
   * @example
   * chartml.setAggregateMiddleware(async (data, aggregateSpec) => {
   *   // Replace default d3 aggregation with DuckDB
   *   return duckDbAggregation(data, aggregateSpec);
   * });
   */
  setAggregateMiddleware(middleware) {
    this.aggregateMiddleware = [middleware];
  }

  /**
   * Register a chart renderer plugin
   *
   * @param {string} type - Chart type (e.g., 'bar', 'line', 'pie')
   * @param {Function} renderer - Renderer function (container, data, config) => void
   *
   * Optional Plugin Interface:
   * Renderers can provide custom default dimensions by implementing:
   * renderer.getDefaultDimensions = (spec, container) => ({ height: number, width?: number })
   *
   * This allows plugins to override the default 400px height with chart-type-specific defaults.
   * For example, metric cards might return { height: 150 } for a more compact display.
   *
   * @example
   * // Basic renderer
   * chartml.registerChartRenderer('bar', (container, data, config) => {
   *   // Render bar chart using D3, Canvas, or any library
   * });
   *
   * @example
   * // Renderer with custom default dimensions
   * const renderer = (container, data, config) => {
   *   // Render metric card
   * };
   * renderer.getDefaultDimensions = () => ({ height: 150 });
   * chartml.registerChartRenderer('metric', renderer);
   */
  registerChartRenderer(type, renderer) {
    if (this.chartRenderers.has(type)) {
      console.warn(
        `⚠️  ChartML: Renderer "${type}" is already registered and will be overwritten.\n` +
        `   Consider using a namespaced type (e.g., "@yourorg/${type}") to avoid conflicts.`
      );
    }
    this.chartRenderers.set(type, renderer);
  }

  /**
   * Resolve data source - determine which handler to use
   */
  async _resolveDataSource(spec, options = {}) {
    console.log('[ChartML._resolveDataSource] options:', options);

    // Inline data (array)
    if (Array.isArray(spec.data)) {
      const handler = this.dataSources.get('inline');
      return await handler(spec, options);
    }

    // HTTP data (URL string)
    if (typeof spec.data === 'string' && (spec.data.startsWith('http://') || spec.data.startsWith('https://'))) {
      const handler = this.dataSources.get('http');
      return await handler(spec, options);
    }

    // Object with type property - plugin data source
    if (spec.data && typeof spec.data === 'object' && spec.data.type) {
      const handler = this.dataSources.get(spec.data.type);
      if (!handler) {
        throw new Error(`Unknown data source type: ${spec.data.type}`);
      }
      console.log('[ChartML._resolveDataSource] Calling handler for type:', spec.data.type, 'with options:', options);
      return await handler(spec, options);
    }

    throw new Error('Unable to resolve data source. Data must be an array, URL string, or object with type property.');
  }

  /**
   * Apply aggregate middleware (includes filter + aggregate stages)
   *
   * MIDDLEWARE-CONTROLLED CACHING:
   * The middleware receives a lazy data fetch callback in context.fetchData.
   * The middleware decides whether to:
   * - Return cached results (never calls fetchData)
   * - Call fetchData() to get fresh data
   *
   * ChartML core is completely unaware of caching - only middleware knows.
   *
   * @param {Function} fetchData - Lazy data fetch callback (only called by middleware if needed)
   * @param {Object} spec - Full chart spec (for cache key generation)
   * @param {Object} context - Context with hooks, options, etc.
   * @returns {Promise<Array>} Processed data
   */
  async _applyAggregate(fetchData, spec, context = {}) {
    // If no middleware registered, fetch data directly
    if (this.aggregateMiddleware.length === 0) {
      return await fetchData();
    }

    // ALWAYS call middleware when registered, even for charts without aggregate config
    // The middleware handles passthrough cases AND is essential for data format conversion
    // (e.g., Arrow buffers must be converted to JS objects via DuckDB)
    const middlewareContext = {
      ...context,
      fetchData,  // Lazy data source callback
      spec        // Full spec for cache key generation
    };

    // Apply each middleware in order
    // First middleware receives null as data (hasn't been fetched yet)
    // Middleware can call context.fetchData() if it needs fresh data
    let result = null;

    for (const middleware of this.aggregateMiddleware) {
      result = await middleware(result, spec, middlewareContext);
    }

    return result;
  }

  /**
   * Register a component (source, style, config, or params)
   *
   * @param {string|object} spec - ChartML YAML string or parsed object
   * @returns {Object} Parsed component with type information
   * @throws {Error} If component is invalid or a chart type
   *
   * @example
   * chartml.registerComponent(`
   *   type: source
   *   name: sales_data
   *   provider: inline
   *   data:
   *     - month: Jan
   *       revenue: 45000
   * `);
   */
  registerComponent(spec) {
    // Parse component
    const component = parseComponent(typeof spec === 'string' ? spec : yaml.dump(spec));

    // Cannot register chart components - they should be rendered
    if (component.type === COMPONENT_TYPES.CHART) {
      throw new Error('Cannot register chart components. Use render() method instead.');
    }

    // Register based on type
    switch (component.type) {
      case COMPONENT_TYPES.SOURCE:
        this.registry.registerSource(component.spec.name, component.spec);
        break;

      case COMPONENT_TYPES.STYLE:
        this.registry.registerStyle(component.spec.name, component.spec);
        break;

      case COMPONENT_TYPES.CONFIG:
        this.registry.registerConfig(component.spec);
        break;

      case COMPONENT_TYPES.PARAMS:
        this.registry.registerParams(component.spec.name, component.spec);
        break;

      default:
        throw new Error(`Unknown component type: ${component.type}`);
    }

    return component;
  }

  /**
   * Resolve data source with registry support
   * Supports both inline data and references to registered sources
   */
  async _resolveDataSource(spec, options = {}) {
    // Check if data is a string reference to a named source (ChartML spec: data: source_name)
    if (typeof spec.data === 'string' && !spec.data.startsWith('http://') && !spec.data.startsWith('https://')) {
      const source = this.registry.resolveSource(spec.data);
      if (!source) {
        throw new Error(`Data source "${spec.data}" not found. Did you register it first?`);
      }

      // Handle different provider types
      if (source.provider === 'inline') {
        return source.rows || source.data;
      } else if (source.provider === 'http') {
        const response = await fetch(source.endpoint);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error('HTTP data source must return a JSON array');
        }
        return data;
      } else {
        // Plugin data source (bigquery, api, etc.)
        const handler = this.dataSources.get(source.provider);
        if (!handler) {
          throw new Error(`Unknown data source provider: ${source.provider}`);
        }
        // Pass hooks and options (including bypassCache) to the handler
        return await handler(source, { hooks: this.hooks, ...options });
      }
    }

    // Legacy dataSource property (deprecated, but still supported)
    if (spec.dataSource && typeof spec.dataSource === 'string') {
      const source = this.registry.resolveSource(spec.dataSource);
      if (!source) {
        throw new Error(`Data source "${spec.dataSource}" not found. Did you register it first?`);
      }

      // Handle different provider types
      if (source.provider === 'inline') {
        return source.rows || source.data;
      } else if (source.provider === 'http') {
        const response = await fetch(source.endpoint);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error('HTTP data source must return a JSON array');
        }
        return data;
      } else {
        // Plugin data source (bigquery, api, etc.)
        const handler = this.dataSources.get(source.provider);
        if (!handler) {
          throw new Error(`Unknown data source provider: ${source.provider}`);
        }
        // Pass hooks and options (including bypassCache) to the handler
        return await handler(source, { hooks: this.hooks, ...options });
      }
    }

    // Inline data (array)
    if (Array.isArray(spec.data)) {
      const handler = this.dataSources.get('inline');
      return await handler(spec);
    }

    // HTTP data (URL string)
    if (typeof spec.data === 'string' && (spec.data.startsWith('http://') || spec.data.startsWith('https://'))) {
      const handler = this.dataSources.get('http');
      return await handler(spec);
    }

    // Object with provider property - plugin data source
    if (spec.data && typeof spec.data === 'object' && spec.data.provider) {
      const handler = this.dataSources.get(spec.data.provider);
      if (!handler) {
        throw new Error(`Unknown data source provider: ${spec.data.provider}`);
      }
      // Pass hooks to the handler
      return await handler(spec.data, { hooks: this.hooks });
    }

    console.error('🔴🔴🔴 [ChartML Core] DATA SOURCE RESOLUTION FAILED 🔴🔴🔴');
    console.error('[ChartML Core] Full spec:', JSON.stringify(spec, null, 2));
    console.error('[ChartML Core] spec.data value:', spec.data);
    console.error('[ChartML Core] spec.data type:', typeof spec.data);
    console.error('[ChartML Core] Type checks:', {
      hasData: !!spec.data,
      isObject: typeof spec.data === 'object',
      hasProvider: spec.data?.provider,
      hasRows: spec.data?.rows,
      isArray: Array.isArray(spec.data),
      isString: typeof spec.data === 'string'
    });
    throw new Error('Unable to resolve data source. Provide "data:" as either a string (source reference), array (inline rows), or object with "provider" property.');
  }

  /**
   * Calculate dimensions for chart rendering
   * DOES NOT modify or return style - only calculates width/height
   *
   * @param {Object} spec - Chart specification
   * @param {HTMLElement} [container] - Optional container to read width from for responsive sizing
   * @returns {Object} - {width, height} calculated dimensions
   */
  _calculateDimensions(spec, container = null) {
    let style = spec.visualize?.style || {};

    // Resolve style reference if needed
    if (typeof style === 'string') {
      const registeredStyle = this.registry.resolveStyle(style);
      if (!registeredStyle) {
        throw new Error(`Style "${style}" not found. Did you register it first?`);
      }
      style = registeredStyle;
    }

    // Read container width for responsive sizing
    const containerWidth = container?.offsetWidth || 600;

    // Check if chart renderer provides custom default dimensions
    let defaultHeight = 400;
    const chartType = spec.visualize?.type;
    if (chartType) {
      const renderer = this.chartRenderers.get(chartType);
      if (renderer?.getDefaultDimensions) {
        try {
          const pluginDefaults = renderer.getDefaultDimensions(spec, container);
          if (pluginDefaults?.height) {
            defaultHeight = pluginDefaults.height;
            console.log(`[ChartML] Using plugin default height for "${chartType}":`, defaultHeight);
          }
        } catch (error) {
          console.warn(`[ChartML] Plugin dimension provider for "${chartType}" failed:`, error);
        }
      }
    }

    return {
      width: style.width || containerWidth,
      height: style.height || defaultHeight
    };
  }

  /**
   * Render ChartML specification into a DOM container
   *
   * Supports both ChartML v1.0 components (with type field) and legacy format.
   * For v1.0 components:
   * - source/style/config components are registered, not rendered
   * - chart components are rendered with reference resolution
   *
   * @param {string|object} spec - ChartML YAML string or parsed object
   * @param {HTMLElement} container - DOM element to render into
   * @param {Object} options - Rendering options
   * @param {HTMLElement} [options.filterContainer] - Optional separate container for filter controls
   * @param {Object} [options.filterValues] - Initial filter values to override defaults
   * @param {string} [options.paramsClassName] - Optional CSS classes for params container (e.g., Tailwind classes)
   * @returns {Object|null} Component info if registered, null if rendered
   *
   * @example
   * // Register a source
   * chartml.render(`
   *   type: source
   *   name: sales_data
   *   provider: inline
   *   data: [...]
   * `, container); // Returns component info, doesn't render
   *
   * // Render a chart that references the source
   * await chartml.render(`
   *   type: chart
   *   dataSource: sales_data
   *   visualize:
   *     type: bar
   *     columns: month
   *     rows: revenue
   * `, container); // Renders the chart
   *
   * // Render with filters
   * await chartml.render(spec, chartContainer, {
   *   filterContainer: document.getElementById('filters')
   * });
   */
  async render(spec, container, options = {}) {
    // Parse YAML if string
    let parsedSpec = typeof spec === 'string' ? yaml.load(spec) : spec;

    // Note: Array handling should be done by markdown plugins (react/markdown-it)
    // Core only handles single components to avoid duplication of grid layout logic

    // Check if this is a ChartML v1.0 component with type field
    if (parsedSpec.type) {
      try {
        const component = parseComponent(typeof spec === 'string' ? spec : yaml.dump(spec));

        // If it's a source, style, or config, register it (no UI)
        if (component.type === COMPONENT_TYPES.SOURCE ||
            component.type === COMPONENT_TYPES.STYLE ||
            component.type === COMPONENT_TYPES.CONFIG) {
          return this.registerComponent(spec);
        }

        // For params blocks, register AND render UI
        if (component.type === COMPONENT_TYPES.PARAMS) {
          this.registerComponent(spec);

          // Render parameter controls
          const paramsSpec = component.spec;
          const paramValues = this.registry.getParamValues(paramsSpec.name);

          renderParams(
            paramsSpec.params,
            paramValues,
            (paramId, newValue) => {
              // Update param value in registry
              this.registry.setParamValue(paramsSpec.name, paramId, newValue);
            },
            container,
            options.paramsClassName || ''
          );

          return component;
        }

        // For chart components, continue to rendering with reference resolution
        // (parsedSpec already has the chart spec)
      } catch (error) {
        // If parsing fails, show error in container (XSS-safe)
        container.innerHTML = '';
        const errorDiv = document.createElement('div');
        errorDiv.style.cssText = 'padding: 1rem; background: #fef2f2; color: #991b1b; border-left: 4px solid #dc2626; border-radius: 4px;';

        const errorLabel = document.createElement('strong');
        errorLabel.textContent = 'ChartML Error: ';

        const errorText = document.createTextNode(error.message);

        errorDiv.appendChild(errorLabel);
        errorDiv.appendChild(errorText);
        container.appendChild(errorDiv);
        throw error;
      }
    }

    // === PARAMETER RESOLUTION ===
    let paramsDefinition = null;
    let paramValues = {};

    // Keep original unresolved spec for re-resolution when params change
    const originalSpec = parsedSpec;

    // Resolve params reference
    if (parsedSpec.params && typeof parsedSpec.params === 'string') {
      paramsDefinition = this.registry.resolveParams(parsedSpec.params);
      if (!paramsDefinition) {
        throw new Error(`Params "${parsedSpec.params}" not found. Register it first.`);
      }
      paramValues = this.registry.getParamValues(parsedSpec.params);

      // Apply parameter resolution ($params.field substitution)
      parsedSpec = resolveParamReferences(parsedSpec, paramValues);
    } else if (parsedSpec.params && Array.isArray(parsedSpec.params)) {
      // Chart-level inline params (not a named reference)
      paramsDefinition = { params: parsedSpec.params };

      // Extract default values from param definitions
      paramValues = {};
      parsedSpec.params.forEach(param => {
        if (param.default !== undefined) {
          paramValues[param.id] = param.default;
        }
      });

      // Apply parameter resolution ($param_id substitution)
      parsedSpec = resolveParamReferences(parsedSpec, paramValues, parsedSpec.params);
    }

    // Store containers and params for re-rendering on filter changes
    this.paramsDefinition = paramsDefinition;
    this.paramValues = paramValues;

    // Create wrapper structure for chart-level params (if no filterContainer provided)
    let chartContainer = container;
    let paramsContainer = options.filterContainer;

    if (paramsDefinition && paramsDefinition.params && !options.filterContainer) {
      // Chart-level params without separate filterContainer - create wrapper structure
      container.innerHTML = '';

      // Create params container at top
      paramsContainer = document.createElement('div');
      paramsContainer.className = 'chartml-params-container';
      container.appendChild(paramsContainer);

      // Create chart container below
      chartContainer = document.createElement('div');
      chartContainer.className = 'chartml-chart-container chartml-chart';

      // Set minHeight on chartContainer for proper loading indicator centering
      try {
        const { height } = this.getExpectedDimensions(parsedSpec);
        chartContainer.style.minHeight = `${height}px`;
      } catch (err) {
        chartContainer.style.minHeight = '400px';  // Fallback
      }

      container.appendChild(chartContainer);
    } else {
      // No chart-level params - add chartml-chart class to container
      chartContainer.classList.add('chartml-chart');
    }

    this.chartContainer = chartContainer;
    this.filterContainer = paramsContainer || container;

    // Render params UI if params exist
    if (paramsDefinition && paramsDefinition.params) {
      const targetContainer = paramsContainer || container;
      renderParams(
        paramsDefinition.params,
        paramValues,
        (paramId, newValue) => {
          // Update param value
          paramValues[paramId] = newValue;

          // If using registry params (not inline), update registry
          if (originalSpec.params && typeof originalSpec.params === 'string') {
            this.registry.setParamValue(originalSpec.params, paramId, newValue);
          }

          // Re-resolve spec with new param values
          const resolvedSpec = Array.isArray(originalSpec.params)
            ? resolveParamReferences(originalSpec, paramValues, originalSpec.params)
            : resolveParamReferences(originalSpec, paramValues);

          // Re-render chart with newly resolved spec
          this._renderChartWithParams(chartContainer, { ...options, spec: resolvedSpec });
        },
        targetContainer,
        options.paramsClassName || ''
      );
    }

    // Render the chart and get metadata (includes resolved dimensions with plugin defaults)
    // Pass spec explicitly instead of relying on this.currentSpec
    const chartMetadata = await this._renderChartWithParams(chartContainer, { ...options, spec: parsedSpec });

    // Return Chart instance for programmatic control (refresh, getMetadata)
    // Pass ORIGINAL spec (before param resolution) so Chart can re-resolve on rerender
    return new Chart(this, originalSpec, container, options, chartMetadata);
  }

  /**
   * Internal method to render the chart with parameter resolution
   * (separated to allow re-rendering on param changes)
   * @private
   *
   * ARCHITECTURE: Two-Layer System
   * Layer 1: Immutable Spec (this.currentSpec) - NEVER modified
   * Layer 2: Render Context (local variable) - Built fresh each render, discarded after
   */
  async _renderChartWithParams(container, options = {}) {
    // Show loading indicator only if NOT a refresh (refresh shows spinner on button instead)
    const loadingIndicator = options.bypassCache ? null : showLoadingIndicator(container, this.loadingIndicator);

    try {
      // === LAYER 1: IMMUTABLE SPEC ===
      // This is the source of truth. NEVER mutate it.
      // Spec MUST be passed in options - no global spec fallback
      const immutableSpec = options.spec;
      if (!immutableSpec) {
        throw new Error('[ChartML] _renderChartWithParams requires spec in options');
      }

      // === LAYER 2: RENDER CONTEXT (Ephemeral - rebuilt every render) ===
      const context = {
        // Reference to immutable spec
        spec: immutableSpec,

        // Extract source name from immutable spec (for refresh coordination)
        sourceName: null,

        // Resolved spec with params applied (new object, doesn't mutate original)
        resolvedSpec: null,

        // Resolved data source definition (for cache keys)
        resolvedDataSource: null,

        // Processed data from middleware
        processedData: null,

        // Calculated dimensions
        dimensions: null,

        // Metadata from data source and middleware
        metadata: {}
      };

      // Extract source name from IMMUTABLE spec (before any resolution)
      if (typeof immutableSpec.data === 'string' && !immutableSpec.data.startsWith('http://') && !immutableSpec.data.startsWith('https://')) {
        context.sourceName = immutableSpec.data;
      } else if (typeof immutableSpec.dataSource === 'string') {
        context.sourceName = immutableSpec.dataSource;
      }

      // === PARAMETER RESOLUTION ===
      // Get ALL param values from registry (all registered params blocks)
      // This creates a flat object: { "dashboard_filters.region": ["US"], "dashboard_filters.status": "active" }
      const allParamValues = {};

      // Get all registered params block names
      const registry = this.registry;
      if (registry && registry.params) {
        for (const [scopeName, paramsBlock] of registry.params) {
          const values = paramsBlock.values || {};

          // Convert to dot notation: { region: "US" } → { "dashboard_filters.region": "US" }
          for (const [paramId, value] of Object.entries(values)) {
            allParamValues[`${scopeName}.${paramId}`] = value;
          }
        }
      }

      // Resolve param references in spec (creates NEW object, doesn't mutate original)
      // Handles both: $dashboard_filters.region (named) and $top_n (chart-level)
      context.resolvedSpec = resolveParamReferences(
        immutableSpec,
        allParamValues,
        immutableSpec.params  // Chart-level inline params (if any)
      );

      // Resolve named data source reference (for cache key generation)
      if (typeof context.resolvedSpec.data === 'string' && this.registry) {
        const sourceName = context.resolvedSpec.data;
        context.resolvedDataSource = this.registry.resolveSource(sourceName);
      }

      // Create lazy data fetch callback (only called if middleware cache misses)
      const fetchData = async () => {
        return await this._resolveDataSource(context.resolvedSpec, options);
      };

      // Apply aggregate middleware with lazy data fetching
      // Middleware can cache and skip data source entirely on cache hits
      const result = await this._applyAggregate(fetchData, context.resolvedSpec, {
        hooks: this.hooks,
        bypassCache: options.bypassCache,
        resolvedDataSource: context.resolvedDataSource
      });

      // Handle Result Object pattern: { data, metadata } or plain data array
      context.processedData = result?.data !== undefined ? result.data : result;
      context.metadata = result?.metadata || {};

      // Calculate dimensions (does NOT mutate spec)
      context.dimensions = this._calculateDimensions(context.resolvedSpec, container);

      // Create instance config to pass runtime defaults to mapper (does NOT mutate spec)
      const instanceConfig = {
        defaultPalette: this.defaultPalette,
        dimensions: context.dimensions
      };

      // Map ChartML to D3 config (pass title and instanceConfig)
      const { chartType, config, data: chartData} = mapChartMLToD3Config(
        context.resolvedSpec.visualize,
        context.processedData,
        context.resolvedSpec.title,
        instanceConfig
      );

      // Check if a renderer is registered (instance first, then global registry)
      let renderer = this.chartRenderers.get(chartType);
      if (!renderer) {
        // Check global registry for auto-registered plugins
        renderer = globalRegistry.getChartRenderer(chartType);
      }
      if (!renderer) {
        throw new Error(
          `No renderer registered for chart type: ${chartType}. ` +
          `Register a renderer using: chartml.registerChartRenderer('${chartType}', rendererFunction) ` +
          `or import a plugin that auto-registers (e.g., import '@chartml/chart-pie')`
        );
      }

      // Call the registered renderer
      renderer(container, chartData, config);

      // Render title if present (after chart renderer, so it doesn't get cleared)
      // Insert at the beginning of the container
      if (context.resolvedSpec.title) {
        const titleDiv = document.createElement('div');
        titleDiv.className = 'chart-title';
        titleDiv.style.cssText = 'font-size: 16px; font-weight: 600; color: #1f2937; margin-bottom: 8px;';
        titleDiv.textContent = context.resolvedSpec.title;
        container.insertBefore(titleDiv, container.firstChild);
      }

      // Hide loading indicator after successful render
      hideLoadingIndicator(loadingIndicator);

      // Build metadata for Chart instance
      // CRITICAL: Include sourceName so Chart constructor doesn't need to extract it
      const chartMetadata = {
        ...context.metadata,
        dimensions: context.dimensions,
        sourceName: context.sourceName  // Pass extracted source name
      };

      // Return chart-specific metadata
      // Context is discarded after this - it was ephemeral
      return chartMetadata;
    } catch (error) {
      // Hide loading indicator on error
      hideLoadingIndicator(loadingIndicator);
      throw error;
    }
  }

  /**
   * Register built-in chart renderers
   * Only cartesian charts (bar, line, area) are built-in.
   * All other chart types (pie, scatter, metric, table, etc.) are plugins.
   * @private
   */
  _registerBuiltInChartRenderers() {
    // Cartesian charts (bar, line, area - can be mixed)
    this.registerChartRenderer('cartesian', (container, data, config) => {
      renderD3CartesianChart(container, data, config);
    });
  }

  /**
   * Get expected dimensions from a ChartML spec without rendering
   *
   * This method calculates the dimensions a chart will have when rendered,
   * allowing wrappers to pre-allocate container space and prevent layout shift
   * during data loading.
   *
   * The dimension calculation follows this priority:
   * 1. Explicit dimensions in spec (visualize.style.width/height)
   * 2. Plugin-provided defaults (via renderer.getDefaultDimensions())
   * 3. ChartML defaults (width: 600px, height: 400px)
   *
   * Note: This is a static method and cannot access plugin defaults.
   * Use the instance method getExpectedDimensions() for full plugin support.
   *
   * @param {string|Object} spec - ChartML specification (YAML string or object)
   * @returns {{width: number|null, height: number}} Expected dimensions
   *          width is null (responsive) unless explicitly set in spec
   *
   * @example
   * // Static method (no plugin defaults)
   * const { width, height } = ChartML.getExpectedDimensions(spec);
   * container.style.minHeight = `${height}px`;
   *
   * @example
   * // Instance method (includes plugin defaults)
   * const chartml = new ChartML();
   * const { width, height } = chartml.getExpectedDimensions(spec);
   * container.style.minHeight = `${height}px`;
   */
  static getExpectedDimensions(spec) {
    try {
      // Parse YAML if string
      const parsedSpec = typeof spec === 'string' ? yaml.load(spec) : spec;

      // Check if title is present (title adds ~32px: 16px font + line-height + 8px margin)
      const hasTitle = !!parsedSpec?.title;
      const titleHeight = hasTitle ? 32 : 0;

      // Extract dimensions from visualize.style
      const style = parsedSpec.visualize?.style || {};

      return {
        width: style.width || null,   // Width is responsive unless explicitly set
        height: (style.height || 400) + titleHeight    // Default height: 400px + title
      };
    } catch (error) {
      // If parsing fails, return defaults
      console.warn('[ChartML] Failed to parse dimensions from spec:', error);
      return { width: null, height: 400 };
    }
  }

  /**
   * Get expected dimensions from a ChartML spec (instance method with plugin support)
   *
   * This instance method provides the full dimension calculation including plugin defaults.
   * Prefer this over the static method when you have a ChartML instance available.
   *
   * The dimension calculation follows this priority:
   * 1. Explicit dimensions in spec (visualize.style.width/height)
   * 2. Plugin-provided defaults (via renderer.getDefaultDimensions())
   * 3. ChartML defaults (width: null/responsive, height: 400px)
   *
   * @param {string|Object} spec - ChartML specification (YAML string or object)
   * @returns {{width: number|null, height: number}} Expected dimensions
   *          width is null (responsive) unless explicitly set in spec
   *
   * @example
   * const chartml = new ChartML();
   * chartml.registerChartRenderer('metric', metricRenderer);
   * const { width, height } = chartml.getExpectedDimensions(metricSpec);
   * container.style.minHeight = `${height}px`;  // Uses plugin's getDefaultDimensions()
   */
  getExpectedDimensions(spec) {
    try {
      // Parse YAML if string
      const parsedSpec = typeof spec === 'string' ? yaml.load(spec) : spec;

      // Check if title is present (title adds ~32px: 16px font + line-height + 8px margin)
      const hasTitle = !!parsedSpec?.title;
      const titleHeight = hasTitle ? 32 : 0;

      // Priority 1: Check explicit dimensions in spec
      const explicitHeight = parsedSpec?.visualize?.style?.height;
      const explicitWidth = parsedSpec?.visualize?.style?.width;

      if (explicitHeight) {
        return {
          width: explicitWidth || null,
          height: explicitHeight + titleHeight
        };
      }

      // Priority 2: Check plugin default dimensions
      const chartType = parsedSpec?.visualize?.type;
      if (chartType && this.chartRenderers) {
        const renderer = this.chartRenderers.get(chartType);
        if (renderer?.getDefaultDimensions) {
          try {
            const pluginDefaults = renderer.getDefaultDimensions(parsedSpec, null);
            if (pluginDefaults?.height) {
              return {
                width: pluginDefaults.width || explicitWidth || null,
                height: pluginDefaults.height + titleHeight
              };
            }
          } catch (error) {
            console.warn(`[ChartML] Plugin dimension provider for "${chartType}" failed:`, error);
          }
        }
      }

      // Priority 3: Fallback to ChartML defaults
      return {
        width: explicitWidth || null,  // Responsive unless explicit
        height: 400 + titleHeight
      };
    } catch (error) {
      console.warn('[ChartML] Failed to parse dimensions from spec:', error);
      return { width: null, height: 400 };
    }
  }
}

/**
 * Create default loading indicator (simple spinner)
 * @returns {HTMLElement} Loading indicator element
 * @private
 */
function createDefaultLoadingIndicator() {
  const loader = document.createElement('div');
  loader.className = 'chartml-loading-indicator';
  loader.innerHTML = '<div class="chartml-spinner"></div>';
  return loader;
}

/**
 * Show loading indicator in container
 * @param {HTMLElement} container - Container to show loading indicator in
 * @param {Function|null} customIndicator - Optional custom loading indicator function
 * @returns {HTMLElement} The loading indicator element (for later removal)
 * @private
 */
function showLoadingIndicator(container, customIndicator) {
  // Use custom indicator if provided, otherwise use default spinner
  const indicator = customIndicator ? customIndicator() : createDefaultLoadingIndicator();

  // Make container position relative if not already positioned
  const computedPosition = window.getComputedStyle(container).position;
  if (computedPosition === 'static') {
    container.style.position = 'relative';
  }

  // Append indicator to container
  container.appendChild(indicator);

  return indicator;
}

/**
 * Hide loading indicator
 * @param {HTMLElement} indicator - Loading indicator element to remove
 * @private
 */
function hideLoadingIndicator(indicator) {
  if (indicator && indicator.parentNode) {
    indicator.parentNode.removeChild(indicator);
  }
}

/**
 * Convenience function to render a chart without creating a ChartML instance
 *
 * @param {string|object} spec - ChartML YAML string or parsed object
 * @param {HTMLElement} container - DOM element to render into
 *
 * @example
 * await renderChart(spec, document.getElementById('chart'));
 */
export async function renderChart(spec, container) {
  const chartml = new ChartML();
  await chartml.render(spec, container);
}

// Export utilities for advanced usage
export { getChartColors } from './colorUtils.js';
export { createFormatter } from './formatters.js';

// Configuration API
export { configure, resetConfig, getSystemDefaults } from './config.js';

// Component Registry API (ChartML v1.0)
export { createRegistry, getGlobalRegistry, resetGlobalRegistry, ComponentRegistry } from './registry.js';
export { parseComponent, parseMultipleComponents, extractReferences, COMPONENT_TYPES } from './componentParser.js';

// Plugin Registry API (Auto-registration)
export { globalRegistry } from './pluginRegistry.js';

// Aggregation API (Built-in d3-array aggregation)
export { d3Aggregate } from './aggregate.js';

// Parameter Resolution API
export { resolveParamReferences, extractParamReferences, validateParamReferences } from './parameterResolver.js';

// Tooltip Utilities (for custom chart renderers)
export { createChartTooltip, positionTooltip } from './tooltipUtils.js';

// Label Utilities (for axis labels, legends, collision detection)
export {
  measureLabelWidths,
  determineLabelStrategy,
  applyLabelStrategy,
  applyHorizontalLabels,
  applyRotatedLabels,
  applyTruncatedLabels,
  applySampledLabels,
  getStrategicIndices,
  DEFAULT_LABEL_FONT_SIZE,
  DEFAULT_LABEL_FONT_FAMILY
} from './labelUtils.js';
