# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`chartml-core`**: named styles and config defaults now apply to charts
  (CHA-48). `register_component` stores a `type: style` component by name and
  each `type: config` as a default; until now both were accepted and
  discarded. Each chart renders with the registered configs' styles in
  registration order, then a config in its own document, then its own
  `style:` (a name or an inline object), then `visualize.style`, each
  deep-merged field by field over the last (spec.md "Resolution Order").
  Style and config components in the same document as a chart apply to it
  too. A chart or config naming a style that is not registered fails with
  `ChartError::InvalidSpec` naming the style; it never falls back to the
  defaults. A re-registered style replaces the earlier one of that name.
  `register_component` with several components registers all of them or,
  when one fails, none.
- **`chartml-core`**: style fields that were parsed but never drawn now take
  effect, inline or from a named style: `fonts.title` / `fonts.axis` /
  `fonts.dataLabel` (family, size, weight, colour), `showDots: false`,
  `strokeWidth`, `grid.opacity`, and `legend.position` (`top`, `right`,
  `bottom`, `left`, `none`) with `legend.orientation` (`horizontal`,
  `vertical`) on bar, line, area, combo, scatter, bubble, pie and doughnut
  charts. Font weights must be a whole number from 1 to 1000 or `normal`,
  `bold`, `bolder` or `lighter` (a number written as a string, such as
  `"600"`, stays accepted); any other weight, a font size of 0 or less, a
  negative `strokeWidth`, a `grid.opacity` or `fillOpacity` outside 0 to 1,
  or an unknown legend value fails with
  `ChartError::InvalidSpec` naming the field. Named and config styles pass the
  same colour check as inline styles.
- **`chartml-core`**: `ChartML::resolve_chart_style`,
  `ChartML::resolve_chart_from_yaml` and `ChartML::chart_theme`, for hosts that
  lay out around a chart (its height, its title font) before its data
  arrives.
- **`chartml-core`**: new `Theme` fields `title_color`, `axis_label_color`,
  `data_label_font_family`, `data_label_font_size`, `data_label_font_weight`
  and `data_label_color`, with `Theme::axis_text_color()` and
  `Theme::data_label_text_color()`; `TextStyle::data_label`. Defaults leave
  output unchanged.
- **`chartml-core`**: legend placement in `layout::legend`
  (`LegendPlacement`, `LegendPosition`, `LegendOrientation`, `LegendBlock`,
  `LegendReserve`, `LEGEND_GAP`, `SIDE_LEGEND_MAX_WIDTH_FRACTION`,
  `calculate_vertical_legend_layout`, `legend_item_elements`,
  `LegendConfig::for_theme`, `LegendBlock::for_chart` / `for_series` /
  `group`) and `layout::GridConfig`, shared by the chart crates. `ChartStyleSpec`,
  `GridSpec`, `FontsSpec`, `FontSpec` and `LegendSpec` implement `Default`;
  `LegendConfig` implements `Debug` and `Clone`.
- **`chartml-core`**: `element::EXPLICIT_PAINT_CLASS` (`explicit-paint`)
  and `element::paint_class`. Axis labels, data labels and grid lines whose
  colour the chart chose (a style's axis, data label or grid colour, a
  field's `dataLabels.color`, a line's series-coloured labels) carry the
  class, and the stylesheet `chartml-leptos` injects no longer replaces their
  colour with the page theme's (`--chartml-text-secondary`,
  `--chartml-grid`), nor a style's `grid.opacity` with its default 0.5. Charts
  coloured by the theme render unchanged.
- **`chartml-chart-cartesian`**: area charts read `style.fillOpacity` (0 to
  1, default 0.6), as spec.md documented; they always drew 0.6. A value
  outside 0 to 1 fails with `ChartError::InvalidSpec`.
