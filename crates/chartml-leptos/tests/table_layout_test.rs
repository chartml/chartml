//! Browser layout tests for the `table` chart (CHA-20, CHA-21): a table in a
//! narrow container must be readable by scrolling sideways; no column is
//! pinned, the whole table scrolls.
//!
//! Each test renders a twelve-column table with the real `TableRenderer`,
//! puts it in a fixed-size container through one of the two ways hosts show
//! a chart — Leptos (`render_element`, mounted) or the SVG string
//! (`element_to_svg`, whose `<foreignObject>` holds the same HTML) — with
//! this crate's `chartml.css` loaded, then scrolls it sideways and measures
//! the laid-out page:
//!
//! - every column can be scrolled into view;
//! - header and body columns line up at every scroll position;
//! - header and body rows span the whole scrolled width;
//! - no header or body cell is sticky (no column is pinned);
//! - the right-edge fade is drawn only while columns are hidden to the
//!   right, and covers no text at the right end;
//! - no header or body cell is cut off and no word or number is broken
//!   across lines; where there is room (375px and wider), every header label
//!   and every value fits on one line (columns are sized from their
//!   content), and in a very narrow container values wrap only at spaces;
//! - scrolled down, the header stays at the top and the pager stays visible.
//!
//! Browser-only, like `chart_provider_test.rs`; native `cargo test` compiles
//! this file as an empty module. Drive with
//! `wasm-pack test --headless --firefox -p chartml-leptos` (or `--chrome` /
//! `--safari`).

#![cfg(target_arch = "wasm32")]

use chartml_chart_table::TableRenderer;
use chartml_core::svg::element_to_svg;
use chartml_core::theme::Theme;
use chartml_core::ChartML;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const STYLESHEET: &str = include_str!("../style/chartml.css");

/// Twelve columns: text category, text measure, nine numeric measures, text.
/// One first-column value is long enough to wrap in a very narrow container.
/// Twenty rows with a page size of 15, so the body scrolls
/// vertically and the pager shows.
fn wide_spec() -> String {
    let regions = [
        "North America", "Northwind Traders International Ltd", "Western Europe", "Eastern Europe", "Middle East",
        "North Africa", "Sub-Saharan Africa", "South Asia", "East Asia", "Southeast Asia",
    ];
    let mut rows = String::new();
    for i in 0..20_u32 {
        let region = format!("{}{}", regions[(i % 10) as usize], if i < 10 { "" } else { " II" });
        let n = f64::from(i) + 1.0;
        rows.push_str(&format!(
            "      - {{ region: \"{region}\", manager: \"Manager {i}\", revenue: {}, cost: {}, units: {}, orders: {}, customers: {}, returns: {}, margin: {}, growth: {}, nps: {}, tier: \"Gold\" }}\n",
            n * 123_456.0, n * 65_432.0, n * 1_234.0, n * 321.0, n * 87.0, n * 3.0, n * 0.031, n * 0.017 - 0.05, n * 7.0 - 20.0
        ));
    }
    format!(
        r#"type: chart
version: 1
data:
  provider: inline
  rows:
{rows}visualize:
  type: table
  columns: region
  rows:
    - manager
    - {{ field: revenue, label: Revenue, format: "$,.0f" }}
    - {{ field: cost, label: Cost, format: "$,.0f" }}
    - {{ field: units, label: Units, format: ",.0f" }}
    - {{ field: orders, label: Orders, format: ",.0f" }}
    - {{ field: customers, label: Customers, format: ",.0f" }}
    - {{ field: returns, label: Returns }}
    - {{ field: margin, label: Margin, format: ".1%" }}
    - {{ field: growth, label: Growth, format: "+.1%" }}
    - {{ field: nps, label: NPS }}
    - tier
  style:
    pageSize: 15
"#
    )
}

/// A single column of long names: it stretches to the container's width, so at
/// 375px each name shows on one line. (The longest name is 18.92em, under the
/// 20em cap, so the column's minimum asks for its whole width; the name fits on
/// one line either way, because the column is wider than its minimum.)
fn one_column_spec() -> String {
    r#"type: chart
version: 1
data:
  provider: inline
  rows:
    - { customer: "Northwind Traders International Ltd" }
    - { customer: "Contoso Pharmaceuticals" }
    - { customer: "Fabrikam" }
visualize:
  type: table
  columns: customer
"#
    .to_string()
}

