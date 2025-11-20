# ChartML v1.0 Examples

Comprehensive examples demonstrating all ChartML v1.0 features including reusable sources, interactive parameters, chart types, and data aggregations. All examples in this file are validated against the JSON schema.

**Related Documents:**
- **Specification**: [ChartML Specification](/spec/) - Complete ChartML v1.0 language reference

---

## Reusable Styles and Configuration

This section demonstrates how to define reusable themes and apply them across multiple charts.

### Define a Reusable Style

```yaml
type: style
version: 1
name: corporate_theme
colors: ["#4285f4", "#ea4335", "#fbbc04", "#34a853"]
grid:
  y: true
  color: "#e0e0e0"
  opacity: 0.5
height: 400
fonts:
  title:
    family: "Inter, sans-serif"
    size: 18
    weight: 600
```

```chartml
type: style
version: 1
name: corporate_theme
colors: ["#4285f4", "#ea4335", "#fbbc04", "#34a853"]
grid:
  y: true
  color: "#e0e0e0"
  opacity: 0.5
height: 400
fonts:
  title:
    family: "Inter, sans-serif"
    size: 18
    weight: 600
```

### Set Dashboard-Level Default Style

All charts below will inherit `corporate_theme` automatically:

```yaml
type: config
version: 1
style: corporate_theme
```

```chartml
type: config
version: 1
style: corporate_theme
```

### Charts Automatically Inherit the Theme

**Chart 1: Revenue Trend** - Uses all defaults from corporate_theme

```chartml
type: chart
version: 1
title: "Monthly Revenue"

data:
  provider: inline
  rows:
    - month: "Jan"
      revenue: 125000
    - month: "Feb"
      revenue: 138000
    - month: "Mar"
      revenue: 152000

visualize:
  type: bar
  columns: month
  rows: revenue
```

**Chart 2: Customer Growth** - Also inherits theme, demonstrates multiple charts sharing style

```chartml
type: chart
version: 1
title: "New Customers"

data:
  provider: inline
  rows:
    - month: "Jan"
      customers: 450
    - month: "Feb"
      customers: 485
    - month: "Mar"
      customers: 520

visualize:
  type: line
  columns: month
  rows: customers
```

### Chart with Selective Overrides (Deep Merge)

This chart overrides just height and grid color while keeping corporate_theme's fonts and base colors:

```chartml
type: chart
version: 1
title: "Regional Breakdown"

data:
  provider: inline
  rows:
    - region: "US"
      revenue: 85000
    - region: "EU"
      revenue: 67000

visualize:
  type: pie
  columns: region
  rows: revenue
  style:
    height: 300
    grid:
      color: "#ff0000"
```

### Config with Inline Style

For one-off dashboard themes without creating a named style, you can define inline styles:

```yaml
type: config
version: 1
style:
  colors: ["#1a237e", "#3949ab", "#5e92f3"]
  height: 500
  grid:
    y: true
    color: "#f0f0f0"
```

*(Note: Example shown as YAML instead of chartml to avoid multiple configs in this file)*

---

## KPI Overview: Executive Metrics Dashboard

This dashboard demonstrates metric cards (KPI cards) that display key performance indicators with trend comparisons.

### Key Performance Metrics

A row of 4 metric cards showing critical business metrics with month-over-month comparisons:

```chartml
- type: chart
  version: 1
  title: "Total Revenue"
  layout:
    colSpan: 3

  data:
    provider: inline
    rows:
        - { current: 1234567, previous: 1100000 }

  visualize:
    type: metric
    value: current
    format: "$,.0f"
    compareWith: previous

- type: chart
  version: 1
  title: "Active Users"
  layout:
    colSpan: 3

  data:
    provider: inline
    rows:
        - { current: 8432, previous: 8100 }

  visualize:
    type: metric
    value: current
    format: ",.0f"
    compareWith: previous

- type: chart
  version: 1
  title: "Conversion Rate"
  layout:
    colSpan: 3

  data:
    provider: inline
    rows:
        - { current: 0.042, previous: 0.038 }

  visualize:
    type: metric
    value: current
    format: ".1%"
    compareWith: previous

- type: chart
  version: 1
  title: "Avg Order Value"
  layout:
    colSpan: 3

  data:
    provider: inline
    rows:
        - { current: 156.78, previous: 142.50 }

  visualize:
    type: metric
    value: current
    format: "$,.2f"
    compareWith: previous
```

### Error Rate Metric (Inverted Trend)

This metric shows error rate where a decrease is positive (green) and an increase is negative (red).

**Metric Label Rules:**

Metric cards support two optional label locations:
- **`chart.title`** → Label shown **above** the card (like all chart types)
- **`visualize.label`** → Label shown **inside** the card (metric-specific feature)
- If neither specified → Only the formatted value is shown

**Examples:**

```chartml
# Just the number (no labels)
type: chart
version: 1
data:
  provider: inline
  rows:
    - revenue: 45000
visualize:
  type: metric
  value: revenue
  format: "$,.0f"
```

```chartml
# Label inside card only
type: chart
version: 1
data:
  provider: inline
  rows:
    - revenue: 45000
visualize:
  type: metric
  value: revenue
  label: "Total Revenue"   # Shows inside card
  format: "$,.0f"
```

```chartml
# Title above + label inside
type: chart
version: 1
title: "Q1 Performance"    # Shows above card
data:
  provider: inline
  rows:
    - revenue: 45000
visualize:
  type: metric
  value: revenue
  label: "Revenue"         # Shows inside card
  format: "$,.0f"
```

**Error Rate Example (inverted trend):**

