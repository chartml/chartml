# Completion Report — chartml 5.0 datasource-provider-pipeline (Phases 1–6)

**Date:** 2026-04-18
**Plan:** `docs/plans/2026-04-18-datasource-provider-pipeline.md`
**Linear:** KYO-79 (consumer side)
**Status:** Phases 1–5 committed and merged on chartml branch `jason/phase-3-provider-trait-resolver` (which carries all 5 phase merges). Phase 6 implementation complete and reviewer-approved in Kyomi worktree but **not committed** — the Kyomi pre-commit hook uses a different signing key than the chartml hook, and the code-review-architect agent only has the chartml key. Phase 7 (publish 5.0.0) was intentionally not started per the orchestrator's instructions.

---

## What was built

| Phase | Branch | Files changed | Insertions | Status |
|-------|--------|---------------|------------|--------|
| 1 | jason/phase-1-transform-named-sources | 11 | +749 / -127 | Merged |
| 2 | jason/phase-2-pipeline-types | 6 | +1377 / -567 | Merged |
| 3 | jason/phase-3-provider-trait-resolver | 9 | +3636 / -19 | Merged |
| 3b | jason/phase-3b-indexeddb-backend | 11 | +1346 / -1 | Merged |
| 3c | jason/phase-3c-resolver-hooks | 6 | +1048 / -29 | Merged |
| 4 | jason/phase-4-leptos-provider-integration | 9 | +1866 / -121 | Merged |
| 5 | jason/phase-5-wasm-markdown-react | 27 | +4137 / -424 | Merged |
| 6 | jason/kyo-79-dashboard-port-reacts-duckdb-named-sources-pattern-to-leptos | 11 | +329 / -1138 | **Staged in Kyomi worktree, not committed (key mismatch)** |

The chartml integration branch carries 5 phase commits + 4 merge commits on top of the pre-existing `971bcb4 docs: add chartml 5.0 datasource-provider-pipeline design`.

## Review summary

| Phase | Issues found | Fix cycles |
|-------|-------------|-----------|
| 1 | 4 (2 MAJOR + 2 MINOR) | 1 |
| 2 | 5 (2 MAJOR + 3 MINOR) | 1 |
| 3 | 8 across 2 reviews (5 MINOR; then 3 WASM-clippy from pre-commit) | 2 |
| 3b | 6 (2 MAJOR + 4 MINOR-accepted) | 1 + a separate doc-comment escape edit |
| 3c | 8 (1 CRIT + 2 MAJOR + 5 MINOR) + 1 typo escape | 2 |
| 4 | 4 (1 MAJOR + 3 MINOR) | 1 |
| 5 | 6 (1 MAJOR + 5 MINOR) | 1 |
| 6 | 5 (1 MAJOR + 4 MINOR) | 1 |

Every commit on the chartml integration branch carries a `code-review-architect` cryptographic signature verified by the chartml pre-commit hook.

## Deferred work

### Cross-phase ChartML 5.0 follow-ups

1. **Loosen `CacheBackend` supertrait to cfg-gated bound** — Phase 3b shipped `unsafe impl Send + Sync for IndexedDbBackend` because the `CacheBackend` trait declares `Send + Sync` unconditionally. On wasm32-unknown-unknown (single-threaded), this is sound, but the unsafe is tech debt. Fix: in `crates/chartml-core/src/resolver/cache.rs`, cfg-gate the supertrait so it requires `Send + Sync` only on non-wasm targets. Then remove the unsafe impl from `IndexedDbBackend`. A `// TODO: Phase X — loosen CacheBackend supertrait to cfg-gated bound` comment marks the spot in `indexeddb.rs`.

2. **`MissReason::Invalidated` is reserved but never emitted** — Phase 3c defined `MissReason::Invalidated` for hook events but the resolver never emits it. The right wiring is for `Resolver::invalidate*` to mark keys as recently-invalidated and for the next miss to report `Invalidated` instead of `NotFound`. Defer to a future phase that needs this distinction (none currently does).

