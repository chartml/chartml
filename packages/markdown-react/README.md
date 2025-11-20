# @chartml/markdown-react

React-markdown plugin for rendering ChartML code blocks in markdown documents.

## Features

- ✅ **React-Markdown Integration** - Drop-in components for `react-markdown`
- ✅ **Multi-Chart Documents** - Render multiple charts in a single markdown file
- ✅ **Named Data Sources** - Define reusable data sources
- ✅ **Interactive Parameters** - Built-in parameter UI with state management
- ✅ **Grid Layouts** - Responsive 12-column grid system for dashboards
- ✅ **Layout Shift Prevention** - Pre-calculates heights to prevent content jumping
- ✅ **Resize Handling** - Automatically re-renders on container resize
- ✅ **Custom Wrappers** - Inject custom chrome around charts and params
- ✅ **All Chart Types** - Includes pie, scatter, and metric plugins
- ✅ **Error Handling** - Graceful error display for invalid specs

## Installation

```bash
npm install @chartml/markdown-react react-markdown
```

**Peer Dependencies:**
- `react` ^18.0.0
- `react-markdown` (any version)

## Quick Start

### Basic Usage

```jsx
import Markdown from 'react-markdown';
import { ChartMLCodeBlock } from '@chartml/markdown-react';
import { ChartML } from '@chartml/core';

// Create ChartML instance
const chartml = new ChartML();
const { code, pre } = ChartMLCodeBlock({ chartmlInstance: chartml });

function Dashboard() {
  const markdown = `
# Sales Dashboard

Here's our revenue trend:

\`\`\`chartml
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
\`\`\`
  `;

  return <Markdown components={{ code, pre }}>{markdown}</Markdown>;
}
```

## Advanced Features

### Multi-Chart Dashboards

Create dashboards with multiple charts using grid layouts:

````markdown
# Executive Dashboard

```chartml
type: chart
version: 1
layout:
  colSpan: 6  # Half width

data:
  - month: "Jan"
    revenue: 45000
  - month: "Feb"
    revenue: 52000

visualize:
  type: line
  columns: month
  rows: revenue
  style:
    title: "Revenue Trend"
---
type: chart
version: 1
layout:
  colSpan: 6  # Half width

data:
  - region: "North"
    sales: 1200
  - region: "South"
    sales: 950

visualize:
  type: pie
  columns: region
  rows: sales
  style:
    title: "Sales by Region"
```
````

### Named Data Sources

Define reusable data sources that multiple charts can reference:

````markdown
```chartml
type: source
name: sales_data
data:
  - month: "Jan"
    revenue: 45000
    costs: 32000
  - month: "Feb"
    revenue: 52000
    costs: 35000
  - month: "Mar"
    revenue: 48000
    costs: 33000
---
type: chart
version: 1

source: sales_data

visualize:
  type: bar
  columns: month
  rows: revenue
  style:
    title: "Revenue"
---
type: chart
version: 1

source: sales_data

visualize:
  type: line
  columns: month
  rows: costs
  style:
    title: "Costs"
```
````

### Interactive Parameters

Add interactive filters with the built-in parameter UI:

````markdown
```chartml
type: params
name: dashboard_filters
params:
  - id: selected_regions
    type: multiselect
    label: "Regions"
    default: ["North", "South", "East", "West"]
    options: ["North", "South", "East", "West"]
---
type: chart
version: 1

data:
  - region: "North"
    sales: 1200
  - region: "South"
    sales: 950
  - region: "East"
    sales: 1100
  - region: "West"
    sales: 880

aggregate:
  dimensions: [region]
  measures:
    - column: sales
      aggregation: sum
      name: total_sales
  filter: region IN $dashboard_filters.selected_regions

visualize:
  type: bar
  columns: region
  rows: total_sales
  style:
    title: "Sales by Region"
```
````

## API Reference

### `ChartMLCodeBlock(options)`

Creates custom components for react-markdown.

**Parameters:**

| Option | Type | Description |
|--------|------|-------------|
| `chartmlInstance` | `ChartML` | **Required.** ChartML instance with plugins registered |
| `containerClassName` | `string` | CSS class for chart containers (default: `'chartml-chart-container'`) |
| `chartWrapper` | `Component` | Custom wrapper component for charts |
| `paramsWrapper` | `Component` | Custom wrapper component for params blocks |

**Returns:** `{ code, pre }` - Components to pass to react-markdown

**Example:**

```jsx
const chartml = new ChartML();
const { code, pre } = ChartMLCodeBlock({
  chartmlInstance: chartml,
  containerClassName: 'my-chart-container'
});

<Markdown components={{ code, pre }}>{markdown}</Markdown>
```

### Custom Chart Wrapper

Add custom chrome (headers, buttons, etc.) around charts:

```jsx
function MyChartWrapper({ spec, chartmlInstance, onChartRender }) {
  const [chartInstance, setChartInstance] = React.useState(null);

  return (
    <div className="chart-card">
      <div className="chart-header">
        <h3>{spec.visualize?.style?.title}</h3>
        <button onClick={() => chartInstance?.refresh()}>
          Refresh
        </button>
      </div>
      <ChartMLChart
        spec={spec}
        chartmlInstance={chartmlInstance}
        onChartRender={setChartInstance}
      />
    </div>
  );
}