```chartml
type: chart
version: 1

data:
  provider: inline
  rows:
      - { current: 0.023, previous: 0.031 }

visualize:
  type: metric
  value: current
  label: "Error Rate"      # Shows "Error Rate" inside the metric card
  format: ".2%"
  compareWith: previous
  invertTrend: true        # Red for up, green for down
```

---

## Reference Lines & Bands: Goal Tracking Dashboard

This dashboard demonstrates reference lines (goal markers) and bands (target ranges) for tracking performance against targets.

### Monthly Revenue with Goal Line

Shows actual revenue with a goal line at $150,000:

```chartml
type: chart
version: 1
title: "Monthly Revenue vs Goal"

data:
  provider: inline
  rows:
      - { month: "Jan", revenue: 120000 }
      - { month: "Feb", revenue: 135000 }
      - { month: "Mar", revenue: 148000 }
      - { month: "Apr", revenue: 162000 }
      - { month: "May", revenue: 145000 }
      - { month: "Jun", revenue: 158000 }

visualize:
  type: bar
  columns: month
  rows: revenue

  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"

  annotations:
    - type: line
      axis: left
      value: 150000
      orientation: horizontal
      label: "Monthly Goal"
      labelPosition: end
      color: "#34a853"
      strokeWidth: 2
      dashArray: "5,5"
      opacity: 1.0
```

### Revenue Trend with Target Range

Shows revenue trend with a target range band:

```chartml
type: chart
version: 1
title: "Revenue Performance with Target Range"

data:
  provider: inline
  rows:
      - { month: "Jan", revenue: 120000 }
      - { month: "Feb", revenue: 135000 }
      - { month: "Mar", revenue: 148000 }
      - { month: "Apr", revenue: 162000 }
      - { month: "May", revenue: 145000 }
      - { month: "Jun", revenue: 158000 }

visualize:
  type: line
  columns: month
  rows: revenue

  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"

  annotations:
    # Target range shown as shaded band
    - type: band
      axis: left
      from: 140000
      to: 160000
      orientation: horizontal
      label: "Target Range"
      color: "#34a853"
      opacity: 0.15
      strokeColor: "#34a853"
      strokeWidth: 1
```

### Combo Chart: Actual vs Target with Stretch Goal

Shows actual revenue (bars) vs target (line) with both target band and stretch goal line:

```chartml
type: chart
version: 1
title: "Revenue Performance - Actuals vs Targets"

data:
  provider: inline
  rows:
      - { month: "Jan", actual: 120000, target: 130000 }
      - { month: "Feb", actual: 135000, target: 135000 }
      - { month: "Mar", actual: 148000, target: 140000 }
      - { month: "Apr", actual: 162000, target: 145000 }
      - { month: "May", actual: 145000, target: 150000 }
      - { month: "Jun", actual: 158000, target: 155000 }

visualize:
  type: bar
  columns: month
  rows:
    - field: actual
      mark: bar
      label: "Actual Revenue"
    - field: target
      mark: line
      label: "Target"
      color: "#ea4335"

  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"

  annotations:
    # Comfortable target range
    - type: band
      axis: left
      from: 130000
      to: 160000
      label: "Healthy Range"
      color: "#fbbc04"
      opacity: 0.12

    # Stretch goal line
    - type: line
      axis: left
      value: 170000
      label: "Stretch Goal"
      labelPosition: end
      color: "#4285f4"
      strokeWidth: 2
      dashArray: "5,5"
```

### Event Marker: Product Launch Impact

Shows daily sales with a vertical line marking a product launch date:

```chartml
type: chart
version: 1
title: "Daily Sales - Product Launch Impact"

data:
  provider: inline
  rows:
      - { date: "2025-03-10", sales: 4200 }
      - { date: "2025-03-11", sales: 4350 }
      - { date: "2025-03-12", sales: 4100 }
      - { date: "2025-03-13", sales: 4500 }
      - { date: "2025-03-14", sales: 4400 }
      - { date: "2025-03-15", sales: 6200 }
      - { date: "2025-03-16", sales: 7100 }
      - { date: "2025-03-17", sales: 6800 }
      - { date: "2025-03-18", sales: 7500 }

visualize:
  type: line
  columns: date
  rows: sales

  axes:
    left:
      label: "Daily Sales"
      format: ",.0f"

  annotations:
    # Vertical line marking product launch
    - type: line
      axis: x
      value: "2025-03-15"
      orientation: vertical
      label: "Product Launch"
      labelPosition: start
      color: "#4285f4"
      strokeWidth: 2
      opacity: 0.8
```

---

## Dashboard 1: Q1 2025 Sales Performance

This executive dashboard shows sales metrics across regions and products for the first quarter of 2025.

### Reusable Source: Q1 Sales Data

We'll define a shared source that multiple charts can reference:

```chartml
type: source
version: 1
name: q1_sales
provider: inline
rows:
  - month: "Jan"
    region: "North"
    product: "Widget A"
    revenue: 45000
    units: 150
  - month: "Jan"
    region: "South"
    product: "Widget A"
    revenue: 38000
    units: 130
  - month: "Jan"
    region: "East"
    product: "Widget A"
    revenue: 42000
    units: 140
  - month: "Feb"
    region: "North"
    product: "Widget A"
    revenue: 52000
    units: 170
  - month: "Feb"
    region: "South"
    product: "Widget A"
    revenue: 44000
    units: 145
  - month: "Feb"
    region: "East"
    product: "Widget A"
    revenue: 42000
    units: 140
  - month: "Mar"
    region: "North"
    product: "Widget A"
    revenue: 58000
    units: 190
  - month: "Mar"
    region: "South"
    product: "Widget A"
    revenue: 46000
    units: 155
  - month: "Mar"
    region: "East"
    product: "Widget A"
    revenue: 48000
    units: 160
```

### Monthly Revenue Trend

Total revenue increased steadily throughout Q1, from $125k in January to $152k in March.

