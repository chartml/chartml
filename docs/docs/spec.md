<style scoped>
.vp-doc {
  max-width: 800px;
}
</style>

# ChartML v1.0 Specification

**Version:** 1.0
**Date:** 2025-10-25

**Related Documents:**
- **JSON Schema**: [chartml_schema.json](./chartml_schema.json) - Machine-readable validation rules
- **Examples**: [Examples](./examples) - Real-world usage patterns
- **Quick Reference**: [Quick Reference](./quick-reference) - Syntax cheatsheet

---

## Overview

ChartML is a declarative YAML-based language for creating data visualizations. All ChartML components use the `````chartml``` markdown code block and are distinguished by their `type:` field.

### Component Types

ChartML has five component types:

1. **Source** - Reusable data source definitions
2. **Params** - Dashboard parameter definitions (interactive controls)
3. **Style** - Visual theming and default styling
4. **Config** - Scope-level configuration and defaults
5. **Chart** - Visualization specifications

All components use:
- `````chartml``` markdown code block
- `type:` field to identify component type
- `version: 1` field for versioning

### Block Format

A `````chartml``` block can contain:
- **Single component**: One Source, Params, Style, Config, or Chart object
- **Component array**: An array of any ChartML components (useful for grid layouts)

```yaml
# Single component
type: chart
version: 1
title: "My Chart"
# ...
```

```yaml
# Component array
- type: style
  version: 1
  name: corporate_theme
  # ...

- type: source
  version: 1
  name: sales_data
  # ...

- type: chart
  version: 1
  title: "Chart 1"
  # ...

- type: chart
  version: 1
  title: "Chart 2"
  # ...
```

---

## Component 1: Source

Reusable data source definitions that can be referenced by multiple charts.

### Structure

```yaml
type: source
version: 1
name: source_name           # Required - unique identifier
provider: inline | http     # Required - data provider type
rows: [...]                 # Required for provider: inline
url: "https://..."          # Required for provider: http
cache:                      # Optional - cache configuration
  ttl: 6h                   # Time-to-live: <number><unit> where unit is s/m/h/d
```

**Cache TTL Format:**
- Format: `<number><unit>`
- Units: `s` (seconds), `m` (minutes), `h` (hours), `d` (days)
- Examples: `"30s"`, `"5m"`, `"6h"`, `"24h"`, `"1d"`, `"7d"`
```

### Examples

**Inline Source:**
```chartml
type: source
version: 1
name: sample_data
provider: inline
rows:
  - region: "US"
    revenue: 15000
    customers: 120
  - region: "EU"
    revenue: 12000
    customers: 95
  - region: "APAC"
    revenue: 8000
    customers: 67
```

---

## Component 2: Params

Dashboard parameter definitions that create interactive controls.

### Structure

```yaml
type: params
version: 1
params:
  - id: param_id              # Required - unique parameter identifier
    type: multiselect | select | daterange | number | text
    label: "Display Label"    # Required - shown in UI
    options: [...]            # Required for select/multiselect
    default: value            # Required - initial value
    placeholder: "text"       # Optional - for text inputs
    layout:                   # Optional - grid layout
      colSpan: 3              # Grid columns (1-12), defaults by type
```

### Grid Layout

Parameters use a 12-column grid system (same as charts). Each parameter can specify `layout.colSpan` to control width.

**Auto-Calculated Column Span (when `layout.colSpan` not specified):**
- **1 parameter**: 12 columns (full width)
- **2 parameters**: 6 columns each (half width)
- **3 parameters**: 4 columns each (third width)
- **4+ parameters**: 3 columns each (quarter width)

This ensures parameters automatically fill the available space intelligently based on how many you have.

**Custom Column Span Example:**
```yaml
- id: long_search
  type: text
  label: "Search Everything"
  layout:
    colSpan: 8            # Override default (4) to be wider
  placeholder: "Enter search terms..."
  default: ""
```

### Parameter Types

**1. Multiselect** - Checkbox group for multiple selections
```yaml
- id: selected_regions
  type: multiselect
  label: "Regions"
  options: ["US", "EU", "APAC", "LATAM"]
  default: ["US", "EU"]
```

**2. Select** - Dropdown for single selection
```yaml
- id: product_category
  type: select
  label: "Category"
  options: ["All", "Electronics", "Clothing", "Home & Garden"]
  default: "All"
