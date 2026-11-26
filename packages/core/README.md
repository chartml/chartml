# @chartml/core

A declarative markup language for creating beautiful, interactive data visualizations.

## Features

- ✅ **Built-in Chart Types**: Bar, line, and area charts
- ✅ **Built-in Aggregation**: GROUP BY, SUM, AVG, COUNT, MIN, MAX using d3-array
- ✅ **Plugin System**: Extend with custom chart types, data sources, and aggregation engines
- ✅ **Inline Data**: Works with JSON arrays out of the box
- ✅ **HTTP Data**: Fetch data from REST APIs
- ✅ **Zero Extra Dependencies**: Only d3 and js-yaml (already included)
- ✅ **Lightweight**: ~21 KB gzipped

## Installation

```bash
npm install @chartml/core
```

## Quick Start

```javascript
import { ChartML } from '@chartml/core';

const spec = `
type: chart
version: 1
title: "Monthly Revenue"

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
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
`;

const chartml = new ChartML();
await chartml.render(spec, document.getElementById('chart'));
```

## Built-in Features

### Chart Types

**Built-in:**
- Bar charts (vertical, horizontal, grouped, stacked)
- Line charts (single and multi-series)
- Area charts (single, stacked, normalized)

**Available as Plugins:**
- `@chartml/chart-pie` - Pie and doughnut charts
- `@chartml/chart-scatter` - Scatter plots and bubble charts

### Aggregation

Built-in d3-array aggregation (no plugin needed):

```yaml
data:
  - region: "North"
    revenue: 1200
  - region: "North"
    revenue: 1350
  - region: "South"
    revenue: 950

transform:
  aggregate:
    dimensions: [region]
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
```

**Supported Aggregations:**
- `sum` - Sum values
- `avg` - Average values
- `count` - Count rows
- `min` - Minimum value
- `max` - Maximum value
- `first` - First value in group
- `last` - Last value in group

**Performance:**
- <1ms for 100 rows
- 2-5ms for 1,000 rows
- 20-50ms for 10,000 rows

### Data Sources

**Built-in:**
- `inline` - Inline JSON arrays (default)
- `http` - Fetch from REST APIs

**Example:**
```yaml
data:
  - month: "Jan"
    revenue: 45000
```

Or fetch from URL:
```yaml
type: source
name: sales_data
provider: http
url: "https://api.example.com/sales"
```

### Paginated tables

A `table` with more rows than `style.pageSize` renders one page at a time, with Previous and Next buttons. `tablePage` (from 0) in the render options picks the page and tells the renderer that a host is driving the pager; a page past the last throws. Leave it out for a static render (page 1, buttons disabled). The output is an HTML/SVG string, so the buttons need a controller once you mount it. `attachTablePager` calls you back with the page a pressed button goes to, and you render that page again:

```javascript
import { attachTablePager } from '@chartml/core';

let page = 0;
const show = (next) => {
  page = next;
  container.innerHTML = chartml.renderToSvg(spec, { tablePage: page });
};
show(0);
const detach = attachTablePager(container, show); // call detach() when you remove the container
```

A render without `tablePage` is static: its buttons stay disabled, even with a controller attached. Start a new spec at page 0. The controller also prepares any chart you render into the container later, and keeps focus on the button that was pressed. Without it (static SVG, markdown rendered to HTML, server-side rendering) the table shows its first page and the buttons are disabled.

## Plugin System

### Chart Renderer Plugins

```javascript
import { ChartML } from '@chartml/core';
import '@chartml/chart-pie';  // Auto-registers

const chartml = new ChartML();
// Pie charts now available
```

### Custom Data Source Plugins

```javascript
import { globalRegistry } from '@chartml/core';

globalRegistry.registerDataSource('postgresql', async (spec) => {
  // Your PostgreSQL implementation
  return rows;
});
```

### Custom Transform Plugins

```javascript
import { globalRegistry } from '@chartml/core';

globalRegistry.registerTransformMiddleware(async (data, spec) => {
  // Your custom transform logic
  return transformedData;
});
```

## Documentation

- **Website**: https://chartml.org
- **Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **Quick Reference**: https://chartml.org/quick-reference

## License

MIT © 2025 Alytic Pty Ltd

## Powered By

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
