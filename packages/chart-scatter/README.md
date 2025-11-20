# @chartml/chart-scatter

Scatter plot and bubble chart renderer plugin for ChartML.

## Features

- ✅ **Scatter Plots** - Classic 2D scatter plots for correlation analysis
- ✅ **Bubble Charts** - Add a third dimension with variable-sized bubbles
- ✅ **Color Grouping** - Group data points by category with automatic color coding
- ✅ **Interactive Tooltips** - Hover to see detailed values for each point
- ✅ **Grid Lines** - Subtle grid lines for easier value reading
- ✅ **Hover Effects** - Points enlarge smoothly on hover (1.3x size)
- ✅ **Entrance Animation** - Staggered point appearance on initial render
- ✅ **Auto-Registration** - Automatically registers with ChartML on import
- ✅ **Responsive Legend** - Centered legend for color-grouped data

## Installation

```bash
npm install @chartml/chart-scatter
```

**Note:** Requires `@chartml/core` and `d3` as peer dependencies.

```bash
npm install @chartml/core d3
```

## Usage

### Auto-Registration (Recommended)

Simply import the package and the `scatter` chart type is automatically registered:

```javascript
import { ChartML } from '@chartml/core';
import '@chartml/chart-scatter';  // Auto-registers scatter

const spec = `
type: chart
version: 1

data:
  - temperature: 65
    ice_cream_sales: 120
  - temperature: 70
    ice_cream_sales: 140
  - temperature: 75
    ice_cream_sales: 180
  - temperature: 80
    ice_cream_sales: 210

visualize:
  type: scatter
  columns: temperature
  rows: ice_cream_sales
  style:
    title: "Ice Cream Sales vs Temperature"
  axes:
    bottom:
      label: "Temperature (°F)"
    left:
      label: "Sales ($)"
`;

const chartml = new ChartML();
await chartml.render(spec, document.getElementById('chart'));
```

### Manual Registration

If you prefer explicit registration:

```javascript
import { ChartML } from '@chartml/core';
import { createScatterPlotRenderer } from '@chartml/chart-scatter';

const chartml = new ChartML();
chartml.registerChartRenderer('scatter', createScatterPlotRenderer());
```

## Chart Variants

### Basic Scatter Plot

Two-dimensional scatter plot showing correlation between variables:

```yaml
visualize:
  type: scatter
  columns: x_variable
  rows: y_variable
```

### Bubble Chart (Size Dimension)

Add a third dimension using variable bubble sizes:

```yaml
visualize:
  type: scatter
  columns: revenue
  rows: profit
  series: company_size  # Controls bubble size
```

### Color-Grouped Scatter Plot

Group data points by category with color coding:

```yaml
visualize:
  type: scatter
  columns: height
  rows: weight
  color: gender  # Groups by category with different colors
```

### Combined: Bubble Chart with Color Groups

Use both size and color for maximum dimensionality:

```yaml
visualize:
  type: scatter
  columns: ad_spend
  rows: conversions
  series: deal_size      # Bubble size
  color: industry        # Color grouping
```

## Features in Detail

### Interactive Tooltips

Hover over any point to see:
- X-axis value with locale formatting
- Y-axis value with locale formatting
- Size field value (if bubble chart)
- Color group category (if grouped)

### Hover Effects

- Points smoothly enlarge to 1.3x size on hover
- Opacity increases to full brightness
- Stroke width increases for better visibility
- Smooth 200ms transitions

### Entrance Animation

- Points appear with staggered animation (20ms delay per point)
- Smooth 600ms growth from 0 to final size
- Creates engaging visual effect on initial render

### Grid Lines

- Subtle semi-transparent grid lines (10% opacity)
- Aligned to axis ticks
- Helps read values without cluttering the chart

### Color Palette

Uses ChartML's color palette system from `style.colors` or workspace/user defaults. First color used for non-grouped data, full palette for color grouping.

## Examples

