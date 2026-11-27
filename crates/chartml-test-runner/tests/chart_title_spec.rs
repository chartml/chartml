//! CHA-59: a chart's `title` shows exactly once wherever the chart is shown,
//! inside the size the host asked for, and no rotated axis label leaves the SVG.
//!
//! A host that draws the title as an HTML heading above the SVG (the Leptos
//! component) passes `RenderOptions::title_heading`; the engine then draws
//! none. Every host that shows the SVG alone (the npm components, markdown
//! plugins, `chartml-render`'s PNG, this crate's golden runner) has no heading,
//! so the engine draws the title inside the requested size. These tests drive
//! the real renderers through the real pipeline.

use std::fs;
use std::path::{Path, PathBuf};

use chartml_chart_cartesian::CartesianRenderer;
use chartml_chart_metric::MetricRenderer;
use chartml_chart_pie::PieRenderer;
use chartml_chart_scatter::ScatterRenderer;
use chartml_chart_table::TableRenderer;
use chartml_datafusion::DataFusionTransform;
use chartml_core::element::{ChartElement, TextAnchor, Transform};
use chartml_core::layout::{measure_text, TextMetrics};
use chartml_core::theme::Theme;
use chartml_core::{ChartML, RenderOptions};
use chartml_render::element_to_svg;

const TITLE: &str = "Quarterly Signups";

fn chartml() -> ChartML {
    let mut c = ChartML::new();
    c.register_renderer("bar", CartesianRenderer::new());
    c.register_renderer("line", CartesianRenderer::new());
    c.register_renderer("area", CartesianRenderer::new());
    c.register_renderer("pie", PieRenderer::new());
    c.register_renderer("doughnut", PieRenderer::new());
    c.register_renderer("scatter", ScatterRenderer::new());
    c.register_renderer("bubble", ScatterRenderer::new());
    c.register_renderer("metric", MetricRenderer::new());
    c.register_renderer("table", TableRenderer::new());
    c.register_transform(DataFusionTransform);
    c
}

/// A chart of `chart_type` with the data and fields it needs.
fn yaml(chart_type: &str, title: Option<&str>) -> String {
    let title_line = title.map(|t| format!("title: \"{t}\"\n")).unwrap_or_default();
    let (rows, viz) = match chart_type {
        "bar" | "line" | "area" => (
            "- { month: Jan, sales: 10 }\n    - { month: Feb, sales: 14 }\n    - { month: Mar, sales: 9 }",
            format!("type: {chart_type}\n  columns: month\n  rows: sales"),
        ),
        "pie" | "doughnut" => (
            "- { region: North, sales: 10 }\n    - { region: South, sales: 14 }",
            format!("type: {chart_type}\n  columns: region\n  rows: sales"),
        ),
        "scatter" => (
            "- { x: 1, y: 2 }\n    - { x: 2, y: 5 }\n    - { x: 3, y: 3 }",
            "type: scatter\n  columns: x\n  rows: y".to_string(),
        ),
        "bubble" => (
            "- { x: 1, y: 2, r: 4 }\n    - { x: 2, y: 5, r: 9 }",
            "type: bubble\n  columns: x\n  rows: y\n  marks:\n    size: r".to_string(),
        ),
        "table" => (
            "- { month: Jan, sales: 10 }\n    - { month: Feb, sales: 14 }",
            "type: table\n  columns: month\n  rows:\n    - field: sales".to_string(),
        ),
        other => panic!("no fixture for {other}"),
    };
    format!(
        "type: chart\nversion: 1\n{title_line}data:\n  provider: inline\n  rows:\n    {rows}\nvisualize:\n  {viz}\n"
    )
}

const TYPES: [&str; 8] = ["bar", "line", "area", "pie", "doughnut", "scatter", "bubble", "table"];

async fn svg_of(yaml: &str, width: f64, height: f64, title_heading: bool) -> String {
    let chartml = chartml();
    let opts = RenderOptions {
        title_heading,
        ..RenderOptions::with_size(Some(width), Some(height))
    };
    let fetched = chartml.fetch(yaml, &opts).await.expect("fetch");
    let prepared = chartml.transform(fetched, &opts).await.expect("transform");
    chartml.render_prepared_to_svg(&prepared, &opts).expect("render")
}