```

**3. Date Range** - Start and end date inputs
```yaml
- id: date_range
  type: daterange
  label: "Date Range"
  default:
    start: "2024-01-01"
    end: "2024-12-31"
```

**4. Number** - Numeric input
```yaml
- id: minimum_revenue
  type: number
  label: "Minimum Revenue ($)"
  default: 1000
```

**5. Text** - Text search input
```yaml
- id: search_term
  type: text
  label: "Search Products"
  placeholder: "Enter product name..."
  default: ""
```

### Dashboard-Level vs Chart-Level Parameters

**Dashboard-Level Named Params** (shared across charts):
```yaml
type: params
version: 1
name: dashboard_filters      # Required - unique block name
params:
  - id: date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"
  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC"]
    default: ["US", "EU"]
```
Referenced as: `$dashboard_filters.date_range` or `$dashboard_filters.selected_regions`

**Chart-Level Inline Params** (private to chart):
```yaml
type: chart
version: 1
title: "Top Revenue Products"

params:  # Chart-specific parameters (no name field)
  - id: top_n
    type: number
    label: "Top N Products"
    default: 10

data: sales_data
aggregate:
  limit: "$top_n"  # Reference chart-level param (no prefix)
```
Referenced as: `$top_n` (no block name prefix)

**Variable Reference Syntax:**
- **Named params**: `$blockname.param_id` (e.g., `$dashboard_filters.selected_regions`)
- **Chart-level params**: `$param_id` (e.g., `$top_n`)

**Resolution Logic:**
- Variable has dot (e.g., `$dashboard_filters.region`) → Look up named params block in registry
- Variable has no dot (e.g., `$top_n`) → Look in current chart's inline params array
- If not found: warning and keep variable as-is

---

## Component 3: Chart

Complete visualization specification using Data → Aggregate → Visualize pipeline.

### Structure

```yaml
type: chart
version: 1
title: "Chart Title"         # Optional

params:                       # Optional - chart-level parameters
  - id: param_id
    type: multiselect
    # ...

data: source_name             # Reference named Source (string)
# OR
data:                         # Inline Source definition (object)
  provider: inline | http     # Built-in providers (or custom plugin)
  rows: [...]                 # For provider: inline
  url: "https://..."          # For provider: http
  cache:                      # Optional
    ttl: 6h

aggregate:                    # Optional - data aggregation
  dimensions: [...]
  measures: [...]
  filters: {...}
  sort: [...]
  limit: 100

visualize:                    # Required - chart rendering
  type: bar | line | area | scatter | pie | doughnut | metric
  columns: field_name
  rows: field_name
  # ... chart-specific options

layout:                       # Optional - grid layout
  colSpan: 12                 # Grid columns (1-12)

style:                        # Optional - visual styling
  height: 400
```

### Data Layer

Specifies the data source. The `data` attribute is always of type **Source** and can be:

**Option 1: Reference Source (string)**
```yaml
data: quarterly_sales         # References a named Source
```

**Option 2: Inline Data (object)**
```yaml
data:
  provider: inline
  rows:
    - month: "Jan"
      sales: 1200
    - month: "Feb"
      sales: 1350
```

**Option 3: HTTP Data Source (object)**
```yaml
data:
  provider: http
  url: "https://api.example.com/sales"
  cache:
    ttl: 6h
```

### Aggregate Layer (Optional)

Operates on data to filter, aggregate, and calculate.

**Dimensions and Measures:**
```yaml
aggregate:
  dimensions:
    - product
    - region
    - column: "DATE_TRUNC(sale_date, 'MONTH')"
      name: year_month
      type: date

  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

    - column: units
      aggregation: count
      name: total_units

    # Post-aggregation calculation
    - expression: "total_revenue / total_units"
      name: avg_price
      type: number
```

**Aggregation Functions:**
`sum`, `avg`, `min`, `max`, `count`, `countDistinct`, `median`, `stddev`, `variance`, `percentile25`, `percentile50`, `percentile75`, `percentile90`, `percentile95`, `percentile99`

**Filters with Parameter References:**
```yaml
aggregate:
  filters:
    combinator: and  # or "or"
    rules:
      - field: region
        operator: in
        value: "$dashboard_filters.selected_regions"  # Named param reference

      - field: total_revenue
        operator: ">="
        value: "$revenue_filter.minimum_revenue"  # Different named block

      - field: date
        operator: between
        value: ["$time_filter.date_range.start", "$time_filter.date_range.end"]  # Nested access