const { code, pre } = ChartMLCodeBlock({
  chartmlInstance: chartml,
  chartWrapper: MyChartWrapper
});
```

### Custom Params Wrapper

Replace the default parameter UI with your own:

```jsx
function MyParamsRenderer({ parameterDefinitions, scope, chartmlInstance }) {
  // Your custom parameter UI implementation
  return (
    <div className="my-params">
      {/* Custom parameter controls */}
    </div>
  );
}

const { code, pre } = ChartMLCodeBlock({
  chartmlInstance: chartml,
  paramsWrapper: MyParamsRenderer
});
```

### `ChartMLChart` Component

Exported for use in custom wrappers. Handles chart rendering with resize support.

**Props:**

| Prop | Type | Description |
|------|------|-------------|
| `spec` | `object` | ChartML specification object |
| `chartmlInstance` | `ChartML` | ChartML instance |
| `className` | `string` | CSS class for container |
| `onChartRender` | `function` | Callback receiving Chart instance: `(chartInstance) => void` |

## Grid Layout System

Use the `layout.colSpan` property to control chart width:

```yaml
layout:
  colSpan: 12  # Full width (default)
  colSpan: 6   # Half width
  colSpan: 4   # One-third width
  colSpan: 3   # One-quarter width
```

**Responsive Behavior:**
- Large screens: Uses specified colSpan
- Medium screens (768px-1024px): Reduces to max 6 columns
- Small screens (<768px): All charts full width (12 columns)

## Parameter System

### Parameter Types

**Text Input:**
```yaml
- id: search_term
  type: text
  label: "Search"
  default: ""
```

**Number Input:**
```yaml
- id: top_n
  type: number
  label: "Top N"
  default: 10
  min: 1
  max: 100
```

**Single Select:**
```yaml
- id: region
  type: select
  label: "Region"
  default: "North"
  options: ["North", "South", "East", "West"]
```

**Multi-Select:**
```yaml
- id: selected_regions
  type: multiselect
  label: "Regions"
  default: ["North", "South"]
  options: ["North", "South", "East", "West"]
```

**Date Range:**
```yaml
- id: date_range
  type: daterange
  label: "Date Range"
  default:
    start: "2024-01-01"
    end: "2024-12-31"
```

### Using Parameters in Charts

Reference params blocks by name:

```yaml
# Dashboard-level params (shared across charts)
type: params
name: dashboard_filters
params: [...]
---
# Use in chart
aggregate:
  filter: region IN $dashboard_filters.selected_regions
```

Or use chart-level params (local to one chart):

```yaml
type: chart
version: 1
params:  # Chart-specific params
  - id: top_n
    type: number
    default: 10

aggregate:
  sort: [...]
  limit: $top_n  # Reference without scope
```

## Examples

### Complete Dashboard Example

````markdown
# Sales Analytics Dashboard

```chartml
type: params
name: filters
params:
  - id: date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"
  - id: regions
    type: multiselect
    label: "Regions"
    default: ["All"]
    options: ["All", "North", "South", "East", "West"]
---
type: source
name: sales
data:
  - date: "2024-01-15"
    region: "North"
    revenue: 12000
  - date: "2024-01-15"
    region: "South"
    revenue: 9500
  # ... more data
---
type: chart
version: 1
layout:
  colSpan: 8

source: sales

aggregate:
  dimensions: [date]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
  filter: date >= $filters.date_range.start AND date <= $filters.date_range.end

visualize:
  type: line
  columns: date
  rows: total_revenue
  style:
    title: "Revenue Trend"
---
type: chart
version: 1
layout:
  colSpan: 4

source: sales

aggregate:
  dimensions: [region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
  filter: region IN $filters.regions

visualize:
  type: pie
  columns: region
  rows: total_revenue
  style:
    title: "Revenue by Region"
```
````

## Styling

### Default Classes

The plugin adds these CSS classes:

- `.chartml-chart-container` - Chart container
- `.chartml-error` - Error display
- `.grid` and `.grid-cols-12` - Grid layout
- `.col-span-*` - Column spanning classes

### Custom Styling

Override with your own CSS or use the `containerClassName` option:

```jsx
const { code, pre } = ChartMLCodeBlock({
  chartmlInstance: chartml,
  containerClassName: 'my-custom-chart'
});
```

```css
.my-custom-chart {
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 1rem;
  background: white;
}
```

## Best Practices

### 1. Reuse ChartML Instance

Create one ChartML instance per page and reuse it:

```jsx
const chartml = useMemo(() => new ChartML(), []);
const { code, pre } = useMemo(
  () => ChartMLCodeBlock({ chartmlInstance: chartml }),
  [chartml]
);
```

### 2. Use Named Sources for Shared Data

If multiple charts use the same data, define it once as a named source:

```yaml
type: source
name: shared_data
data: [...]
```

### 3. Layout Multi-Chart Dashboards

Use `colSpan` for side-by-side charts:

```yaml
layout:
  colSpan: 6  # Two charts per row
```

### 4. Namespace Parameters

Use meaningful names for params blocks:

```yaml
type: params
name: dashboard_filters  # Not just "params"
```

## Browser Support

- React 18+
- Modern browsers with SVG and CSS Grid support
- IE11+ (with polyfills)

## Documentation

- **ChartML Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **API Reference**: https://chartml.org/api

## License

MIT © 2025 Alytic Pty Ltd

## Powered By

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
