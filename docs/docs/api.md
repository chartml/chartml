# API Reference

Complete API documentation for ChartML packages.

## Installation

```bash
# Static site generators (VitePress, Docusaurus, etc.)
npm install @chartml/markdown-it

# React apps with react-markdown
npm install @chartml/markdown-react

# React apps (direct component usage)
npm install @chartml/react

# Vanilla JavaScript
npm install @chartml/core

# Chart plugins (automatically included in above packages)
npm install @chartml/chart-pie
npm install @chartml/chart-scatter
npm install @chartml/chart-metric
```

---

## @chartml/react

React wrapper component and hook for ChartML.

### ChartMLChart

React component for rendering ChartML specifications.

**Parameters:**

- `spec` (string | object) - ChartML specification (YAML string or object)
- `chartml` (ChartML) - Optional ChartML instance (for custom plugins)
- `options` (Object) - Options for ChartML instance (if not providing instance)
  - `onProgress` (Function) - Progress callback
  - `onCacheHit` (Function) - Cache hit callback
  - `onCacheMiss` (Function) - Cache miss callback
  - `onError` (Function) - Error callback
  - `palettes` (Object) - Custom color palettes
- `className` (string) - CSS class for container
- `style` (Object) - Inline styles for container

**Examples:**

```jsx
// Basic usage
import { ChartMLChart } from '@chartml/react';

function MyComponent() {
  const spec = `
    data:
      - month: Jan
        revenue: 45000
      - month: Feb
        revenue: 52000

    visualize:
      type: bar
      columns: month
      rows: revenue
  `;

  return <ChartMLChart spec={spec} />;
}
```

```jsx
// With custom ChartML instance (for plugins)
import { ChartMLChart } from '@chartml/react';
import { ChartML } from '@chartml/core';
import { createPieChartRenderer } from '@chartml/chart-pie';

function MyComponent() {
  const chartml = new ChartML();
  chartml.registerChartRenderer('pie', createPieChartRenderer());

  return <ChartMLChart spec={spec} chartml={chartml} />;
}
```

```jsx
// With event hooks
import { ChartMLChart } from '@chartml/react';

function MyComponent() {
  return (
    <ChartMLChart
      spec={spec}
      options={{
        onProgress: (event) => console.log(`Progress: ${event.percent}%`),
        onCacheHit: (event) => console.log('Cache hit!'),
        onError: (error) => console.error('Chart error:', error),
        palettes: {
          myPalette: ['#ff0000', '#00ff00', '#0000ff']
        }
      }}
    />
  );
}
```

### useChartML

React hook for creating and managing a ChartML instance with plugins.

**Parameters:**

- `options` (Object) - ChartML options

**Returns:** ChartML instance

**Example:**

```jsx
import { useChartML, ChartMLChart } from '@chartml/react';
import { createPieChartRenderer } from '@chartml/chart-pie';
import { createMetricRenderer } from '@chartml/chart-metric';
import { useEffect } from 'react';

function MyComponent() {
  const chartml = useChartML({
    onProgress: (e) => console.log(`Progress: ${e.percent}%`)
  });

  useEffect(() => {
    // Register custom chart renderers
    chartml.registerChartRenderer('pie', createPieChartRenderer());
    chartml.registerChartRenderer('metric', createMetricRenderer());
  }, [chartml]);

  return <ChartMLChart spec={spec} chartml={chartml} />;
}
```

---

## @chartml/core

Core ChartML library for vanilla JavaScript usage.

### ChartML Class

Main class for rendering ChartML specifications.

**Constructor:**

```javascript
import { ChartML } from '@chartml/core';

const chartml = new ChartML(options);
```

**Constructor Options:**

- `registry` (Object) - Global registry for shared data sources and renderers
- `defaultPalette` (string) - Default color palette name ('autumn_forest', 'spectrum_pro', 'horizon_suite')
- `loadingIndicator` (Function) - Custom loading indicator function
- `onProgress` (Function) - Progress event callback
- `onCacheHit` (Function) - Cache hit event callback
- `onCacheMiss` (Function) - Cache miss event callback
- `onError` (Function) - Error event callback