### Basic Scatter Plot

```yaml
type: chart
version: 1

data:
  - study_hours: 2
    test_score: 65
  - study_hours: 3
    test_score: 72
  - study_hours: 5
    test_score: 85
  - study_hours: 7
    test_score: 92

visualize:
  type: scatter
  columns: study_hours
  rows: test_score
  style:
    title: "Study Time vs Test Scores"
  axes:
    bottom:
      label: "Hours Studied"
    left:
      label: "Test Score (%)"
```

### Bubble Chart with Custom Size Range

```yaml
type: chart
version: 1

data:
  - ad_spend: 5000
    conversions: 120
    deal_size: 50000
  - ad_spend: 8000
    conversions: 180
    deal_size: 125000
  - ad_spend: 12000
    conversions: 240
    deal_size: 200000

visualize:
  type: scatter
  columns: ad_spend
  rows: conversions
  series: deal_size
  style:
    title: "Marketing Performance"
    radiusRange: [5, 30]  # Control min/max bubble size
  axes:
    bottom:
      label: "Ad Spend ($)"
    left:
      label: "Conversions"
```

### Color-Grouped Scatter Plot

```yaml
type: chart
version: 1

data:
  - height: 165
    weight: 60
    gender: "Female"
  - height: 170
    weight: 65
    gender: "Female"
  - height: 175
    weight: 75
    gender: "Male"
  - height: 180
    weight: 82
    gender: "Male"

visualize:
  type: scatter
  columns: height
  rows: weight
  color: gender
  style:
    title: "Height vs Weight by Gender"
    colors: ["#ec4899", "#3b82f6"]
  axes:
    bottom:
      label: "Height (cm)"
    left:
      label: "Weight (kg)"
```

### Full Example: Bubble Chart with Grouping

```yaml
type: chart
version: 1

data:
  - region: "North America"
    customers: 1200
    revenue: 45000
    avg_deal: 1500
  - region: "Europe"
    customers: 950
    revenue: 38000
    avg_deal: 1200
  - region: "Asia Pacific"
    customers: 1800
    revenue: 52000
    avg_deal: 900

visualize:
  type: scatter
  columns: customers
  rows: revenue
  series: avg_deal
  color: region
  style:
    title: "Regional Performance Analysis"
    radiusRange: [10, 35]
  axes:
    bottom:
      label: "Customer Count"
    left:
      label: "Revenue ($)"
      format: "$,.0f"
```

## API

### `createScatterPlotRenderer()`

Creates a chart renderer function compatible with ChartML.

**Returns:** `Function` - Renderer function

**Renderer Parameters:**
- `container` (HTMLElement) - DOM element to render into
- `data` (Array) - Chart data
- `config` (Object) - Chart configuration
  - `xField` (string) - Field name for x-axis (default: 'x')
  - `yField` (string) - Field name for y-axis (default: 'y')
  - `sizeField` (string, optional) - Field for bubble size
  - `colorField` (string, optional) - Field for color grouping
  - `width` (number) - Chart width in pixels (default: 600)
  - `height` (number) - Chart height in pixels (default: 400)
  - `xAxisLabel` (string, optional) - X-axis label
  - `yAxisLabel` (string, optional) - Y-axis label
  - `colors` (Array) - Color palette array
  - `defaultRadius` (number) - Default point radius (default: 5)
  - `radiusRange` (Array) - [min, max] radius for bubble sizing (default: [3, 20])

## Configuration Options

### Bubble Size Range

Control the minimum and maximum bubble sizes:

```yaml
style:
  radiusRange: [5, 30]  # Min 5px, max 30px radius
```

### Axis Labels

Add descriptive labels to axes:

```yaml
axes:
  bottom:
    label: "X-Axis Label"
  left:
    label: "Y-Axis Label"
```

### Custom Colors

Override default color palette:

```yaml
style:
  colors: ["#10b981", "#3b82f6", "#f59e0b"]
```

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
