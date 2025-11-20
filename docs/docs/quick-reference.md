# ChartML Quick Reference

**Purpose:** Syntax reference for creating ChartML visualizations. For full docs, see the [Full Specification](/spec/).

---

## Code Block Format

All ChartML must be wrapped in a ````chartml` markdown code block:

````markdown
```chartml
type: chart
version: 1
# ... your chart spec
```
````

---

## Basic Structure

```yaml
type: chart
version: 1
title: "Chart Title"       # Optional - shown above chart

data:
  - category: "A"
    value: 100
  - category: "B"
    value: 200

visualize:
  type: bar | line | area | scatter | pie | doughnut | table | metric
  columns: field_name      # X-axis / categories
  rows: field_name         # Y-axis / values
```

---

## Component Types

ChartML has five component types:

1. **Chart** - Visualizations (bar, line, pie, etc.)
2. **Source** - Reusable data source definitions
3. **Params** - Interactive dashboard parameters (filters, date ranges)
4. **Style** - Visual theming and color palettes
5. **Config** - Scope-level configuration defaults

This quick reference focuses on charts. See the [Full Specification](/spec/) for details on other components.

---

## Common Chart Types

### 1. Bar Chart (Vertical)
```yaml
type: chart
version: 1
title: "Revenue by Region"

data:
  - region: "North"
    revenue: 125000
  - region: "South"
    revenue: 98000
  - region: "East"
    revenue: 112000

visualize:
  type: bar
  columns: region            # Categories on x-axis
  rows: revenue              # Values on y-axis
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

### 2. Bar Chart (Grouped/Colored)
```yaml
type: chart
version: 1
title: "Revenue by Month and Product"

data:
  - month: "Jan"
    product: "Widget A"
    revenue: 45000
  - month: "Jan"
    product: "Widget B"
    revenue: 38000
  - month: "Feb"
    product: "Widget A"
    revenue: 52000
  - month: "Feb"
    product: "Widget B"
    revenue: 41000

aggregate:
  dimensions: [month, product]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: bar
  mode: grouped              # or 'stacked'
  columns: month
  rows: total_revenue
  marks:
    color: product           # Separate bar per product
```

### 3. Line Chart
```yaml
type: chart
version: 1
title: "Daily Sales Trends"

data:
  - date: "2024-01-01"
    store: "Store A"
    daily_sales: 4200
  - date: "2024-01-01"
    store: "Store B"
    daily_sales: 3800
  - date: "2024-01-02"
    store: "Store A"
    daily_sales: 4500
  - date: "2024-01-02"
    store: "Store B"
    daily_sales: 4100

visualize:
  type: line
  columns: date
  rows: daily_sales
  marks:
    color: store             # Separate line per store
  axes:
    left:
      label: "Sales"
      format: "$,.0f"
  style:
    height: 400
```

### 4. Pie Chart
```yaml
type: chart
version: 1
title: "Sales by Category"

data:
  - category: "Electronics"
    amount: 125000
  - category: "Clothing"
    amount: 98000
  - category: "Home"
    amount: 87000
  - category: "Sports"
    amount: 65000

visualize:
  type: pie
  columns: category          # Slice labels
  rows: amount               # Slice sizes
```

### 5. Scatter Plot
```yaml
type: chart
version: 1
title: "Price vs Sales"

data:
  - price: 29.99
    units_sold: 450
  - price: 39.99
    units_sold: 380
  - price: 49.99
    units_sold: 320
  - price: 19.99
    units_sold: 520

visualize:
  type: scatter
  columns: price
  rows: units_sold
  axes:
    left:
      label: "Units Sold"
  style:
    height: 400

---

## Formatting Numbers

Use the `format` option in axes or metric values:

```yaml
visualize:
  axes:
    left:
      format: "$,.0f"        # Currency: $1,234
      # or ",.2f"           # Decimal: 1,234.56
      # or ".1%"            # Percent: 45.6%
