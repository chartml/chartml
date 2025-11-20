# ChartML

<div align="center">
  <h3>A declarative markup language for creating beautiful, interactive data visualizations</h3>
  <p>
    <a href="https://chartml.org">Website</a> •
    <a href="https://chartml.org/spec">Specification</a> •
    <a href="https://chartml.org/examples">Examples</a> •
    <a href="https://chartml.org/quick-reference">Quick Reference</a>
  </p>
</div>

---

## What is ChartML?

ChartML is a YAML-based markup language that lets you create charts and dashboards with simple, declarative syntax. No JavaScript required—just describe what you want, and ChartML handles the rest.

```yaml
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
```

That's it! ChartML transforms this into a beautiful, interactive D3-powered chart.

## Features

- ✅ **Built-in Chart Types**: Bar, line, and area charts included
- ✅ **Plugin System**: Extend with pie charts, scatter plots, and custom renderers
- ✅ **Built-in Aggregation**: GROUP BY, SUM, AVG, COUNT, MIN, MAX using d3-array
- ✅ **Zero Extra Dependencies**: Only d3 and js-yaml
- ✅ **Lightweight**: ~21 KB gzipped
- ✅ **Framework Agnostic**: Works with React, Vue, vanilla JS, or as markdown

## Quick Start

### Installation

```bash
npm install @chartml/core
```

### Basic Usage

```javascript
import { ChartML } from '@chartml/core';

const spec = `
data:
  - month: "Jan"
    revenue: 45000
  - month: "Feb"
    revenue: 52000

visualize:
  type: bar
  columns: month
  rows: revenue
`;

const chartml = new ChartML();
await chartml.render(spec, document.getElementById('chart'));
```

### With Plugins

```javascript
import { ChartML } from '@chartml/core';
import '@chartml/chart-pie'; // Auto-registers pie chart renderer

const chartml = new ChartML();
// Now you can use type: pie in your specs
```

## Packages

This monorepo contains:

### Core Library
- **[@chartml/core](./packages/core)** - Core library with parser and built-in charts

### Chart Plugins
- **[@chartml/chart-pie](./packages/chart-pie)** - Pie and doughnut chart plugin
- **[@chartml/chart-scatter](./packages/chart-scatter)** - Scatter plot and bubble chart plugin
- **[@chartml/chart-metric](./packages/chart-metric)** - Metric card plugin for KPIs

### Framework Integrations
- **[@chartml/react](./packages/react)** - React wrapper component
- **[@chartml/markdown-react](./packages/markdown-react)** - React-markdown plugin
- **[@chartml/markdown-it](./packages/markdown-it)** - Markdown-it plugin for static sites
- **[@chartml/markdown-common](./packages/markdown-common)** - Shared utilities for markdown plugins

### Documentation
- **[docs](./docs)** - Documentation website (VitePress)

## Documentation

- **Website**: https://chartml.org
- **Full Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **Quick Reference**: https://chartml.org/quick-reference

## Community Plugins

ChartML has a growing ecosystem of community-created plugins:

- Create your own chart renderers
- Add custom data sources (PostgreSQL, GraphQL, CSV, etc.)
- Build custom aggregation engines

See the [Plugin Architecture Guide](./packages/core/PLUGIN_ARCHITECTURE.md) to learn how to create plugins.

## Development

This is a pnpm monorepo. To get started:

```bash
# Install dependencies
pnpm install

# Build all packages
pnpm build

# Run tests
pnpm test

# Start documentation site
pnpm docs:dev
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

## License

MIT © 2025 Alytic Pty Ltd

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