- **`chartml-chart-cartesian`**: single-field line charts
  (`rows: { field, dataLabels: { show: true } }`) and horizontal bar charts
  draw their data labels; both drew none. Horizontal labels sit just past the
  bar end (`top`, the default, with room kept on the right for the widest
  label in the data label font), in its middle (`center`) or inside its start
  (`bottom`). Combo charts draw their line fields' labels with the same
  helper as line charts, so `position: bottom` now puts them below the point.
- **`chartml-chart-scatter`**: scatter and bubble charts read `style.grid`
  (visibility, colour, opacity, dash pattern), which they ignored.
- **`chartml-leptos`**: the title heading uses the chart's resolved title
  font and colour, and the loading placeholder reserves the resolved style's
  height, including a height from a named or config style.
- **Schema**: `fonts.axis.weight` and `fonts.dataLabel.color`, which the
  engine applies, are documented.

### Changed

- **`chartml-core`** (breaking for code that builds these with a struct
  literal or matches them exhaustively): `ChartElement::Line` has a new
  `opacity: Option<f64>` field (serialized only when set), `TextRole` has a
  new `DataLabel` variant, and `MarginConfig` has new `legend_top_height`,
  `legend_left_width` and `legend_right_width` fields (`Default` still
  available).
- **`chartml-chart-cartesian`**: bar, line, area and combo charts reserve
  legend room only when they draw a legend (two or more entries), sized from
  the entries they draw. A chart with a single colour series (or a
  single-field `rows` array) no longer keeps an empty band for it.
- **`chartml-core`**: `fetch` (and so the async render paths, including
  `render_from_yaml_with_data_async`, and the Leptos component) resolves
  `type: params` defaults from the chart's own document,
  as the sync path did, through one shared step. Interactive param overrides
  now win over those document defaults on every path; the sync path dropped
  them when the document declared its own params. Document defaults now
  always win over registered defaults for the same key; the sync path applied
  them only when the document also declared a key that was not registered.
- **`chartml-render`**: `render_to_png` and `render_to_png_async` size the
  PNG canvas from the rendered chart (a style's height beats the requested
  height), so the image is no longer cropped or padded with blank space.

### Security

