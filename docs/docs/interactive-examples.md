---
layout: page
---

<script setup>
import MarkdownEditor from './.vitepress/components/MarkdownEditor.vue';

const initialSource = `# ChartML Interactive Examples

Edit this markdown to see live updates! Try modifying the data, changing chart types, or adding new visualizations.

## Interactive Parameters

\`\`\`chartml
type: params
version: 1
name: dashboard_filters
params:
  - id: selected_quarter
    type: select
    label: "Select Quarter"
    options: ["Q1", "Q2", "Q3", "Q4"]
    default: "Q1"
  - id: min_revenue
    type: number
    label: "Minimum Revenue"
    default: 50000
\`\`\`

## Key Metrics

\`\`\`chartml
type: chart
version: 1

data:
  provider: inline
  rows:
    - current: 245000
      previous: 198000

visualize:
  type: metric
  value: current
  label: "Total Revenue"
  format: "$,.0f"
  compareWith: previous
\`\`\`

## Bar Chart - Monthly Revenue

\`\`\`chartml
type: chart
version: 1
title: "Monthly Revenue"

data:
  provider: inline
  rows:
    - month: "Jan"
      revenue: 125000
      quarter: "Q1"
    - month: "Feb"
      revenue: 138000
      quarter: "Q1"
    - month: "Mar"
      revenue: 152000
      quarter: "Q1"
    - month: "Apr"
      revenue: 168000
      quarter: "Q2"
    - month: "May"
      revenue: 145000
      quarter: "Q2"
    - month: "Jun"
      revenue: 172000
      quarter: "Q2"

aggregate:
  dimensions: [month]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
  filters:
    rules:
      - field: quarter
        operator: "="
        value: "$dashboard_filters.selected_quarter"
      - field: revenue
        operator: ">="
        value: "$dashboard_filters.min_revenue"

visualize:
  type: bar
  columns: month
  rows: total_revenue
\`\`\`

## Line Chart - Customer Growth

\`\`\`chartml
type: chart
version: 1
title: "New Customers Over Time"

data:
  provider: inline
  rows:
    - date: "2024-01-01"
      customers: 450
    - date: "2024-01-08"
      customers: 485
    - date: "2024-01-15"
      customers: 520
    - date: "2024-01-22"
      customers: 562
    - date: "2024-01-29"
      customers: 598

visualize:
  type: line
  columns: date
  rows: customers
\`\`\`

## Area Chart - Traffic Volume

\`\`\`chartml
type: chart
version: 1
title: "Website Traffic"

data:
  provider: inline
  rows:
    - day: "Mon"
      visitors: 1200
    - day: "Tue"
      visitors: 1450
    - day: "Wed"
      visitors: 1380
    - day: "Thu"
      visitors: 1520
    - day: "Fri"
      visitors: 1680
    - day: "Sat"
      visitors: 980
    - day: "Sun"
      visitors: 1100

visualize:
  type: area
  columns: day
  rows: visitors
\`\`\`

## Pie Chart - Market Share

\`\`\`chartml
type: chart
version: 1
title: "Product Distribution"

data:
  provider: inline
  rows:
    - product: "Product A"
      share: 35
    - product: "Product B"
      share: 28
    - product: "Product C"
      share: 22
    - product: "Product D"
      share: 15

visualize:
  type: pie
  columns: product
  rows: share
\`\`\`

## Grouped Bar Chart

\`\`\`chartml
type: chart
version: 1
title: "Sales by Region and Product"

data:
  provider: inline
  rows:
    - region: "North"
      product: "Widget A"
      sales: 45000
    - region: "North"
      product: "Widget B"
      sales: 38000
    - region: "South"
      product: "Widget A"
      sales: 52000
    - region: "South"
      product: "Widget B"
      sales: 41000
    - region: "East"
      product: "Widget A"
      sales: 48000
    - region: "East"
      product: "Widget B"
      sales: 45000

aggregate:
  dimensions: [region, product]
  measures:
    - column: sales
      aggregation: sum
      name: total_sales

visualize:
  type: bar
  mode: grouped
  columns: region
  rows: total_sales
  marks:
    color: product
\`\`\`

## Horizontal Bar Chart

\`\`\`chartml
type: chart
version: 1
title: "Top Products by Revenue"

data:
  provider: inline
  rows:
    - product: "Widget Pro"
      revenue: 125000
    - product: "Gadget Plus"
      revenue: 98000
    - product: "Tool Master"
      revenue: 87000
    - product: "Device X"
      revenue: 76000
    - product: "Smart Item"
      revenue: 65000

visualize:
  type: bar
  orientation: horizontal
  columns: product
  rows: revenue
\`\`\`

## Scatter Plot

\`\`\`chartml
type: chart
version: 1
title: "Price vs Sales Volume"

data:
  provider: inline
  rows:
    - price: 29.99
      units: 450
      category: "Electronics"
    - price: 39.99
      units: 380
      category: "Electronics"
    - price: 49.99
      units: 320
      category: "Electronics"
    - price: 19.99
      units: 520
      category: "Clothing"
    - price: 24.99
      units: 480
      category: "Clothing"
    - price: 34.99
      units: 410
      category: "Clothing"

visualize:
  type: scatter
  columns: price
  rows: units
  marks:
    color: category
\`\`\`
`;
</script>

