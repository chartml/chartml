/**
 * Minimal smoke test — verify the WASM module loads and exposes the
 * expected method surface. Useful baseline for diagnosing the heavier
 * provider/hook integration tests.
 */

import { describe, it, expect } from 'vitest';
import { WasmChartML } from '../pkg/node/chartml_wasm.js';

describe('WasmChartML smoke', () => {
  it('constructs and exposes new chartml-5 methods', () => {
    const c = new WasmChartML();
    expect(typeof c.renderToSvg).toBe('function');
    expect(typeof c.renderToSvgAsync).toBe('function');
    expect(typeof c.registerProvider).toBe('function');
    expect(typeof c.setHooks).toBe('function');
    expect(typeof c.setCache).toBe('function');
    expect(typeof c.setNamespace).toBe('function');
    expect(typeof c.fetch).toBe('function');
    expect(typeof c.transform).toBe('function');
    expect(typeof c.renderPreparedToSvg).toBe('function');
    expect(typeof c.shutdown).toBe('function');
    expect(typeof c.resolverInvalidate).toBe('function');
    expect(typeof c.resolverInvalidateAll).toBe('function');
    expect(typeof c.resolverInvalidateBySlug).toBe('function');
    expect(typeof c.resolverInvalidateByNamespace).toBe('function');
    expect(typeof c.resolverKeyFor).toBe('function');
  });

  it('renderToSvg works for an inline-data spec (sync path)', () => {
    const c = new WasmChartML();
    const yaml = `
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
    const svg = c.renderToSvg(yaml, {});
    expect(typeof svg).toBe('string');
    expect(svg).toContain('<svg');
  });
});
