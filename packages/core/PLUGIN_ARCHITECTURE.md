# ChartML Plugin Architecture

## Overview

ChartML is designed as a minimal core with a rich plugin ecosystem. The core handles parsing, orchestration, and configuration, while plugins provide data sources, aggregation engines, chart renderers, and framework integrations.

## Package Naming Convention

All ChartML packages follow a strict naming convention for clarity and discoverability:

### Core Package
- `@chartml/core` - Parser, orchestration, configuration system

### Data Source Plugins: `data-{provider}`
Fetch data from external sources

**Built-in:**
- `inline` - Inline JSON arrays (no plugin needed)
- `http` - HTTP/REST APIs (no plugin needed)

**Plugin examples:**
- `@chartml/data-postgres` - PostgreSQL databases
- `@chartml/data-mysql` - MySQL databases
- `@chartml/data-csv` - CSV file parsing
- `@chartml/data-json` - JSON file parsing

### Aggregation Middleware: `aggregate-{engine}`
Transform and aggregate data

**Built-in:**
- d3-array aggregation (no plugin needed) - GROUP BY, SUM/AVG/COUNT/MIN/MAX, filters, sort, limit

**Plugin examples:**
- Custom aggregation engines for specialized use cases

### Chart Renderers: `chart-{type}`
Render visualizations

**Important:** Plugin authors can specify any type name when registering. If two plugins register the same type, the last one wins (with a console warning).

**Naming Convention:**
- Official plugins use simple names: `pie`, `scatter`, `heatmap`
- Third-party plugins should namespace: `@yourorg/pie` or `yourorg-pie`

- `@chartml/chart-bar` - Bar charts
- `@chartml/chart-line` - Line charts
- `@chartml/chart-area` - Area charts
- `@chartml/chart-pie` - Pie/doughnut charts
- `@chartml/chart-scatter` - Scatter plots
- `@chartml/chart-table` - Data tables
- `@chartml/chart-metric` - Metric cards
- `@chartml/chart-heatmap` - Heatmaps
- `@chartml/chart-treemap` - Treemaps
- `@chartml/chart-sankey` - Sankey diagrams
- `@chartml/chart-gauge` - Gauge charts

### Framework Adapters
Integrate with UI frameworks
- `@chartml/react` - React components
- `@chartml/vue` - Vue components
- `@chartml/svelte` - Svelte components
- `@chartml/angular` - Angular components

### Markdown Integrations: `markdown-{parser}`
Integrate with markdown parsers
- `@chartml/markdown-it` - markdown-it plugin
- `@chartml/markdown-remark` - Remark plugin
- `@chartml/markdown-marked` - Marked plugin
- `@chartml/markdown-docusaurus` - Docusaurus plugin

### Themes (Future): `theme-{name}`
Pre-built style themes
- `@chartml/theme-material` - Material Design
- `@chartml/theme-bootstrap` - Bootstrap
- `@chartml/theme-corporate` - Corporate/enterprise
- `@chartml/theme-minimal` - Minimalist theme

## Plugin Types

### 1. Data Source Plugins

**Purpose**: Fetch data from external sources

**Interface**:
```javascript
/**
 * @param {Object} spec - ChartML data specification
 * @returns {Promise<Array>} Array of data rows
 */
async function dataSourceHandler(spec) {
  // Extract data from source
  return rows;
}
```

**Registration**:
```javascript
import { ChartML } from '@chartml/core';
import postgresSource from '@chartml/data-postgres';

const chartml = new ChartML();
chartml.registerDataSource('postgresql', postgresSource({
  host: 'localhost',
  database: 'mydb',
  user: 'user',
  password: 'pass'
}));
```

**Usage in ChartML**:
```yaml
type: source
name: sales_data
provider: postgresql
query: |
  SELECT date, revenue
  FROM sales
  WHERE date >= '2024-01-01'
```

### 2. Aggregation Middleware Plugins

**Purpose**: Transform and aggregate data

**Interface**:
```javascript
/**
 * @param {Array} data - Input data rows
 * @param {Object} aggregateSpec - ChartML aggregate specification
 * @returns {Promise<Array>} Transformed data rows
 */
async function aggregateMiddleware(data, aggregateSpec) {
  // Apply transformations
  return transformedData;
}
```

**Registration**:
```javascript
// Built-in d3-array aggregation is registered by default
// For custom aggregation, register your own middleware
import customMiddleware from '@mycompany/aggregate-custom';

chartml.registerAggregateMiddleware(customMiddleware());
```

