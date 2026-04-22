# KYO-131 Completion Report

**Ticket:** [KYO-131 — chartml: metric card label/value alignment across side-by-side cards](https://linear.app/kyomi/issue/KYO-131/chartml-metric-card-labelvalue-alignment-across-side-by-side-cards)

**Branch:** `jason/kyo-131-chartml-metric-card-labelvalue-alignment`

**Commits:**
- `8284170` — fix(chart-metric): CSS grid layout for aligned label/value across cards
- `5e58a90` — test(chart-metric): regenerate + re-sign golden SVGs for grid layout

## What was built

Replaced the metric card's per-card flex-column layout with a CSS grid that has a deterministic label slot and value slot. Side-by-side metric cards on a dashboard row now align their labels at the top and their big-number values in a consistent band, regardless of label length. The label is clamped to 2 lines via `-webkit-line-clamp` to prevent overlap with the value in narrow cards.

Files changed (2 commits combined):

| File | Lines | Change |
|------|-------|--------|
| `crates/chartml-chart-metric/src/lib.rs` | +20 / −4 | Grid layout; label 2-line clamp; grid-row placement on spans |
| `crates/chartml-test-runner/snapshots/pre-theme-hooks/metric/*.svg` | 7 × 1-line | Baseline refresh for intentional layout change |
| `crates/chartml-test-runner/snapshots/pre-theme-hooks/sizing/metric_tiny_200x100.svg` | 1 line | Same |
| `test-output/golden/metric/*.svg` + `.sig` | 7 SVGs + 7 sigs | Regenerated + re-signed |
| `test-output/golden/sizing/metric_tiny_200x100.svg` + `.sig` | 1 SVG + 1 sig | Regenerated + re-signed |

## Review summary

- Tasks reviewed: 2
- Total issues found: 1 (critical: stale backward-compat baselines)
- Issues fixed: 1
- Fix cycles: 1

All reviews performed by `code-review-architect`. All 8 golden SVGs evaluated by `chart-evaluator` against their YAML assertions; all PASS.

## Deferred work

### 1. chartml release (crates.io + npm)

- **What:** Bump versions on `chartml-chart-metric` and its dependents, tag `v*` to trigger the npm release workflow, publish affected crates to crates.io.
- **Why deferred:** Release action has a blast radius outside this repo (external consumers) and is not one-off reversible. Needs a human decision on version number (patch vs minor) and on timing relative to any other in-flight chartml changes.
- **Impact:** Kyomi and other external consumers won't pick up the fix until a new version is published.
- **Resolution:** User runs `cargo publish` per the crate dependency order documented in `CLAUDE.md` and pushes a `v*` tag to trigger the npm workflow.

### 2. Kyomi-side lockfile bump

- **What:** After a chartml release, Kyomi's `Cargo.lock` needs to be updated to reference the new `chartml-chart-metric` version; `package-lock.json` to the new `@chartml/core` version.
- **Why deferred:** Downstream to the release above. Not in-scope for this repo.
- **Resolution:** Separate Kyomi PR after chartml publishes.

### 3. Dashboard-row subgrid follow-up (speculative, noted in the ticket)

- **What:** If this internal-grid fix isn't sufficient when cards in the same row have different heights (e.g. mixed chart types), consider a subgrid at the dashboard layout level.
- **Why deferred:** Explicitly called out in the ticket as "not in scope, do not do preemptively." The right move is to ship this internal-grid fix, observe, and only pull the trigger on subgrid if the user still sees drift.
- **Resolution:** New Kyomi-side ticket if/when the user confirms residual drift.

## Behavioral divergences from reference

None. The element structure (Div → label Span + value Span + optional trend Span, in that order) is unchanged. Only the inline style attributes on the card div and its three children were modified. Unit tests in `chartml-chart-metric` (`metric_renders`, `metric_has_formatted_value`, `metric_has_trend_indicator`, `metric_inverted_trend`, `metric_default_dimensions`, `metric_empty_data_errors`) all pass without modification.

## Security notes

None. This is a presentation-only change to inline CSS styles. No user input, no data path, no trust boundary is affected.

## Integration notes

- **Other golden SVGs are stale relative to current renderer output.** When regenerating metric goldens, we noticed that ~190 other signed goldens (bar, line, pie, scatter, etc.) also differ from the current renderer output due to earlier theme refactors. Their on-disk SVGs and signatures remain valid — `verify-charts.sh` passes — but a future `--accept` run would flag them as drifted. Out of scope for this ticket; do not "fix" them as a side effect.
- **`test-output/all/` tree.json is stale.** The `chart-evaluator` noted that `test-output/all/` contains tree.json files from a prior render with the old flex layout. It doesn't affect evaluation (no YAML assertion depends on layout CSS) and isn't checked into git. The next full batch run will refresh it.
- **No WASM build performed.** The metric renderer is pure Rust compiled to WASM; `packages/core/build.sh` was NOT rerun in this PR since the release cut is a separate deferred step.

## Compilation status

- `cargo check --workspace` — clean
- `cargo clippy --workspace -- -D warnings` — clean (native + wasm32 via pre-commit hook)
- `cargo test --workspace` — 38 test binaries, all pass (metric unit tests: 6/6; backward-compat: 1/1)
- `bash scripts/verify-charts.sh` — all 198 golden SVGs signed and valid

## Acceptance criteria status

| Criterion | Status |
|-----------|--------|
| Dashboard with 4 metric cards of varying label lengths aligns labels and values | ✅ Layout change in place; physical browser-side verification is a user step since the dashboard lives in Kyomi, not in this repo |
| Labels exceeding 1 line wrap to 2; exceeding 2 lines truncate with ellipsis | ✅ Via `-webkit-box` + `-webkit-line-clamp: 2` |
| `/chartml-test metric` passes | ✅ chart-evaluator PASSed all 7 metric fixtures + 1 sizing fixture |
| Visual regression snapshot updated and reviewed | ✅ Golden SVGs regenerated and re-signed; before/after diff visible via `git diff 75b8f59..HEAD -- test-output/golden/metric/` |
| Version bump + tag push triggers crates.io release | ⏭ Deferred (see Deferred Work #1) |