### render()

Renders a ChartML specification to a DOM container.

**Signature:**

```javascript
await chartml.render(spec, container);
```

**Parameters:**

- `spec` (string | object) - ChartML specification (YAML string or parsed object)
- `container` (HTMLElement) - DOM element to render into

**Returns:** Promise

**Example:**

```javascript
import { ChartML } from '@chartml/core';

const chartml = new ChartML();
const container = document.getElementById('chart');

const spec = `
type: chart
version: 1
title: "Monthly Revenue"

data:
  - month: Jan
    revenue: 45000
  - month: Feb
    revenue: 52000
  - month: Mar
    revenue: 61000

visualize:
  type: line
  columns: month
  rows: revenue
  axes:
    left:
      format: "$,.0f"
`;

await chartml.render(spec, container);
```

### registerChartRenderer()

Registers a custom chart type renderer.

**Signature:**

```javascript
chartml.registerChartRenderer(type, renderer);
```

**Parameters:**

- `type` (string) - Chart type name (e.g., 'pie', 'metric', 'custom')
- `renderer` (Function) - Renderer function `(container, data, config) => void`

**Example:**

```javascript
import { ChartML } from '@chartml/core';
import { createPieChartRenderer } from '@chartml/chart-pie';
import { createScatterPlotRenderer } from '@chartml/chart-scatter';
import { createMetricRenderer } from '@chartml/chart-metric';

const chartml = new ChartML();

// Register plugin renderers
chartml.registerChartRenderer('pie', createPieChartRenderer());
chartml.registerChartRenderer('doughnut', createPieChartRenderer());
chartml.registerChartRenderer('scatter', createScatterPlotRenderer());
chartml.registerChartRenderer('metric', createMetricRenderer());
```

**Custom Renderer Example:**

```javascript
// Create a custom chart renderer
function customRenderer(container, data, config) {
  // container: HTMLElement to render into
  // data: Array of data rows
  // config: Chart configuration object

  container.innerHTML = `
    <div class="custom-chart">
      <h3>${config.title || 'Chart'}</h3>
      <p>Data points: ${data.length}</p>
    </div>
  `;
}

chartml.registerChartRenderer('custom', customRenderer);
```

### registerDataSource()

Registers a custom data source handler.

**Signature:**

```javascript
chartml.registerDataSource(name, handler);
```

**Parameters:**

- `name` (string) - Data source provider name (e.g., 'bigquery', 'postgres', 'api')
- `handler` (Function) - Async handler function `(spec, context) => Promise<{ data, metadata }>`

**Example:**

```javascript
import { ChartML } from '@chartml/core';

const chartml = new ChartML();

// Register custom data source
chartml.registerDataSource('postgres', async (spec, context) => {
  const { query, connection } = spec;

  // Fetch data from PostgreSQL
  const response = await fetch('/api/query', {
    method: 'POST',
    body: JSON.stringify({ query, connection })
  });

  const data = await response.json();

  return {
    data: data.rows,
    metadata: {
      rowCount: data.rows.length,
      executionTime: data.executionTime
    }
  };
});

// Use in ChartML spec
const spec = `
type: chart
version: 1

data:
  provider: postgres
  connection: "my-db"
  query: "SELECT * FROM sales"

visualize:
  type: bar
  columns: product
  rows: revenue
`;

await chartml.render(spec, container);
```

### setAggregateMiddleware()

Sets custom aggregation middleware for transforming data.

**Signature:**

```javascript
chartml.setAggregateMiddleware(handler);
```

**Parameters:**

- `handler` (Function) - Async handler `(data, spec, context) => Promise<{ data, metadata }>`

**Example:**

```javascript
import { ChartML } from '@chartml/core';

const chartml = new ChartML();

// Custom aggregation middleware
chartml.setAggregateMiddleware(async (data, spec, context) => {
  // data: Array of raw data rows
  // spec: Aggregate specification (dimensions, measures, filters, etc.)
  // context: Execution context

  // Perform custom aggregation
  const aggregated = performCustomAggregation(data, spec);

  return {
    data: aggregated,
    metadata: {
      inputRows: data.length,
      outputRows: aggregated.length
    }
  };
});
```

