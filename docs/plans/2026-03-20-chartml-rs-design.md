# chartml-rs Design Document

**Date:** 2026-03-20
**Status:** Approved
**Author:** Jason Adams

---

## Overview

chartml-rs is a Rust/WASM library that renders ChartML specifications natively in Rust-based frontend frameworks, starting with Leptos. It preserves ChartML's plugin architecture — data sources, transforms, and chart renderers are all extensible via traits — so new chart types, data providers, or transform engines can be added as independent crates without modifying core.

The project produces three deliverables:
1. A framework-agnostic core library (`chartml-core`)
2. A Leptos component library (`chartml-leptos`)
3. A working demo site with live YAML editing and a chart gallery

---

## Repository Structure

New repo: `~/repos/chartml-rs` (GitHub: `chartml/chartml-rs`)

```
chartml-rs/
├── Cargo.toml                    # Workspace definition
├── LICENSE                       # MIT (matches chartml)
├── README.md
├── crates/
│   ├── chartml-core/             # Pure Rust — no framework, no WASM deps
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── spec/             # ChartML YAML parsing → typed structs
│   │       │   ├── mod.rs
│   │       │   ├── chart.rs      # ChartSpec, VisualizeSpec, etc.
│   │       │   ├── source.rs     # SourceSpec
│   │       │   ├── transform.rs  # TransformSpec, AggregateSpec
│   │       │   ├── style.rs      # StyleSpec
│   │       │   ├── params.rs     # ParamsSpec
│   │       │   └── config.rs     # ConfigSpec
│   │       ├── scales/           # Scale implementations
│   │       │   ├── mod.rs
│   │       │   ├── linear.rs     # ScaleLinear (continuous → continuous)
│   │       │   ├── band.rs       # ScaleBand (discrete → continuous)
│   │       │   ├── time.rs       # ScaleTime (temporal → continuous)
│   │       │   ├── ordinal.rs    # ScaleOrdinal (discrete → discrete, for colors)
│   │       │   └── sqrt.rs       # ScaleSqrt (for bubble sizes)
│   │       ├── shapes/           # SVG path generators
│   │       │   ├── mod.rs
│   │       │   ├── line.rs       # Line path generator
│   │       │   ├── area.rs       # Area path generator
│   │       │   ├── arc.rs        # Arc path generator (pie slices)
│   │       │   └── pie.rs        # Pie layout (angles from data)
│   │       ├── layout/           # Chart layout computation
│   │       │   ├── mod.rs
│   │       │   ├── axes.rs       # Tick generation, positioning
│   │       │   ├── labels.rs     # Smart label strategy (rotate/truncate/sample)
│   │       │   ├── margins.rs    # Dynamic margin calculation
│   │       │   ├── legend.rs     # Legend layout computation
│   │       │   └── stack.rs      # Stacking/grouping layout
│   │       ├── format/           # Number and date formatting
│   │       │   ├── mod.rs
│   │       │   ├── number.rs     # d3-format compatible ("$,.0f", "~s", ".2%")
│   │       │   └── date.rs       # strftime-style date formatting
│   │       ├── color/            # Palette and color utilities
│   │       │   ├── mod.rs
│   │       │   ├── palettes.rs   # Built-in palettes (autumn_forest, spectrum_pro, horizon_suite)
│   │       │   └── utils.rs      # HSL conversion, fallback color generation
│   │       ├── plugin/           # Plugin trait definitions
│   │       │   ├── mod.rs
│   │       │   ├── data_source.rs
│   │       │   ├── transform.rs
│   │       │   ├── renderer.rs
│   │       │   └── resolver.rs
│   │       ├── registry.rs       # Plugin registry (trait object storage)
│   │       ├── element.rs        # ChartElement tree (renderer output)
│   │       └── data.rs           # Row type, data utilities (extent, max, sum)
│   │
│   ├── chartml-chart-cartesian/  # Bar, line, area renderer
│   │   ├── Cargo.toml            # Depends on chartml-core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── bar.rs
│   │       ├── line.rs
│   │       ├── area.rs
│   │       └── combo.rs          # Mixed mark types (line + bar)
│   │
│   ├── chartml-chart-pie/        # Pie/doughnut renderer
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   │
│   ├── chartml-chart-scatter/    # Scatter/bubble renderer
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   │
│   ├── chartml-chart-metric/     # Metric card renderer
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   │
│   └── chartml-leptos/           # Leptos framework adapter
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── chart.rs          # <ChartMLChart /> component
│           ├── element.rs        # ChartElement → view! rendering
│           ├── tooltip.rs        # Reactive tooltip component
│           └── hooks.rs          # use_chartml() hook
│
├── demo/                         # Leptos demo site
│   ├── Cargo.toml
│   ├── index.html
│   ├── style/
│   │   └── main.css
│   └── src/
│       ├── main.rs
│       ├── app.rs                # Main app layout
│       ├── editor.rs             # YAML editor panel
│       └── gallery.rs            # Example chart gallery
│
└── fixtures/                     # ChartML YAML test fixtures
    ├── bar_basic.yaml
    ├── bar_stacked.yaml
    ├── bar_grouped.yaml
    ├── bar_horizontal.yaml
    ├── line_basic.yaml
    ├── line_multi_series.yaml
    ├── area_stacked.yaml
    ├── scatter_basic.yaml
    ├── scatter_bubble.yaml
    ├── pie_basic.yaml
    ├── doughnut_basic.yaml
    └── metric_basic.yaml
```