```chartml
type: chart
version: 1
title: "Monthly Revenue Trend"

data: q1_sales

aggregate:
  dimensions: [month]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: bar
  columns: month
  rows: total_revenue
  style:
    height: 300
```

### Revenue Distribution by Region

The North region consistently outperformed other regions throughout the quarter.

```chartml
type: chart
version: 1
title: "Revenue by Region"

data: q1_sales

aggregate:
  dimensions: [region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: pie
  columns: region
  rows: total_revenue
  style:
    height: 300
```

### Top Regions (Horizontal Bar Chart)

A horizontal bar chart is useful for comparing categories when you have long labels or want to emphasize rankings.

```chartml
type: chart
version: 1
title: "Total Revenue by Region (Ranked)"

data: q1_sales

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
  orientation: horizontal  # Horizontal bars with categories on Y-axis
  columns: region          # Still the category dimension
  rows: total_revenue      # Still the metric measure
  style:
    height: 250
```

### Regional Performance Over Time

This grouped bar chart shows how each region's revenue changed month-over-month.

```chartml
type: chart
version: 1
title: "Revenue by Region and Month"

data: q1_sales

aggregate:
  dimensions: [month, region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: bar
  mode: grouped
  columns: month
  rows: total_revenue
  marks:
    color: region
  style:
    height: 350
    colors: ["#4285f4", "#ea4335", "#34a853"]
```

### Revenue Composition (Inline Data Example)

Sometimes you need a one-off chart with custom data that doesn't need to be reused:

```chartml
type: chart
version: 1
title: "Revenue Composition by Month"

data:
  provider: inline
  rows:
      - month: "Jan"
        product_line: "Hardware"
        revenue: 65000
      - month: "Jan"
        product_line: "Software"
        revenue: 40000
      - month: "Jan"
        product_line: "Services"
        revenue: 20000
      - month: "Feb"
        product_line: "Hardware"
        revenue: 72000
      - month: "Feb"
        product_line: "Software"
        revenue: 45000
      - month: "Feb"
        product_line: "Services"
        revenue: 21000
      - month: "Mar"
        product_line: "Hardware"
        revenue: 78000
      - month: "Mar"
        product_line: "Software"
        revenue: 52000
      - month: "Mar"
        product_line: "Services"
        revenue: 22000

aggregate:
  dimensions: [month, product_line]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: bar
  mode: stacked
  columns: month
  rows: total_revenue
  marks:
    color: product_line
  style:
    height: 350
```

---

## Dashboard 2: Advanced Analytics

Advanced chart types including combo charts, dual y-axis, multi-line trends, area charts, and scatter plots.

### Actual Revenue vs Target

Combo chart showing actual revenue (bars) against target goals (line).

```chartml
type: chart
version: 1
title: "Actual Revenue vs Target"

data:
  provider: inline
  rows:
      - month: "Jan"
        actual: 125000
        target: 120000
      - month: "Feb"
        actual: 138000
        target: 130000
      - month: "Mar"
        actual: 152000
        target: 145000

visualize:
  type: bar
  columns: month
  rows:
    - field: actual
      mark: bar
      color: "#4285f4"
      label: "Actual Revenue"
    - field: target
      mark: line
      color: "#ea4335"
      label: "Target"
  style:
    height: 300
```

### Revenue and Customer Growth (Dual Y-Axis)

This chart shows two different scales: revenue (left axis) and customer count (right axis).

```chartml
type: chart
version: 1
title: "Revenue and Customer Growth"

data:
  provider: inline
  rows:
      - month: "Jan"
        revenue: 125000
        customers: 450
      - month: "Feb"
        revenue: 138000
        customers: 485
      - month: "Mar"
        revenue: 152000
        customers: 520

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
  style:
    height: 350
```

### Regional Revenue Trends

Multi-line chart showing weekly revenue trends for each region.

```chartml
type: chart
version: 1
title: "Regional Revenue Trends"

data:
  provider: inline
  rows:
      - week: "Week 1"
        region: "North"
        revenue: 42000
      - week: "Week 1"
        region: "South"
        revenue: 38000
      - week: "Week 1"
        region: "East"
        revenue: 35000
      - week: "Week 2"
        region: "North"
        revenue: 45000
      - week: "Week 2"
        region: "South"
        revenue: 40000
      - week: "Week 2"
        region: "East"
        revenue: 37000
      - week: "Week 3"
        region: "North"
        revenue: 48000
      - week: "Week 3"
        region: "South"
        revenue: 42000
      - week: "Week 3"
        region: "East"
        revenue: 40000
      - week: "Week 4"
        region: "North"
        revenue: 52000
      - week: "Week 4"
        region: "South"
        revenue: 45000
      - week: "Week 4"
        region: "East"
        revenue: 43000

aggregate:
  dimensions: [week, region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: line
  columns: week
  rows: total_revenue
  marks:
    color: region
  style:
    height: 300
```

### Cumulative Revenue Growth

Area chart showing total cumulative revenue across all regions over time.

```chartml
type: chart
version: 1
title: "Cumulative Revenue Growth"

data:
  provider: inline
  rows:
      - week: "Week 1"
        revenue: 115000
      - week: "Week 2"
        revenue: 122000
      - week: "Week 3"
        revenue: 130000
      - week: "Week 4"
        revenue: 140000
      - week: "Week 5"
        revenue: 145000
      - week: "Week 6"
        revenue: 158000

visualize:
  type: area
  columns: week
  rows: revenue
  axes:
    left:
      label: "Revenue ($)"
  style:
    height: 300
```

### Regional Revenue Composition Over Time

Stacked area chart showing how each region contributes to total revenue week-over-week.