### getExpectedDimensions()

Get expected chart dimensions without rendering (for layout pre-calculation).

**Signature:**

```javascript
const dimensions = chartml.getExpectedDimensions(spec);
```

**Parameters:**

- `spec` (string | object) - ChartML specification

**Returns:** Object with `{ width, height }` - width is null (responsive) unless explicitly set

**Example:**

```javascript
import { ChartML } from '@chartml/core';
import { createMetricRenderer } from '@chartml/chart-metric';

const chartml = new ChartML();
chartml.registerChartRenderer('metric', createMetricRenderer());

const spec = `
type: chart
version: 1

visualize:
  type: metric
  value: 52000
  format: "$,.0f"
`;

const { width, height } = chartml.getExpectedDimensions(spec);
container.style.minHeight = `${height}px`;  // Pre-allocate space
```

---

## Event Hooks

ChartML provides event hooks for monitoring rendering progress and handling events.

### onProgress

Called during data fetching and aggregation to report progress.

**Event Object:**

```javascript
{
  phase: 'data' | 'aggregate' | 'render',
  percent: 0-100,
  message: 'Loading data...'
}
```

**Example:**

```javascript
const chartml = new ChartML({
  onProgress: (event) => {
    console.log(`${event.phase}: ${event.percent}% - ${event.message}`);
    // Update progress bar UI
    progressBar.style.width = `${event.percent}%`;
  }
});
```

### onCacheHit

Called when data is loaded from cache instead of source.

**Event Object:**

```javascript
{
  source: 'data-source-name',
  cacheKey: 'abc123',
  age: 3600  // seconds since cached
}
```

**Example:**

```javascript
const chartml = new ChartML({
  onCacheHit: (event) => {
    console.log(`Cache hit for ${event.source} (${event.age}s old)`);
  }
});
```

### onCacheMiss

Called when data must be fetched from source.

**Event Object:**

```javascript
{
  source: 'data-source-name',
  reason: 'expired' | 'not-found'
}
```

**Example:**

```javascript
const chartml = new ChartML({
  onCacheMiss: (event) => {
    console.log(`Cache miss for ${event.source}: ${event.reason}`);
  }
});
```

### onError

Called when rendering fails.

**Parameters:**

- `error` (Error) - Error object

**Example:**

```javascript
const chartml = new ChartML({
  onError: (error) => {
    console.error('ChartML error:', error);
    // Show user-friendly error message
    showNotification('Failed to load chart', 'error');
  }
});
```

---

## Plugin Development

### Chart Renderer Interface

Chart renderers are functions that receive data and configuration and render visualizations.

**Signature:**

```javascript
function chartRenderer(container, data, config) {
  // Render chart into container
}
```

**Parameters:**

- `container` (HTMLElement) - DOM element to render into
- `data` (Array) - Array of data rows (already aggregated)
- `config` (Object) - Chart configuration from ChartML spec
  - `config.type` - Chart type
  - `config.title` - Chart title
  - `config.columns` - Column field(s)
  - `config.rows` - Row field(s)
  - `config.marks` - Additional encodings (color, size, text)
  - `config.axes` - Axis configuration
  - `config.style` - Style overrides
  - `config.palette` - Color palette array

**Optional: Default Dimensions**

Renderers can specify default dimensions:

```javascript
chartRenderer.getDefaultDimensions = () => ({
  width: null,   // null = responsive
  height: 150    // custom default height
});
```

**Example:**

```javascript
import * as d3 from 'd3';

export function createCustomRenderer() {
  function customRenderer(container, data, config) {
    const width = container.clientWidth;
    const height = config.style?.height || 400;

    // Create SVG
    const svg = d3.select(container)
      .append('svg')
      .attr('width', width)
      .attr('height', height);

    // Render your visualization
    // ... D3 rendering code ...
  }

  // Optional: specify default dimensions
  customRenderer.getDefaultDimensions = () => ({
    width: null,
    height: 300
  });

  return customRenderer;
}

// Register
chartml.registerChartRenderer('custom', createCustomRenderer());
```