/// Count of the chart's title in `svg`: the element carrying the title class.
fn titles(svg: &str) -> usize {
    svg.matches(r#"class="chart-title""#).count()
}

fn root_attr(svg: &str, attr: &str) -> f64 {
    let root = &svg[..svg.find('>').expect("root tag")];
    let needle = format!(" {attr}=\"");
    let start = root.find(&needle).unwrap_or_else(|| panic!("{attr} on {root}")) + needle.len();
    let end = start + root[start..].find('"').expect("closing quote");
    root[start..end].parse().expect("number")
}

#[tokio::test]
async fn every_chart_type_shows_its_title_once_where_no_heading_is_drawn() {
    for chart_type in TYPES {
        let svg = svg_of(&yaml(chart_type, Some(TITLE)), 640.0, 360.0, false).await;
        assert_eq!(titles(&svg), 1, "{chart_type}: {svg}");
        assert_eq!(svg.matches(TITLE).count(), 1, "{chart_type}: the title text once: {svg}");
    }
}

#[tokio::test]
async fn a_host_heading_means_the_engine_draws_no_title() {
    for chart_type in TYPES {
        let svg = svg_of(&yaml(chart_type, Some(TITLE)), 640.0, 360.0, true).await;
        assert_eq!(titles(&svg), 0, "{chart_type}: {svg}");
        assert_eq!(svg.matches(TITLE).count(), 0, "{chart_type}: {svg}");
    }
}

#[tokio::test]
async fn a_chart_without_a_title_draws_none() {
    for chart_type in TYPES {
        let svg = svg_of(&yaml(chart_type, None), 640.0, 360.0, false).await;
        assert_eq!(titles(&svg), 0, "{chart_type}: {svg}");
    }
}

#[tokio::test]
async fn the_title_is_drawn_inside_the_requested_size() {
    for chart_type in TYPES {
        for (width, height) in [(640.0, 360.0), (300.0, 200.0)] {
            let with = svg_of(&yaml(chart_type, Some(TITLE)), width, height, false).await;
            let without = svg_of(&yaml(chart_type, None), width, height, false).await;
            assert_eq!(root_attr(&with, "width"), width, "{chart_type} width: {with}");
            assert_eq!(root_attr(&with, "height"), height, "{chart_type} height: {with}");
            assert_eq!(root_attr(&without, "width"), width, "{chart_type}");
            assert_eq!(root_attr(&without, "height"), height, "{chart_type}");
        }
    }
}

#[test]
fn the_title_band_and_the_chart_fit_the_requested_height() {
    // The sync path (the test runner, `renderToElement`, PNG) has no options;
    // its chart takes the renderer's default size.
    let element = chartml().render_from_yaml(&yaml("bar", Some(TITLE))).expect("render");
    let ChartElement::Svg { viewbox, height, children, .. } = &element else {
        panic!("bar draws an SVG root");
    };
    assert_eq!(*height, Some(viewbox.height));
    let [title, body] = children.as_slice() else {
        panic!("title group and chart body, got {} children", children.len());
    };
    let ChartElement::Group { class, .. } = title else { panic!("title group") };
    assert_eq!(class, "chart-title-group");
    let ChartElement::Group { transform: Some(Transform::Translate(0.0, band)), .. } = body else {
        panic!("body is shifted down by the title band");
    };
    assert!(*band > 0.0 && *band < viewbox.height, "band {band}");
    // The chart behind the band was laid out in the height the band leaves.
    let plain = chartml().render_from_yaml(&yaml("bar", None)).expect("render");
    let ChartElement::Svg { viewbox: plain_viewbox, .. } = &plain else { panic!("svg root") };
    assert_eq!(plain_viewbox.height, viewbox.height, "same size with and without a title");
}

#[tokio::test]
async fn a_long_title_is_cut_with_an_ellipsis_and_keeps_its_full_text_as_the_svg_title() {
    let long = "Quarterly signups by acquisition channel across every region and plan tier";
    let svg = svg_of(&yaml("bar", Some(long)), 300.0, 300.0, false).await;
    assert_eq!(titles(&svg), 1, "{svg}");
    assert!(svg.contains('\u{2026}'), "ellipsis: {svg}");
    assert!(svg.contains(&format!("<title>{long}</title>")), "full text as <title>: {svg}");

    let short = svg_of(&yaml("bar", Some(TITLE)), 640.0, 360.0, false).await;
    assert!(!short.contains("<title>"), "a title that fits has no <title>: {short}");
}

// ---------------------------------------------------------------------------
// Every spec under tests/charts.
// ---------------------------------------------------------------------------

fn specs() -> Vec<(String, serde_yaml::Value)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                out.push(path);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/charts");
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let name = p.strip_prefix(&root).expect("under root").display().to_string();
            let value: serde_yaml::Value =
                serde_yaml::from_str(&fs::read_to_string(&p).expect("read")).expect("yaml");
            (name, value)
        })
        .filter(|(_, v)| {
            let expects_error = v
                .get("assertions")
                .and_then(|a| a.get("expect_error"))
                .and_then(|e| e.as_bool())
                .unwrap_or(false);
            !expects_error
        })
        .collect()
}