```chartml
type: chart
version: 1
title: "Regional Revenue Composition"

data:
  provider: inline
  rows:
      - week: "Week 1"
        region: "North"
        revenue: 42000
      - week: "Week 1"
        region: "South"
        revenue: 38000
      - week: "Week 1"
        region: "East"
        revenue: 35000
      - week: "Week 2"
        region: "North"
        revenue: 45000
      - week: "Week 2"
        region: "South"
        revenue: 40000
      - week: "Week 2"
        region: "East"
        revenue: 37000
      - week: "Week 3"
        region: "North"
        revenue: 48000
      - week: "Week 3"
        region: "South"
        revenue: 42000
      - week: "Week 3"
        region: "East"
        revenue: 40000
      - week: "Week 4"
        region: "North"
        revenue: 52000
      - week: "Week 4"
        region: "South"
        revenue: 45000
      - week: "Week 4"
        region: "East"
        revenue: 43000
      - week: "Week 5"
        region: "North"
        revenue: 54000
      - week: "Week 5"
        region: "South"
        revenue: 46000
      - week: "Week 5"
        region: "East"
        revenue: 45000
      - week: "Week 6"
        region: "North"
        revenue: 58000
      - week: "Week 6"
        region: "South"
        revenue: 50000
      - week: "Week 6"
        region: "East"
        revenue: 50000

aggregate:
  dimensions: [week, region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: area
  mode: stacked
  columns: week
  rows: total_revenue
  marks:
    color: region
  axes:
    left:
      label: "Revenue ($)"
  style:
    height: 300
```

### Daily Sales Trend (Responsive Label Test)

This chart has many data points (30 days) to demonstrate automatic label reduction on narrow screens. Try resizing the browser to see labels automatically skip.

```chartml
type: chart
version: 1
title: "Daily Sales Trend - 30 Days"

data:
  provider: inline
  rows:
      - day: "Day 1"
        sales: 12000
      - day: "Day 2"
        sales: 13500
      - day: "Day 3"
        sales: 11800
      - day: "Day 4"
        sales: 14200
      - day: "Day 5"
        sales: 15600
      - day: "Day 6"
        sales: 14800
      - day: "Day 7"
        sales: 16200
      - day: "Day 8"
        sales: 13900
      - day: "Day 9"
        sales: 15200
      - day: "Day 10"
        sales: 14500
      - day: "Day 11"
        sales: 16800
      - day: "Day 12"
        sales: 15900
      - day: "Day 13"
        sales: 17200
      - day: "Day 14"
        sales: 16500
      - day: "Day 15"
        sales: 18100
      - day: "Day 16"
        sales: 17400
      - day: "Day 17"
        sales: 16900
      - day: "Day 18"
        sales: 18500
      - day: "Day 19"
        sales: 17800
      - day: "Day 20"
        sales: 19200
      - day: "Day 21"
        sales: 18600
      - day: "Day 22"
        sales: 17300
      - day: "Day 23"
        sales: 19800
      - day: "Day 24"
        sales: 18900
      - day: "Day 25"
        sales: 20100
      - day: "Day 26"
        sales: 19500
      - day: "Day 27"
        sales: 21000
      - day: "Day 28"
        sales: 20300
      - day: "Day 29"
        sales: 19700
      - day: "Day 30"
        sales: 21500

visualize:
  type: line
  columns: day
  rows: sales
  axes:
    left:
      label: "Sales ($)"
  style:
    height: 300
```

### Regional Market Share Over Time

Normalized stacked area chart (100% stacked) showing each region's percentage share of total revenue over time, similar to a time-series pie chart. This chart shows North losing market share to East over the quarter.

```chartml
type: chart
version: 1
title: "Regional Market Share Over Time"

data:
  provider: inline
  rows:
      - week: "Week 1"
        region: "North"
        revenue: 50000
      - week: "Week 1"
        region: "South"
        revenue: 35000
      - week: "Week 1"
        region: "East"
        revenue: 30000
      - week: "Week 2"
        region: "North"
        revenue: 52000
      - week: "Week 2"
        region: "South"
        revenue: 38000
      - week: "Week 2"
        region: "East"
        revenue: 35000
      - week: "Week 3"
        region: "North"
        revenue: 51000
      - week: "Week 3"
        region: "South"
        revenue: 40000
      - week: "Week 3"
        region: "East"
        revenue: 44000
      - week: "Week 4"
        region: "North"
        revenue: 48000
      - week: "Week 4"
        region: "South"
        revenue: 42000
      - week: "Week 4"
        region: "East"
        revenue: 52000
      - week: "Week 5"
        region: "North"
        revenue: 46000
      - week: "Week 5"
        region: "South"
        revenue: 43000
      - week: "Week 5"
        region: "East"
        revenue: 58000
      - week: "Week 6"
        region: "North"
        revenue: 42000
      - week: "Week 6"
        region: "South"
        revenue: 45000
      - week: "Week 6"
        region: "East"
        revenue: 68000

aggregate:
  dimensions: [week, region]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue

visualize:
  type: area
  mode: normalized
  columns: week
  rows: total_revenue
  marks:
    color: region
  axes:
    left:
      label: "Market Share"
  style:
    height: 300
    colors: ["#4285f4", "#ea4335", "#34a853"]
```

### Revenue Efficiency Analysis

Scatter plot showing the relationship between revenue and profit, sized by unit sales.

