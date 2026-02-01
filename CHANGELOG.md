# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