```

**Filter Operators:**
`=`, `!=`, `<`, `>`, `<=`, `>=`, `in`, `notIn`, `contains`, `startsWith`, `endsWith`, `between`, `isNull`, `isNotNull`

**Smart Filtering:**
- Dimension fields → WHERE clause (pre-aggregation)
- Measure fields → HAVING clause (post-aggregation)

**Sort and Limit:**
```yaml
aggregate:
  sort:
    - field: total_revenue
      direction: desc
    - field: region
      direction: asc

  limit: 100
```

### Visualize Layer

Describes how to render data as visual marks.

**Core Concept:**
- **Columns**: Categories / X-axis (the independent variable - what you're grouping or organizing by)
- **Rows**: Values / Y-axis (the dependent variable - the measurements or quantities)
- **Marks**: Additional encoding channels (color, size, text)

**Important: Understanding columns vs rows**

The most common mistake is reversing `columns` and `rows`. Remember:
- **`columns:`** = Categories (region, month, product name, etc.)
- **`rows:`** = Numbers (revenue, count, score, etc.)

**Correct:**
```yaml
columns: region      # Categories on x-axis
rows: revenue        # Values on y-axis
```

**Incorrect:**
```yaml
columns: revenue     # Wrong - revenue is a value, not a category
rows: region         # Wrong - region is a category, not a value
```

**Basic Structure:**
```yaml
visualize:
  type: bar | line | area | scatter | pie | doughnut | metric
  mode: stacked | grouped | normalized  # Optional, for bar/area
  orientation: vertical | horizontal   # Optional, for bar charts

  columns: field_name
  rows: field_name

  marks:              # Optional
    color: field_name
    size: field_name
    text: field_name

  axes:               # Optional
    left:
      label: "Label"
      format: "$,.0f"
      min: 0
      max: 100
    right:            # For dual-axis charts
      label: "Label"
      format: ",.0f"

  annotations:        # Optional - reference lines, bands, markers
    - type: line | band
      axis: left | right | x
      value: number     # For line
      from: number      # For band start
      to: number        # For band end
      label: "text"
      color: "#color"

  style:              # Optional
    height: 400
```

**Chart Types:**

1. **Bar Chart**
```yaml
visualize:
  type: bar
  mode: grouped        # or stacked
  orientation: vertical
  columns: region
  rows: revenue
  marks:
    color: product     # Group by product
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

2. **Line Chart**
```yaml
visualize:
  type: line
  columns: month
  rows: revenue
  marks:
    color: region     # Separate line per region
  axes:
    left:
      format: "$,.0f"
  style:
    height: 400
```

3. **Pie/Doughnut Chart**
```yaml
visualize:
  type: pie           # or doughnut
  columns: category   # Slice labels
  rows: revenue       # Slice sizes
  style:
    height: 400
```

4. **Metric Card**
```yaml
visualize:
  type: metric
  value: current_value
  label: "Revenue"             # Optional - label shown inside card
  format: "$,.0f"
  compareWith: previous_value  # Optional - show trend
  invertTrend: false           # Optional - invert trend colors (true = red for increase, green for decrease)
```

**Metric Labeling:**
- `chart.title` (optional) → Label shown **above** card (consistent with all chart types)
- `visualize.label` (optional) → Label shown **inside** card (metric-specific)
- If neither specified, only the formatted value is shown
```

5. **Dual-Axis Chart**
```chartml
visualize:
  type: bar
  columns: month
  rows:
    - field: revenue
      mark: bar
      axis: left
      color: "#4285f4"

    - field: customers
      mark: line
      axis: right
      color: "#34a853"

  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
    right:
      label: "Customers"
      format: ",.0f"
```

### Annotations (Reference Lines & Bands)

Add visual markers to highlight goals, targets, or significant values.

**Reference Line (horizontal or vertical):**
```yaml
annotations:
  - type: line
    axis: left              # Which axis to attach to
    value: 150000           # Y-value for horizontal line
    orientation: horizontal
    label: "Goal"
    labelPosition: end      # start | center | end
    color: "#34a853"
    strokeWidth: 2
    dashArray: "5,5"        # Dashed line