```chartml
type: chart
version: 1
title: "Revenue Efficiency Analysis"

data:
  provider: inline
  rows:
      - product: "Widget A"
        revenue: 125000
        profit: 45000
        units: 2400
        category: "Hardware"
      - product: "Widget B"
        revenue: 98000
        profit: 38000
        units: 1800
        category: "Hardware"
      - product: "Software X"
        revenue: 156000
        profit: 92000
        units: 450
        category: "Software"
      - product: "Software Y"
        revenue: 134000
        profit: 78000
        units: 380
        category: "Software"
      - product: "Service A"
        revenue: 67000
        profit: 28000
        units: 120
        category: "Services"
      - product: "Service B"
        revenue: 89000
        profit: 35000
        units: 150
        category: "Services"

visualize:
  type: scatter
  columns: revenue
  rows: profit
  marks:
    color: category
    size: units
  style:
    height: 400
```

---

## Dashboard 3: Product Performance Analysis

Advanced data transformations with filtering, calculated measures, and sorting.

### Top Products with Complex Filtering

This chart demonstrates filtering at both the pre-aggregation (WHERE) and post-aggregation (HAVING) levels.

```chartml
type: chart
version: 1
title: "Top 10 Products by Revenue"

data:
  provider: inline
  rows:
      - product: "Widget A"
        category: "Electronics"
        sale_date: "2025-01-15"
        revenue: 1200
        units: 24
      - product: "Widget A"
        category: "Electronics"
        sale_date: "2025-01-22"
        revenue: 1350
        units: 27
      - product: "Widget B"
        category: "Electronics"
        sale_date: "2025-01-10"
        revenue: 950
        units: 19
      - product: "Widget B"
        category: "Electronics"
        sale_date: "2025-01-25"
        revenue: 1100
        units: 22
      - product: "Gadget X"
        category: "Electronics"
        sale_date: "2025-01-12"
        revenue: 2200
        units: 11
      - product: "Gadget X"
        category: "Electronics"
        sale_date: "2025-01-28"
        revenue: 2400
        units: 12
      - product: "Tool Pro"
        category: "Hardware"
        sale_date: "2025-01-08"
        revenue: 450
        units: 45
      - product: "Tool Pro"
        category: "Hardware"
        sale_date: "2025-01-20"
        revenue: 520
        units: 52

aggregate:
  dimensions:
    - product
    - category

  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
    - column: units
      aggregation: sum
      name: total_units
    - expression: "total_revenue / total_units"
      name: avg_price

  filters:
    rules:
      - field: category
        operator: "="
        value: "Electronics"
      - field: total_revenue
        operator: ">="
        value: 2000

  sort:
    - field: total_revenue
      direction: desc

  limit: 10

visualize:
  type: bar
  columns: product
  rows: total_revenue
  style:
    height: 350
```

### Profit Margin Analysis

Chained calculated measures: profit = revenue - cost, then margin = profit / revenue.

```chartml
type: chart
version: 1
title: "Profit Margin by Product Line"

data:
  provider: inline
  rows:
      - product_line: "Hardware"
        revenue: 125000
        cost: 78000
      - product_line: "Software"
        revenue: 180000
        cost: 65000
      - product_line: "Services"
        revenue: 95000
        cost: 42000

aggregate:
  dimensions: [product_line]

  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
    - column: cost
      aggregation: sum
      name: total_cost
    - expression: "total_revenue - total_cost"
      name: profit
    - expression: "profit / total_revenue"
      name: profit_margin

visualize:
  type: bar
  columns: product_line
  rows:
    - field: profit_margin
      format: ".1%"
      label: "Profit Margin"
  style:
    height: 300
```

---

## Dashboard 5: Market Share Analysis

Pie and doughnut charts for showing proportional data.

### Market Share by Category

```chartml
type: chart
version: 1
title: "Market Share by Category"

data:
  provider: inline
  rows:
      - category: "Electronics"
        revenue: 385000
      - category: "Hardware"
        revenue: 245000
      - category: "Software"
        revenue: 420000
      - category: "Services"
        revenue: 180000

visualize:
  type: pie
  columns: category
  rows: revenue
  style:
    height: 400
```

### Revenue Distribution by Region

```chartml
type: chart
version: 1
title: "Revenue Distribution by Region"

data:
  provider: inline
  rows:
      - region: "North America"
        revenue: 520000
      - region: "Europe"
        revenue: 380000
      - region: "Asia Pacific"
        revenue: 450000
      - region: "Latin America"
        revenue: 125000

visualize:
  type: doughnut
  columns: region
  rows: revenue
  style:
    height: 400
    colors: ["#4285f4", "#ea4335", "#fbbc04", "#34a853"]
```

---

## Dashboard 6: Advanced Transformations

Complex examples with calculated dimensions and chained calculated measures.

### Monthly Trends with Date Truncation

Demonstrates calculated dimensions using SQL expressions.

```chartml
type: chart
version: 1
title: "Monthly Revenue Trends (Last 90 Days)"

data:
  provider: inline
  rows:
      - sale_date: "2025-01-05"
        product: "Widget A"
        revenue: 1200
        units: 24
      - sale_date: "2025-01-12"
        product: "Widget A"
        revenue: 1350
        units: 27
      - sale_date: "2025-01-20"
        product: "Widget B"
        revenue: 950
        units: 19
      - sale_date: "2025-02-03"
        product: "Widget A"
        revenue: 1400
        units: 28
      - sale_date: "2025-02-15"
        product: "Widget B"
        revenue: 1100
        units: 22
      - sale_date: "2025-03-08"
        product: "Widget A"
        revenue: 1550
        units: 31
      - sale_date: "2025-03-22"
        product: "Widget B"
        revenue: 1250
        units: 25

aggregate:
  dimensions:
    - product
    - column: "DATE_TRUNC('month', sale_date::DATE)"
      name: month
      type: date

  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
    - column: units
      aggregation: sum
      name: total_units

  sort:
    - field: month
      direction: asc

visualize:
  type: line
  columns: month
  rows: total_revenue
  marks:
    color: product
  style:
    height: 350
```

### Performance Metrics with Chained Calculations

