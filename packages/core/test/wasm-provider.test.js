/**
 * Smoke tests for the chartml-5 WASM `ChartML` JS callback bridges.
 *
 * Loads the Node-target WASM binding directly (the `web` build needs a
 * browser to instantiate). Validates two end-to-end paths:
 *  1. `registerProvider(...)` — a JS callback registered as a provider is
 *     invoked when the resolver dispatches a matching `data: { datasource:
 *     ..., query: ... }` source.
 *  2. `setHooks(...)` — JS hook handlers fire on the lifecycle events the
 *     resolver emits (progress + cache-miss for the first call).
 */

import { describe, it, expect } from 'vitest';

// Load the Node-target WASM binding. wasm-pack with `--target nodejs` ships
// a CJS module that auto-loads its companion `.wasm` on require — no manual
// init step needed (unlike the `web` build).
import { WasmChartML } from '../pkg/node/chartml_wasm.js';

function makeChart() {
  return new WasmChartML();
}

describe('WasmChartML — provider/hook bridges', () => {
  it('test_js_callback_provider_registered_and_called', async () => {
    const calls = [];
    const chart = makeChart();

    chart.registerProvider('datasource', async (request) => {
      // The resolver hands us the resolved spec — validate its shape so a
      // future serializer regression surfaces here, not 100 layers down.
      calls.push({
        sourceName: request.sourceName,
        spec: request.spec,
        namespace: request.namespace,
      });
      return {
        data: [
          { x: 'a', y: 10 },
          { x: 'b', y: 20 },
        ],
      };
    });

    const yaml = `
type: chart
version: 1
data:
  datasource: warehouse
  query: SELECT 1
visualize:
  type: bar
  columns: x
  rows: y
`;

    const svg = await chart.renderToSvgAsync(yaml, {});
    expect(typeof svg).toBe('string');
    expect(svg).toContain('<svg');

    expect(calls).toHaveLength(1);
    expect(calls[0].spec.datasource).toBe('warehouse');
    expect(calls[0].spec.query).toBe('SELECT 1');
  });

  it('test_js_callback_hooks_invoked', async () => {
    const events = { progress: [], hits: [], misses: [], errors: [] };
    const chart = makeChart();

    chart.setHooks({
      onProgress: (e) => events.progress.push(e),
      onCacheHit: (e) => events.hits.push(e),
      onCacheMiss: (e) => events.misses.push(e),
      onError: (e) => events.errors.push(e),
    });

    chart.registerProvider('datasource', async () => ({
      data: [{ a: 1 }, { a: 2 }],
    }));

    const yaml = `
type: chart
version: 1
data:
  datasource: warehouse
  query: SELECT a FROM t
  cache:
    ttl: 5m
visualize:
  type: bar
  columns: a
  rows: a
`;

    await chart.renderToSvgAsync(yaml, {});

    // First call: provider should miss the cache (no entry yet) and we
    // should see at least one progress event for the fetch phase.
    expect(events.misses.length).toBeGreaterThanOrEqual(1);
    expect(events.errors).toHaveLength(0);
    expect(events.progress.length).toBeGreaterThan(0);
    expect(events.progress.some((e) => e.phase === 'fetch')).toBe(true);

    // Reason MUST be camelCase — `serde(rename_all = "camelCase")` on the Rust
    // `MissReasonDto` enum maps `NotFound` → `"notFound"`. A regression that
    // flips this back to PascalCase would silently break every JS consumer
    // pattern-matching on `event.reason`, so pin the wire format here.
    expect(['notFound', 'expired', 'invalidated']).toContain(events.misses[0].reason);
    // First-ever lookup of a key cannot have been previously stored or
    // expired, so the reason MUST be `notFound` specifically.
    expect(events.misses[0].reason).toBe('notFound');

    // Second call with same spec: should be served from the in-memory cache.
    await chart.renderToSvgAsync(yaml, {});
    expect(events.hits.length).toBeGreaterThanOrEqual(1);
    expect(events.hits[0].tier).toBe('memory');
  });

  it('shutdown is awaitable and idempotent', async () => {
    const chart = makeChart();
    chart.registerProvider('datasource', async () => ({ data: [{ a: 1 }] }));
    await chart.shutdown();
    // Calling twice must not throw — `shutdown` is documented as idempotent.
    await chart.shutdown();
  });
});
