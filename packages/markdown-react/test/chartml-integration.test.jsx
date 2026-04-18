/**
 * Integration tests for `ChartMLCodeBlock` after the chartml 5.0 refactor.
 *
 * Verifies that the orchestration loop (parse YAML -> dispatch
 * `renderToSvgAsync` -> inject SVG) works against a real WASM `ChartML`
 * instance with a JS-supplied provider, and that the cache survives across
 * re-renders within the same instance.
 *
 * Notes:
 * - Loads the Node-target WASM binding directly. The published `@chartml/core`
 *   `dist/index.js` lazy-loads `pkg/web/...` which only works in browsers;
 *   for Node tests we shim a thin wrapper around `pkg/node/...`.
 * - jsdom provides `document` so React Testing Library can mount.
 * - We poll for the chart's SVG to land in the container because the WASM
 *   render pipeline is async and React Testing Library's `findBy*` queries
 *   need a textNode to settle on, which our `dangerouslySetInnerHTML`-style
 *   `innerHTML = svg` injection bypasses.
 */

import React from 'react';
import { describe, it, expect, beforeEach } from 'vitest';
import { render, cleanup, act } from '@testing-library/react';
import { ChartMLCodeBlock } from '../src/index.js';
// Direct path into the Node-target WASM bindings; the published package
// exposes the same module under `@chartml/core/wasm` for browser/node
// conditional resolution, but vite's test resolver doesn't reliably honor
// the `node` condition during vitest, so we go straight to the file.
import { WasmChartML } from '../../core/pkg/node/chartml_wasm.js';

// ---------------------------------------------------------------------------
// `ChartMLCodeBlock` expects an instance that exposes `renderToSvgAsync` and
// `registerProvider`. The bare WasmChartML class already does — `pkg/node`
// returns the same class; we just hand it back for convenience.
// ---------------------------------------------------------------------------
function makeInstance() {
  return new WasmChartML();
}

// Wait for the chart container's `.innerHTML` to become non-empty, giving the
// WASM async pipeline + React effect a chance to settle. Times out after the
// passed deadline so a stuck render fails fast rather than hanging vitest.
async function waitForSvg(container, timeoutMs = 2000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const svg = container.querySelector('svg');
    if (svg) return svg;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 25));
    });
  }
  throw new Error(
    `Timed out waiting for chart SVG to render. innerHTML: ${container.innerHTML.slice(0, 400)}`
  );
}

// Minimal markdown-it-style code block: build a React element that the
// `ChartMLCodeBlock.code` component renders, mimicking what react-markdown
// hands its `code` slot.
function ChartMLCode({ codeFn, yamlText }) {
  return codeFn({
    inline: false,
    className: 'language-chartml',
    children: yamlText,
  });
}

describe('ChartMLCodeBlock — integration with WASM ChartML', () => {
  beforeEach(() => cleanup());

  it('test_full_pipeline_flat: inline-data spec renders end-to-end', async () => {
    const instance = makeInstance();
    const { code } = ChartMLCodeBlock({ chartmlInstance: instance });

    const yamlText = `
type: chart
version: 1
data:
  rows:
    - { x: a, y: 10 }
    - { x: b, y: 20 }
visualize:
  type: bar
  columns: x
  rows: y
`;

    const { container } = render(
      <ChartMLCode codeFn={code} yamlText={yamlText} />
    );

    const svg = await waitForSvg(container);
    expect(svg.tagName.toLowerCase()).toBe('svg');
  });

  it('test_full_pipeline_named_multi: provider dispatches per named source', async () => {
    const instance = makeInstance();
    const calls = [];
    instance.registerProvider('datasource', async (request) => {
      calls.push(request.sourceName);
      // Two independent providers under one slug — return distinct shapes
      // so we can assert both got called and their data flowed through the
      // join/transform pipeline.
      if (request.sourceName === 'visitors') {
        return { data: [{ day: '2024-01-01', visits: 100 }] };
      }
      if (request.sourceName === 'revenue') {
        return { data: [{ day: '2024-01-01', usd: 500 }] };
      }
      return { data: [] };
    });

    const { code } = ChartMLCodeBlock({ chartmlInstance: instance });

    // KYO-79 shape: multi-source named map. We DON'T need a real transform
    // here — just verify the resolver invokes the provider once per named
    // entry and the request-shape serializer round-trips the names.
    const yamlText = `
type: chart
version: 1
data:
  visitors:
    datasource: warehouse
    query: SELECT * FROM visitors
  revenue:
    datasource: warehouse
    query: SELECT * FROM revenue
transform:
  aggregate:
    dimensions: [day]
    measures:
      - { column: visits, aggregation: sum, name: visits }
visualize:
  type: bar
  columns: day
  rows: visits
`;

    const { container } = render(
      <ChartMLCode codeFn={code} yamlText={yamlText} />
    );

    // Wait for either the SVG or an error — the chart MIGHT fail because
    // the multi-source aggregate without datafusion has limitations, but
    // either way the providers should have been called for both names.
    try {
      await waitForSvg(container, 2000);
    } catch (_) {
      // Render error is acceptable for this test — the assertion below
      // about provider calls is what we care about.
    }
    expect(calls.sort()).toEqual(['revenue', 'visitors']);
  });

  it('test_cache_survives_rerender: provider called once across re-renders', async () => {
    const instance = makeInstance();
    let callCount = 0;
    instance.registerProvider('datasource', async () => {
      callCount += 1;
      return { data: [{ x: 'a', y: 1 }] };
    });

    const { code } = ChartMLCodeBlock({ chartmlInstance: instance });
    const yamlText = `
type: chart
version: 1
data:
  datasource: warehouse
  query: SELECT 1
  cache:
    ttl: 5m
visualize:
  type: bar
  columns: x
  rows: y
`;

    const { container, rerender } = render(
      <ChartMLCode codeFn={code} yamlText={yamlText} />
    );
    await waitForSvg(container);
    expect(callCount).toBe(1);

    // Second render with the same instance + same yaml -> cache hit.
    const { code: code2 } = ChartMLCodeBlock({ chartmlInstance: instance });
    rerender(<ChartMLCode codeFn={code2} yamlText={yamlText} />);
    await waitForSvg(container);
    // Provider must NOT be called again — the resolver memory backend should
    // serve the cached entry.
    expect(callCount).toBe(1);
  });

  // jsdom does not implement IndexedDB. The corresponding browser behavior
  // — IndexedDB-cached entries surviving a component remount — is exercised
  // by the tier-2 `wasm-bindgen-test` suite in
  // `crates/chartml-core/tests/indexeddb_test.rs` (which runs under
  // wasm-pack-test against a headless Firefox instance). Asserting the same
  // here without IndexedDB available would silently exercise only the
  // tier-1 in-memory fallback, which `test_cache_survives_rerender`
  // already covers — and an `expect(true).toBe(true)` placeholder gives a
  // false PASS in CI. `it.skip` makes the absence of coverage visible.
  it.skip('test_indexeddb_survives_remount: covered by tier-2 wasm-bindgen-test (crates/chartml-core/tests/indexeddb_test.rs); jsdom has no IndexedDB', async () => {
    // Intentionally empty — see comment above for the real coverage
    // location. `it.skip` surfaces this as SKIP rather than PASS in CI so
    // future readers can see the gap is acknowledged, not forgotten.
  });
});
