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

/// A table spec that uses the schema's per-column `align` and `width`: Month
/// centred and fixed at 200px, Revenue forced left, Cost fixed at 120px.
fn align_and_width_spec() -> String {
    r#"type: chart
version: 1
data:
  provider: inline
  rows:
    - { month: "January", revenue: 1234.5, cost: 400 }
    - { month: "February", revenue: 987.25, cost: 450 }
    - { month: "March", revenue: 1800.0, cost: 500 }
visualize:
  type: table
  columns:
    - { field: month, label: Month, width: 200, align: center }
  rows:
    - { field: revenue, label: Revenue, format: "$,.0f", align: left }
    - { field: cost, label: Cost, format: "$,.0f", width: 120 }
"#
    .to_string()
}

/// CHA-22 line 1 and line 4: the schema's per-column `align` and `width`
/// really change the rendered table. `align` must reach the computed
/// `text-align` of the header and every body cell (and override the numeric
/// default); a fixed `width` must be the column's rendered width in the
/// header and the body, keep the header and body aligned, and leave no column
/// pinned.
#[wasm_bindgen_test]
fn column_align_and_width_render_as_documented() {
    let spec = align_and_width_spec();
    for path in [Path::Leptos, Path::Svg] {
        let host = mount(path, &spec, Theme::default(), 800);
        let state = measure_align_and_width(&host);
        let ctx = format!("{path:?}: align + width at 800px");
        // Month is explicitly centred; Revenue is explicitly left even though
        // it is numeric; Cost has no align, so it keeps the numeric default.
        assert_eq!(state.align, vec!["center", "left", "right"], "{ctx}: header text-align");
        // Every body row carries the same alignment, not only the header.
        assert_eq!(state.body_align.len(), 3, "{ctx}: body rows");
        for (row, align) in state.body_align.iter().enumerate() {
            assert_eq!(align, &vec!["center", "left", "right"], "{ctx}: body row {row} text-align");
        }
        assert_eq!(state.header_widths.len(), 3, "{ctx}: header cells");
        // A fixed width is the column's content width; the padding the table
        // applies is added by layout (24px of theme padding here).
        assert!((state.header_widths[0] - 224.0).abs() < 0.6, "{ctx}: Month width {} (200px content + 24px padding)", state.header_widths[0]);
        assert!((state.header_widths[2] - 144.0).abs() < 0.6, "{ctx}: Cost width {} (120px content + 24px padding)", state.header_widths[2]);
        // The same widths in the body: a per-column track, not a per-row one.
        for row in &state.body_widths {
            assert_eq!(row.len(), 3, "{ctx}: body cells");
            assert!((row[0] - 224.0).abs() < 0.6, "{ctx}: body Month {} (200px content + padding)", row[0]);
            assert!((row[2] - 144.0).abs() < 0.6, "{ctx}: body Cost {} (120px content + padding)", row[2]);
        }
        assert!(state.misalign_px < 0.5, "{ctx}: header and body misaligned by {}px", state.misalign_px);
        assert_eq!(state.sticky_cells, 0, "{ctx}: a column is pinned");
        assert!(!state.sideways, "{ctx}: the fitting table must not scroll sideways");
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AlignWidthState {
    align: Vec<String>,
    body_align: Vec<Vec<String>>,
    header_widths: Vec<f64>,
    body_widths: Vec<Vec<f64>>,
    misalign_px: f64,
    sticky_cells: u32,
    sideways: bool,
}

/// Read the computed `text-align` of each column's cells, the rendered widths
/// of the fixed columns, and the no-pin/inline-alignment invariants.
fn measure_align_and_width(host: &web_sys::HtmlElement) -> AlignWidthState {
    let f = js_sys::Function::new_with_args(
        "host",
        r#"
        const sc = host.querySelector('.chartml-table-scroll');
        const header = host.querySelector('.chartml-table-header');
        const rows = [...host.querySelectorAll('.chartml-table-row')];
        const cellsOf = r => [...r.children].filter(c =>
            c.classList.contains('chartml-table-cell') || c.classList.contains('chartml-table-header-cell'));
        const hdr = cellsOf(header);
        const widths = cells => cells.map(c => Math.round(c.getBoundingClientRect().width * 100) / 100);
        return JSON.stringify({
            align: hdr.map(c => getComputedStyle(c).textAlign),
            bodyAlign: rows.map(r => cellsOf(r).map(c => getComputedStyle(c).textAlign)),
            headerWidths: widths(hdr),
            bodyWidths: rows.map(r => widths(cellsOf(r))),
            misalignPx: Math.max(...hdr.map((c, i) => Math.abs(c.getBoundingClientRect().left - cellsOf(rows[0])[i].getBoundingClientRect().left))),
            stickyCells: [header, ...rows].flatMap(cellsOf).filter(c => getComputedStyle(c).position === 'sticky').length,
            sideways: sc.scrollWidth - sc.clientWidth > 0,
        });
        "#,
    );
    let json = f
        .call1(&JsValue::NULL, host)
        .expect("align/width measurement runs")
        .as_string()
        .expect("JSON string");
    serde_json::from_str(&json).expect("align/width state parses")
}

/// A table with a fixed 120px content column and a host padding that takes most
/// of it: `--chartml-table-cell-padding: 8px 40px` leaves the 120px for text
/// and adds 80px of padding around it.
fn narrow_fixed_width_spec() -> String {
    r#"type: chart
version: 1
data:
  provider: inline
  rows:
    - { status: "Ok", amount: 1200 }
    - { status: "Failed", amount: 500 }
visualize:
  type: table
  columns:
    - { field: status, width: 120 }
  rows:
    - { field: amount, label: Amount, format: "$,.0f" }
"#
    .to_string()
}

/// A table whose every column is fixed-width.
fn all_fixed_width_spec() -> String {
    r#"type: chart
version: 1
data:
  provider: inline
  rows:
    - { a: "Alpha", b: "Beta", c: "Gamma" }
    - { a: "A2", b: "B2", c: "C2" }
visualize:
  type: table
  columns:
    - { field: a, width: 100 }
    - { field: b, width: 120 }
    - { field: c, width: 90 }
"#
    .to_string()
}

/// Regression (CHA-22 lifecycle): a table of only fixed-width columns must
/// still fill its scroll area. Rows floored at `min-content` shrank to the sum
/// of the fixed tracks, leaving the row background and its bottom border short
/// of the row's end; the scroll area is now a grid of
/// `max(100%, min-content)`, so every row spans at least the visible width.
#[wasm_bindgen_test]
fn all_fixed_columns_still_fill_the_row() {
    let spec = all_fixed_width_spec();
    for path in [Path::Leptos, Path::Svg] {
        let host = mount(path, &spec, Theme::default(), 800);
        let state = measure_row_span(&host);
        let ctx = format!("{path:?}: all-fixed table at 800px");
        assert_eq!(state.rows, 3, "{ctx}: header + two body rows");
        // The rule must be one the browser actually accepts — an invalid
        // track list is silently dropped and the rows then fill only by the
        // implicit auto track's default stretch, which a host can override.
        assert_eq!(
            state.emitted_track.as_deref(),
            Some("minmax(min-content, 100%)"),
            "{ctx}: the scroll area's track rule was dropped by the CSS parser",
        );
        for (row, width) in state.row_widths.iter().enumerate() {
            assert!(
                (width - state.client_width).abs() < 1.0,
                "{ctx}: row {row} is {width}px across a {}px area",
                state.client_width,
            );
        }
        // The row owns its bottom rule, so the rule spans the row's width
        // rather than stopping at the last (fixed) column.
        for (row, width) in state.border_widths.iter().enumerate() {
            assert!(
                (width - state.client_width).abs() < 1.0,
                "{ctx}: row {row} draws its bottom rule only {width}px across a {}px area",
                state.client_width,
            );
        }
        // The fill must not depend on the default stretch of an implicit
        // track: with `justify-content: start` forced on the scroll area, the
        // rows still span the area because the track rule itself says so.
        let forced = measure_row_span_with_justify(&host, "start");
        for (row, width) in forced.row_widths.iter().enumerate() {
            assert!(
                (width - forced.client_width).abs() < 1.0,
                "{ctx}: with justify-content:start, row {row} is {width}px across a {}px area",
                forced.client_width,
            );
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RowSpanState {
    client_width: f64,
    rows: usize,
    row_widths: Vec<f64>,
    border_widths: Vec<f64>,
    emitted_track: Option<String>,
}

/// Re-measure row widths after forcing `justify-content` on the scroll area,
/// the property a host could set through the published class.
fn measure_row_span_with_justify(host: &web_sys::HtmlElement, justify: &str) -> RowSpanState {
    let f = js_sys::Function::new_with_args(
        "host, justify",
        r#"
        const sc = host.querySelector('.chartml-table-scroll');
        sc.style.justifyContent = justify;
        void host.offsetWidth;
        const rows = [host.querySelector('.chartml-table-header'), ...host.querySelectorAll('.chartml-table-row')];
        return JSON.stringify({
            clientWidth: sc.clientWidth,
            rows: rows.length,
            rowWidths: rows.map(r => Math.round(r.getBoundingClientRect().width * 100) / 100),
            // Where the row's own bottom rule reaches: the row box, which is
            // what the border paints across.
            borderWidths: rows.map(r => {
                const rb = r.getBoundingClientRect();
                return getComputedStyle(r).borderBottomWidth === '0px' ? 0 : Math.round(rb.width * 100) / 100;
            }),
            emittedTrack: sc.style.getPropertyValue('grid-template-columns') || null,
        });
        "#,
    );
    let json = f
        .call2(&JsValue::NULL, host, &JsValue::from_str(justify))
        .expect("justify measurement runs")
        .as_string()
        .expect("JSON string");
    serde_json::from_str(&json).expect("row span with justify parses")
}

/// Measure each row's painted width against the scroll area's visible width.
fn measure_row_span(host: &web_sys::HtmlElement) -> RowSpanState {
    let f = js_sys::Function::new_with_args(
        "host",
        r#"
        const sc = host.querySelector('.chartml-table-scroll');
        const rows = [host.querySelector('.chartml-table-header'), ...host.querySelectorAll('.chartml-table-row')];
        return JSON.stringify({
            clientWidth: sc.clientWidth,
            rows: rows.length,
            rowWidths: rows.map(r => Math.round(r.getBoundingClientRect().width * 100) / 100),
            // Where the row's own bottom rule reaches: the row box, which is
            // what the border paints across.
            borderWidths: rows.map(r => {
                const rb = r.getBoundingClientRect();
                return getComputedStyle(r).borderBottomWidth === '0px' ? 0 : Math.round(rb.width * 100) / 100;
            }),
            // What the CSS engine kept for the track list: empty when the
            // declaration was invalid and dropped.
            emittedTrack: sc.style.getPropertyValue('grid-template-columns') || null,
        });
        "#,
    );
    let json = f.call1(&JsValue::NULL, host).expect("row span runs").as_string().expect("JSON string");
    serde_json::from_str(&json).expect("row span parses")
}

/// CHA-22 line 1 and line 4: a fixed `width` is the column's content width, so
/// the cell's content box is exactly that and the padding the host applies is
/// added around it by layout. The track is `max-content`, so it always fits the
/// cell and its padding: the cell never reaches into the next column, at any
/// padding. A track built from the theme padding instead overflowed by the
/// difference — with `width: 60` and `--chartml-table-cell-padding: 8px 40px`
/// an 80px cell sat in a 60px track and overlapped by 20px.
#[wasm_bindgen_test]
fn fixed_width_cells_stay_inside_their_track() {
    let spec = narrow_fixed_width_spec();
    for path in [Path::Leptos, Path::Svg] {
        let host = mount(path, &spec, Theme::default(), 800);
        for padding in [None, Some("8px 40px")] {
            let state = measure_fixed_tracks(&host, padding);
            let ctx = format!("{path:?}: 120px fixed column with padding {padding:?}");
            // The track is `max-content`: the declared content width plus
            // whatever padding is applied, so the cell always fits it.
            let host_pad = if padding.is_some() { 80.0 } else { 24.0 };
            let expected = 120.0 + host_pad;
            assert!(
                (state.track - expected).abs() < 0.7,
                "{ctx}: resolved track is {}px, expected {}px (120px content + {host_pad}px padding)",
                state.track,
                expected,
            );
            assert!(
                (state.cell - expected).abs() < 0.7,
                "{ctx}: cell is {}px, expected {}px (120px content + {host_pad}px padding)",
                state.cell,
                expected,
            );
            // The real defect: the cell reaching into the next column. The
            // track is `max-content`, so it fits whatever padding the host
            // applies and the cell never overlaps, at any padding.
            assert!(
                state.cell_right_gap >= -0.7,
                "{ctx}: the fixed cell overlaps the next column by {}px",
                -state.cell_right_gap,
            );
            // A 60px column legitimately wraps its label onto a couple of
            // lines; one-letter-per-line (a squeezed content box) is far
            // taller than that.
            assert!(
                state.header_height < 90.0,
                "{ctx}: the header collapsed to {}px",
                state.header_height,
            );
            if padding.is_none() {
                assert!(!state.sideways, "{ctx}: the table fits its 800px host");
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FixedTrackState {
    track: f64,
    cell: f64,
    cell_right_gap: f64,
    header_height: f64,
    sideways: bool,
}

/// Compare the first row's fixed cell with the grid track it lands in.
fn measure_fixed_tracks(host: &web_sys::HtmlElement, padding: Option<&str>) -> FixedTrackState {
    let f = js_sys::Function::new_with_args(
        "host, padding",
        r#"
        if (padding) host.style.setProperty('--chartml-table-cell-padding', padding);
        else host.style.removeProperty('--chartml-table-cell-padding');
        void host.offsetWidth;
        const header = host.querySelector('.chartml-table-header');
        const cells = [...header.children].filter(c => c.classList.contains('chartml-table-header-cell'));
        const sc = host.querySelector('.chartml-table-scroll');
        // The first track is the fixed column; the last is the fade.
        const tracks = getComputedStyle(header).gridTemplateColumns.split(' ').map(v => parseFloat(v));
        const fixedBox = cells[0].getBoundingClientRect();
        const nextBox = cells[1].getBoundingClientRect();
        return JSON.stringify({
            track: Math.round(tracks[0] * 100) / 100,
            cell: Math.round(fixedBox.width * 100) / 100,
            cellRightGap: Math.round((nextBox.left - fixedBox.right) * 100) / 100,
            headerHeight: Math.round(cells[0].getBoundingClientRect().height * 100) / 100,
            sideways: sc.scrollWidth - sc.clientWidth > 0,
        });
        "#,
    );
    let arg = match padding {
        Some(p) => JsValue::from_str(p),
        None => JsValue::NULL,
    };
    let json = f
        .call2(&JsValue::NULL, host, &arg)
        .expect("fixed-track measurement runs")
        .as_string()
        .expect("JSON string");
    serde_json::from_str(&json).expect("fixed-track state parses")
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

/// Regression (CHA-22 F4): a table that overflows in a narrow container and is
/// then resized in place to a width where its columns fit must have no sideways
/// scroll and no visible fade. This is the resize path a dashboard tile takes
/// when it grows after first layout, and the case a fade whose visibility came
/// from real scroll-time overflow could not clear (its own width kept the
/// overflow it measured, so it stayed on for good).
///
/// The Leptos mount reflows into the host's new width. The SVG's
/// `<foreignObject>` carries the size it was rendered at, so resizing a host
/// around it is not a re-render — that path is checked with a fresh render at
/// the fitting width, which is the *other* half of the same contract (a table
/// that fits has no fade, regardless of how it got there).
#[wasm_bindgen_test]
fn resized_to_fit_clears_the_fade() {
    let spec = wide_spec();
    let narrow = 375;
    for path in [Path::Leptos, Path::Svg] {
        let ctx = format!("{path:?}: resized from {narrow}px to fit");
        let host = mount(path, &spec, Theme::default(), narrow);
        let before = measure_grow(&host, None);
        assert!(before.narrow_sideways, "{ctx}: the narrow table must scroll sideways first");
        assert!(before.narrow_fade > 31.5, "{ctx}: fade {} at {narrow}px", before.narrow_fade);
        let wide = (before.needed_width + 40.0).ceil() as u32;
        let after = measure_grow(&host, Some(wide));
        assert!(
            !after.narrow_sideways,
            "{ctx}: sideways scroll {}px after resizing to {wide}px",
            after.narrow_sideways_by,
        );
        assert!(
            after.narrow_fade < 0.5,
            "{ctx}: fade {}px after resizing to {wide}px",
            after.narrow_fade,
        );
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
            // Resize this same host, in place, until the columns fit: their
            // minimums are capped against the visible width when narrow, so
            // one guess can be short. On the SVG path the foreignObject carries
            // the render size, so it follows the host (a host that re-renders
            // at the new size); on the Leptos path the mounted DOM reflows.
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