**Usage in ChartML**:
```yaml
aggregate:
  group: [region, product]
  measures:
    - field: revenue
      aggregation: sum
      alias: total_revenue
  sort:
    - field: total_revenue
      direction: desc
  limit: 10
```

### 3. Chart Renderer Plugins

**Purpose**: Render visualizations

**Interface**:
```javascript
/**
 * @param {HTMLElement} container - DOM container
 * @param {Array} data - Processed data rows
 * @param {Object} config - Chart configuration
 */
function chartRenderer(container, data, config) {
  // Render visualization
}
```

**Registration**:
```javascript
import barRenderer from '@chartml/chart-bar';
import lineRenderer from '@chartml/chart-line';

chartml.registerChartRenderer('bar', barRenderer);
chartml.registerChartRenderer('line', lineRenderer);
```

**Usage in ChartML**:
```yaml
visualize:
  type: bar  # Delegates to registered 'bar' renderer
  columns: month
  rows: revenue
```

## Core Architecture

### Minimal Core Responsibilities

`@chartml/core` is intentionally minimal and focused on:

1. **YAML Parsing**: Parse ChartML specifications
2. **Component Registry**: Manage sources, styles, configs
3. **Configuration System**: Deep merge configuration hierarchy
4. **Plugin Orchestration**: Coordinate data sources, middleware, renderers
5. **Reference Resolution**: Resolve cross-component references

**What Core Does NOT Do**:
- ❌ Fetch data (delegated to data source plugins)
- ❌ Aggregate data (delegated to middleware plugins)
- ❌ Render charts (delegated to chart renderer plugins)

### Data Flow

```
ChartML Spec
    ↓
[YAML Parser] (Core)
    ↓
[Component Registry] (Core) → Resolve references
    ↓
[Data Source Plugin] → Fetch data
    ↓
[Aggregation Middleware Plugin] → Transform data
    ↓
[Configuration System] (Core) → Merge configs
    ↓
[Chart Renderer Plugin] → Render visualization
    ↓
DOM Container
```

## Plugin Development

### Creating a Data Source Plugin

**File**: `packages/chartml-data-example/src/index.js`

```javascript
/**
 * Example data source plugin
 */
export function createExampleDataSource(options = {}) {
  return async function exampleDataSource(spec) {
    // Validate spec
    if (!spec.query) {
      throw new Error('Example data source requires a query field');
    }

    // Fetch data using options
    const response = await fetch(options.apiUrl, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${options.getAccessToken()}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({ query: spec.query })
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const result = await response.json();
    return result.rows; // Must return array of objects
  };
}
```

**Package.json**:
```json
{
  "name": "@chartml/data-example",
  "version": "1.0.0",
  "main": "dist/index.js",
  "module": "dist/index.mjs",
  "peerDependencies": {
    "@chartml/core": "^1.0.0"
  }
}
```

### Creating an Aggregation Middleware Plugin

**File**: `packages/chartml-aggregate-example/src/index.js`

```javascript
/**
 * Example aggregation middleware plugin
 */
export function createExampleMiddleware(options = {}) {
  return async function exampleMiddleware(data, aggregateSpec) {
    if (!aggregateSpec) return data;

    // Apply grouping
    if (aggregateSpec.group) {
      data = applyGrouping(data, aggregateSpec.group, aggregateSpec.measures);
    }

    // Apply sorting
    if (aggregateSpec.sort) {
      data = applySorting(data, aggregateSpec.sort);
    }

    // Apply limit
    if (aggregateSpec.limit) {
      data = data.slice(0, aggregateSpec.limit);
    }

    return data;
  };
}

function applyGrouping(data, groupFields, measures) {
  // Implementation here
}

function applySorting(data, sortSpec) {
  // Implementation here
}
```

### Creating a Chart Renderer Plugin

**File**: `packages/chartml-chart-example/src/index.js`

```javascript
import * as d3 from 'd3';

/**
 * Example chart renderer plugin
 */
export function createExampleRenderer(options = {}) {
  return {
    render(container, data, config) {
      // Clear container
      container.innerHTML = '';

      // Create SVG
      const svg = d3.select(container)
        .append('svg')
        .attr('width', config.width)
        .attr('height', config.height);

      // Render chart using D3, Canvas, WebGL, etc.
      // ...

      return svg.node();
    },

    // Optional: cleanup when chart is removed
    destroy(container) {
      container.innerHTML = '';
    }
  };
}
```