Multiple calculated measures that reference each other.

```chartml
type: chart
version: 1
title: "Performance Metrics Dashboard"

data:
  provider: inline
  rows:
      - region: "North"
        revenue: 250000
        cost: 165000
        customers: 850
      - region: "South"
        revenue: 180000
        cost: 125000
        customers: 620
      - region: "East"
        revenue: 220000
        cost: 145000
        customers: 750

aggregate:
  dimensions: [region]

  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
    - column: cost
      aggregation: sum
      name: total_cost
    - column: customers
      aggregation: sum
      name: total_customers
    - expression: "total_revenue - total_cost"
      name: profit
    - expression: "profit / total_revenue"
      name: profit_margin
    - expression: "total_revenue / total_customers"
      name: revenue_per_customer

  filters:
    rules:
      - field: profit
        operator: ">"
        value: 50000

  sort:
    - field: profit_margin
      direction: desc

visualize:
  type: bar
  columns: region
  rows:
    - field: profit
      format: "$,.0f"
      label: "Profit"
  style:
    height: 300
```

---

## Phase 1 Customization Features Demo

### Number Formatting & Data Labels

This chart demonstrates three Phase 1 features working together:
1. **Number formatting** on axis (`.format`)
2. **Data labels** on marks (`.dataLabels`)
3. **Axis min/max** control

```chartml
type: chart
version: 1
title: Monthly Sales with Formatting & Labels
data:
  provider: inline
  rows:
  - month: Jan
    revenue: 45000
    units: 1200
  - month: Feb
    revenue: 52000
    units: 1350
  - month: Mar
    revenue: 48000
    units: 1280
  - month: Apr
    revenue: 61000
    units: 1520
  - month: May
    revenue: 58000
    units: 1450
  - month: Jun
    revenue: 67000
    units: 1680
visualize:
  type: bar
  columns: month
  rows:
    field: revenue
    label: Revenue
    dataLabels:
      show: true
      position: top
      format: $,.0f
  axes:
    left:
      label: Revenue ($)
      format: $,.0f
      min: 0
      max: 70000
  style:
    height: 400
```

---

### Grid Lines Customization

Demonstrating configurable grid lines with custom color and opacity:

```chartml
type: chart
version: 1
title: Sales Trend with Custom Grid
data:
  provider: inline
  rows:
  - month: Jan
    sales: 42000
  - month: Feb
    sales: 48000
  - month: Mar
    sales: 51000
  - month: Apr
    sales: 47000
  - month: May
    sales: 55000
  - month: Jun
    sales: 59000
visualize:
  type: line
  columns: month
  rows:
    field: sales
    label: Sales
  axes:
    left:
      label: Sales
      format: $,.0f
  style:
    height: 400
    grid:
      x: true
      y: true
      color: '#e0e0e0'
      opacity: 0.5
      dashArray: 2,2
    showDots: true
```

---

### Percentage Formatting with Data Labels

Perfect for displaying percentages on charts:

```chartml
type: chart
version: 1
title: Conversion Rates by Channel
data:
  provider: inline
  rows:
  - channel: Email
    conversion_rate: 0.042
  - channel: Social
    conversion_rate: 0.028
  - channel: Search
    conversion_rate: 0.055
  - channel: Direct
    conversion_rate: 0.067
  - channel: Referral
    conversion_rate: 0.031
visualize:
  type: bar
  columns: channel
  rows:
    field: conversion_rate
    label: Conversion Rate
    dataLabels:
      show: true
      position: top
      format: .1%
  axes:
    left:
      label: Conversion Rate
      format: .1%
      min: 0
      max: 0.08
  style:
    height: 400
    grid:
      y: true
```

---

### SI Prefix Formatting (K, M, B)

Using abbreviated number format for large values:

```chartml
type: chart
version: 1
title: Annual Revenue by Product Line
data:
  provider: inline
  rows:
  - product: Enterprise
    revenue: 4500000
  - product: Professional
    revenue: 2800000
  - product: Starter
    revenue: 950000
  - product: Premium
    revenue: 1750000
visualize:
  type: bar
  columns: product
  rows:
    field: revenue
    label: Revenue
    dataLabels:
      show: true
      position: top
      format: ~s
  axes:
    left:
      label: Revenue
      format: ~s
      min: 0
  style:
    height: 400
```

---

### Multi-Line Chart with All Features

Combining formatting, grid lines, and data labels on a multi-series chart:

```chartml
type: chart
version: 1
title: Revenue vs Target
data:
  provider: inline
  rows:
  - month: Jan
    revenue: 85000
    target: 90000
  - month: Feb
    revenue: 92000
    target: 90000
  - month: Mar
    revenue: 88000
    target: 95000
  - month: Apr
    revenue: 105000
    target: 95000
  - month: May
    revenue: 98000
    target: 100000
  - month: Jun
    revenue: 112000
    target: 100000
visualize:
  type: line
  columns: month
  rows:
  - field: revenue
    label: Actual Revenue
    mark: line
    color: '#4285f4'
    dataLabels:
      show: true
      position: top
      format: $,.0f
      fontSize: 11
  - field: target
    label: Target
    mark: line
    color: '#ea4335'
    dataLabels:
      show: true
      position: top
      format: $,.0f
      fontSize: 11
      color: '#ea4335'
  axes:
    left:
      label: Amount ($)
      format: $,.0f
      min: 80000
      max: 115000
  style:
    height: 400
    showDots: true
    grid:
      y: true
      color: '#f0f0f0'
```

---

### Combo Chart with Formatted Axes

Bar + line combo with different formatting on each axis:

```chartml
type: chart
version: 1
title: Revenue & Customer Count
data:
  provider: inline
  rows:
  - month: Q1
    revenue: 285000
    customers: 1250
  - month: Q2
    revenue: 312000
    customers: 1380
  - month: Q3
    revenue: 298000
    customers: 1320
  - month: Q4
    revenue: 345000
    customers: 1520
visualize:
  type: bar
  columns: month
  rows:
  - field: revenue
    label: Revenue
    mark: bar
    axis: left
    color: '#4285f4'
    dataLabels:
      show: true
      position: top
      format: $,.0f
  - field: customers
    label: Customers
    mark: line
    axis: right
    color: '#34a853'
    dataLabels:
      show: true
      position: top
      format: ',.0f'
  axes:
    left:
      label: Revenue ($)
      format: $,.0f
      min: 0
    right:
      label: Customers
      format: ',.0f'
      min: 0
  style:
    height: 400
    showDots: true
    grid:
      y: true
```

---

### Data Labels with Center Positioning

Using `position: "center"` to place labels inside bars:

```chartml
type: chart
version: 1
title: Product Mix (with centered labels)
data:
  provider: inline
  rows:
  - category: Electronics
    revenue: 125000
  - category: Apparel
    revenue: 98000
  - category: Home
    revenue: 82000
  - category: Sports
    revenue: 67000
visualize:
  type: bar
  columns: category
  rows:
    field: revenue
    label: Revenue
    dataLabels:
      show: true
      position: center
      format: $,.0f
      color: '#ffffff'
      fontSize: 14
  axes:
    left:
      label: Revenue ($)
      format: $,.0f
  style:
    height: 400
```

---

## Interactive Dashboard Parameters

This example demonstrates the dashboard parameters pattern for creating interactive dashboards where parameter controls update all charts dynamically.

### How It Works

**1. Define Named Params Block** - Creates shared interactive UI controls:
```chartml
type: params
version: 1
name: dashboard_filters          # Required - unique block name
params:
  - id: region
    type: multiselect
    label: "Region"
    options: ["US", "EU", "APAC", "LATAM"]
    default: ["US", "EU"]

  - id: date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"

  - id: min_revenue
    type: number
    label: "Minimum Revenue ($)"
    default: 1000
```

**2. Reference Parameters in Charts** - Use `$blockname.param_id` variable syntax:
```chartml
type: chart
version: 1
title: "Regional Sales"

data:
  provider: inline
  rows:
      - region: "US"
        month: "2024-01"
        revenue: 15000
      - region: "EU"
        month: "2024-01"
        revenue: 12000
      - region: "APAC"
        month: "2024-01"
        revenue: 8000

aggregate:
  filters:
    combinator: and
    rules:
      - field: region
        operator: in
        value: "$dashboard_filters.region"       # ← References named param

      - field: revenue
        operator: ">="
        value: "$dashboard_filters.min_revenue"  # ← References named param

visualize:
  type: bar
  columns: region
  rows: revenue
  style:
    height: 400
```

### Complete Dashboard Example

**Named params block (shared across all charts):**

```chartml
type: params
version: 1
name: dashboard_filters
params:
  - id: region
    type: multiselect
    label: "Region"
    options: ["US", "EU", "APAC"]
    default: ["US", "EU"]

  - id: min_revenue
    type: number
    label: "Minimum Revenue ($)"
    default: 5000
```

**Chart using named params:**

```chartml
type: chart
version: 1
title: "Quarterly Revenue by Region"

data:
  provider: inline
  rows:
      - region: "US"
        q1: 125000
        q2: 138000
        customers: 1200
      - region: "EU"
        q1: 98000
        q2: 112000
        customers: 980
      - region: "APAC"
        q1: 75000
        q2: 88000
        customers: 750

aggregate:
  dimensions: [region]
  measures:
    - column: q1
      aggregation: sum
      name: q1_revenue
    - column: q2
      aggregation: sum
      name: q2_revenue
    - expression: "q1_revenue + q2_revenue"
      name: total_revenue

  filters:
    combinator: and
    rules:
      - field: region
        operator: in
        value: "$dashboard_filters.region"      # Named param reference

      - field: total_revenue
        operator: ">="
        value: "$dashboard_filters.min_revenue" # Named param reference

visualize:
  type: bar
  mode: grouped
  columns: region
  rows:
    - field: q1_revenue
      label: "Q1"
    - field: q2_revenue
      label: "Q2"
  axes:
    left:
      label: "Revenue ($)"
      format: "$,.0f"
  style:
    height: 400
```

**Chart with Chart-Level Inline Params:**

```chartml
type: chart
version: 1
title: "Top Products"

params:  # Chart-level params (no name field)
  - id: top_n
    type: number
    label: "Top N"
    default: 10

data:
  provider: inline
  rows:
    - product: "Widget A"
      revenue: 50000
    - product: "Widget B"
      revenue: 45000
    - product: "Widget C"
      revenue: 38000

aggregate:
  dimensions: [product]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
  sort:
    - field: total_revenue
      direction: desc
  limit: "$top_n"  # Chart-level param (no prefix)

visualize:
  type: bar
  columns: product
  rows: total_revenue
  style:
    height: 400
```

### Parameter Types

**Available parameter types:**

1. **multiselect** - Checkbox group for multiple selections
   ```yaml
   - id: regions
     type: multiselect
     label: "Regions"
     options: ["US", "EU", "APAC"]
     default: ["US"]
   ```

2. **select** - Dropdown for single selection
   ```yaml
   - id: category
     type: select
     label: "Category"
     options: ["All", "Electronics", "Clothing"]
     default: "All"
   ```

3. **daterange** - Start and end date inputs
   ```yaml
   - id: period
     type: daterange
     label: "Date Range"
     default:
       start: "2024-01-01"
       end: "2024-12-31"
   ```