3. **Hooks ordering guarantee** — Phase 3c shipped fire-and-forget hook delivery via `tokio::spawn` / `wasm_bindgen_futures::spawn_local`. The trait docs accurately state "no ordering guarantee between events." If a future consumer needs per-source ordering, switch `spawn_hook` to a bounded per-source channel that serializes events.

4. **`HttpProvider` JSON wrapper-key support** — The Phase 3 implementer added unwrapping of `rows`/`data`/`results` wrapper keys on top of the spec's bare-array case. The plan didn't require this. Reviewer accepted as pragmatic real-world support; document in the spec if it becomes load-bearing.

5. **Phase 6 demo `LastRefreshedReadout`-style imperative refresh API** — Phase 4's demo uses YAML-comment mutation as a refresh-trigger workaround because `ChartMLChart` doesn't expose `refresh_count` as a writable prop. Phase 6 (Kyomi) ended up needing a similar shape. Phase 7+ should expose either an imperative `chart_ref.refresh()` handle or a writable `refresh_trigger` prop on `ChartMLChart`.

### Phase-specific deferred work

- **Phase 5 — `test_indexeddb_survives_remount` is `it.skip(...)`** in `packages/markdown-react/test/chartml-integration.test.jsx` because jsdom doesn't have IndexedDB. The real coverage is in `crates/chartml-core/tests/indexeddb_test.rs` (wasm-bindgen-test under headless Firefox). When jsdom gains IndexedDB or the test moves to Playwright, unskip.

- **Phase 6 — Workspace_id sentinel `"default"` for free-tier users** without a workspace_id. Acceptable for single-tenant/self-hosted (no regression from the prior bespoke path), but a future enterprise tier may want to refuse-mount instead.

- **Phase 6 — Chartml-* path deps** in Kyomi's workspace `Cargo.toml`. Will swap to `version = "5"` after chartml's Phase 7 publish.

## Behavioral divergences from reference

- **Param substitution in markdown-react and Kyomi continues to use `{{key}}` text-level substitution** rather than chartml's `$param.name` runtime substitution. Switching would require migrating every dashboard's stored YAML and is intentionally out of scope.

- **Phase 6 refresh button calls `resolver.invalidate_all()`** rather than per-source invalidation. Each `ChartBlock` owns its own `ChartML` instance and therefore its own resolver, so this scopes to one chart. Per-source scoping would require parsing the spec to compute every key.

- **Phase 6 `last_refreshed` indicator** stamps `js_sys::Date::now()` on input change rather than wiring through `ResolverHooks::on_progress`. Slightly off-by-one-frame from "render complete" but always within the source's TTL.

## Security notes

- **Phase 3 `xxh3_64` for cache keys** is a non-crypto hash. This is fine for in-memory cache (resolver-local) but means content-addressed cache tampering is possible if an attacker controls a write to IndexedDB. IndexedDB is origin-scoped, so the threat model is "shared machine, multiple users, one user reads another's cache after IDB write" — mitigated by Phase 3b's mandatory namespace constraint and Phase 6's `workspace_id` namespace wiring.

- **Phase 5 `JsCallbackProvider` strict-Promise enforcement** rejects non-thenable returns from JS callbacks with a clear error. This is the right call — silently auto-wrapping sync returns into `Promise.resolve` would mask provider-implementation bugs.

- **Phase 5 lint policy widening `forbid` → `deny` in `chartml-wasm/Cargo.toml`** was needed because `tracing` macros emit internal `#[allow(unused_imports)]`. Surgical: only the two lints that `tracing` trips on changed level. User-code lint enforcement is unchanged.

## Integration notes