<style scoped>
/* Full-width editor, no container constraints */
:deep(.VPDoc) {
  padding: 0 !important;
  max-width: none !important;
}

:deep(.vp-doc) {
  padding: 0 !important;
  max-width: none !important;
}

:deep(.content) {
  max-width: none !important;
  padding: 0 !important;
}

.page-header {
  max-width: 1200px;
  margin: 0 auto;
  padding: 3rem 2rem 2rem;
}

.page-header h1 {
  font-size: 2.5rem;
  font-weight: 700;
  margin-bottom: 1rem;
  color: var(--vp-c-text-1);
}

.page-header p {
  font-size: 1.125rem;
  color: var(--vp-c-text-2);
  margin-bottom: 2rem;
}

.tips {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
  margin-top: 1.5rem;
}

.tip-card {
  padding: 1rem;
  background: var(--vp-c-bg-soft);
  border-radius: var(--radius-md);
  border: var(--border-width-thin) solid var(--vp-c-divider);
}

.tip-card strong {
  display: block;
  margin-bottom: 0.5rem;
  color: var(--vp-c-text-1);
  font-size: 0.875rem;
}

.tip-card span {
  color: var(--vp-c-text-2);
  font-size: 0.875rem;
}

:deep(.markdown-editor) {
  height: calc(100vh - 200px);
  min-height: 800px;
  border-radius: 0;
  border-left: none;
  border-right: none;
  margin: 0;
}

.page-footer {
  max-width: 1200px;
  margin: 0 auto;
  padding: 3rem 2rem;
  border-top: var(--border-width-thin) solid var(--vp-c-divider);
  margin-top: 3rem;
}

.footer-links {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 2rem;
}

.footer-section h3 {
  font-size: 1rem;
  font-weight: 600;
  margin-bottom: 1rem;
  color: var(--vp-c-text-1);
}

.footer-section ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.footer-section li {
  margin-bottom: 0.5rem;
}

.footer-section a {
  color: var(--vp-c-text-2);
  text-decoration: none;
  font-size: 0.9rem;
  transition: color 0.2s;
}

.footer-section a:hover {
  color: var(--vp-c-brand-1);
}
</style>

<div class="page-header">
  <h1>Interactive ChartML Playground</h1>
  <p>Edit markdown in real-time and see your charts come to life. Perfect for learning ChartML syntax and experimenting with visualizations.</p>

  <div class="tips">
    <div class="tip-card">
      <strong>Live Preview</strong>
      <span>Changes appear instantly as you type</span>
    </div>
    <div class="tip-card">
      <strong>Copy & Reset</strong>
      <span>Use toolbar buttons to copy code or reset to original</span>
    </div>
    <div class="tip-card">
      <strong>Full ChartML Support</strong>
      <span>All chart types, parameters, and interactive features</span>
    </div>
  </div>
</div>

<MarkdownEditor :source="initialSource" />

<div class="page-footer">
  <div class="footer-links">
    <div class="footer-section">
      <h3>Documentation</h3>
      <ul>
        <li><a href="/spec">Full Specification</a></li>
        <li><a href="/quick-reference">Quick Reference</a></li>
        <li><a href="/api">API Reference</a></li>
      </ul>
    </div>
    <div class="footer-section">
      <h3>Examples</h3>
      <ul>
        <li><a href="/examples">Code Examples</a></li>
        <li><a href="/examples#reusable-styles-and-configuration">Reusable Styles</a></li>
        <li><a href="/examples#interactive-parameters">Interactive Parameters</a></li>
      </ul>
    </div>
    <div class="footer-section">
      <h3>Quick Tips</h3>
      <ul>
        <li>Use <code>provider: inline</code> for static data</li>
        <li>All charts need <code>type: chart</code> and <code>version: 1</code></li>
        <li>Parameters let you create interactive filters</li>
      </ul>
    </div>
  </div>
</div>
