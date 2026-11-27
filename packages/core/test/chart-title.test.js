/**
 * CHA-59: the npm components return the SVG alone (React, markdown plugins), so
 * the engine draws a chart's `title` at the top of the SVG, once, inside the
 * size that was asked for. A long title is cut with an ellipsis and keeps its
 * full text as the SVG <title>; long rotated axis labels do the same instead of
 * leaving the SVG.
 */

import { describe, it, expect } from 'vitest';
import { WasmChartML } from '../pkg/node/chartml_wasm.js';

const spec = (title, viz = 'type: bar\n  columns: month\n  rows: sales') => `
type: chart
version: 1
${title ? `title: "${title}"\n` : ''}data:
  rows:
    - { month: Jan, sales: 10 }
    - { month: Feb, sales: 14 }
    - { month: Mar, sales: 9 }
visualize:
  ${viz}
`;

const count = (svg, text) => svg.split(`>${text}<`).length - 1;
const attr = (svg, name) => Number(new RegExp(`^<svg[^>]* ${name}="([^"]+)"`).exec(svg)[1]);

describe('chart title (renderToSvg)', () => {
  for (const type of ['bar', 'line', 'area']) {
    it(`draws a ${type} chart's title once`, () => {
      const viz = `type: ${type}\n  columns: month\n  rows: sales`;
      const svg = new WasmChartML().renderToSvg(spec('Monthly Sales', viz), { width: 500, height: 300 });
      expect(count(svg, 'Monthly Sales')).toBe(1);
      expect(svg.split('class="chart-title"').length - 1).toBe(1);
    });
  }

  it('draws a pie chart\'s title once', () => {
    const svg = new WasmChartML().renderToSvg(
      spec('Share', 'type: pie\n  columns: month\n  rows: sales'),
      { width: 400, height: 300 },
    );
    expect(count(svg, 'Share')).toBe(1);
  });

  it('keeps the SVG the requested size with and without a title', () => {
    for (const type of ['bar', 'line', 'area', 'pie']) {
      const viz = type === 'pie' ? 'type: pie\n  columns: month\n  rows: sales' : `type: ${type}\n  columns: month\n  rows: sales`;
      for (const [width, height] of [[500, 300], [320, 200]]) {
        const titled = new WasmChartML().renderToSvg(spec('Monthly Sales', viz), { width, height });
        const plain = new WasmChartML().renderToSvg(spec(null, viz), { width, height });
        expect([type, attr(titled, 'width'), attr(titled, 'height')]).toEqual([type, width, height]);
        expect([type, attr(plain, 'width'), attr(plain, 'height')]).toEqual([type, width, height]);
      }
    }
  });

  it('cuts a long title with an ellipsis and keeps the full text as the SVG title', () => {
    const long = 'Monthly sales by acquisition channel across every region and plan tier';
    const svg = new WasmChartML().renderToSvg(spec(long), { width: 300, height: 300 });
    expect(svg).toContain('…');
    expect(svg).toContain(`<title>${long}</title>`);
  });

  it('draws no title for a chart without one', () => {
    const svg = new WasmChartML().renderToSvg(spec(null), { width: 500, height: 300 });
    expect(svg).not.toContain('chart-title');
  });
});

describe('rotated axis labels (renderToSvg)', () => {
  it('cuts labels too long for the SVG and keeps their full text as the SVG title', () => {
    const names = Array.from({ length: 6 }, (_, i) => `A deliberately very long category label number ${i} that keeps going`);
    const rows = names.map((name, i) => `    - { name: "${name}", v: ${i + 1} }`).join('\n');
    const yaml = `type: chart\nversion: 1\ndata:\n  rows:\n${rows}\nvisualize:\n  type: bar\n  columns: name\n  rows: v\n`;
    const svg = new WasmChartML().renderToSvg(yaml, { width: 500, height: 300 });
    expect(svg).toContain('…');
    for (const name of names) expect(svg).toContain(`<title>${name}</title>`);
  });
});