4. **number** - Numeric input
   ```yaml
   - id: threshold
     type: number
     label: "Min Revenue"
     default: 1000
   ```

5. **text** - Text search input
   ```yaml
   - id: search
     type: text
     label: "Search Products"
     placeholder: "Enter product name..."
     default: ""
   ```

### Variable Resolution

**Nested parameter values** are accessed using dot notation (named params):
```yaml
aggregate:
  filters:
    rules:
      - field: date
        operator: between
        value: ["$dashboard_filters.date_range.start", "$dashboard_filters.date_range.end"]
```

**Resolution happens before pipeline execution:**
- `"$dashboard_filters.region"` → `["US", "EU"]` (from named params block state)
- `"$dashboard_filters.min_revenue"` → `5000` (from named params block state)
- ChartML pipeline receives resolved values and executes normally

### Chart-Level vs Dashboard-Level Parameters

**Dashboard-Level** (standalone params block):
```chartml
type: params
version: 1
name: global_filters  # Required - unique block name
params:
  - id: global_date_range
    type: daterange
    label: "Date Range"
    default:
      start: "2024-01-01"
      end: "2024-12-31"
```

**Chart-Level** (inline in chart):
```chartml
type: chart
version: 1
title: "Revenue by Region"

params:  # Chart-specific parameters
  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC"]
    default: ["US"]

data:
  provider: inline
  rows:
    - region: "US"
      revenue: 25000
    - region: "EU"
      revenue: 18000
    - region: "APAC"
      revenue: 12000

visualize:
  type: bar
  columns: region
  rows: revenue
```

**Variable Syntax:**
- Dashboard-level params: `$blockname.param_id` (e.g., `$global_filters.global_date_range`)
- Chart-level params: `$param_id` (e.g., `$selected_regions`)

**Resolution:**
- Has dot → named param from registry (e.g., `$global_filters.global_date_range`)
- No dot → chart-level param from inline `params:` section (e.g., `$selected_regions`)

### Complete Chart-Level Params Example

This example shows a chart with its own inline parameters that render above the chart:

```chartml
type: chart
version: 1
title: "Regional Revenue Analysis"

params:
  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC", "LATAM"]
    default: ["US", "EU"]
    layout:
      colSpan: 6

  - id: min_revenue
    type: number
    label: "Minimum Revenue ($)"
    default: 10000
    layout:
      colSpan: 6

data:
  provider: inline
  rows:
      - region: "US"
        product: "Widget A"
        revenue: 25000
      - region: "EU"
        product: "Widget A"
        revenue: 18000
      - region: "APAC"
        product: "Widget A"
        revenue: 12000
      - region: "LATAM"
        product: "Widget A"
        revenue: 8000
      - region: "US"
        product: "Widget B"
        revenue: 30000
      - region: "EU"
        product: "Widget B"
        revenue: 22000

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

      - field: total_revenue
        operator: ">="
        value: "$min_revenue"  # Chart-level param (no prefix)

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

**How it renders:**
1. Parameter controls appear above the chart (responsive grid layout)
2. User changes "Regions" multiselect → chart filters to selected regions
3. User changes "Minimum Revenue" → chart filters to regions above threshold
4. Parameters only affect this specific chart (not other charts on the page)

**When to use chart-level params:**
- ✅ Chart-specific filtering (only applies to one chart)
- ✅ Self-contained charts that don't share parameters
- ✅ Rapid prototyping without creating dashboard-level params

**When to use dashboard-level params:**
- ✅ Multiple charts share the same parameters
- ✅ Global filtering across entire dashboard
- ✅ Cleaner markdown (params defined once at top)

### Key Features

- ✅ **Parameters defined once** - Used across all charts in the dashboard
- ✅ **Chart-level control** - Each chart chooses which parameters to use
- ✅ **URL-based sharing** - Parameter state stored in URL query params (bookmarkable)
- ✅ **No spec changes** - Uses existing `transform.filters` structure
- ✅ **Portable** - Parameter values can come from URL, React state, CLI, config files
- ✅ **Inline rendering** - Parameters render where defined in markdown

### Implementation Notes

**How parameter blocks work:**
1. `````chartml``` block with `type: params` parsed from markdown
2. Renders interactive DashboardParams component at block location
3. User changes parameter → updates parameter state → updates URL
4. Before chart execution: `$blockname.param_id` variables resolved to actual values
5. ChartML pipeline executes with resolved parameter values
6. Charts re-render with filtered data

**Benefits:**
- No ChartML spec changes required
- Parameters are just ChartML blocks with `type: params`
- Variable resolution is simple string replacement
- URL updates enable shareable filtered views
- Each chart independently chooses which parameters apply

---

## Summary

These mock dashboards demonstrate:

**Core Features:**
- **Reusable sources** - Define once, reference multiple times
- **Interactive parameters** - Dynamic dashboard filtering with URL-based sharing
- **All chart types** - Bar, line, area, scatter, pie, doughnut, table, metric
- **Chart modes** - Grouped bars, stacked bars, stacked areas, combo charts, dual y-axis
- **Data transformations** - Dimensions, measures, filters, calculations
- **Calculated fields** - Pre and post-aggregation calculations
- **Smart filtering** - WHERE vs HAVING auto-detection
- **Field references** - Chained calculated measures
- **Inline vs source** - Both patterns demonstrated
- **Markdown integration** - Narrative content with executable specs

**Phase 1 Customization Features:**
- **Number/Date Formatting** - Format axis labels and data labels (`$,.0f`, `.1%`, `~s`, date formats)
- **Axis Min/Max Control** - Override auto-scaling with custom bounds
- **Grid Line Customization** - Control visibility, color, opacity, dash patterns (x/y grids)
- **Data Labels on Marks** - Display values on bars, lines, and areas with positioning and formatting