fn chart_yaml(spec: &serde_yaml::Value) -> String {
    serde_yaml::to_string(spec.get("chart").expect("chart key")).expect("yaml")
}

#[test]
fn every_titled_spec_in_the_suite_shows_its_title_once() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _guard = runtime.enter(); // the sql transform needs a reactor
    let mut checked = 0;
    for (name, spec) in specs() {
        let chart = spec.get("chart").expect("chart key");
        let title = chart.get("title").and_then(|t| t.as_str());
        let chart_type = chart.get("visualize").and_then(|v| v.get("type")).and_then(|t| t.as_str());
        let Some(title) = title.filter(|t| !t.trim().is_empty()) else { continue };
        if chart_type == Some("metric") {
            continue; // a metric card keeps its title in the card (CHA-49)
        }
        let element = chartml().render_from_yaml(&chart_yaml(&spec)).expect(&name);
        let svg = element_to_svg(&element, 800.0, 400.0);
        assert_eq!(titles(&svg), 1, "{name}");
        assert!(svg.contains("chart-title"), "{name}");
        let shown = svg.contains(&format!(">{}<", xml_escape(title.trim()))) || svg.contains('\u{2026}');
        assert!(shown, "{name}: title text missing");
        checked += 1;
    }
    assert!(checked > 100, "checked only {checked} specs");
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Rotated axis labels stay inside the SVG.
// ---------------------------------------------------------------------------