### Data Source Interface

Data sources are async functions that fetch data from external systems.

**Signature:**

```javascript
async function dataSourceHandler(spec, context) {
  // Fetch data
  return { data, metadata };
}
```

**Parameters:**

- `spec` (Object) - Data source specification from ChartML
  - `spec.provider` - Provider name (your custom name)
  - `spec.*` - Any custom fields you define
- `context` (Object) - Execution context
  - `context.cache` - Cache interface (optional)
  - `context.registry` - Global registry (optional)

**Returns:** Promise resolving to:

```javascript
{
  data: Array,      // Array of data rows
  metadata: Object  // Optional metadata
}
```

**Example:**

```javascript
chartml.registerDataSource('rest-api', async (spec, context) => {
  const { url, headers = {}, method = 'GET' } = spec;

  const response = await fetch(url, {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...headers
    }
  });

  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
  }

  const data = await response.json();

  return {
    data: Array.isArray(data) ? data : data.results,
    metadata: {
      statusCode: response.status,
      contentType: response.headers.get('content-type')
    }
  };
});
```

### Aggregate Middleware Interface

Aggregate middleware transforms data (filtering, grouping, calculations).

**Signature:**

```javascript
async function aggregateMiddleware(data, spec, context) {
  // Transform data
  return { data, metadata };
}
```

**Parameters:**

- `data` (Array) - Raw data rows from data source
- `spec` (Object) - Aggregate specification
  - `spec.dimensions` - Array of dimension fields
  - `spec.measures` - Array of measure definitions
  - `spec.filters` - Filter rules
  - `spec.sort` - Sort configuration
  - `spec.limit` - Row limit
- `context` (Object) - Execution context

**Returns:** Promise resolving to:

```javascript
{
  data: Array,      // Transformed data rows
  metadata: Object  // Optional metadata
}
```

**Example:**

```javascript
chartml.setAggregateMiddleware(async (data, spec, context) => {
  const { dimensions, measures, filters, sort, limit } = spec;

  // Apply filters
  let filtered = filters ? applyFilters(data, filters) : data;

  // Group by dimensions
  const grouped = groupBy(filtered, dimensions);

  // Calculate measures
  const aggregated = grouped.map(group => {
    const row = {};
    dimensions.forEach(dim => row[dim] = group.key[dim]);
    measures.forEach(measure => {
      row[measure.name] = aggregate(group.rows, measure);
    });
    return row;
  });

  // Sort
  const sorted = sort ? orderBy(aggregated, sort) : aggregated;

  // Limit
  const limited = limit ? sorted.slice(0, limit) : sorted;

  return {
    data: limited,
    metadata: {
      inputRows: data.length,
      outputRows: limited.length
    }
  };
});
```

---

## Global Registry

The global registry allows sharing data sources and chart renderers across multiple ChartML instances.

**Import:**

```javascript
import { globalRegistry } from '@chartml/core';
```

**Methods:**

- `globalRegistry.registerDataSource(name, handler)` - Register global data source
- `globalRegistry.registerChartRenderer(type, renderer)` - Register global chart renderer
- `globalRegistry.getDataSource(name)` - Get data source handler
- `globalRegistry.getChartRenderer(type)` - Get chart renderer

**Example:**

```javascript
import { globalRegistry } from '@chartml/core';
import { createPieChartRenderer } from '@chartml/chart-pie';

// Register globally (shared across all instances)
globalRegistry.registerChartRenderer('pie', createPieChartRenderer());

// All ChartML instances can now use pie charts
const chartml1 = new ChartML();
const chartml2 = new ChartML();
// Both can render pie charts without re-registering
```

---

## See Also

- [ChartML Specification](/spec) - Complete language specification
- [Examples](/examples) - Real-world usage examples
- [Quick Reference](/quick-reference) - Syntax cheatsheet