- **Phase 5 carried a Phase 3 root-cause fix**: `std::time::SystemTime::now()` panics on wasm32-unknown-unknown with "time not implemented on this platform." The fix swaps `std::time::SystemTime` → `web_time::SystemTime` in `crates/chartml-core/src/lib.rs`, `pipeline/mod.rs`, `resolver/{mod.rs, cache.rs, backends/codec.rs}`. `web_time` is a transparent re-export of `std::time` on native (zero behavioral change there) and uses `js_sys::Date::now()` on WASM. Reviewer endorsed bundling this with Phase 5 rather than splitting a phantom Phase 3.x patch — the bisect history is cleaner as "Phase 5 discovered and fixed the blocker it required."

- **Phase 4 ripple effect into chartml-leptos and chartml-demo**: Phase 3 introduced the resolver, which made `ChartML` `?Send` on WASM (the inflight `Shared<LocalBoxFuture>` is single-threaded). This broke `chartml-leptos`'s `Arc<ChartML>` clippy lint and `chartml-demo`'s `Send + 'static` reactive-function bound. Phase 3 added cfg-gated `ChartMLRef = Arc on native / Rc on WASM` and `SendWrapper` in the demo to fix.

- **Phase 6 picked up Phase 4's `ChartMLRef` change** — Kyomi's pre-existing `chart_builder.rs` and `chartml_extension.rs` had to migrate from `Arc<ChartML>` to `chartml_leptos::ChartMLRef` and wrap non-`Send` storage in `SendWrapper`. Acceptable as ripple cleanup — they're not directly part of Phase 6 but had to compile for the Kyomi workspace to build.

- **Phase 6 cannot commit until the Kyomi signing key is provisioned to the code-review-architect agent.** Implementation is staged in worktree `/home/jason/repos/kyomi-wt-kyo-79-named-sources` on branch `jason/kyo-79-dashboard-port-reacts-duckdb-named-sources-pattern-to-leptos` with diff hash `f62bb689e6eca771aa3da0207cf336534350bc3c1e78d6fa8fd78a5ba09804cf`. The reviewer agent's prior chartml signature for that hash is at `.review-approval` in the worktree but Kyomi's hook rejects it ("INVALID — the approval file may have been tampered with"). User action required: either provision the Kyomi signing key to the agent OR commit Phase 6 manually with their own approval flow.

- **Phase 6 workspace `Cargo.toml`** carries path deps to `/home/jason/repos/chartml/crates/<name>`. Phase 7 must swap these to `version = "5"` after the chartml crates are published.

## Compilation status

**chartml integration branch (`jason/phase-3-provider-trait-resolver`):**
- `cargo check --workspace` — clean
- `cargo clippy --workspace --exclude chartml-wasm --exclude chartml-wasm-datafusion 2>&1 | grep "^warning:" | grep -v "cargo:warning\|generated\|Model file\|Downloaded\|Downloading"` — empty
- `cargo clippy --workspace --target wasm32-unknown-unknown 2>&1 | grep "^warning:" | grep -v "cargo:warning\|generated\|Model file\|Downloaded\|Downloading"` — empty
- `cargo test --workspace` — every "test result" is "ok"
- `bash packages/core/build.sh` — clean (web + nodejs targets)
- `cd packages/core && npm test` — 54/54 pass
- `cd packages/markdown-react && npm test` — 3 passed + 1 explicit it.skip

**Kyomi worktree (`jason/kyo-79-dashboard-port-reacts-duckdb-named-sources-pattern-to-leptos`):**
- `cargo check --workspace` — clean
- `cargo clippy --workspace` — zero warnings on Phase 6 code
- `cargo test -p kyomi-ui --test chartml_provider_test` — 6/6 integration tests pass (including the new `test_provider_calls_query_datasource_arrow`)
- `cargo test -p kyomi-ui --lib` — 7 chartml_provider unit tests pass; one pre-existing `chartml_completion::tests::existing_keys_filtered_at_root` failure exists on `main` too (unrelated)
- `cargo check --target wasm32-unknown-unknown --no-default-features --features hydrate -p kyomi-ui` — clean
- **Pre-commit hook blocks commit due to signing-key mismatch (see Integration notes above).**
