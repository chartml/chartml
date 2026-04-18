/**
 * Async pipeline smoke — bisecting whether renderToSvgAsync without any
 * provider/hook customization works on the Node-target WASM. If this passes
 * but `wasm-provider.test.js` hangs, the issue is in the JS callback bridges.
 */

import { describe, it, expect } from 'vitest';
import { WasmChartML } from '../pkg/node/chartml_wasm.js';

describe('WasmChartML async pipeline', () => {
  it('renderToSvgAsync handles inline-data without any custom providers', async () => {
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
    const svg = await c.renderToSvgAsync(yaml, {});
    expect(typeof svg).toBe('string');
    expect(svg).toContain('<svg');
  });
});