- **`chartml-core`**: every colour in a chart spec (`style.colors`, field
  `color`, `dataLabels.color`, grid, font and annotation colours, and the
  host's default palette) is checked before rendering, because paint values
  are copied into SVG `fill` / `stroke`. Allowed: hex, `rgb()`/`rgba()`,
  `hsl()`/`hsla()`, CSS colour names, and same-document `url(#id)`
  references. A value outside that (external `url(...)`, `javascript:`,
  `data:`, `var()`) now fails the render with `ChartError::InvalidSpec`
  naming the field; it is not replaced by a default colour.

### Changed

- **`chartml-chart-metric`**: a metric card with a `title` and no `label` shows
  the title once. Where the host draws the title as a heading above the card
  (the Leptos component) the card holds no label of its own; where the SVG is
  shown alone (`renderToSvg`, `render_to_png`) the title is the in-card label.
  The value field's name is the label only when there is neither a title nor a
  label.
- **`chartml-core`**: new public field `title_heading: bool` on `RenderOptions`
  (still `Default`, `false`) and on `ChartConfig` (`#[serde(default)]`). Set it
  to `true` when your host draws `title` itself above the rendered SVG. Code
  that builds `ChartConfig` with a struct literal must add the field.

## [4.1.0] - 2026-04-18

### Added

- **`chartml-core`**: the YAML parser now accepts two spec shapes the JS
  package has long accepted but the Rust port rejected. Kyomi's production
  dashboards use these shapes, and moving the Leptos frontend onto the Rust
  engine surfaced the gap as `data did not match any variant of untagged
  enum ChartMLSpec` on every dashboard.
  - **Multi-named `data:` map.** `data:` may now be a map of user-chosen
    source names to per-source specs — e.g.
    `data: { visitors: { datasource: ..., query: ..., cache: { ttl: 6h,
    autoRefresh: true } } }`. A new `DataRef::NamedMap` variant
    (`IndexMap<String, InlineData>`) preserves declaration order. The core
    renderer itself does not fetch these sources; the host app is expected
    to pre-register each by name and then rewrite `data:` to the chosen
    source — the rendering path returns a clear error if a `NamedMap` is
    left unresolved.
  - **Range-mark `rows:` items without `field`.** Forecast confidence-band
    shading uses `{ mark: range, upper: ..., lower: ... }` with no single
    `field` name. `FieldSpec.field` is now `Option<String>`, and all
    downstream cartesian/pie/scatter/table renderers handle the missing
    field safely.
  - **`InlineData` accepts `datasource` + `query`**; `provider` is now
    optional. The slug-based datasource shape
    `{ datasource: "production-postgres", query: "..." }` parses cleanly.
  - **`CacheConfig` gained `autoRefresh: bool`** and `ttl` is now optional.

### Migration

- `spec::FieldSpec.field: String` → `Option<String>`. Rust consumers that
  construct `FieldSpec` literals must wrap the name in `Some(...)`. Reads
  should either branch on `mark == Some("range")` first or use
  `field.as_deref().unwrap_or("")` for tolerant data lookups.
- `spec::InlineData.provider: String` → `Option<String>`. Comparisons
  against `"inline"` need `.as_deref() == Some("inline")`.
- `spec::source::CacheConfig.ttl: String` → `Option<String>`.

## [4.0.2] - 2026-04-17

### Added

- **`chartml-leptos` is now published to crates.io.** Previously held off
  the registry (`publish = false`) during the v3 migration while the
  public API stabilized; Kyomi consumed it via local path. It's stable
  enough now to ship on crates.io so downstream Leptos apps can depend
  on it like any other crate. No API change — just flipping the publish
  bit. First public version is `4.0.2` to align with the workspace
  release train.

## [4.0.1] - 2026-04-17

### Fixed

- **`@chartml/core`**: restore the `"./style.css"` entry in the npm
  package's `exports` map. `dist/style.css` still shipped (via
  `files: ["dist/"]`) in 4.0.0 but Node / Vite / Webpack resolvers
  couldn't reach it, so consumers doing the 2.x-era
  `import "@chartml/core/style.css"` hit `Missing "./style.css"
  specifier in "@chartml/core"`. No code change; packaging fix only.

## [4.0.0] - 2026-04-17

### BREAKING CHANGES

- **`chartml_core::theme::Theme` is now `#[non_exhaustive]`.** Downstream
  crates can no longer construct `Theme` with a struct literal
  (`Theme { axis_line: "…".into(), .. }`). Use `Theme::default()` /
  `Theme::dark()` as a base and mutate fields, or use the struct update
  syntax with `..Theme::default()`. Builder-pattern construction via the
  public setters continues to work unchanged.
- **`bar_corner_radius` is now `BarCornerRadius` (enum), not `f32`.** See
  migration note at the bottom of this entry.

### Added

- **`chartml-chart-table`** — new crate providing a native table renderer.
  Registered via `register_chart_table()` just like the other built-in
  chart types.
- **Opt-in font database registration for SSR** — `chartml-render` no
  longer loads the system font database at import time. Consumers that
  render to PNG/raster in a server context call
  `chartml_render::register_fontdb(..)` to supply their own preloaded
  `fontdb::Database`. This avoids the multi-second startup cost for
  consumers that only need SVG.

### Fixed

