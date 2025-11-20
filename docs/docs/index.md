---
layout: home

hero:
  name: "ChartML"
  text: "Interactive Dashboards with Markdown & YAML"
  tagline: Create beautiful data visualizations and interactive dashboards using simple markdown and YAML syntax
  image:
    src: /logo.svg
    alt: ChartML
  actions:
    - theme: brand
      text: Get Started
      link: /spec
    - theme: alt
      text: View Examples
      link: /examples
---

<div class="features-grid" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; margin: 48px auto; max-width: 1200px; padding: 0 24px;">
  <FeatureCard icon="ChartBarIcon" title="Write Dashboards in Markdown" details="Combine charts, text, and parameters in markdown documents. Perfect for documentation and reports." />
  <FeatureCard icon="PaintBrushIcon" title="Style with YAML" details="Clean, readable YAML syntax for data visualization. No JavaScript required." />
  <FeatureCard icon="ArrowPathIcon" title="Interactive Parameters" details="Dynamic filtering and dashboard controls. Parameters update all charts in real-time." />
  <FeatureCard icon="ChartPieIcon" title="Multiple Chart Types" details="Bar, line, area, scatter, pie, doughnut, and metric cards. Built-in and plugin-based renderers." />
  <FeatureCard icon="PuzzlePieceIcon" title="Plugin Architecture" details="Extend with custom chart types, data sources, and aggregation middleware." />
  <FeatureCard icon="SparklesIcon" title="Beautiful by Default" details="Professional-looking visualizations with built-in color palettes and responsive design." />
</div>

---

## What is ChartML?

ChartML is a declarative markup language designed for creating beautiful, interactive dashboards in markdown. It combines the simplicity of YAML with the power of D3.js to make data visualization accessible to everyone.

Write your dashboards as markdown documents with embedded ChartML blocks:

````markdown
# Sales Dashboard

Interactive parameters that control all charts:

```chartml
type: params
version: 1
params:
  - id: selected_regions
    type: multiselect
    label: "Regions"
    options: ["US", "EU", "APAC"]
    default: ["US", "EU"]
```

Revenue trends over time:

```chartml
type: chart
version: 1
title: "Monthly Revenue"

data:
  - month: Jan
    region: US
    revenue: 45000
  - month: Feb
    region: US
    revenue: 52000
  - month: Jan
    region: EU
    revenue: 38000
  - month: Feb
    region: EU
    revenue: 41000

aggregate:
  dimensions: [month]
  measures:
    - column: revenue
      aggregation: sum
      name: total_revenue
  filters:
    rules:
      - field: region
        operator: in
        value: "$selected_regions"

visualize:
  type: line
  columns: month
  rows: total_revenue
  axes:
    left:
      format: "$,.0f"
```
````

## Why ChartML?

**For Data Analysts:** Create interactive dashboards in markdown without writing code. Focus on insights, not implementation.

**For Developers:** Declarative YAML syntax makes dashboards versionable, reviewable, and maintainable. Store charts in git alongside your code.

**For Technical Writers:** Embed live, interactive visualizations directly in documentation. Parameters let readers explore data themselves.

**For Teams:** Share dashboard specifications across tools. ChartML is an open format that works everywhere markdown does.

## Quick Start

```bash
# Static site generators (VitePress, Docusaurus, etc.)
npm install @chartml/markdown-it

# React apps with react-markdown
npm install @chartml/markdown-react

# React apps (direct component usage)
npm install @chartml/react

# Vanilla JavaScript
npm install @chartml/core
```

**All packages include:**
- ✅ Core ChartML library
- ✅ Bar, line, and area charts (built-in)
- ✅ Pie and doughnut charts (plugin)
- ✅ Scatter plots (plugin)
- ✅ Metric cards (plugin)

One command gets you everything you need to start building dashboards!

Check out the [full specification](/spec/) or browse [examples](/examples/) to get started.

## Built With ChartML

ChartML powers [Kyomi](https://kyomi.ai), an AI-powered data analytics platform that generates visualizations from natural language queries.

## Open Source

ChartML is open source and MIT licensed. Contributions welcome!

[View on GitHub](https://github.com/chartml/chartml)