/// A theme like an app that draws tables on a card: transparent header and
/// rows, with the card colour as the table surface.
fn transparent_theme() -> Theme {
    let mut t = Theme::default();
    t.table_header_bg = "transparent".into();
    t.table_row_bg = "transparent".into();
    t.table_row_bg_alt = "transparent".into();
    t.table_surface = "#ffffff".into();
    t
}

fn render(spec: &str, theme: Theme) -> chartml_core::element::ChartElement {
    let mut chartml = ChartML::new();
    chartml.register_renderer("table", TableRenderer::new());
    chartml.set_theme(theme);
    chartml.render_from_yaml(spec).expect("table renders")
}

fn document() -> web_sys::Document {
    web_sys::window().expect("browser DOM").document().expect("browser DOM")
}

fn ensure_stylesheet() {
    let doc = document();
    if doc.get_element_by_id("chartml-css").is_none() {
        let style = doc.create_element("style").expect("browser DOM");
        style.set_id("chartml-css");
        style.set_text_content(Some(STYLESHEET));
        doc.head().expect("browser DOM").append_child(&style).expect("browser DOM");
    }
}

/// A `width`×400 container on a white card, appended to the page.
fn host(width: u32) -> web_sys::HtmlElement {
    ensure_stylesheet();
    let doc = document();
    let card = doc.create_element("div").expect("browser DOM");
    card.set_attribute("style", "background: #ffffff; padding: 0; margin: 0;").expect("browser DOM");
    let host = doc.create_element("div").expect("browser DOM");
    host.set_attribute("style", &format!("width: {width}px; height: 400px;")).expect("browser DOM");
    card.append_child(&host).expect("browser DOM");
    doc.body().expect("browser DOM").append_child(&card).expect("browser DOM");
    host.dyn_into::<web_sys::HtmlElement>().expect("browser DOM")
}

#[derive(Clone, Copy, Debug)]
enum Path {
    Leptos,
    Svg,
}

