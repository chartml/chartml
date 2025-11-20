# @chartml/react

React wrapper component for ChartML - render beautiful data visualizations in React with ease.

## Features

- ✅ **React Component** - Clean, idiomatic React API
- ✅ **useChartML Hook** - Manage ChartML instances with React hooks
- ✅ **Auto-Cleanup** - Automatic cleanup on unmount
- ✅ **Plugin Support** - Register custom chart renderers and data sources
- ✅ **Event Callbacks** - Progress, cache hit/miss, and error events
- ✅ **TypeScript Ready** - Includes TypeScript definitions
- ✅ **All Chart Types** - Includes pie, scatter, and metric chart plugins
- ✅ **Zero Config** - Works out of the box with sensible defaults

## Installation

```bash
npm install @chartml/react
```

**Peer Dependencies:**
- `react` ^18.0.0
- `react-dom` ^18.0.0

The core ChartML library is included automatically.

## Quick Start

### Basic Usage

```jsx
import { ChartMLChart } from '@chartml/react';

function SalesChart() {
  const spec = `
type: chart
version: 1

data:
  - month: "Jan"
    revenue: 45000
  - month: "Feb"
    revenue: 52000
  - month: "Mar"
    revenue: 48000

visualize:
  type: bar
  columns: month
  rows: revenue
  style:
    title: "Monthly Revenue"
  `;

  return <ChartMLChart spec={spec} />;
}
```

### With Styling

```jsx
<ChartMLChart
  spec={spec}
  className="my-chart"
  style={{ maxWidth: '800px', margin: '0 auto' }}
/>
```

### With Event Callbacks

```jsx
import { ChartMLChart } from '@chartml/react';
import { useState } from 'react';

function ChartWithStatus() {
  const [status, setStatus] = useState('idle');

  return (
    <div>
      <p>Status: {status}</p>
      <ChartMLChart
        spec={spec}
        options={{
          onProgress: (e) => setStatus(`Loading: ${e.percent}%`),
          onCacheHit: () => setStatus('Loaded from cache'),
          onCacheMiss: () => setStatus('Fetching data'),
          onError: (err) => setStatus(`Error: ${err.message}`)
        }}
      />
    </div>
  );
}
```

## Advanced Usage

### Using the useChartML Hook

For more control, use the `useChartML` hook to manage your own ChartML instance:

```jsx
import { ChartMLChart, useChartML } from '@chartml/react';
import { useEffect } from 'react';

function CustomChart() {
  const chartml = useChartML({
    onProgress: (e) => console.log(`Progress: ${e.percent}%`),
    palettes: {
      custom: ['#ff0000', '#00ff00', '#0000ff']
    }
  });

  // Register custom renderers if needed
  useEffect(() => {
    // Custom plugins would be registered here
    // chartml.registerChartRenderer('custom', myRenderer);
  }, [chartml]);

  return <ChartMLChart spec={spec} chartml={chartml} />;
}
```

### Dynamic Specs with State

```jsx
import { ChartMLChart } from '@chartml/react';
import { useState } from 'react';

function DynamicChart() {
  const [chartType, setChartType] = useState('bar');

  const spec = `
type: chart
version: 1

data:
  - month: "Jan"
    revenue: 45000
  - month: "Feb"
    revenue: 52000

visualize:
  type: ${chartType}
  columns: month
  rows: revenue
  `;

  return (
    <div>
      <button onClick={() => setChartType('bar')}>Bar</button>
      <button onClick={() => setChartType('line')}>Line</button>
      <button onClick={() => setChartType('area')}>Area</button>
      <ChartMLChart spec={spec} />
    </div>
  );
}
```

### Fetching Data from API

```jsx
import { ChartMLChart } from '@chartml/react';
import { useState, useEffect } from 'react';

function APIChart() {
  const [chartData, setChartData] = useState(null);

  useEffect(() => {
    fetch('/api/sales-data')
      .then(res => res.json())
      .then(data => {
        const spec = {
          type: 'chart',
          version: 1,
          data: data,
          visualize: {
            type: 'bar',
            columns: 'month',
            rows: 'revenue'
          }
        };
        setChartData(spec);
      });
  }, []);

  if (!chartData) return <div>Loading...</div>;

  return <ChartMLChart spec={chartData} />;
}
```

## All Chart Types Available

This package includes all ChartML chart plugins by default:

### Built-in Chart Types
- `bar` - Vertical and horizontal bar charts
- `line` - Single and multi-series line charts
- `area` - Stacked and normalized area charts