- **Bar entrance animation grew from the wrong anchor under top-rounded and
  square bars (third reintroduction of the same root cause).** The leptos
  renderer (`chartml-leptos/src/element.rs`) and the SVG renderer were both
  guessing orientation from a `width > height` heuristic and only computing
  `transform-origin` for `ChartElement::Rect` — so any bar emitted as a
  `ChartElement::Path` (top-rounded `BarCornerRadius::Top`, used by Kyomi)
  animated growing from its geometric center, and any square or wider-than-
  tall vertical bar animated as if horizontal. Fix moves the anchor
  computation into the emission layer: `chartml_chart_cartesian::bar::
  bar_animation_origin` now computes the correct baseline anchor per
  orientation/sign (vertical+ → bottom-center, vertical- → top-center,
  horizontal+ → left-center, horizontal- → right-center), and every bar
  emission site in `build_bar_element` populates the new
  `ChartElement::{Rect,Path}::animation_origin` field. The leptos renderer
  applies the value verbatim; its old heuristic is deleted. The static SVG
  renderer (`chartml-render/src/svg.rs`) keeps its legacy `Rect` heuristic
  for byte-identical golden compatibility — `transform-origin` has no visual
  effect in a static snapshot anyway — and honors `animation_origin` for
  `Path`. A new regression test
  (`chartml-test-runner/tests/bar_animation_origin.rs`) pins all 12
  combinations of `{Uniform(0.0), Uniform(4.0), Top(4.0)} × {vertical+,
  vertical-, horizontal+, horizontal-}`, plus six new
  `phase12_kyomi_bar_origin_*` cases under the Kyomi theme.
  `chartml-leptos/style/chartml.css` also gains a `.bar { transform-origin:
  50% 100% }` safety net for the common case if any future emission site
  forgets to populate the field.

### Theme hooks (additive, backward-compatible)

`chartml_core::theme::Theme` now exposes typography, shape, grid, zero-line,
and dot-halo hooks. Every new field has a default that produces byte-identical
output vs 3.0.0 — verified by `cargo test -p chartml-test-runner --test backward_compat`
against all 191 golden charts.

#### New `Theme` fields

**Typography — title**
- `title_font_family`, `title_font_size`, `title_font_weight`, `title_font_style`

**Typography — labels** (tick labels, axis labels, data labels)
- `label_font_family`, `label_font_size`, `label_font_weight`,
  `label_letter_spacing`, `label_text_transform`

**Typography — numeric tick labels**
- `numeric_font_family`, `numeric_font_size`

**Typography — legend**
- `legend_font_family`, `legend_font_size`, `legend_font_weight`

**Shape / stroke**
- `axis_line_weight`, `grid_line_weight`, `series_line_weight`,
  `annotation_line_weight`
- `bar_corner_radius: BarCornerRadius` — **API change** (see migration note
  below). The previous `f32` field is now an enum supporting `Uniform(r)`
  (all four corners, current behavior) and `Top(r)` (only the two corners at
  the max-value end of the bar — the top of a vertical bar, the right end of
  a horizontal bar, and for negative values the opposite end pointing away
  from the zero baseline). Default is `BarCornerRadius::Uniform(0.0)`.
- `dot_radius`, `dot_halo_color`, `dot_halo_width`

**Grid + baseline**
- `grid_style: GridStyle` — `Both` / `HorizontalOnly` / `VerticalOnly` /
  `None`. Controls which gridlines are drawn.
- `zero_line: Option<ZeroLineSpec>` — emphasized baseline on charts whose
  data crosses zero.

#### New public types
- `GridStyle`, `TextTransform`, `ZeroLineSpec`, `BarCornerRadius`

#### CSS class contract

Every themed SVG element now carries a stable CSS class: `.chart-title`,
`.axis-label`, `.tick-value`, `.legend-label`, `.series-line`, `.bar-rect`,
`.dot-marker`, `.dot-halo`, `.zero-line`.

#### Browser CSS variable overrides

`crates/chartml-leptos/style/chartml.css` now ships `--chartml-*` custom
property overrides with `revert` fallback semantics — consumers can override
any theme field from a parent container without mirroring the Rust `Theme`
in CSS. Requires Chrome 84+, Firefox 67+, Safari 9.1+.

### Self-correcting text measurement