## Bundle Size Strategy

### Problem: Monolithic Bundles

If all chart types are in core:
- `@chartml/core` = 100kb+ (all D3 code, all chart types)
- User only needs bar charts = Still loads 100kb

### Solution: Plugin-Based Loading

**Minimal Core**:
```javascript
import { ChartML } from '@chartml/core';  // 10kb (parser + orchestration)
```

**Load Only What You Need**:
```javascript
import barRenderer from '@chartml/chart-bar';      // 15kb
import tableRenderer from '@chartml/chart-table';  // 8kb

const chartml = new ChartML();
chartml.registerChartRenderer('bar', barRenderer);
chartml.registerChartRenderer('table', tableRenderer);

// Total: 33kb instead of 100kb
```

**Dynamic Loading** (Future):
```javascript
// Automatically load renderers on demand
const chartml = new ChartML({
  autoLoadRenderers: true,
  rendererBaseUrl: 'https://cdn.chartml.org/renderers/'
});

// When spec.visualize.type === 'bar', automatically fetches @chartml/chart-bar
```

## Alternative Rendering Engines

The plugin system allows multiple implementations of the same chart type:

```
@chartml/chart-bar           # D3-based (default)
@chartml/chart-bar-plotly    # Plotly.js-based
@chartml/chart-bar-chartjs   # Chart.js-based
@chartml/chart-bar-echarts   # Apache ECharts-based
```

**User Choice**:
```javascript
// Use D3 version
import barRenderer from '@chartml/chart-bar';

// Or use Plotly version
import barRenderer from '@chartml/chart-bar-plotly';

chartml.registerChartRenderer('bar', barRenderer);
```

**Benefit**: Users can choose their preferred rendering library without being locked in.

## Community Plugins

Third parties can create their own plugins:

```
@acme/chartml-chart-gantt        # Custom Gantt chart
@company/chartml-data-graphql    # GraphQL data source
@user/chartml-aggregate-sql      # SQL-based aggregations
```

**Installation**:
```bash
npm install @acme/chartml-chart-gantt
```

**Usage**:
```javascript
import ganttRenderer from '@acme/chartml-chart-gantt';

chartml.registerChartRenderer('gantt', ganttRenderer);
```

## Plugin Discovery

**NPM Search**:
```bash
# Find all ChartML data sources
npm search @chartml/data-

# Find all chart renderers
npm search @chartml/chart-

# Find all aggregation middleware
npm search @chartml/aggregate-
```

**Documentation Site**:
- Plugin directory at chartml.org/plugins
- Filter by category (data, aggregate, chart)
- Community ratings and downloads
- Installation guides and examples

## Best Practices

### For Plugin Authors

1. **Naming**: Follow the convention (`data-`, `aggregate-`, `chart-`)
2. **Type Names**:
   - Official plugins: Use simple names (`pie`, `scatter`)
   - Third-party: Use namespaced names (`@yourorg/pie`, `acme-pie`)
   - Avoid collisions: ChartML will warn if a type is already registered
3. **Peer Dependencies**: Declare `@chartml/core` as peer dependency
4. **TypeScript**: Provide type definitions
5. **Documentation**: Clear README with examples
6. **Testing**: Unit tests for all functionality
7. **Versioning**: Follow semantic versioning
8. **Bundle Size**: Keep plugins focused and minimal
9. **Error Handling**: Clear, actionable error messages

### For Plugin Users

1. **Install Only What You Need**: Don't install all plugins
2. **Tree Shaking**: Use ES modules for smaller bundles
3. **Dynamic Loading**: Consider lazy loading for large renderers
4. **Version Compatibility**: Match plugin versions with core
5. **Security**: Audit third-party plugins before use

## Future Enhancements

### Auto-Registration (Future)
```javascript
// Automatically detect and register installed plugins
import { ChartML } from '@chartml/core';

const chartml = new ChartML({ autoRegister: true });
// Scans node_modules for @chartml/* packages and registers them
```

### Plugin Templates (Future)
```bash
npm create @chartml/plugin my-chart-plugin
# Scaffolds a complete plugin package with tests, build, docs
```

## Conclusion

The plugin architecture makes ChartML:
- **Modular**: Load only what you need
- **Extensible**: Easy to add new features
- **Maintainable**: Clear separation of concerns
- **Community-Driven**: Anyone can contribute plugins
- **Future-Proof**: Easy to evolve without breaking changes