```

**Reference Band (range):**
```yaml
annotations:
  - type: band
    axis: left
    from: 140000            # Start of band
    to: 160000              # End of band
    orientation: horizontal
    label: "Target Range"
    color: "#34a853"
    opacity: 0.15
```

**Event Marker (vertical line):**
```yaml
annotations:
  - type: line
    axis: x
    value: "2025-03-15"     # X-value for vertical line
    orientation: vertical
    label: "Product Launch"
    color: "#4285f4"
```

See the [Examples page](./examples) for complete annotation examples with goals, targets, and event markers.

### Complete Chart Examples

**Example 1: Simple Bar Chart with Source Reference**
```yaml
type: chart
version: 1
title: "Revenue by Region"

data: quarterly_sales         # Reference named Source

aggregate:
  dimensions: [region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: bar
  columns: region
  rows: total_revenue
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

**Example 2: Chart with Inline Parameters and Filtering**
```yaml
type: chart
version: 1
title: "Filtered Regional Sales"

params:
  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC", "LATAM"]
    default: ["US", "EU"]

data: quarterly_sales

aggregate:
  dimensions: [region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

  filters:
    combinator: and
    rules:
      - field: region
        operator: in
        value: "$selected_regions"  # Chart-level param (no prefix)

visualize:
  type: bar
  columns: region
  rows: total_revenue
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

**Example 3: Inline Data with Multiple Charts in Grid**
```yaml
- type: chart
  version: 1
  title: "Q1 Revenue"
  layout:
    colSpan: 6

  data:
    provider: inline
    rows:
      - region: "US"
        revenue: 25000
      - region: "EU"
        revenue: 18000

  visualize:
    type: bar
    columns: region
    rows: revenue
    style:
      height: 300

- type: chart
  version: 1
  title: "Q1 Customers"
  layout:
    colSpan: 6

  data:
    provider: inline
    rows:
      - region: "US"
        customers: 150
      - region: "EU"
        customers: 120

  visualize:
    type: bar
    columns: region
    rows: customers
    style:
      height: 300
```

---

## Component 4: Style

Reusable visual theming component that defines default appearance for charts.

### Mental Model

Styles are **default bundles** that cascade down the scope hierarchy (system → workspace → user → dashboard → chart), with more specific scopes overriding less specific ones. System defaults are defined in `system-defaults.chartml` and provide built-in themes.

### Structure

```yaml
type: style
version: 1
name: corporate_theme

# Color palette
colors: ["#4285f4", "#ea4335", "#fbbc04", "#34a853"]

# Grid defaults
grid:
  x: false
  y: true
  color: "#e0e0e0"
  opacity: 0.5
  dashArray: "2,2"

# Default height
height: 400

# Line chart defaults
showDots: false
strokeWidth: 2

# Typography (optional)
fonts:
  title:
    family: "Inter, sans-serif"
    size: 18
    weight: 600
    color: "#202124"

  axis:
    family: "Inter, sans-serif"
    size: 12
    color: "#5f6368"

  dataLabel:
    family: "Inter, sans-serif"
    size: 11
    weight: 500
```

### Properties

**Core Properties:**
- `colors` - Array of color values for multi-series charts (12-color system with automatic fallback)
- `grid` - Grid line configuration (x/y visibility, color, opacity, dashArray)
- `height` - Default chart height in pixels
- `showDots` - Show dots on line charts (boolean)
- `strokeWidth` - Line thickness (number)
- `legend` - Legend configuration (position, orientation)

**Typography (Optional):**
- `fonts.title` - Title font configuration (family, size, weight, color)
- `fonts.axis` - Axis label font configuration
- `fonts.dataLabel` - Data label font configuration

### Built-in Color Palettes

The system provides three professionally-designed 12-color palettes optimized for data visualization:

**1. autumn_forest** (Default)
- Improved BI palette with accessibility focus
- Colors: Ocean, Amber, Forest, Coral, Violet, Chartreuse, Burgundy, Indigo, Sienna, Teal, Sage, Mauve
- Best for: General purpose dashboards, mixed data types

**2. spectrum_pro**
- Inspired by Tableau with warmer tones
- Colors: Azure, Tangerine, Seafoam, Crimson, Orchid, Marigold, Steel, Jade, Burgundy, Periwinkle, Chartreuse, Slate Blue
- Best for: Professional presentations, warm color schemes

**3. horizon_suite**
- Inspired by Looker with deeper saturation
- Colors: Cobalt, Emerald, Sunset, Lavender, Gold, Teal, Berry, Moss, Peach, Indigo, Pine, Rose
- Best for: High-contrast displays, vibrant visualizations

### Automatic Fallback Colors

The system automatically handles charts with more than 12 series:

- **1-12 series**: Uses the base 12-color palette for maximum contrast
- **13-24 series**: Automatically generates 12 additional desaturated variants by:
  - Reducing saturation by 40%
  - Normalizing luminosity toward mid-range
  - Maintaining hue relationships for consistency
- **25+ series**: Cycles through the combined 24-color palette

**Recommendation**: For charts with >12 categories, consider:
- Filtering data to show top categories
- Using small multiples (separate charts)
- Grouping smaller categories into "Other"

### Usage in Charts

**Reference by name:**
```yaml
type: chart
version: 1
title: "Revenue Trend"
style: corporate_theme  # Reference style by name

data: sales_data
visualize:
  type: line
  columns: month
  rows: revenue
  # Inherits colors, grid, height, fonts from corporate_theme
```

**Inline override (deep merge):**
```yaml
type: chart
version: 1
style: corporate_theme  # Use as base

data: sales_data
visualize:
  type: bar
  style:
    height: 600           # Override just height
    grid:
      color: "#ff0000"    # Override just grid color
    # Colors, fonts, other grid props still from corporate_theme
```

### Deep Merge Behavior

Chart inline styles are **deep merged** with referenced styles. This allows surgical overrides without losing inherited values.

**Example:**
```yaml
# Referenced style has
grid:
  x: false
  y: true
  color: "#e0e0e0"
  opacity: 0.5

# Chart overrides just color
visualize:
  style:
    grid:
      color: "#ff0000"

# Effective result (deep merge)
grid:
  x: false           # From style
  y: true            # From style
  color: "#ff0000"   # From chart override
  opacity: 0.5       # From style
```

---

## Component 5: Config

Scope-level configuration that sets defaults for all charts within that scope.

### Purpose

Sets scope-level defaults (system/workspace/user/dashboard) without repeating `style:` on every chart. System defaults are defined in `system-defaults.chartml`.

### Structure

```yaml
type: config
version: 1

# Reference named style
style: corporate_theme

# OR inline style definition
style:
  colors: ["#4285f4", "#ea4335"]
  grid:
    y: true
    color: "#e0e0e0"
  height: 400
```

### Usage

**Dashboard with config:**
```yaml
type: config
version: 1
style: corporate_theme  # All charts inherit this by default
```

**Charts automatically inherit:**
```yaml
type: chart
version: 1
title: "Revenue"
data: sales_data
visualize:
  type: bar
  # corporate_theme applied automatically from config
```

**Charts can override:**
```yaml
type: chart
version: 1
style: dark_theme  # Override config default
data: sales_data
visualize:
  type: line
```

### Resolution Order

When resolving a chart's style, the system cascades through six levels of specificity:

**1. System Config** (Base)
   - Defined in `system-defaults.chartml`
   - Provides `autumn_forest` as the default palette
   - All charts inherit these unless overridden

**2. Workspace Config** (Organization Defaults)
   - Set by workspace admins in Settings → Chart Styles
   - Applies to all users in the workspace
   - Accessed via: `/api/v1/workspaces/chartml-config`
   - Example:
     ```yaml
     type: config
     version: 1
     style: spectrum_pro  # All workspace charts use this palette
     ```

**3. User Config** (Personal Defaults)
   - Set by individual users in Settings → Chart Styles
   - Overrides workspace defaults for that user only
   - Accessed via: `/api/v1/users/chartml-config`
   - Example:
     ```yaml
     type: config
     version: 1
     style: horizon_suite  # Override workspace default
     ```

**4. Dashboard Config** (Dashboard Defaults)
   - Defined in dashboard markdown with `type: config` block
   - Applies to all charts in that dashboard
   - Overrides user and workspace defaults
   - Example:
     ```yaml
     type: config
     version: 1
     style: corporate_theme  # Dashboard-specific style
     ```

**5. Chart Style Reference** (Chart-Specific)
   - Defined with `style:` field on chart
   - References a named style by name
   - Overrides all config defaults
   - Example:
     ```yaml
     type: chart
     version: 1
     style: dark_theme  # This chart uses dark_theme
     data: sales_data
     visualize:
       type: bar
     ```

**6. Inline Style** (Surgical Overrides)
   - Defined with `visualize.style` in chart spec
   - Highest specificity - overrides everything
   - Use for one-off customizations
   - Example:
     ```yaml
     type: chart
     version: 1
     data: sales_data
     visualize:
       type: bar
       style:
         height: 600           # Override just height
         colors: ["#ff0000"]   # Override just colors
     ```

**Deep Merge Behavior**: Each level is deep-merged with the previous, so you only override what you specify. All other properties are inherited.

---

## Variable References

Charts can reference parameters using `$params.*` syntax:

**Simple Reference:**
```yaml
value: "$params.minimum_revenue"
```

**Nested Reference:**
```yaml
value: ["$params.date_range.start", "$params.date_range.end"]
```

**Resolution:**
1. Variables are resolved before chart execution
2. `"$params.region"` → `["US", "EU"]` (from param state)
3. Resolved values flow through Data → Aggregate → Visualize pipeline

---

## Number and Date Formatting

ChartML uses [d3-format](https://d3js.org/d3-format) for number formatting and [d3-time-format](https://d3js.org/d3-time-format) for date formatting.

### Common Number Formats

| Format | Example Output | Description |
|--------|---------------|-------------|
| `$,.0f` | $1,234 | Currency with thousands separator, no decimals |
| `$,.2f` | $1,234.56 | Currency with thousands separator, 2 decimals |
| `,.0f` | 1,234 | Thousands separator, no decimals |
| `.1%` | 12.3% | Percentage with 1 decimal place |
| `.0%` | 12% | Percentage with no decimals |
| `~s` | 1.2K, 3.4M, 5.6B | SI-prefix notation (K, M, B, T) |
| `.2e` | 1.23e+4 | Scientific notation |
| `+,.0f` | +1,234 | Always show sign |

### Usage in ChartML

Formats can be applied to:
- **Axis labels**: `axes.left.format`
- **Data labels**: `rows.dataLabels.format`
- **Metric values**: `visualize.format`

**Example:**
```yaml
visualize:
  type: bar
  columns: month
  rows:
    field: revenue
    dataLabels:
      show: true
      format: "$,.0f"    # Format data labels
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"    # Format axis ticks
```

**Reference:** See [d3-format documentation](https://d3js.org/d3-format) for complete format specification.

---

## Grid Layout

Charts can be arranged in a responsive 12-column grid:

```yaml
layout:
  colSpan: 6  # Takes 6 columns (half width)
```

- Default: `colSpan: 12` (full width)
- Responsive: Automatically adjusts for mobile/tablet
- Multiple charts in one block create a grid automatically

---

## Complete Dashboard Example

```markdown
# Sales Dashboard

Dashboard-level parameters (affect all charts):

```chartml
type: params
version: 1
params:
  - id: global_date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"

  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC", "LATAM"]
    default: ["US", "EU"]
```

Reusable data source:

```yaml
type: source
version: 1
name: sales_data
provider: http
url: "https://api.example.com/sales?year=2024"
cache:
  ttl: 6h
```

Charts using source and parameters:

```yaml
- type: chart
  version: 1
  title: "Revenue by Region"
  layout:
    colSpan: 6

  data: sales_data        # Reference the Source

  aggregate:
    dimensions: [region]
    measures:
      - column: revenue
        aggregation: sum
        name: total_revenue

    filters:
      combinator: and
      rules:
        - field: region
          operator: in
          value: "$params.selected_regions"

        - field: sale_date
          operator: between
          value: ["$params.global_date_range.start", "$params.global_date_range.end"]

  visualize:
    type: bar
    columns: region
    rows: total_revenue
    axes:
      left:
        label: "Revenue ($)"
        format: "$,.0f"
    style:
      height: 400

- type: chart
  version: 1
  title: "Customer Count"
  layout:
    colSpan: 6

  data: sales_data

  aggregate:
    dimensions: [region]
    measures:
      - column: customers
        aggregation: countDistinct
        name: unique_customers

    filters:
      combinator: and
      rules:
        - field: region
          operator: in
          value: "$params.selected_regions"

  visualize:
    type: bar
    columns: region
    rows: unique_customers
    style:
      height: 400
```
```

---

**End of Specification**
