# ChartML

<div align="center">
  <h3>Declarative chart markup, powered by Rust/WASM</h3>
  <p>
    <a href="https://chartml.org">Website</a> •
    <a href="https://chartml.org/spec">Specification</a> •
    <a href="https://chartml.org/examples">Examples</a> •
    <a href="https://www.npmjs.com/package/@chartml/core">npm</a> •
    <a href="https://crates.io/crates/chartml-core">crates.io</a> •
    <a href="https://github.com/AlyticInc/chartml">GitHub</a>
  </p>
</div>

---

## What is ChartML?

ChartML is a YAML-based markup language for creating charts and dashboards with simple, declarative syntax. The spec is unchanged from v2 — all existing chart definitions work without modification. In v3, the rendering engine has been rewritten in Rust and compiled to WASM, replacing D3 with native-speed rendering.

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

## Features

- **All Chart Types Built In**: Bar, line, area, pie, doughnut, scatter, bubble, and metric charts — no separate packages needed
- **Rust/WASM Engine**: Native-speed rendering compiled to WebAssembly (~487 KB gzipped, includes all chart types)
- **Plugin System**: `registerRenderer()`, `registerDataSource()`, `registerTransform()`, `setDatasourceResolver()`
- **Framework Support**: React, markdown-it, and Leptos (Rust) integrations
- **Optional SQL Transforms**: `@chartml/datafusion` for SQL-based data transforms
- **Same YAML Spec**: All v2 chart specs work without modification

## Quick Start

### JavaScript / TypeScript

```bash
npm install @chartml/core
```

```javascript
import { ChartML } from '@chartml/core';

const chartml = await ChartML.create();

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

const svg = chartml.renderToSvg(spec);
document.getElementById('chart').innerHTML = svg;
```

### Rust

```bash
cargo add chartml-core chartml-chart-cartesian chartml-render
```

### Framework Integrations

- **[@chartml/react](https://www.npmjs.com/package/@chartml/react)** — React wrapper component
- **[@chartml/markdown-it](https://www.npmjs.com/package/@chartml/markdown-it)** — Markdown-it plugin for static sites
- **[@chartml/markdown-react](https://www.npmjs.com/package/@chartml/markdown-react)** — React markdown component
- **[chartml-leptos](https://crates.io/crates/chartml-leptos)** — Leptos component for Rust web apps

## Migration from v2

- **Async init**: `const chartml = await ChartML.create()` replaces `new ChartML()`
- **API rename**: `renderToSvg()` and `renderToElement()` replace `render()`
- **No separate chart packages**: `@chartml/chart-pie`, `@chartml/chart-scatter`, `@chartml/chart-metric` are deprecated — all chart types are bundled in `@chartml/core`
- **`@chartml/markdown-common` deprecated** — functionality merged into core
- **YAML specs unchanged** — no changes to your chart definitions

## Documentation

- **Website**: https://chartml.org
- **Full Specification**: https://chartml.org/spec
- **Examples**: https://chartml.org/examples
- **Quick Reference**: https://chartml.org/quick-reference

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

## License

MIT © 2025 Alytic Pty Ltd

ChartML is maintained by the team at [Kyomi](https://kyomi.ai) and is the visualization engine that powers the platform.
