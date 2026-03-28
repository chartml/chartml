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

All commits require a cryptographically signed approval from the **code-review-architect** agent. The pre-commit hook verifies this signature — commits without it are blocked.

### Rules:
- **Never skip the review step** — the pre-commit hook will reject unsigned commits
- **Any change after review invalidates the signature** — re-review after any modification
- **The reviewer must not sign if critical or major issues exist** — fix them first
- **Implementation agents cannot sign their own reviews** — only code-review-architect has signing authority
- **Do NOT tell the reviewer how to sign** — it has its own signing instructions

## Mandatory Chart Evaluation Before Commit

All golden SVGs (`test-output/golden/**/*.svg`) require a cryptographic signature from the **chart-evaluator** agent.

### Rules:
- **Every golden SVG must be signed** — unsigned SVGs block the commit
- **Modifying a golden SVG invalidates its signature** — re-evaluate and re-sign
- **Only the chart-evaluator agent can sign charts**
- Verify: `bash scripts/verify-charts.sh`

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