Layout decisions (margin width, tick label strategy, legend packing, label
truncation, label rotation) now measure label text under the same theme
typography that will be applied at paint time. Previously every layout pass
called `approximate_text_width`, which was hardcoded to a 12px sans-serif
calibration and ignored `label_font_size`, `label_letter_spacing`,
`label_text_transform`, and font family. With the theme hooks exposed in this
release, that approximation became visible: consumers setting `Uppercase`,
letter-spacing, or monospace numeric ticks would see overlapping tick labels
and legend items because the layout passes thought the text was narrower
than it actually rendered.

#### New API in `chartml_core::layout::labels`
- `TextMetrics` — text shaping inputs that drive width measurement
  (`font_size_px`, `letter_spacing_px`, `text_transform`, `monospace`).
- `measure_text(text, &TextMetrics)` — theme-aware width estimator. Accounts
  for font size scaling, CSS `letter-spacing`, `text-transform: uppercase`
  (with a 1.10× width correction since uppercase glyphs are wider than the
  mixed-case calibration), and monospace face widths.
- `truncate_label_with_metrics(label, max_width, &TextMetrics)` — truncation
  that uses the same theme-aware width.
- `TextMetrics::from_theme_tick_value`, `from_theme_axis_label`,
  `from_theme_legend`, `from_theme_title` — build metrics from a `Theme` for
  the appropriate text role.

#### Layout config additions
- `LabelStrategyConfig.text_metrics`
- `LegendConfig.text_metrics`
- `MarginConfig.tick_value_metrics` / `axis_label_metrics`

All chart renderers (bar, line, area, scatter, pie) now thread theme metrics
into these configs.

#### Backward compatibility
- `TextMetrics::default()` matches the legacy 12px sans calibration exactly,
  so `measure_text(text, &TextMetrics::default()) == approximate_text_width(text)`
  for every input.
- All four `Theme::*_text_metrics()` constructors return `TextMetrics::default()`
  when the relevant fields on the theme equal `Theme::default()`'s values, so
  the legacy short-circuit fires for every un-themed render.
- Verified by the `backward_compat_goldens_byte_identical` test over all 191
  golden charts.

#### Acceptance gate
`crates/chartml-test-runner/tests/phase10_kyomi_sanity.rs` adds four
`phase11_kyomi_typography_no_overlap_*` tests that render bar, line, scatter
and pie charts under the aggressive Kyomi typography (Instrument Serif 22px
title, DM Sans 10px Uppercase 1.2px tracking, Geist Mono 11px numerics) and
walk every emitted `<text>` element to assert that no two boxes inside the
same role group intersect.

### Backward compatibility
- `Theme::default()` and `Theme::dark()` produce byte-identical SVG output
  vs 3.0.0. Verified by the `backward_compat` test over all 191 golden charts.
- No call-site changes required for existing consumers **except** the
  `bar_corner_radius` API change below.

### Migration

If you were constructing `Theme { bar_corner_radius: 4.0, .. }`, change it to:

```rust
use chartml_core::theme::{BarCornerRadius, Theme};

Theme {
    bar_corner_radius: BarCornerRadius::Uniform(4.0),
    ..Theme::default()
}
```

Along with the `#[non_exhaustive]` change above, this is the breaking
source-level change in 4.0.0. For top-only
rounding (the Kyomi visual target), use `BarCornerRadius::Top(r)` — the
renderer emits bars as `ChartElement::Path` with custom `d` geometry rather
than `ChartElement::Rect` so only the two corners at the max-value end of
the bar are rounded.

## [3.0.0] - 2026-03-28

### Changed

- Rendering engine rewritten in Rust, compiled to WASM — D3 dependency removed
- Initialization is now async: `const chartml = await ChartML.create()` (was `new ChartML()`)
- `render()` replaced by `renderToSvg()` and `renderToElement()`

### Added

