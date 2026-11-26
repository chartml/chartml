// @vitest-environment jsdom
/**
 * CHA-30: the JavaScript table pager controller, driven against the real
 * renderer's output (the WASM build), not a hand-written fixture.
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, it, expect, afterEach } from 'vitest';
import { WasmChartML } from '../pkg/node/chartml_wasm.js';
import { attachTablePager, TABLE_PAGER } from '../src/tablePager.ts';

const RUST_CONTRACT = readFileSync(
  resolve(__dirname, '../../../crates/chartml-core/src/table_pager.rs'),
  'utf8',
);

function spec(rows, pageSize) {
  const data = Array.from({ length: rows }, (_, i) => `    - { name: "Row ${i + 1}", value: ${i + 1} }`).join('\n');
  return `
type: chart
version: 1
data:
  rows:
${data}
visualize:
  type: table
  columns: name
  rows:
    - field: value
  style:
    pageSize: ${pageSize}
`;
}

const render = (rows, pageSize, tablePage) =>
  new WasmChartML().renderToSvg(spec(rows, pageSize), { width: 600, height: 400, tablePage });

/**
 * A host: renders `page` of the table into a container, and re-renders when
 * the controller reports a page (what a real host does).
 */
function host(rows, pageSize) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const calls = [];
  const show = (page) => {
    container.innerHTML = render(rows, pageSize, page);
  };
  show(0);
  return { container, calls, show, onPage: (p) => { calls.push(p); show(p); } };
}

const btn = (c, action) => c.querySelector(`[${TABLE_PAGER.ATTR_ACTION}="${action}"]`);
const rowsOf = (c) => [...c.querySelectorAll('.chartml-table-row')].map((r) => r.firstElementChild.textContent);
const status = (c) => c.querySelector('.chartml-table-pager-info')?.textContent;
const settle = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => {
  document.body.innerHTML = '';
});

describe('the contract', () => {
  it('JS names equal the Rust constants', () => {
    const rust = Object.fromEntries(
      [...RUST_CONTRACT.matchAll(/pub const (\w+): &str = "([^"]*)";/g)].map((m) => [m[1], m[2]]),
    );
    expect(rust).toEqual({ ...TABLE_PAGER });
  });
});

describe('the renderer output', () => {
  it('renders only the requested page, with disabled buttons that carry their target', () => {
    const c = document.createElement('div');
    c.innerHTML = render(7, 3, 1);
    expect(rowsOf(c)).toEqual(['Row 4', 'Row 5', 'Row 6']);
    expect(status(c)).toBe('Showing 4–6 of 7 · Page 2 of 3');
    expect(btn(c, 'prev').getAttribute(TABLE_PAGER.ATTR_TARGET_PAGE)).toBe('0');
    expect(btn(c, 'next').getAttribute(TABLE_PAGER.ATTR_TARGET_PAGE)).toBe('2');
    expect(btn(c, 'prev').hasAttribute('disabled') && btn(c, 'next').hasAttribute('disabled')).toBe(true);
  });

  it('without tablePage the render is static: page 1 and no button has a target', () => {
    const c = document.createElement('div');
    c.innerHTML = new WasmChartML().renderToSvg(spec(7, 3), { width: 600, height: 400 });
    expect(rowsOf(c)).toEqual(['Row 1', 'Row 2', 'Row 3']);
    expect(c.querySelectorAll(`[${TABLE_PAGER.ATTR_TARGET_PAGE}]`).length).toBe(0);
    attachTablePager(c, () => {});
    expect(btn(c, 'next').disabled, 'the controller leaves it disabled').toBe(true);
  });

  it('a page past the last throws', () => {
    expect(() => render(7, 3, 3)).toThrow(/out of range/);
    expect(() => render(7, 3, -1)).toThrow(/tablePage/);
    expect(() => render(7, 3, 1.5)).toThrow(/tablePage/);
  });
});

