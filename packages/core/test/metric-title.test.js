/**
 * CHA-49: the npm components return the SVG alone (React, markdown plugins),
 * so a metric card with a `title` and no `label` must hold the title itself,
 * once, and never fall back to the value field's name.
 */

import { describe, it, expect } from 'vitest';
import { WasmChartML } from '../pkg/node/chartml_wasm.js';

const spec = (extra) => `
type: chart
version: 1
${extra}data:
  rows:
    - { new_signups: 4827 }
visualize:
  type: metric
  value: new_signups
`;

const count = (svg, text) => svg.split(`>${text}<`).length - 1;

describe('metric card title (renderToSvg)', () => {
  it('shows a title-only card\'s title once and no field name', () => {
    const svg = new WasmChartML().renderToSvg(spec('title: Quarterly Signups\n'), {});
    expect(count(svg, 'Quarterly Signups')).toBe(1);
    expect(count(svg, 'new_signups')).toBe(0);
  });

  it('shows the label, not the title, when both are set', () => {
    const svg = new WasmChartML().renderToSvg(
      spec('title: Quarterly Signups\n').replace('value: new_signups', 'value: new_signups\n  label: Signups'),
      {},
    );
    expect(count(svg, 'Signups')).toBe(1);
    expect(count(svg, 'Quarterly Signups')).toBe(0);
  });

  it('keeps the field name when there is neither title nor label', () => {
    const svg = new WasmChartML().renderToSvg(spec(''), {});
    expect(count(svg, 'new_signups')).toBe(1);
  });
});