/// Every rotated (`-45°`) text in `element`, as its corners in SVG
/// coordinates, measured in the label's own font.
fn rotated_label_corners(element: &ChartElement) -> Vec<(String, Vec<(f64, f64)>)> {
    fn walk(
        element: &ChartElement,
        (dx, dy): (f64, f64),
        out: &mut Vec<(String, Vec<(f64, f64)>)>,
    ) {
        match element {
            ChartElement::Svg { children, .. } => {
                for child in children {
                    walk(child, (dx, dy), out);
                }
            }
            ChartElement::Group { transform, children, .. } => {
                let (tx, ty) = match transform {
                    Some(Transform::Translate(x, y)) => (*x, *y),
                    _ => (0.0, 0.0),
                };
                for child in children {
                    walk(child, (dx + tx, dy + ty), out);
                }
            }
            ChartElement::Text {
                x, y, content, anchor, transform: Some(Transform::Rotate(angle, cx, cy)),
                font_size, ..
            } => {
                let size: f64 = font_size
                    .as_deref()
                    .and_then(|s| s.trim_end_matches("px").parse().ok())
                    .unwrap_or(12.0);
                // The axis font metrics the production truncation measures with.
                let metrics = TextMetrics::from_theme_axis_label(&Theme::default());
                assert_eq!(metrics.font_size_px, size, "the label carries the theme's axis size");
                let length = measure_text(content, &metrics);
                if *anchor != TextAnchor::End || *angle != -45.0 {
                    return; // axis titles, not tick labels
                }
                let (top, bottom) = (y - 0.8 * size, y + 0.2 * size);
                let theta = angle.to_radians();
                let corners = [(x - length, top), (*x, top), (*x, bottom), (x - length, bottom)]
                    .into_iter()
                    .map(|(px, py)| {
                        let (ox, oy) = (px - cx, py - cy);
                        (
                            dx + cx + ox * theta.cos() - oy * theta.sin(),
                            dy + cy + ox * theta.sin() + oy * theta.cos(),
                        )
                    })
                    .collect();
                out.push((content.clone(), corners));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(element, (0.0, 0.0), &mut out);
    out
}

#[test]
fn no_rotated_axis_label_in_the_suite_leaves_the_svg() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _guard = runtime.enter(); // the sql transform needs a reactor
    let mut rotated = 0;
    let mut shortened = 0;
    for (name, spec) in specs() {
        let element = chartml().render_from_yaml(&chart_yaml(&spec)).expect(&name);
        let ChartElement::Svg { viewbox, width, height, .. } = &element else { continue };
        let (w, h) = (width.unwrap_or(viewbox.width), height.unwrap_or(viewbox.height));
        for (label, corners) in rotated_label_corners(&element) {
            rotated += 1;
            if label.ends_with('\u{2026}') {
                shortened += 1;
            }
            for (px, py) in corners {
                assert!(
                    (0.0..=w).contains(&px) && (0.0..=h).contains(&py),
                    "{name}: label {label:?} reaches ({px:.1}, {py:.1}) outside {w}x{h}"
                );
            }
        }
    }
    assert!(rotated > 50, "only {rotated} rotated labels seen");
    assert!(shortened > 0, "no label needed shortening: the suite no longer covers the case");
}

#[test]
fn a_label_that_fits_keeps_its_full_text_and_one_that_does_not_keeps_it_as_the_svg_title() {
    let chart = r#"type: chart
version: 1
title: Labels
data:
  provider: inline
  rows:
    - { name: "Short", v: 1 }
    - { name: "A really long category name that needs far more room than the chart has", v: 2 }
    - { name: "Another very long category name for the same chart, longer still", v: 3 }
    - { name: "Mid length label here", v: 4 }
    - { name: "Mid length label too", v: 5 }
    - { name: "Mid length label again", v: 6 }
visualize:
  type: bar
  columns: name
  rows: v
"#;
    let element = chartml().render_from_yaml(chart).expect("render");
    let svg = element_to_svg(&element, 800.0, 400.0);
    assert!(svg.contains(">Short<"), "{svg}");
    for full in [
        "A really long category name that needs far more room than the chart has",
        "Another very long category name for the same chart, longer still",
    ] {
        assert!(svg.contains(&format!("<title>{full}</title>")), "full text kept: {svg}");
    }
}

// ---------------------------------------------------------------------------
// Legend entries do not overlap in the real output.
// ---------------------------------------------------------------------------

/// Each legend item's symbol x and label (x, text) in `element`, in drawing order.
fn legend_items(element: &ChartElement) -> Vec<(f64, f64, String)> {
    fn find_legend(element: &ChartElement) -> Option<&Vec<ChartElement>> {
        match element {
            ChartElement::Group { class, children, .. } if class == "legend" => Some(children),
            ChartElement::Svg { children, .. } | ChartElement::Group { children, .. } => {
                children.iter().find_map(find_legend)
            }
            _ => None,
        }
    }
    let Some(children) = find_legend(element) else { return Vec::new() };
    let mut items = Vec::new();
    let mut symbol_x = 0.0;
    for child in children {
        match child {
            ChartElement::Rect { x, class, .. } if class.contains("legend-symbol") => symbol_x = *x,
            ChartElement::Text { x, content, class, .. } if class == "legend-label" => {
                items.push((symbol_x, *x, content.clone()));
            }
            _ => {}
        }
    }
    items
}

#[test]
fn normalized_market_share_legend_entries_do_not_overlap() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _guard = runtime.enter(); // specs() holds sql-transform specs
    for spec_name in ["bar/normalized_market_share.yaml", "area/normalized_market_share.yaml"] {
        let (_, spec) = specs()
            .into_iter()
            .find(|(name, _)| name == spec_name)
            .unwrap_or_else(|| panic!("{spec_name} exists"));
        let element = chartml().render_from_yaml(&chart_yaml(&spec)).expect("render");
        let items = legend_items(&element);
        assert!(items.len() >= 3, "{spec_name}: a legend entry per region, got {}", items.len());
        let metrics = TextMetrics::from_theme_legend(&Theme::default());
        for pair in items.windows(2) {
            let (_, label_x, text) = &pair[0];
            let (next_symbol_x, _, _) = &pair[1];
            let label_end = label_x + measure_text(text, &metrics);
            assert!(
                *next_symbol_x > label_end,
                "{spec_name}: {text:?} ends at {label_end}, the next symbol starts at {next_symbol_x}"
            );
        }
    }
}

#[test]
fn a_titled_table_fills_the_box_its_host_gives_it() {
    // The wasm `renderToSvg` and `render_to_png` put an HTML root in a box
    // sized by the caller; the table's wrapper must fill that box (as the
    // untitled table does), not hold a pixel size of its own.
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _guard = runtime.enter();
    let chart = format!("{}  style:\n    height: 200\n", yaml("table", Some(TITLE)));
    let element = chartml().render_from_yaml(&chart).expect("render");
    let ChartElement::Div { class, style, .. } = &element else { panic!("a titled table wraps in a Div") };
    assert_eq!(class, "chart-titled");
    assert_eq!(style.get("width").map(String::as_str), Some("100%"));
    assert_eq!(style.get("height").map(String::as_str), Some("100%"));
}