---

## Plugin Architecture

### Design Principle

Extensibility without open-heart surgery. Adding a new chart type, data source, or transform engine is:
1. Create a new crate
2. Implement the relevant trait
3. Register it with the `ChartML` instance

Core never changes.

### Plugin Traits

```rust
// ── crates/chartml-core/src/plugin/data_source.rs ──

/// Data source plugin — fetches raw data from a provider.
///
/// Built-in: InlineDataSource, HttpDataSource
/// External: BigQuery, Postgres, etc.
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Fetch data rows from this source.
    async fn fetch(&self, spec: &DataSpec, options: &FetchOptions) -> Result<Vec<Row>, ChartError>;
}
```

```rust
// ── crates/chartml-core/src/plugin/transform.rs ──

/// Transform middleware — processes data between fetch and render.
///
/// Built-in: aggregate (group by, sum, avg, etc.)
/// External: DuckDB-based transforms, SQL transforms
#[async_trait]
pub trait TransformMiddleware: Send + Sync {
    /// Transform input data according to the spec.
    /// Returns transformed data plus optional metadata.
    async fn transform(
        &self,
        data: Vec<Row>,
        spec: &TransformSpec,
        context: &TransformContext,
    ) -> Result<TransformResult, ChartError>;
}

pub struct TransformResult {
    pub data: Vec<Row>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

```rust
// ── crates/chartml-core/src/plugin/renderer.rs ──

/// Chart renderer plugin — converts data + config into a ChartElement tree.
///
/// Each chart type (bar, pie, scatter, etc.) implements this trait.
/// The framework adapter (Leptos, Dioxus) then renders the ChartElement tree to DOM.
pub trait ChartRenderer: Send + Sync {
    /// Render data with the given config into a ChartElement tree.
    fn render(&self, data: &[Row], config: &ChartConfig) -> Result<ChartElement, ChartError>;

    /// Optional: provide default dimensions for this chart type.
    /// e.g., metric cards return height=150, most charts return height=400.
    fn default_dimensions(&self, _spec: &VisualizeSpec) -> Option<Dimensions> {
        None
    }
}
```

```rust
// ── crates/chartml-core/src/plugin/resolver.rs ──

/// Datasource resolver — resolves a slug (e.g., "production-postgres") to connection config.
///
/// Used by host applications (like Kyomi) that manage datasource connections.
#[async_trait]
pub trait DatasourceResolver: Send + Sync {
    async fn resolve(&self, slug: &str) -> Result<ConnectionConfig, ChartError>;
}
```

### Registry

```rust
// ── crates/chartml-core/src/registry.rs ──

