# @chartml/chart-metric

Metric card chart renderer plugin for ChartML - displays KPI metrics with comparison indicators and trend colors.

## Installation

```bash
npm install @chartml/chart-metric
```

## Usage

```javascript
import { ChartML } from '@chartml/core';
import { createMetricRenderer } from '@chartml/chart-metric';

const chartml = new ChartML();
chartml.registerChartRenderer('metric', createMetricRenderer());

const spec = `
data:
  - current: 52000
    previous: 45000

visualize:
  type: metric
  value: current
  format: $,.0f
  compareWith: previous
  style:
    title: "Monthly Revenue"
`;

await chartml.render(spec, container);
```

## Features

- 📊 **Large value display** - Prominent metric value with responsive font sizing
- 📈 **Comparison indicators** - Up/down arrows with percentage change
- 🎨 **Trend coloring** - Green for positive, red for negative, gray for neutral
- 🔢 **D3 formatting** - Support for all D3 format specs (currency, percent, SI-prefix, etc.)
- 📱 **Responsive** - Adapts font size based on container width

## Format Specs

Supports all D3 format specifications:

- `.1%` - Percent with 1 decimal (e.g., "4.2%")
- `$,.2f` - Currency with thousands separator (e.g., "$52,000.00")
- `.3s` - SI-prefix notation (e.g., "52.0k", "1.2M")
- `d` - Integer (e.g., "52000")

See [D3 Format Documentation](https://github.com/d3/d3-format) for more options.

## ChartML Spec

```yaml
data:
  - current: 0.042
    previous: 0.038

visualize:
  type: metric
  value: current          # Field to display
  format: .1%             # D3 format spec
  compareWith: previous   # Optional comparison field
  align: center           # left | center | right
  showLabel: true         # Show title
  style:
    title: "Conversion Rate"
```

## License

MIT © Jason Adams
