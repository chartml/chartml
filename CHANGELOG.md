# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.1.0] - unreleased (feat/theme-hooks branch)

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

This is the only breaking source-level change in 3.1.0. For top-only
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