/// Put the table into a fresh `width`-wide container via `path`.
fn mount(path: Path, spec: &str, theme: Theme, width: u32) -> web_sys::HtmlElement {
    let element = render(spec, theme);
    let host = host(width);
    match path {
        Path::Leptos => {
            let handle = leptos::mount::mount_to(host.clone(), move || {
                chartml_leptos::render_element(&element)
            });
            // Keep the view mounted for the rest of the test page.
            std::mem::forget(handle);
        }
        Path::Svg => host.set_inner_html(&element_to_svg(&element, f64::from(width), 400.0)),
    }
    host
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct State {
    scroll_left: f64,
    max_scroll_left: f64,
    rows_span_scroll_width: bool,
    misalign_px: f64,
    sticky_cells: u32,
    fade_width: f64,
    fade_covers_text_px: f64,
    clipped_cells: u32,
    broken_words: Vec<String>,
    wrapped_cells: Vec<String>,
    columns_in_view: Vec<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Vertical {
    max_scroll_top: f64,
    header_offset_px: f64,
    has_pager: bool,
    pager_inside: bool,
    pager_below_scroll_area: bool,
}

#[derive(Deserialize, Debug)]
struct Measurements {
    states: Vec<State>,
    vertical: Vertical,
}

/// Measure the laid-out table in `host` at every 40px of sideways scroll
/// (and the right end), then scrolled to the bottom. Runs in the page so it
/// reads real layout.
fn measure(host: &web_sys::HtmlElement) -> Measurements {
    let f = js_sys::Function::new_with_args(
        "host",
        r#"
        host.scrollIntoView({ block: 'start' });
        const sc = host.querySelector('.chartml-table-scroll');
        const header = host.querySelector('.chartml-table-header');
        const rows = [...host.querySelectorAll('.chartml-table-row')];
        const cellsOf = r => [...r.children].filter(c =>
            c.classList.contains('chartml-table-cell') || c.classList.contains('chartml-table-header-cell'));
        const one = () => {
            const sr = sc.getBoundingClientRect(); const visL = sr.left, visR = sr.left + sc.clientWidth;
            const hdr = cellsOf(header), r0 = cellsOf(rows[0]);
            const fade = rows[0].querySelector('.chartml-table-fade'); const fr = fade.getBoundingClientRect();
            let covers = 0;
            for (const r of [header, ...rows]) for (const c of cellsOf(r)) {
                const rg = document.createRange(); rg.selectNodeContents(c);
                for (const t of rg.getClientRects()) covers += Math.max(0, Math.min(t.right, fr.right, visR) - Math.max(t.left, fr.left, visL));
            }
            return {
                scrollLeft: sc.scrollLeft, maxScrollLeft: sc.scrollWidth - sc.clientWidth,
                rowsSpanScrollWidth: [header, ...rows].every(r => Math.abs(r.getBoundingClientRect().width - sc.scrollWidth) < 1),
                misalignPx: Math.max(...hdr.map((c, i) => Math.abs(c.getBoundingClientRect().left - r0[i].getBoundingClientRect().left))),
                stickyCells: [header, ...rows].flatMap(cellsOf).filter(c => getComputedStyle(c).position === 'sticky').length,
                fadeWidth: fr.width,
                fadeCoversTextPx: (sc.scrollWidth - sc.clientWidth > 0 && sc.scrollLeft >= sc.scrollWidth - sc.clientWidth - 0.5) ? covers : 0,
                clippedCells: [header, ...rows].flatMap(cellsOf).filter(c => c.scrollWidth > c.clientWidth + 0.5 || c.scrollHeight > c.clientHeight + 0.5).length,
                brokenWords: [header, ...rows].flatMap(cellsOf).flatMap(c => {
                    const node = c.firstChild; if (!node) return [];
                    const out = []; const re = /\S+/g; let m;
                    while ((m = re.exec(node.textContent))) {
                        const rg = document.createRange(); rg.setStart(node, m.index); rg.setEnd(node, m.index + m[0].length);
                        if (new Set([...rg.getClientRects()].map(t => Math.round(t.top))).size > 1) out.push(m[0]);
                    }
                    return out;
                }),
                wrappedCells: [header, ...rows].flatMap(cellsOf).filter(c => {
                    const rg = document.createRange(); rg.selectNodeContents(c);
                    return new Set([...rg.getClientRects()].map(t => Math.round(t.top))).size > 1;
                }).map(c => c.textContent),
                // A column is in view when it shows whole, or fills the
                // visible area, at this scroll position.
                columnsInView: r0.map((c, i) => { const b = c.getBoundingClientRect();
                    const shown = Math.min(b.right, visR) - Math.max(b.left, visL);
                    return shown >= Math.min(b.width, visR - visL) - 0.5 ? i : -1; }).filter(i => i >= 0),
            };
        };
        const states = []; const max = sc.scrollWidth - sc.clientWidth;
        for (let x = 0; ; x += 40) { sc.scrollLeft = Math.min(x, max); states.push(one()); if (x >= max) break; }
        sc.scrollLeft = 0;
        // Scrolled to the bottom: the header stays at the top of the scroll
        // area and the pager stays inside the table, below it.
        const vmax = sc.scrollHeight - sc.clientHeight;
        sc.scrollTop = vmax;
        const pager = host.querySelector('.chartml-table-pager');
        const vertical = {
            maxScrollTop: vmax,
            headerOffsetPx: header.getBoundingClientRect().top - sc.getBoundingClientRect().top,
            hasPager: !!pager,
            pagerInside: !!pager && pager.getBoundingClientRect().bottom <= host.querySelector('.chartml-table').getBoundingClientRect().bottom + 0.5,
            pagerBelowScrollArea: !!pager && pager.getBoundingClientRect().top >= sc.getBoundingClientRect().bottom - 0.5,
        };
        sc.scrollTop = 0;
        return JSON.stringify({ states, vertical });
        "#,
    );
    let json = f.call1(&JsValue::NULL, host).expect("measure runs").as_string().expect("JSON string");
    serde_json::from_str(&json).expect("measurements parse")
}

struct Case<'a> {
    path: Path,
    spec: &'a str,
    columns: usize,
    theme: Theme,
    width: u32,
    expect_sideways: bool,
    expect_vertical: bool,
    expect_pager: bool,
    /// Every header label and every value on one line (true wherever each
    /// column's one-line width fits in the visible area).
    expect_single_line: bool,
}

fn check(case: Case<'_>) {
    let host = mount(case.path, case.spec, case.theme, case.width);
    let Measurements { states, vertical } = measure(&host);
    let ctx = format!("{:?} at {}px", case.path, case.width);
    let overflow = states[0].max_scroll_left > 0.0;
    assert_eq!(overflow, case.expect_sideways, "{ctx}: sideways scroll {overflow}");
    let mut seen = std::collections::BTreeSet::new();
    for s in &states {
        let at = format!("{ctx}, scrollLeft {}", s.scroll_left);
        assert!(s.rows_span_scroll_width, "{at}: a row is narrower than the scrolled width");
        assert!(s.misalign_px < 0.5, "{at}: header and body columns misaligned by {}px", s.misalign_px);
        assert_eq!(s.sticky_cells, 0, "{at}: {} header or body cells are sticky", s.sticky_cells);
        assert_eq!(s.clipped_cells, 0, "{at}: {} cells cut off", s.clipped_cells);
        assert!(s.broken_words.is_empty(), "{at}: words broken across lines: {:?}", s.broken_words);
        if case.expect_single_line {
            assert!(s.wrapped_cells.is_empty(), "{at}: wrapped although sized to fit: {:?}", s.wrapped_cells);
        }
        if overflow {
            assert!((s.fade_width - 32.0).abs() < 0.5, "{at}: fade {}px while columns overflow", s.fade_width);
            assert!(s.fade_covers_text_px < 0.5, "{at}: fade covers {}px of text at the right end", s.fade_covers_text_px);
        } else {
            assert!(s.fade_width < 0.5, "{at}: fade {}px with nothing hidden", s.fade_width);
        }
        seen.extend(s.columns_in_view.iter().copied());
    }
    let missing: Vec<usize> = (0..case.columns).filter(|i| !seen.contains(i)).collect();
    assert!(missing.is_empty(), "{ctx}: columns never scrolled into view: {missing:?}");
    assert_eq!(vertical.max_scroll_top > 0.0, case.expect_vertical, "{ctx}: vertical scroll");
    assert!(vertical.header_offset_px.abs() < 0.5, "{ctx}: header left the top by {}px when scrolled down", vertical.header_offset_px);
    assert_eq!(vertical.has_pager, case.expect_pager, "{ctx}: pager shown");
    if case.expect_pager {
        assert!(vertical.pager_inside && vertical.pager_below_scroll_area, "{ctx}: pager not visible below the rows");
    }
}

fn wide(path: Path, theme: Theme, width: u32, expect_sideways: bool, expect_single_line: bool) {
    let spec = wide_spec();
    check(Case {
        path,
        spec: &spec,
        columns: 12,
        theme,
        width,
        expect_sideways,
        expect_vertical: true,
        expect_pager: true,
        expect_single_line,
    });
}

#[wasm_bindgen_test]
fn narrow_leptos_default_theme() {
    wide(Path::Leptos, Theme::default(), 375, true, true);
}

#[wasm_bindgen_test]
fn narrow_svg_default_theme() {
    wide(Path::Svg, Theme::default(), 375, true, true);
}

#[wasm_bindgen_test]
fn narrow_leptos_transparent_rows() {
    wide(Path::Leptos, transparent_theme(), 375, true, true);
}

#[wasm_bindgen_test]
fn narrow_svg_transparent_rows() {
    wide(Path::Svg, transparent_theme(), 375, true, true);
}

#[wasm_bindgen_test]
fn wide_leptos_default_theme() {
    wide(Path::Leptos, Theme::default(), 1500, false, true);
}

#[wasm_bindgen_test]
fn wide_svg_default_theme() {
    wide(Path::Svg, Theme::default(), 1500, false, true);
}

#[wasm_bindgen_test]
fn one_column_fits_without_scrolling() {
    let spec = one_column_spec();
    for path in [Path::Leptos, Path::Svg] {
        check(Case {
            path,
            spec: &spec,
            columns: 1,
            theme: Theme::default(),
            width: 375,
            expect_sideways: false,
            expect_vertical: false,
            expect_pager: false,
            expect_single_line: true,
        });
    }
}

/// A very narrow container (a narrow dashboard column): the columns are
/// wider than the visible area, so the table scrolls and
/// longer values may wrap at their spaces, but no word or number is broken.
#[wasm_bindgen_test]
fn very_narrow_leptos_default_theme() {
    wide(Path::Leptos, Theme::default(), 240, true, false);
}

#[wasm_bindgen_test]
fn very_narrow_svg_transparent_rows() {
    wide(Path::Svg, transparent_theme(), 240, true, false);
}

/// Regression (CHA-22 F1): a host that widens `--chartml-table-cell-padding`
/// must get wider columns, not squeezed text. Every column's minimum is its
/// content only and layout adds the padding actually applied, so with a wide
/// override in a narrow container there are still no cells cut off, no words
/// broken across lines, and no cell overflowing its row.
#[wasm_bindgen_test]
fn wide_cell_padding_widens_columns_instead_of_breaking_text() {
    let spec = wide_spec();
    for path in [Path::Leptos, Path::Svg] {
        let host = mount(path, &spec, Theme::default(), 375);
        let state = measure_with_padding(&host, "8px 40px");
        let ctx = format!("{path:?}: --chartml-table-cell-padding: 8px 40px at 375px");
        assert!(state.clipped_cells == 0, "{ctx}: {} cells cut off", state.clipped_cells);
        assert!(state.broken_words.is_empty(), "{ctx}: words broken across lines: {:?}", state.broken_words);
        assert!(state.cell_overflow_px < 0.5, "{ctx}: a cell overflows its row by {}px", state.cell_overflow_px);
        // The override really applied (otherwise the assertions above are vacuous).
        assert_eq!(state.padding, "8px 40px", "{ctx}: padding was {:?}", state.padding);
        // And the columns are wider than the theme padding would give: the
        // table still needs to scroll sideways at this width.
        assert!(state.sideways, "{ctx}: the widened padding must not fit at 375px");
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PaddedState {
    padding: String,
    sideways: bool,
    clipped_cells: u32,
    broken_words: Vec<String>,
    cell_overflow_px: f64,
}

/// Apply `padding` as the host's `--chartml-table-cell-padding`, force layout
/// and measure the table's text integrity.
fn measure_with_padding(host: &web_sys::HtmlElement, padding: &str) -> PaddedState {
    let f = js_sys::Function::new_with_args(
        "host, padding",
        r#"
        host.style.setProperty('--chartml-table-cell-padding', padding);
        void host.offsetWidth;
        const sc = host.querySelector('.chartml-table-scroll');
        const cellsOf = r => [...r.children].filter(c =>
            c.classList.contains('chartml-table-cell') || c.classList.contains('chartml-table-header-cell'));
        const rows = [host.querySelector('.chartml-table-header'), ...host.querySelectorAll('.chartml-table-row')];
        let overflow = 0;
        for (const r of rows) {
            const rb = r.getBoundingClientRect();
            for (const c of cellsOf(r)) {
                const cb = c.getBoundingClientRect();
                overflow = Math.max(overflow, cb.right - rb.right);
            }
        }
        const first = cellsOf(rows[0])[0];
        return JSON.stringify({
            padding: getComputedStyle(first).padding,
            sideways: sc.scrollWidth - sc.clientWidth > 0,
            clippedCells: rows.flatMap(cellsOf).filter(c => c.scrollWidth > c.clientWidth + 0.5 || c.scrollHeight > c.clientHeight + 0.5).length,
            brokenWords: rows.flatMap(cellsOf).flatMap(c => {
                const node = c.firstChild; if (!node) return [];
                const out = []; const re = /\S+/g; let m;
                while ((m = re.exec(node.textContent))) {
                    const rg = document.createRange(); rg.setStart(node, m.index); rg.setEnd(node, m.index + m[0].length);
                    if (new Set([...rg.getClientRects()].map(t => Math.round(t.top))).size > 1) out.push(m[0]);
                }
                return out;
            }),
            cellOverflowPx: Math.max(0, overflow),
        });
        "#,
    );
    let json = f
        .call2(&JsValue::NULL, host, &JsValue::from_str(padding))
        .expect("padding measurement runs")
        .as_string()
        .expect("JSON string");
    serde_json::from_str(&json).expect("padded state parses")
}

/// Regression (CHA-22 F4): a table that overflows in a narrow container and
/// is then given a wider one must have no sideways scroll and no visible fade.
/// A fade whose visibility depends on real scroll-time overflow stays on here,
/// because its own width created the overflow that kept it on; the estimate
/// cannot, because it is a function of the columns' minimums alone.
///
/// The two host paths resize differently — the Leptos mount reflows into the
/// host's new width, while the SVG's `<foreignObject>` carries the size it was
/// rendered at — so each path is re-hosted at the wider size the way a host
/// would (a re-render for the SVG, a resize for the mounted DOM) and measured
/// after layout settles.
#[wasm_bindgen_test]
fn grown_container_fits_without_a_fade() {
    let spec = wide_spec();
    let narrow = 375;
    for path in [Path::Leptos, Path::Svg] {
        let ctx = format!("{path:?}: grown from {narrow}px to fit");
        let narrow_host = mount(path, &spec, Theme::default(), narrow);
        let before = measure_grow(&narrow_host, None);
        assert!(before.narrow_sideways, "{ctx}: the narrow table must scroll sideways first");
        assert!(before.narrow_fade > 31.5, "{ctx}: fade {} at {narrow}px", before.narrow_fade);

        // The width the laid-out table actually occupies (columns plus the
        // fade's track), plus room to spare.
        let wide = (before.needed_width + 40.0).ceil() as u32;
        let wide_host = mount(path, &spec, Theme::default(), wide);
        let after = measure_grow(&wide_host, Some(wide));
        assert!(!after.narrow_sideways, "{ctx}: sideways scroll {}px at {wide}px", after.narrow_sideways_by);
        assert!(after.narrow_fade < 0.5, "{ctx}: fade {} at {wide}px", after.narrow_fade);
    }
}

/// Measure a freshly mounted table: whether it scrolls sideways, its fade
/// width, and the width its columns need. `grown_to` sets the host width and
/// the table's own SVG box, as a host that resizes its container does.
fn measure_grow(host: &web_sys::HtmlElement, grown_to: Option<u32>) -> GrowResult {
    let f = js_sys::Function::new_with_args(
        "host, grownTo",
        r#"
        const svg = host.querySelector('svg');
        const fo = host.querySelector('foreignObject');
        if (grownTo != null) {
            // Grow, then keep growing while the columns still overflow: the
            // columns' minimums are capped against the visible width at
            // narrow sizes, so one guess can be short.
            let width = grownTo;
            for (let i = 0; i < 12; i++) {
                if (fo) { fo.setAttribute('width', width); svg.setAttribute('width', width); }
                host.style.width = width + 'px';
                void host.offsetWidth;
                const s2 = host.querySelector('.chartml-table-scroll');
                const over = s2.scrollWidth - s2.clientWidth;
                if (over <= 0) break;
                width += over + 8;
            }
        }
        void host.offsetWidth;
        const sc = host.querySelector('.chartml-table-scroll');
        const fade = host.querySelector('.chartml-table-fade').getBoundingClientRect();
        // The width the columns need: the fit box is at least the scroll
        // area's width, so use the scroll area's own content width, which is
        // the columns plus the fade track.
        const columnsPlusFade = sc.scrollWidth;
        return JSON.stringify({
            narrowSideways: sc.scrollWidth - sc.clientWidth > 0,
            narrowSidewaysBy: sc.scrollWidth - sc.clientWidth,
            narrowFade: fade.width,
            neededWidth: columnsPlusFade,
        });
        "#,
    );
    let grown = match grown_to {
        Some(width) => JsValue::from(width),
        None => JsValue::NULL,
    };
    let json = f
        .call2(&JsValue::NULL, host, &grown)
        .expect("grow measurement runs")
        .as_string()
        .expect("JSON string");
    serde_json::from_str(&json).expect("grow result parses")
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GrowResult {
    narrow_sideways: bool,
    narrow_sideways_by: f64,
    narrow_fade: f64,
    needed_width: f64,
}

