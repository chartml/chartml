# @chartml/chart-pie

Pie and doughnut chart renderer plugin for ChartML.

## Features

- ✅ **Pie Charts** - Classic pie charts for showing proportions
- ✅ **Doughnut Charts** - Hollow center variant for modern dashboards
- ✅ **Interactive Tooltips** - Hover to see values and percentages
- ✅ **Responsive Legend** - Automatically positions legend (right or bottom) based on chart width
- ✅ **Hover Effects** - Smooth slice expansion on hover
- ✅ **Auto-Registration** - Automatically registers with ChartML on import
- ✅ **Lightweight** - Built on D3.js with no extra dependencies

## Installation

```bash
npm install @chartml/chart-pie
```

**Note:** Requires `@chartml/core` and `d3` as peer dependencies.

```bash
npm install @chartml/core d3
```

## Usage

### Auto-Registration (Recommended)

Simply import the package and both `pie` and `doughnut` chart types are automatically registered:

```javascript
import { ChartML } from '@chartml/core';
import '@chartml/chart-pie';  // Auto-registers pie and doughnut

const spec = `
type: chart
version: 1

data:
  - category: "North America"
    revenue: 45000
  - category: "Europe"
    revenue: 38000
  - category: "Asia Pacific"
    revenue: 52000
  - category: "Latin America"
    revenue: 18000

visualize:
  type: pie
  columns: category
  rows: revenue
  style:
    title: "Revenue by Region"
`;

const chartml = new ChartML();
await chartml.render(spec, document.getElementById('chart'));
```

### Manual Registration

If you prefer explicit registration:

```javascript
import { ChartML } from '@chartml/core';
import { createPieChartRenderer } from '@chartml/chart-pie';

const chartml = new ChartML();
chartml.registerChartRenderer('pie', createPieChartRenderer());
chartml.registerChartRenderer('doughnut', createPieChartRenderer());
```

## Chart Types

### Pie Chart

Classic pie chart showing proportions as slices of a circle:

```yaml
visualize:
  type: pie
  columns: category
  rows: value
```

### Doughnut Chart

Pie chart with a hollow center (60% inner radius):

```yaml
visualize:
  type: doughnut
  columns: category
  rows: value
```

## Features in Detail

### Responsive Legend

The legend automatically adapts to chart width:
- **Wide charts (≥400px)**: Legend positioned on the right side
- **Narrow charts (<400px)**: Legend positioned at the bottom in horizontal layout

### Interactive Tooltips

Hover over any slice to see:
- Category name
- Absolute value with locale formatting
- Percentage of total

### Hover Effects

- Slices smoothly expand by 5px on hover
- Opacity increases to full brightness
- Smooth 200ms transitions

### Color Palette

Uses ChartML's color palette system from `style.colors` or workspace/user defaults. Falls back to generating colors if needed.

## Examples

### Basic Pie Chart

```yaml
type: chart
version: 1

data:
  - product: "Widget A"
    sales: 1200
  - product: "Widget B"
    sales: 850
  - product: "Widget C"
    sales: 1450

visualize:
  type: pie
  columns: product
  rows: sales
  style:
    title: "Sales by Product"
```

### Doughnut Chart with Custom Colors

```yaml
type: chart
version: 1

data:
  - status: "Complete"
    count: 75
  - status: "In Progress"
    count: 20
  - status: "Blocked"
    count: 5

visualize:
  type: doughnut
  columns: status
  rows: count
  style:
    title: "Project Status"
    colors: ["#10b981", "#3b82f6", "#ef4444"]
```

### With Aggregation

```yaml
type: chart
version: 1

data:
  - region: "North"
    sales: 1200
  - region: "North"
    sales: 1350
  - region: "South"
    sales: 950
  - region: "South"
    sales: 1100

aggregate:
  dimensions: [region]
  measures:
    - column: sales
      aggregation: sum
      name: total_sales

visualize:
  type: pie
  columns: region
  rows: total_sales
  style:
    title: "Total Sales by Region"
```

## API

### `createPieChartRenderer()`

Creates a chart renderer function compatible with ChartML.

**Returns:** `Function` - Renderer function

**Renderer Parameters:**
- `container` (HTMLElement) - DOM element to render into
- `data` (Array) - Chart data
- `config` (Object) - Chart configuration
  - `categoryField` (string) - Field name for categories
  - `valueField` (string) - Field name for values
  - `type` (string) - Chart type ('pie' or 'doughnut')
  - `width` (number) - Chart width in pixels
  - `height` (number) - Chart height in pixels
  - `colors` (Array) - Color palette array

## Browser Support

- Modern browsers with SVG support
- IE11+ (with polyfills for ES6 features)

## Documentation

- **ChartML Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **API Reference**: https://chartml.org/api

## License

MIT © 2025 Alytic Pty Ltd

## Powered By

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
