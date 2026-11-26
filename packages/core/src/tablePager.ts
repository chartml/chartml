/**
 * Table pager controller (CHA-30).
 *
 * A table with more rows than `style.pageSize` renders one page at a time:
 * `RenderOptions.tablePage` (from 0) picks the page, and only that page's rows
 * are in the output. The output has Previous and Next buttons that start
 * disabled and carry the page they go to. The rendering is a string, so
 * nothing in it can run: `attachTablePager` is the host's half. It listens on a
 * container for clicks on those buttons and calls you back with the page to
 * show; you render that page again, which replaces the table. It also enables
 * the buttons that have a page to go to (the first page's Previous and the
 * last page's Next have none and stay disabled) and, after a page change,
 * puts focus back on the button that was pressed.
 *
 * The names below are defined once, in Rust, in
 * `crates/chartml-core/src/table_pager.rs`; this file repeats them, and
 * `test/table-pager.test.js` fails if the two drift apart or if the real
 * renderer's output stops matching them.
 */

export const TABLE_PAGER = {
  ATTR_PAGE_COUNT: 'data-chartml-page-count',
  ATTR_CURRENT_PAGE: 'data-chartml-current-page',
  ATTR_ACTION: 'data-chartml-pager-action',
  ATTR_TARGET_PAGE: 'data-chartml-target-page',
  ACTION_PREV: 'prev',
  ACTION_NEXT: 'next',
  CLASS_TABLE: 'chartml-table',
} as const;

const ACTION_SELECTOR = `[${TABLE_PAGER.ATTR_ACTION}]`;
const TARGET_SELECTOR = `[${TABLE_PAGER.ATTR_TARGET_PAGE}]`;

/**
 * Make the paged tables in `container` work, now and whenever one is mounted
 * into it later (a re-render replaces the table).
 *
 * `onPageChange` is called with the page (from 0) a pressed button goes to and
 * the table (`.chartml-table`) it belongs to; render that page
 * (`tablePage` in the render options). With several tables in one container
 * the table says which one to render. Returns a function that detaches the
 * controller; call it when the host unmounts the container.
 */
export function attachTablePager(
  container: Element,
  onPageChange: (page: number, table: Element) => void,
): () => void {
  // Which button of which table was pressed, until it has been re-rendered.
  let pending: { action: string; table: number } | null = null;

  const tables = (): Element[] => [...container.querySelectorAll(`.${TABLE_PAGER.CLASS_TABLE}`)];

  const restoreFocus = (): void => {
    if (pending === null) return;
    const { action: pressed, table: index } = pending;
    pending = null;
    // A re-render drops focus to the page body. Put it back only if that is
    // where it went: never take it from something the user moved on to.
    const active = document.activeElement;
    if (active && active !== document.body && !container.contains(active)) return;
    const table = tables()[index];
    if (!table) return;
    const other =
      pressed === TABLE_PAGER.ACTION_PREV ? TABLE_PAGER.ACTION_NEXT : TABLE_PAGER.ACTION_PREV;
    // The pressed button again if it still has somewhere to go, else the other.
    for (const action of [pressed, other]) {
      const el = table.querySelector<HTMLElement>(
        `[${TABLE_PAGER.ATTR_ACTION}="${action}"]${TARGET_SELECTOR}`,
      );
      if (el) {
        el.focus();
        return;
      }
    }
  };

  const prepare = (): void => {
    for (const button of container.querySelectorAll(ACTION_SELECTOR)) {
      if (button.hasAttribute(TABLE_PAGER.ATTR_TARGET_PAGE)) button.removeAttribute('disabled');
    }
    restoreFocus();
  };

  const onClick = (event: Event): void => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest(TARGET_SELECTOR);
    if (!button || !container.contains(button) || button.hasAttribute('disabled')) return;
    const page = Number.parseInt(button.getAttribute(TABLE_PAGER.ATTR_TARGET_PAGE) ?? '', 10);
    const table = button.closest(`.${TABLE_PAGER.CLASS_TABLE}`);
    const action = button.getAttribute(TABLE_PAGER.ATTR_ACTION);
    if (Number.isNaN(page) || !table || !action) return;
    pending = { action, table: tables().indexOf(table) };
    onPageChange(page, table);
  };

  container.addEventListener('click', onClick);
  const observer = new MutationObserver(prepare);
  observer.observe(container, { childList: true, subtree: true });
  prepare();
  return () => {
    observer.disconnect();
    container.removeEventListener('click', onClick);
  };
}