/// Central plugin registry. Holds all registered plugins and provides lookup.
pub struct ChartMLRegistry {
    data_sources: HashMap<String, Box<dyn DataSource>>,
    transforms: Vec<Box<dyn TransformMiddleware>>,
    renderers: HashMap<String, Box<dyn ChartRenderer>>,
    datasource_resolver: Option<Box<dyn DatasourceResolver>>,
}

impl ChartMLRegistry {
    pub fn new() -> Self { /* ... */ }

    pub fn register_data_source(&mut self, name: &str, source: impl DataSource + 'static) { /* ... */ }
    pub fn register_transform(&mut self, middleware: impl TransformMiddleware + 'static) { /* ... */ }
    pub fn set_transform(&mut self, middleware: impl TransformMiddleware + 'static) { /* ... */ }
    pub fn register_renderer(&mut self, chart_type: &str, renderer: impl ChartRenderer + 'static) { /* ... */ }
    pub fn set_datasource_resolver(&mut self, resolver: impl DatasourceResolver + 'static) { /* ... */ }
}
```

### Usage Pattern (mirrors JS API)

```rust
use chartml_core::ChartML;
use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_metric::MetricRenderer;

let mut chartml = ChartML::new();

// Register chart renderers (like JS: chartml.registerChartRenderer('pie', createPieChartRenderer()))
chartml.register_renderer("bar", CartesianRenderer::new());
chartml.register_renderer("line", CartesianRenderer::new());
chartml.register_renderer("area", CartesianRenderer::new());
chartml.register_renderer("pie", PieRenderer::new());
chartml.register_renderer("doughnut", PieRenderer::new());
chartml.register_renderer("scatter", ScatterRenderer::new());
chartml.register_renderer("metric", MetricRenderer::new());

// Render a spec → ChartElement tree
let spec = chartml_core::parse(yaml_string)?;
let element = chartml.render(&spec)?;

// In Leptos, the adapter renders the element tree:
// view! { <ChartMLChart spec=yaml_string /> }
```

---

## ChartElement Tree

The intermediate representation between renderers and framework adapters.

```rust
// ── crates/chartml-core/src/element.rs ──

/// The output of any ChartRenderer. Framework adapters walk this tree
/// and produce framework-specific DOM/view output.
#[derive(Debug, Clone)]
pub enum ChartElement {
    Svg {
        viewbox: ViewBox,
        width: Option<f64>,
        height: Option<f64>,
        class: String,
        children: Vec<ChartElement>,
    },
    Group {
        class: String,
        transform: Option<Transform>,
        children: Vec<ChartElement>,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill: String,
        stroke: Option<String>,
        class: String,
        /// Data attached for tooltips/interactivity
        data: Option<ElementData>,
    },
    Path {
        d: String,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<f64>,
        stroke_dasharray: Option<String>,
        class: String,
        data: Option<ElementData>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: String,
        stroke: Option<String>,
        class: String,
        data: Option<ElementData>,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: String,
        stroke_width: Option<f64>,
        stroke_dasharray: Option<String>,
        class: String,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        anchor: TextAnchor,
        dominant_baseline: Option<String>,
        transform: Option<Transform>,
        font_size: Option<String>,
        fill: Option<String>,
        class: String,
    },
    /// Non-SVG container (e.g., metric card uses div-based layout)
    Div {
        class: String,
        style: HashMap<String, String>,
        children: Vec<ChartElement>,
    },
    /// Raw text node (for metric values, labels in div-based charts)
    Span {
        class: String,
        style: HashMap<String, String>,
        content: String,
    },
}

/// Data attached to interactive elements for tooltips
#[derive(Debug, Clone)]
pub struct ElementData {
    pub label: String,
    pub value: String,
    pub series: Option<String>,
    pub raw: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub enum Transform {
    Translate(f64, f64),
    Rotate(f64, f64, f64),
    Multiple(Vec<Transform>),
}

#[derive(Debug, Clone)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone)]
pub struct Dimensions {
    pub width: Option<f64>,
    pub height: f64,
}
```

---

## D3 Replacement Mapping

ChartML's JS implementation uses D3 selectively. Here is what replaces each D3 module in Rust:

### Scales (`crates/chartml-core/src/scales/`)

| D3 | Rust | Notes |
|---|---|---|
| `d3.scaleLinear()` | `ScaleLinear` | Maps continuous domain → continuous range |
| `d3.scaleBand()` | `ScaleBand` | Maps discrete domain → continuous range with bandwidth |
| `d3.scaleUtc()` | `ScaleTime` | Maps temporal domain → continuous range |
| `d3.scaleOrdinal()` | `ScaleOrdinal` | Maps discrete domain → discrete range (colors) |
| `d3.scaleSqrt()` | `ScaleSqrt` | Square root scale for bubble sizes |

All scales implement a common `Scale` trait:

```rust
pub trait Scale {
    type Domain;
    type Range;

    fn map(&self, value: &Self::Domain) -> Self::Range;
    fn domain(&self) -> &[Self::Domain];
    fn range(&self) -> &[Self::Range];
    fn ticks(&self, count: usize) -> Vec<Self::Domain>;
}
```

### Shapes (`crates/chartml-core/src/shapes/`)

| D3 | Rust | Output |
|---|---|---|
| `d3.line()` | `LineGenerator` | SVG path `d` string ("M0,10 L50,20 ...") |
| `d3.area()` | `AreaGenerator` | SVG path `d` string (closed area shape) |
| `d3.arc()` | `ArcGenerator` | SVG path `d` string (pie slice) |
| `d3.pie()` | `PieLayout` | Angle calculations (start/end per slice) |
| `d3.stack()` | `StackLayout` | Stacked y0/y1 values per series |

These are pure functions — no DOM, no framework. They take data and scale mappings, output SVG path strings or layout coordinates.

### Data Utilities (`crates/chartml-core/src/data.rs`)

| D3 | Rust | Notes |
|---|---|---|
| `d3.extent(data, accessor)` | `extent(&data, \|d\| d.value)` | Returns `(min, max)` — trivial with iterators |
| `d3.max(data, accessor)` | `data.iter().map(accessor).max()` | Standard library |
| `d3.sum(data, accessor)` | `data.iter().map(accessor).sum()` | Standard library |

### Formatting (`crates/chartml-core/src/format/`)

| D3 | Rust | Notes |
|---|---|---|
| `d3.format("$,.0f")` | `NumberFormatter::new("$,.0f")` | Custom parser for d3-format spec |
| `d3.timeFormat("%Y-%m-%d")` | `chrono::NaiveDate::format()` | chrono's strftime covers this |
| `d3.format("~s")` | `NumberFormatter::new("~s")` | SI prefix (1.2k, 3.4M) — custom impl |
| `d3.format(".2%")` | `NumberFormatter::new(".2%")` | Percentage format — custom impl |

The d3-format spec is a mini-language: `[[fill]align][sign][symbol][0][width][,][.precision][~][type]`. Needs a custom parser (~200 lines).

### Axes & Labels (`crates/chartml-core/src/layout/`)

| D3 | Rust | Notes |
|---|---|---|
| `d3.axisBottom()` | `AxisLayout::bottom()` | Generates tick positions + label text |
| `d3.axisLeft()` | `AxisLayout::left()` | Same for vertical axis |
| `.ticks(n)` | `scale.ticks(n)` | Part of Scale trait |
| `.tickFormat(fmt)` | `AxisLayout::with_formatter()` | Uses NumberFormatter/DateFormatter |
| Label collision detection | `LabelStrategy::determine()` | Ports JS labelUtils.js (~387 lines) |

Label strategies (horizontal → rotated → truncated → sampled) are computed as pure layout math. Text width measurement uses a character-width approximation table in core, with an optional browser-based exact measurement path in the Leptos adapter.

### Colors (`crates/chartml-core/src/color/`)

Direct port of JS `colorUtils.js` (173 lines):
- 3 built-in palettes: `autumn_forest`, `spectrum_pro`, `horizon_suite` (12 colors each)
- Fallback color generation for 13+ categories (desaturation algorithm via HSL conversion)
- Custom palette support via configuration

### Animations

CSS transitions replace D3's imperative `selection.transition()`:
- Bar entrance: `transition: height 300ms ease, y 300ms ease`
- Hover effects: `transition: opacity 200ms`
- Line drawing: CSS `stroke-dasharray` + `@keyframes`

The ChartElement tree carries CSS class names. The Leptos adapter ships a default stylesheet.

---

## Spec Parsing

ChartML YAML specs are parsed into typed Rust structs using `serde` + `serde_yaml`.

```rust
// Top-level spec — can be a single component or array of components
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ChartMLSpec {
    Single(Component),
    Array(Vec<Component>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Component {
    #[serde(rename = "chart")]
    Chart(ChartSpec),
    #[serde(rename = "source")]
    Source(SourceSpec),
    #[serde(rename = "style")]
    Style(StyleSpec),
    #[serde(rename = "config")]
    Config(ConfigSpec),
    #[serde(rename = "params")]
    Params(ParamsSpec),
}

#[derive(Debug, Deserialize)]
pub struct ChartSpec {
    pub version: u32,
    pub title: Option<String>,
    pub data: DataRef,
    pub transform: Option<TransformSpec>,
    pub visualize: VisualizeSpec,
    pub layout: Option<LayoutSpec>,
    pub style: Option<StyleOverrides>,
    pub params: Option<Vec<ParamDef>>,
}

#[derive(Debug, Deserialize)]
pub struct VisualizeSpec {
    #[serde(rename = "type")]
    pub chart_type: String,
    pub mode: Option<ChartMode>,
    pub orientation: Option<Orientation>,
    pub columns: FieldRef,
    pub rows: FieldRef,
    pub marks: Option<MarksSpec>,
    pub axes: Option<AxesSpec>,
    pub annotations: Option<Vec<AnnotationSpec>>,
    pub style: Option<ChartStyleSpec>,
}
```

Key advantage over JS: spec violations become deserialization errors, not runtime rendering bugs.

---

## Leptos Adapter

### `<ChartMLChart />` Component

```rust
// ── crates/chartml-leptos/src/chart.rs ──

/// Main ChartML component for Leptos.
/// Mirrors the JS API: <ChartMLChart spec={yaml} />
#[component]
pub fn ChartMLChart(
    /// ChartML YAML specification string
    spec: MaybeSignal<String>,
    /// Optional pre-configured ChartML instance (for custom plugins)
    #[prop(optional)]
    chartml: Option<ChartML>,
    /// Optional CSS class for the container
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    let chartml = chartml.unwrap_or_else(ChartML::with_defaults);

    // Parse spec and render to ChartElement tree (reactive)
    let chart = create_memo(move |_| {
        let yaml = spec.get();
        chartml.render_from_yaml(&yaml).ok()
    });

    view! {
        <div class=format!("chartml-container {}", class)>
            {move || chart.get().map(|el| render_element(&el))}
        </div>
    }
}
```

### `use_chartml()` Hook

```rust
/// Create and manage a ChartML instance with custom plugins.
/// Mirrors the JS useChartML() hook.
pub fn use_chartml() -> ChartML {
    // Returns a ChartML instance that persists across re-renders
    use_context::<ChartML>().unwrap_or_else(ChartML::with_defaults)
}
```

### ChartElement → `view!` Rendering

```rust
// ── crates/chartml-leptos/src/element.rs ──

/// Recursively render a ChartElement tree into Leptos view nodes.
pub fn render_element(element: &ChartElement) -> impl IntoView {
    match element {
        ChartElement::Svg { viewbox, children, class, .. } => {
            view! {
                <svg viewBox=viewbox.to_string() class=class.clone()>
                    {children.iter().map(render_element).collect_view()}
                </svg>
            }
        }
        ChartElement::Rect { x, y, width, height, fill, class, data, .. } => {
            // Reactive hover state for tooltips
            let (hovered, set_hovered) = create_signal(false);
            view! {
                <rect
                    x=*x y=*y width=*width height=*height
                    fill=fill.clone()
                    class=class.clone()
                    on:mouseenter=move |_| set_hovered(true)
                    on:mouseleave=move |_| set_hovered(false)
                />
            }
        }
        // ... other element types
    }
}
```

---

## Demo Site

A Leptos CSR app deployed to GitHub Pages via `trunk`.

### Layout

```
┌──────────────────────────────────────────────┐
│  chartml-rs                        [GitHub]  │
├──────────────────────────────────────────────┤
│                                              │
│  ┌─────────────────┐  ┌──────────────────┐   │
│  │                  │  │                  │   │
│  │  YAML Editor     │  │  Live Chart      │   │
│  │                  │  │  Preview         │   │
│  │  (textarea with  │  │                  │   │
│  │   syntax hints)  │  │  (reactive —     │   │
│  │                  │  │   updates as     │   │
│  │                  │  │   you type)      │   │
│  │                  │  │                  │   │
│  └─────────────────┘  └──────────────────┘   │
│                                              │
├──────────────────────────────────────────────┤
│  Gallery: [Bar] [Line] [Area] [Scatter]      │
│           [Pie] [Metric] [Stacked] [Combo]   │
└──────────────────────────────────────────────┘
```

- Left panel: YAML textarea, updates chart on change (debounced)
- Right panel: Rendered chart via `<ChartMLChart spec=yaml />`
- Bottom gallery: Clickable example specs that load into the editor
- Parse errors shown inline below the editor

---

## Scope

### v0.1 — Core Rendering + Demo Site

**In scope:**
- ChartML YAML spec parsing to typed Rust structs
- Scale implementations: Linear, Band, Time, Ordinal, Sqrt
- Shape generators: Line, Area, Arc, Pie
- Layout: Stack, axes, tick generation, smart label strategies, legend
- d3-format compatible number formatting
- Date formatting via chrono
- Color palettes (all 3 built-in) + fallback color generation
- Plugin traits: ChartRenderer, DataSource, TransformMiddleware, DatasourceResolver
- Plugin registry with registration API
- ChartElement intermediate representation
- Chart renderers (as plugin crates):
  - Cartesian (bar vertical/horizontal, stacked, grouped, line, area)
  - Pie/Doughnut
  - Scatter/Bubble (with size and color encoding)
  - Metric card
- Leptos adapter: `<ChartMLChart />` component, `use_chartml()` hook
- Tooltips (Leptos-native, reactive)
- Hover effects via CSS transitions
- Entrance animations via CSS
- Demo site with YAML editor + chart gallery
- Test fixtures (YAML specs from JS repo examples)
- Unit tests for all core modules (scales, shapes, layout, format, spec parsing)
- GitHub Actions CI (cargo check, cargo test, trunk build)

**Out of scope (future versions):**
- Source system (HTTP data fetching, caching, refresh coordination)
- Transform/aggregate pipeline (the d3Transform equivalent)
- Params UI / interactive controls
- Annotations (reference lines, bands)
- Dual-axis support
- Dashboard layouts (multi-chart grids)
- Component registry (source/style/config cross-referencing)
- Parameter resolution (`$dashboard_filters.region`)
- Source refresh registry
- Param change registry
- `chartml-dioxus` adapter
- Server-side SVG rendering crate
- Visual regression tests (JS vs Rust rendering comparison)

---

## Build Order (Implementation Phases)

Each phase builds on the previous and teaches new Rust/Leptos concepts.

### Phase 1: Project Scaffolding + Spec Parsing

Set up the Cargo workspace, all crate skeletons, and implement ChartML YAML parsing.

**What you learn:** Cargo workspaces, `serde`, `serde_yaml`, Rust enums with `#[serde(tag)]`, error handling with `thiserror`.

**Deliverables:**
- Workspace compiles
- All YAML test fixtures parse successfully into typed Rust structs
- Unit tests for spec parsing

### Phase 2: Scales

Implement ScaleLinear, ScaleBand, ScaleTime, ScaleOrdinal, ScaleSqrt.

**What you learn:** Generics, trait definitions and implementations, `where` bounds, iterator patterns.

**Deliverables:**
- All 5 scale types with `map()`, `ticks()`, `domain()`, `range()`
- Unit tests: domain/range mapping, tick generation, edge cases (empty domain, single value)

### Phase 3: Shapes + Data Utilities

Implement SVG path generators (line, area, arc, pie) and data utilities (extent, stack).

**What you learn:** Iterator adapters, `f64` math, SVG path specification, the builder pattern.

**Deliverables:**
- LineGenerator, AreaGenerator, ArcGenerator produce valid SVG path `d` strings
- PieLayout computes start/end angles from data values
- StackLayout computes y0/y1 for stacked charts
- Unit tests with known SVG path outputs

### Phase 4: Formatting + Colors

Implement d3-format compatible number formatter and color utilities.

**What you learn:** String parsing/tokenization, `chrono` for dates, HSL color math.

**Deliverables:**
- `NumberFormatter::new("$,.0f").format(1234.0)` → `"$1,234"`
- SI prefix, percentage, and standard number formats
- Date formatting via chrono strftime
- 3 built-in palettes + fallback generation for 13+ series
- Unit tests for all format strings from COMMON_FORMATS

### Phase 5: Layout (Axes, Labels, Margins, Legend)

Implement axis tick generation, smart label strategies, dynamic margin calculation, and legend layout.

**What you learn:** Complex layout algorithms, text measurement approximation, multi-pass layout computation.

**Deliverables:**
- AxisLayout generates tick positions and formatted labels
- LabelStrategy selects horizontal/rotated/truncated/sampled
- MarginCalculator determines margins from label measurements
- LegendLayout computes legend item positions
- Unit tests for tick generation, label strategy selection

### Phase 6: Plugin Traits + ChartElement + Registry

Define the plugin traits, ChartElement enum, and registry. Wire up the `ChartML` struct.

**What you learn:** Trait objects (`Box<dyn Trait>`), dynamic dispatch, the registry pattern in Rust, `HashMap<String, Box<dyn T>>`.

**Deliverables:**
- `ChartRenderer`, `DataSource`, `TransformMiddleware`, `DatasourceResolver` traits
- `ChartElement` enum with all variants
- `ChartMLRegistry` with register/lookup methods
- `ChartML` struct that orchestrates parse → render pipeline

### Phase 7: Chart Renderers

Implement the 4 chart renderer crates using core primitives.

**What you learn:** Implementing traits across crate boundaries, the cartesian coordinate system, combining scales + shapes + layout into full charts.

**Deliverables:**
- `chartml-chart-cartesian`: bar (vertical, horizontal, stacked, grouped), line, area
- `chartml-chart-pie`: pie and doughnut
- `chartml-chart-scatter`: scatter with size/color encoding
- `chartml-chart-metric`: metric card with trend indicator
- Integration tests: spec YAML → ChartElement tree (verify structure, not pixels)

### Phase 8: Leptos Adapter

Build the Leptos component library that renders ChartElement trees to reactive SVG.

**What you learn:** Leptos `view!` macro, `create_signal`, `create_memo`, component props, event handling, SVG rendering in Leptos.

**Deliverables:**
- `<ChartMLChart spec=yaml />` component
- `use_chartml()` hook for custom plugin registration
- ChartElement → view! recursive rendering
- Reactive tooltips (show on hover with data from ElementData)
- CSS stylesheet for transitions and hover effects

### Phase 9: Demo Site

Build the Leptos demo site with YAML editor and chart gallery.

**What you learn:** Full Leptos app structure, CSR with `trunk`, textarea binding, debounced reactive updates, GitHub Pages deployment.

**Deliverables:**
- Two-panel layout: YAML editor + live chart preview
- Gallery of clickable example specs
- Error display for invalid YAML / spec errors
- `trunk build --release` produces deployable static files
- GitHub Actions workflow for GitHub Pages deployment

---

## Testing Strategy

### Unit Tests (chartml-core)

Every module gets `#[cfg(test)]` tests. Scales, shapes, layout, and formatting are all pure math — deterministic and easy to test.

```rust
#[test]
fn linear_scale_maps_midpoint() {
    let scale = ScaleLinear::new(0.0..100.0, 0.0..500.0);
    assert_eq!(scale.map(50.0), 250.0);
}

#[test]
fn band_scale_computes_bandwidth() {
    let scale = ScaleBand::new(&["A", "B", "C"], 0.0..300.0);
    assert_eq!(scale.bandwidth(), 100.0);
    assert_eq!(scale.map("B"), 100.0);
}

#[test]
fn line_generator_produces_valid_path() {
    let points = vec![(0.0, 10.0), (50.0, 20.0), (100.0, 5.0)];
    let path = LineGenerator::new().generate(&points);
    assert_eq!(path, "M0,10L50,20L100,5");
}

#[test]
fn number_format_currency() {
    let fmt = NumberFormatter::new("$,.0f");
    assert_eq!(fmt.format(1234567.0), "$1,234,567");
}
```

### Spec Fixture Tests

Parse every YAML fixture file and assert it deserializes without errors:

```rust
#[test]
fn parse_all_fixtures() {
    for entry in std::fs::read_dir("../../fixtures").unwrap() {
        let path = entry.unwrap().path();
        let yaml = std::fs::read_to_string(&path).unwrap();
        let result = chartml_core::parse(&yaml);
        assert!(result.is_ok(), "Failed to parse {}: {:?}", path.display(), result.err());
    }
}
```

### Integration Tests (renderers)

Each renderer crate tests that spec → ChartElement produces a structurally correct tree:

```rust
#[test]
fn bar_chart_produces_correct_rect_count() {
    let spec = parse_fixture("bar_basic.yaml");
    let renderer = CartesianRenderer::new();
    let element = renderer.render(&data, &config).unwrap();

    let rects = count_elements(&element, |e| matches!(e, ChartElement::Rect { .. }));
    assert_eq!(rects, 3); // 3 data points = 3 bars
}
```

### Leptos Component Tests

Use `leptos::mount_to` in WASM test harness to verify components render without panics:

```rust
#[wasm_bindgen_test]
fn chart_component_renders() {
    let document = leptos::document();
    let container = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&container).unwrap();

    leptos::mount_to(container.clone(), || {
        view! { <ChartMLChart spec="..." /> }
    });

    assert!(container.inner_html().contains("<svg"));
}
```

---

## Dependencies

### chartml-core
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"
thiserror = "2"
chrono = { version = "0.4", default-features = false, features = ["std"] }
async-trait = "0.1"
```

### chartml-chart-* (each renderer)
```toml
[dependencies]
chartml-core = { path = "../chartml-core" }
```

### chartml-leptos
```toml
[dependencies]
chartml-core = { path = "../chartml-core" }
leptos = "0.7"
```

### demo
```toml
[dependencies]
chartml-core = { path = "../crates/chartml-core" }
chartml-leptos = { path = "../crates/chartml-leptos" }
chartml-chart-cartesian = { path = "../crates/chartml-chart-cartesian" }
chartml-chart-pie = { path = "../crates/chartml-chart-pie" }
chartml-chart-scatter = { path = "../crates/chartml-chart-scatter" }
chartml-chart-metric = { path = "../crates/chartml-chart-metric" }
leptos = "0.7"
console_error_panic_hook = "0.1"
```

---

## Open Questions (to resolve during implementation)

1. **Text width measurement** — Start with character-width approximation. If label collision detection is noticeably worse than JS, add optional browser-based measurement via `web-sys` in the Leptos adapter.

2. **WASM bundle size** — Measure after Phase 8. If too large, consider: dropping `chrono` for a lighter date library, using `serde_yaml` feature flags, running `wasm-opt -Oz`.

3. **Leptos 0.7 stability** — Pin to 0.7.x. If breaking changes land in 0.8, the adapter crate isolates the impact.

4. **`Row` type** — Use `HashMap<String, serde_json::Value>` for dynamic data (matches JS behavior) or define a typed `Value` enum. Start with `serde_json::Value` and optimize later if needed.

5. **Async in renderers** — Chart renderers are sync (data is already fetched). Data sources and transforms are async. The `ChartML::render()` pipeline is async at the top level but sync at the render step.
