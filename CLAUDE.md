# ChartML — Project Instructions

## Overview

ChartML v3 — Rust/WASM chart rendering engine. Publishes to both npm and crates.io.

- **Rust crates** in `crates/` — the rendering engine
- **npm packages** in `packages/` — WASM bindings + JS/TS wrappers
- **VitePress docs** in `docs/` — documentation site at chartml.org
- **Test specs** in `tests/charts/` — 188 YAML chart specs with golden baselines

## Development Server Ports

- **5173** — Kyomi frontend (separate project)
- **5174** — ChartML docs (VitePress): `cd docs && npm run docs:dev`
- **8642** — Chart test gallery: `cargo run -p chartml-test-runner -- --gallery`

**DO NOT change port 5174 without explicit user approval.**

## Build Commands

```bash
# Rust
cargo build                    # build all crates
cargo test --workspace         # run all tests
cargo clippy --workspace       # lint

# WASM (for npm packages)
bash packages/core/build.sh          # builds @chartml/core WASM (web + nodejs)
bash packages/datafusion/build.sh    # builds @chartml/datafusion WASM

# npm
npm run build                  # build WASM + TypeScript
npm run test                   # run JS tests

# Docs
cd docs && npm run docs:dev    # dev server on port 5174
cd docs && npm run docs:build  # static build
```

## Mandatory Code Review Before Commit

All commits require an independent review from the **code-review-architect** agent.
Resolve critical and major findings before committing. Re-review affected changes
after modifying reviewed code. Review approval is recorded in the report.

## Mandatory Chart Evaluation Before Commit

Evaluate changed golden SVGs (`test-output/golden/**/*.svg`) with the
**chart-evaluator** agent against their YAML assertions and visual expectations.
Resolve failed evaluations before committing; re-evaluate affected SVGs after changes.
Record the evaluation results and evidence in the report.

## Theming

Chart chrome colors (axes, grid, text, background) are controlled by `chartml_core::theme::Theme`.

### Architecture

- **SVG attributes** carry plain hex defaults (e.g. `stroke="#374151"`) — works in every SVG renderer
- **Browser CSS overrides** — `chartml.css` maps element classes to CSS custom properties (e.g. `.axis-line { stroke: var(--chartml-axis-line) }`), enabling live dark/light mode switching
- **Server-side rendering** — uses `Theme` values directly, no CSS engine needed

### Usage (Rust)

```rust
use chartml_core::{ChartML, theme::Theme};

let mut chartml = ChartML::new();
// ... register renderers ...

// Set dark theme before rendering
chartml.set_theme(Theme::dark());

// Or custom theme
chartml.set_theme(Theme {
    axis_line: "#9ca3af".into(),
    grid: "#374151".into(),
    ..Theme::dark()
});
```

### Browser theming (CSS custom properties)

Consuming apps set CSS custom properties to override chart colors:

```css
.dark {
    --chartml-axis-line: #9ca3af;
    --chartml-grid: #374151;
    --chartml-text-secondary: #9ca3af;
    --chartml-text-strong: #f3f4f6;
    --chartml-bg: #1f2937;
}
```

### Parity between browser and server

For visual consistency, the `Theme` passed to server-side rendering must match the CSS custom properties set browser-side. `Theme::default()` matches light mode, `Theme::dark()` matches the dark mode CSS variables.

## Lint Suppression Policy

Lint suppressions (`#[allow(...)]` in .rs files, `= "allow"` in Cargo.toml) are blocked by the pre-commit hook and CI. Fix the underlying warning instead.

## Publishing

### npm (v* tags trigger GitHub Actions)
1. Bump versions in `packages/*/package.json`
2. Rebuild: `npm run build`
3. Tag: `git tag v3.x.x && git push --tags`

### crates.io
`cargo publish` in dependency order: core → forecast → chart-* → datafusion → render

### Post-Release (Kyomi)
Update Kyomi's lockfile to reference the new @chartml/core version.