```

**Common formats:**

| Format | Example | Description |
|--------|---------|-------------|
| `$,.0f` | $1,234 | Currency, no decimals |
| `,.2f` | 1,234.56 | Comma separator, 2 decimals |
| `.1%` | 45.6% | Percentage, 1 decimal |

---

## Multiple Charts in Grid

```yaml
# Array of charts - each takes half width (6 columns)
- type: chart
  version: 1
  title: "Revenue by Region"
  layout:
    colSpan: 6             # Half width (12-column grid)
  data:
    - region: "North"
      revenue: 125000
    - region: "South"
      revenue: 98000
    - region: "East"
      revenue: 112000
  visualize:
    type: bar
    columns: region
    rows: revenue

- type: chart
  version: 1
  title: "Product Distribution"
  layout:
    colSpan: 6             # Half width
  data:
    - product: "Widget A"
      count: 45
    - product: "Widget B"
      count: 38
    - product: "Gadget X"
      count: 52
  visualize:
    type: pie
    columns: product
    rows: count
```

**Grid sizes:** 1-12 columns (12 = full width, 6 = half, 4 = third, 3 = quarter)

---

## Styling Tips

### Chart Height
```yaml
visualize:
  style:
    height: 400            # Default: 400px
```

### Axis Labels
```yaml
visualize:
  axes:
    x:
      label: "Month"
    left:
      label: "Revenue ($)"
      format: "$,.0f"
      min: 0               # Force axis minimum
      max: 100000          # Force axis maximum
```

### Horizontal Bar Chart
```yaml
visualize:
  type: bar
  orientation: horizontal  # Flip to horizontal
  columns: category
  rows: value
```

### Mixed Chart with Dual Axes
```yaml
type: chart
version: 1
title: "Daily Volume with 7-Day Average"

data:
  - transaction_date: "2024-01-01"
    daily_volume: 450
    rolling_7day_avg: 420
  - transaction_date: "2024-01-02"
    daily_volume: 485
    rolling_7day_avg: 435
  - transaction_date: "2024-01-03"
    daily_volume: 510
    rolling_7day_avg: 448
  - transaction_date: "2024-01-04"
    daily_volume: 475
    rolling_7day_avg: 455

visualize:
  type: bar
  columns: transaction_date
  rows:
    - field: daily_volume
      mark: bar
      axis: left
    - field: rolling_7day_avg
      mark: line
      axis: right
  axes:
    x:
      label: "Date"
    left:
      label: "Daily Volume"
      format: ",.0f"
    right:
      label: "7-Day Average"
      format: ",.1f"
  style:
    height: 400
```

---

## Common Patterns

### Aggregation with Built-in Pipeline
```yaml
# Aggregate raw data before charting
data:
  - region: "North"
    sale_date: "2024-01-15"
    revenue: 1200
  - region: "North"
    sale_date: "2024-01-22"
    revenue: 1350
  - region: "South"
    sale_date: "2024-01-10"
    revenue: 950

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

---

## Common Mistakes

::: warning Common Mistakes to Avoid
**Reversed columns/rows** - Remember: `columns` = categories (x-axis), `rows` = values (y-axis)

**Field name mismatch** - Use exact field names from your data

**Wrong chart type** - For single number KPIs, use `type: metric` instead of bar/line/pie

**Title placement** - Put `title:` at the chart level, not inside `style` or `visualize`
:::

---

## Practical Examples

### Revenue by Region (with formatting)
```yaml
type: chart
version: 1
title: "Revenue by Region"

data:
  - region: "North"
    revenue: 125000
  - region: "South"
    revenue: 98000
  - region: "East"
    revenue: 112000
  - region: "West"
    revenue: 87000

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
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

### Monthly Trends (line chart)
```yaml
type: chart
version: 1
title: "Monthly Revenue by Product"

data:
  - date: "2024-01-15"
    product: "Widget A"
    revenue: 2100
  - date: "2024-01-22"
    product: "Widget A"
    revenue: 2350
  - date: "2024-02-10"
    product: "Widget A"
    revenue: 2200
  - date: "2024-01-12"
    product: "Widget B"
    revenue: 1800
  - date: "2024-02-05"
    product: "Widget B"
    revenue: 1950

aggregate:
  dimensions: [product]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: line
  columns: product
  rows: total_revenue
  marks:
    color: product
  axes:
    x:
      label: "Product"
    left:
      label: "Revenue"
      format: "$,.0f"
  style:
    height: 400
```

---

## Additional Resources

For complete documentation, see the [Full Specification](/spec/).
