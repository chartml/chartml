# chartml-leptos

Leptos components, hooks, and tooltip context for rendering ChartML charts in reactive Leptos apps.

Part of [ChartML](https://chartml.org) — declarative chart markup powered by Rust.

## Usage

```toml
[dependencies]
chartml-leptos = "4.0.2"
```

Provides:
- `ChartMLChart` — reactive chart component that re-renders on spec/data changes and resizes via `ResizeObserver`.
- `use_chartml` / `use_chartml_configured` — hooks for accessing the `ChartML` instance from context.
- `ParamsControls` — UI controls bound to a chart's interactive parameters.
- `TooltipState` + `provide_tooltip_context` / `use_tooltip` — shared tooltip state across charts on a page.
- `CHARTML_CSS` — chart stylesheet as a `&'static str` for SSR or non-Leptos embedding. The `ChartMLChart` component injects this automatically on mount.

See [chartml.org](https://chartml.org) for full documentation.