describe('attachTablePager on the real renderer output', () => {
  it('a static render has no controller: every button stays disabled and inert', () => {
    const h = host(5, 2);
    expect(btn(h.container, 'next').disabled).toBe(true);
    btn(h.container, 'next').click();
    expect(rowsOf(h.container)).toEqual(['Row 1', 'Row 2']);
  });

  it('enables only the buttons with a target and moves through every page to the last', async () => {
    const h = host(5, 2);
    attachTablePager(h.container, h.onPage);
    expect(btn(h.container, 'prev').disabled).toBe(true);
    expect(btn(h.container, 'next').disabled).toBe(false);
    btn(h.container, 'prev').click(); // disabled: nothing happens
    expect(h.calls).toEqual([]);
    btn(h.container, 'next').click();
    await settle();
    expect(h.calls).toEqual([1]);
    expect(rowsOf(h.container)).toEqual(['Row 3', 'Row 4']);
    expect(status(h.container)).toBe('Showing 3–4 of 5 · Page 2 of 3');
    expect(btn(h.container, 'prev').disabled).toBe(false);
    expect(btn(h.container, 'next').disabled).toBe(false);
    btn(h.container, 'next').click();
    await settle();
    expect(rowsOf(h.container)).toEqual(['Row 5']);
    expect(btn(h.container, 'next').disabled, 'Next on the last page').toBe(true);
    btn(h.container, 'next').click();
    expect(h.calls).toEqual([1, 2]);
    btn(h.container, 'prev').click();
    await settle();
    btn(h.container, 'prev').click();
    await settle();
    expect(rowsOf(h.container)).toEqual(['Row 1', 'Row 2']);
    expect(btn(h.container, 'prev').disabled, 'Previous on the first page').toBe(true);
    expect(h.calls).toEqual([1, 2, 1, 0]);
  });

  it('keeps focus across the re-render: on the pressed button, else on the other one', async () => {
    const h = host(5, 2);
    attachTablePager(h.container, h.onPage);
    btn(h.container, 'next').focus();
    btn(h.container, 'next').click();
    await settle();
    expect(document.activeElement).toBe(btn(h.container, 'next'));
    btn(h.container, 'next').click();
    await settle();
    expect(btn(h.container, 'next').disabled).toBe(true);
    expect(document.activeElement, 'focus moves to Previous at the last page').toBe(btn(h.container, 'prev'));
  });

  it('a table of one page has no pager', () => {
    for (const [rows, size] of [[3, 50], [3, 3]]) {
      const h = host(rows, size);
      attachTablePager(h.container, h.onPage);
      expect(h.container.querySelector('.chartml-table-pager')).toBeNull();
      expect(rowsOf(h.container).length).toBe(rows);
    }
  });

  it('prepares a table mounted after attach', async () => {
    const h = host(5, 2);
    attachTablePager(h.container, h.onPage);
    h.container.innerHTML = render(5, 2, 1);
    await settle();
    expect(btn(h.container, 'prev').disabled).toBe(false);
    expect(btn(h.container, 'next').disabled).toBe(false);
  });

  it('detaching stops the controller: no listener and no observer remain', async () => {
    const h = host(5, 2);
    const detach = attachTablePager(h.container, h.onPage);
    detach();
    btn(h.container, 'next').removeAttribute('disabled');
    btn(h.container, 'next').click();
    expect(h.calls).toEqual([]);
    h.show(0);
    await settle();
    expect(btn(h.container, 'next').disabled, 'a re-render is not prepared after detach').toBe(true);
  });

  it('two tables in one container report their own page and table', async () => {
    const c = document.createElement('div');
    c.innerHTML = render(5, 2, 0) + render(4, 2, 1);
    document.body.appendChild(c);
    const seen = [];
    attachTablePager(c, (p, table) => seen.push([p, table]));
    const [a, b] = c.querySelectorAll('.chartml-table');
    a.querySelector(`[${TABLE_PAGER.ATTR_ACTION}="next"]`).click();
    b.querySelector(`[${TABLE_PAGER.ATTR_ACTION}="prev"]`).click();
    expect(seen).toEqual([[1, a], [0, b]]);
  });

  it('puts focus back in the table that was pressed, not the first one', async () => {
    const c = document.createElement('div');
    const html = [render(5, 2, 0), render(5, 2, 0)];
    c.innerHTML = html.join('');
    document.body.appendChild(c);
    attachTablePager(c, (p, table) => {
      const index = [...c.querySelectorAll('.chartml-table')].indexOf(table);
      html[index] = render(5, 2, p);
      c.innerHTML = html.join('');
    });
    const second = c.querySelectorAll('.chartml-table')[1];
    const next = second.querySelector(`[${TABLE_PAGER.ATTR_ACTION}="next"]`);
    next.focus();
    next.click();
    await settle();
    const tables = c.querySelectorAll('.chartml-table');
    expect(document.activeElement).toBe(tables[1].querySelector(`[${TABLE_PAGER.ATTR_ACTION}="next"]`));
  });

  it('does not take focus from something the user moved on to', async () => {
    const h = host(5, 2);
    const input = document.createElement('input');
    document.body.appendChild(input);
    attachTablePager(h.container, (p) => {
      // A host whose re-render lands after the user clicked elsewhere.
      input.focus();
      h.onPage(p);
    });
    btn(h.container, 'next').click();
    await settle();
    expect(document.activeElement).toBe(input);
  });
});