### Included Plugins
- `pie` - Pie charts (from @chartml/chart-pie)
- `doughnut` - Doughnut charts (from @chartml/chart-pie)
- `scatter` - Scatter plots and bubble charts (from @chartml/chart-scatter)
- `metric` - Metric cards for KPIs (from @chartml/chart-metric)

All types work out of the box - no additional imports needed!

## API Reference

### `<ChartMLChart>` Component

**Props:**

| Prop | Type | Description |
|------|------|-------------|
| `spec` | `string \| object` | ChartML specification (YAML string or object) |
| `chartml` | `ChartML` | Optional ChartML instance (for custom plugins) |
| `options` | `object` | Options for ChartML instance |
| `options.onProgress` | `function` | Progress callback: `(event) => void` |
| `options.onCacheHit` | `function` | Cache hit callback: `(event) => void` |
| `options.onCacheMiss` | `function` | Cache miss callback: `(event) => void` |
| `options.onError` | `function` | Error callback: `(error) => void` |
| `options.palettes` | `object` | Custom color palettes |
| `className` | `string` | CSS class for container |
| `style` | `object` | Inline styles for container |

**Example:**

```jsx
<ChartMLChart
  spec={mySpec}
  className="chart-container"
  style={{ maxWidth: '100%' }}
  options={{
    onProgress: (e) => console.log(e.percent),
    onError: (err) => console.error(err)
  }}
/>
```

### `useChartML()` Hook

Creates and manages a ChartML instance with React hooks.

**Parameters:**
- `options` (object, optional) - ChartML options

**Returns:** `ChartML` instance

**Example:**

```jsx
const chartml = useChartML({
  onProgress: (e) => console.log(`Loading: ${e.percent}%`),
  palettes: {
    custom: ['#ff0000', '#00ff00', '#0000ff']
  }
});
```

## Examples

### Pie Chart

```jsx
const pieSpec = `
type: chart
version: 1

data:
  - category: "Product A"
    sales: 1200
  - category: "Product B"
    sales: 850
  - category: "Product C"
    sales: 1450

visualize:
  type: pie
  columns: category
  rows: sales
  style:
    title: "Sales by Product"
`;

<ChartMLChart spec={pieSpec} />
```

### Scatter Plot

```jsx
const scatterSpec = `
type: chart
version: 1

data:
  - temperature: 65
    sales: 120
  - temperature: 70
    sales: 140
  - temperature: 75
    sales: 180

visualize:
  type: scatter
  columns: temperature
  rows: sales
  style:
    title: "Sales vs Temperature"
`;

<ChartMLChart spec={scatterSpec} />
```

### Metric Card

```jsx
const metricSpec = `
type: chart
version: 1

data:
  - value: 1234

visualize:
  type: metric
  rows: value
  style:
    title: "Total Revenue"
    format: "$,.0f"
`;

<ChartMLChart spec={metricSpec} />
```

## TypeScript Support

Type definitions are included:

```tsx
import { ChartMLChart, useChartML } from '@chartml/react';
import type { ChartML } from '@chartml/core';

interface ChartProps {
  data: any[];
}

function TypedChart({ data }: ChartProps) {
  const chartml: ChartML = useChartML();

  const spec = {
    type: 'chart' as const,
    version: 1,
    data,
    visualize: {
      type: 'bar' as const,
      columns: 'month',
      rows: 'revenue'
    }
  };

  return <ChartMLChart spec={spec} chartml={chartml} />;
}
```

## Best Practices

### 1. Memoize Spec Objects

If passing spec as an object (not string), memoize it to prevent unnecessary re-renders:

```jsx
import { useMemo } from 'react';

const spec = useMemo(() => ({
  type: 'chart',
  version: 1,
  data: chartData,
  visualize: { type: 'bar', columns: 'x', rows: 'y' }
}), [chartData]);

<ChartMLChart spec={spec} />
```

### 2. Use String Specs for Static Charts

For static charts, YAML strings are more readable:

```jsx
const spec = `
type: chart
version: 1
data: [...]
visualize: { type: bar }
`;
```

### 3. Handle Loading States

Always handle loading states for async data:

```jsx
if (!data) return <div>Loading chart...</div>;
return <ChartMLChart spec={buildSpec(data)} />;
```

### 4. Error Boundaries

Wrap charts in error boundaries for production:

```jsx
<ErrorBoundary fallback={<div>Chart failed to load</div>}>
  <ChartMLChart spec={spec} />
</ErrorBoundary>
```

## Browser Support

- React 18+
- Modern browsers with SVG support
- IE11+ (with polyfills)

## Documentation

- **ChartML Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **API Reference**: https://chartml.org/api

## License

MIT © 2025 Alytic Pty Ltd

## Powered By

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