- Rust/WASM rendering engine with native-speed performance
- All chart types (bar, line, area, pie, doughnut, scatter, bubble, metric) built into core
- Plugin system: `registerRenderer()`, `registerDataSource()`, `registerTransform()`, `setDatasourceResolver()`
- `@chartml/datafusion` — optional SQL transform plugin (replaces DuckDB)
- Rust crates published to crates.io for native Rust/Leptos usage
- Node.js + browser dual-target WASM builds
- Full backward compatibility with existing YAML chart specifications

### Deprecated

- `@chartml/chart-pie`, `@chartml/chart-scatter`, `@chartml/chart-metric` — all chart types bundled into `@chartml/core`
- `@chartml/markdown-common` — functionality merged into core

## [2.0.0] - 2026-02-01

### BREAKING CHANGES

This release renames the top-level `aggregate:` property to `transform:` with a nested `aggregate:` stage. This restructuring prepares ChartML for a multi-stage data transformation pipeline where aggregation is one of several possible stages.

#### ChartML Spec Property Rename

The top-level `aggregate:` block is now nested under `transform:`:

```yaml
# BEFORE (v1.x)
aggregate:
  dimensions: [category]
  measures:
    - column: revenue
      function: sum

# AFTER (v2.0)
transform:
  aggregate:
    dimensions: [category]
    measures:
      - column: revenue
        function: sum
```

#### API Method Renames

| v1.x Method | v2.0 Method |
|---|---|
| `chartml.registerAggregateMiddleware(fn)` | `chartml.registerTransformMiddleware(fn)` |
| `chartml.setAggregateMiddleware(fn)` | `chartml.setTransformMiddleware(fn)` |

Internal methods (for middleware authors):

| v1.x Internal | v2.0 Internal |
|---|---|
| `this.aggregateMiddleware` | `this.transformMiddleware` |
| `this._applyAggregate(fetchData, spec, context)` | `this._applyTransform(fetchData, spec, context)` |
| `this._registerBuiltInAggregation()` | `this._registerBuiltInTransform()` |
| Pipeline phase: `'aggregate'` | Pipeline phase: `'transform'` |

#### Schema Changes

- The `Aggregate` type definition is now `Transform`
- The top-level schema property `"aggregate"` is now `"transform"`
- The `Transform` type contains a nested `"aggregate"` object with the same dimensions/measures/filters/sort/limit properties
- `Transform` uses `additionalProperties: true` to allow consumer extensions

#### Built-in D3 Middleware

The exported function is renamed from `d3Aggregate` to `d3Transform`. The source file is renamed from `aggregate.js` to `transform.js`. The middleware now reads its configuration from `spec.transform?.aggregate` instead of `spec.aggregate`.

#### Plugin Naming Convention

Plugin naming convention changed from `aggregate-{engine}` to `transform-{engine}`.

### Migration Guide

**For spec consumers** (e.g., applications rendering ChartML):

1. Wrap all `aggregate:` blocks inside `transform:`:
   ```yaml
   # Change this:
   aggregate:
     dimensions: [...]
     measures: [...]

   # To this:
   transform:
     aggregate:
       dimensions: [...]
       measures: [...]
   ```

**For middleware authors**:

1. Rename your registration calls:
   ```javascript
   // Change this:
   chartml.registerAggregateMiddleware(myMiddleware);

   // To this:
   chartml.registerTransformMiddleware(myMiddleware);
   ```

2. Update how your middleware reads the spec:
   ```javascript
   // Change this:
   const config = spec.aggregate || {};

   // To this:
   const config = spec.transform?.aggregate || {};
   ```

**For plugin authors**:

1. Rename your plugin from `aggregate-{engine}` to `transform-{engine}`

## [1.5.0] - 2025-01-15

### Added
- Range marks for highlighting data regions
- Line style customization (dashed, dotted)
- Named data source support

## [1.4.1] - 2024-12-20

### Fixed
- Stacked bar charts with date x-axis rendering incorrectly
- Smart timestamp x-axis auto-format based on data granularity

## [1.4.0] - 2024-12-15

### Added
- Empty state rendering when chart data has zero rows
- React 19 peer dependency support
